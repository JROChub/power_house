#![cfg(feature = "net")]

use crate::julian::anchor_digest;
use crate::net::sign::{
    decode_public_key_base64, encode_public_key_base64, encode_signature_base64, sign_payload,
    verify_signature_base64, KeyError, KeyMaterial,
};
use crate::net::{
    attestation::{aggregate_attestations, Attestation},
    availability::{self, encode_shares, AvailabilityEvidence},
    blob::BlobJson,
    checkpoint::{
        latest_log_cutoff, load_latest_checkpoint, write_checkpoint, AnchorCheckpoint,
        CheckpointSignature,
    },
    governance::MembershipPolicy,
    schema::{
        AnchorCodecError, AnchorEnvelope, AnchorJson, AnchorVoteJson, DaCommitmentJson,
        ENVELOPE_SCHEMA_VERSION, NETWORK_ID, SCHEMA_ENVELOPE, SCHEMA_VOTE,
    },
    stake_registry::StakeRegistry,
};
use crate::{
    build_merkle_proof, compute_fold_digest, julian_genesis_anchor, merkle_root, parse_log_file,
    read_fold_digest_hint,
    rollup::{
        settle_rollup_with_rewards, RollupCommitment, RollupFaultEvidence, RollupSettlementMode,
        ZkRollupProof,
    },
    AnchorVote, EntryAnchor, LedgerAnchor,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use blake2::digest::{consts::U32, Digest as BlakeDigest};
use ed25519_dalek::{Signer, SigningKey};
use futures::StreamExt;
use hex;
use libp2p::{
    gossipsub::{self, IdentTopic, MessageAuthenticity, PublishError, ValidationMode},
    identify, identity,
    kad::{self, store::MemoryStore},
    multiaddr::Protocol,
    noise,
    swarm::{NetworkBehaviour, Swarm, SwarmEvent},
    tcp, yamux, Multiaddr, PeerId, SwarmBuilder,
};
use once_cell::sync::Lazy;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json;
use sha2::Sha256;
use std::net::SocketAddr;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    env, fs, io,
    io::Write,
    path::{Path, PathBuf},
    str,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{Mutex, Semaphore};
use tokio::{select, signal, time};

const DEFAULT_ANCHOR_TOPIC: &str = "mfenx/powerhouse/anchors/v1";
static TOPIC_EVIDENCE: Lazy<IdentTopic> =
    Lazy::new(|| IdentTopic::new("mfenx/powerhouse/evidence/v1"));
static TOPIC_VOTES: Lazy<IdentTopic> = Lazy::new(|| IdentTopic::new("mfenx/powerhouse/votes/v1"));
static NO_GOSSIP_PEERS_LOGGED: AtomicBool = AtomicBool::new(false);
const MAX_ENVELOPE_BYTES: usize = 64 * 1024;
const MAX_ANCHOR_ENTRIES: usize = 10_000;
const SEEN_CACHE_LIMIT: usize = 2048;
const INVALID_THRESHOLD: usize = 5;
const MAX_HEADER_BYTES: usize = 32 * 1024;
const DEFAULT_MAX_REQUEST_BYTES: usize = 64 * 1024 * 1024;
const DEFAULT_REQUEST_TIMEOUT_MS: u64 = 10_000;
const DEFAULT_BLOB_MAX_CONCURRENCY: usize = 128;

struct BftState {
    round: u64,
    votes: HashMap<String, HashSet<Vec<u8>>>,
}

impl BftState {
    fn new(bft_round_ms: u64) -> Self {
        let round = current_round(bft_round_ms);
        Self {
            round,
            votes: HashMap::new(),
        }
    }

    fn maybe_advance(&mut self, bft_round_ms: u64) {
        let now_round = current_round(bft_round_ms);
        if now_round != self.round {
            self.round = now_round;
            self.votes.clear();
        }
    }

    fn record_vote(&mut self, anchor_hash: &str, key_bytes: &[u8]) -> usize {
        let entry = self
            .votes
            .entry(anchor_hash.to_string())
            .or_insert_with(HashSet::new);
        entry.insert(key_bytes.to_vec());
        entry.len()
    }

    // vote_count intentionally omitted to keep the state minimal.
}

/// Per-namespace limits applied to blob ingestion.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NamespaceRule {
    /// Maximum blob size in bytes permitted for this namespace.
    pub max_bytes: Option<usize>,
    /// Retention policy (days) before pruning stored blobs.
    pub retention_days: Option<u64>,
    /// Rate limit in blobs per minute.
    pub max_per_min: Option<u32>,
    /// Minimum fee required to accept a blob.
    pub min_fee: Option<u64>,
    /// Operator reward in basis points (default 50%).
    #[serde(default)]
    pub operator_reward_bps: Option<u16>,
}

fn policy_permits(policy: &dyn MembershipPolicy, key: &[u8]) -> bool {
    let members = policy.current_members();
    if members.is_empty() {
        // Empty list means everyone allowed (bootstrap mode)
        return true;
    }
    members.iter().any(|vk| vk.to_bytes().as_slice() == key)
}

/// Configuration and runtime context for the JULIAN network node.
pub struct NetConfig {
    /// Human-readable node identifier used in logs and envelopes.
    pub node_id: String,
    /// Multiaddr on which the node listens for incoming peers.
    pub listen_addr: Multiaddr,
    /// Optional bootstrap peers dialed on startup.
    pub bootstraps: Vec<Multiaddr>,
    /// Directory containing ledger transcript logs to monitor.
    pub log_dir: PathBuf,
    /// Quorum threshold used when reconciling anchors.
    pub quorum: usize,
    /// Anchor gossip topic used for load balancing/sharding.
    pub anchor_topic: IdentTopic,
    /// Anchor gossip topics used for bridging across shards (includes anchor_topic).
    pub bridge_topics: Vec<IdentTopic>,
    /// Enable BFT-style anchor votes before broadcast.
    pub bft_enabled: bool,
    /// Round duration (ms) used to align vote rounds.
    pub bft_round_ms: u64,
    /// Interval between anchor recomputation and gossip broadcasts.
    pub broadcast_interval: Duration,
    /// Signing and libp2p keys backing the node identity.
    pub key_material: KeyMaterial,
    /// Membership governance policy.
    pub membership_policy: Arc<dyn MembershipPolicy>,
    /// Optional checkpoint interval (in broadcasts).
    pub checkpoint_interval: Option<u64>,
    /// Directory used to store blobs and share commitments.
    pub blob_dir: Option<PathBuf>,
    /// TCP socket for the blob ingest server.
    pub blob_listen: Option<SocketAddr>,
    /// Maximum blob size in bytes.
    pub max_blob_bytes: Option<usize>,
    /// Retention window in days for stored blobs.
    pub blob_retention_days: Option<u64>,
    /// Optional per-namespace policy overrides.
    pub blob_policies: Option<HashMap<String, NamespaceRule>>,
    /// Optional bearer token for blob HTTP endpoints.
    pub blob_auth_token: Option<String>,
    /// Maximum concurrent blob HTTP connections.
    pub blob_max_concurrency: usize,
    /// Request read timeout for blob HTTP endpoints.
    pub blob_request_timeout: Duration,
    /// Attestation quorum required for DA commitments.
    pub attestation_quorum: usize,
    /// Path to the stake registry used for fees and slashing.
    pub stake_registry_path: Option<PathBuf>,
    /// Optional public token contract used during migration dual-mode.
    pub token_mode_contract: Option<String>,
    /// Optional JSON-RPC endpoint used for token migration oracle checks.
    pub token_oracle_rpc: Option<String>,
    metrics: Arc<Metrics>,
    metrics_addr: Option<SocketAddr>,
}

impl NetConfig {
    /// Constructs a networking configuration for the JULIAN net CLI.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        node_id: String,
        listen_addr: Multiaddr,
        bootstraps: Vec<Multiaddr>,
        log_dir: PathBuf,
        quorum: usize,
        broadcast_interval: Duration,
        key_material: KeyMaterial,
        anchor_topic: Option<String>,
        bridge_topics: Option<Vec<String>>,
        bft_enabled: bool,
        bft_round_ms: Option<u64>,
        metrics_addr: Option<SocketAddr>,
        membership_policy: Arc<dyn MembershipPolicy>,
        checkpoint_interval: Option<u64>,
        blob_dir: Option<PathBuf>,
        blob_listen: Option<SocketAddr>,
        max_blob_bytes: Option<usize>,
        blob_retention_days: Option<u64>,
        blob_policies: Option<HashMap<String, NamespaceRule>>,
        blob_auth_token: Option<String>,
        blob_max_concurrency: Option<usize>,
        blob_request_timeout_ms: Option<u64>,
        attestation_quorum: Option<usize>,
        token_mode_contract: Option<String>,
        token_oracle_rpc: Option<String>,
    ) -> Self {
        let attestation_quorum = attestation_quorum.unwrap_or(quorum);
        let anchor_topic =
            IdentTopic::new(anchor_topic.unwrap_or_else(|| DEFAULT_ANCHOR_TOPIC.to_string()));
        let mut bridge_topics_vec = Vec::new();
        let mut seen_topics: HashSet<String> = HashSet::new();
        let anchor_str = anchor_topic.to_string();
        seen_topics.insert(anchor_str.clone());
        bridge_topics_vec.push(anchor_topic.clone());
        if let Some(topics) = bridge_topics {
            for topic in topics {
                let trimmed = topic.trim();
                if trimmed.is_empty() {
                    continue;
                }
                if seen_topics.insert(trimmed.to_string()) {
                    bridge_topics_vec.push(IdentTopic::new(trimmed));
                }
            }
        }
        let stake_registry_path = blob_dir.as_ref().map(|dir| dir.join("stake_registry.json"));
        let blob_max_concurrency = blob_max_concurrency.unwrap_or(DEFAULT_BLOB_MAX_CONCURRENCY);
        let blob_request_timeout =
            Duration::from_millis(blob_request_timeout_ms.unwrap_or(DEFAULT_REQUEST_TIMEOUT_MS));
        let bft_round_ms = bft_round_ms.unwrap_or_else(|| broadcast_interval.as_millis() as u64);
        Self {
            node_id,
            listen_addr,
            bootstraps,
            log_dir,
            quorum,
            broadcast_interval,
            key_material,
            anchor_topic,
            bridge_topics: bridge_topics_vec,
            bft_enabled,
            bft_round_ms,
            membership_policy,
            checkpoint_interval,
            blob_dir,
            blob_listen,
            max_blob_bytes,
            blob_retention_days,
            blob_policies,
            blob_auth_token,
            blob_max_concurrency,
            blob_request_timeout,
            attestation_quorum,
            stake_registry_path,
            token_mode_contract,
            token_oracle_rpc,
            metrics: Arc::new(Metrics::default()),
            metrics_addr,
        }
    }
}

#[derive(Clone)]
struct BlobServiceConfig {
    base_dir: PathBuf,
    listen: SocketAddr,
    max_bytes: Option<usize>,
    retention_days: Option<u64>,
    policies: Option<HashMap<String, NamespaceRule>>,
    signing: SigningKey,
    verifying_b64: String,
    stake_registry_path: Option<PathBuf>,
    token_mode_contract: Option<String>,
    token_oracle_rpc: Option<String>,
    membership_policy: Arc<dyn MembershipPolicy>,
    auth_token: Option<String>,
    max_concurrency: usize,
    request_timeout: Duration,
    rate_limits: Arc<Mutex<HashMap<String, RateState>>>,
    da_publish: Option<DaPublishConfig>,
}

#[derive(Debug, Clone)]
struct DaPublishConfig {
    provider: String,
    endpoint: String,
    auth_token: Option<String>,
    timeout: Duration,
    publish_interval: Duration,
    inline: bool,
    prune_after_publish: bool,
    retry_backoff: Duration,
}

#[derive(Debug, Clone)]
struct RateState {
    window_start: Instant,
    count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EvidenceRecord {
    namespace: String,
    blob_hash: String,
    pk: String,
    reason: String,
    ts: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EvidenceEnvelope {
    public_key: String,
    signature: String,
    evidence: EvidenceRecord,
}

fn sign_evidence(sk: &SigningKey, ev: &EvidenceRecord) -> EvidenceEnvelope {
    let payload = serde_json::to_vec(ev).expect("evidence encode");
    let sig = sk.sign(&payload);
    EvidenceEnvelope {
        public_key: encode_public_key_base64(&sk.verifying_key()),
        signature: BASE64.encode(sig.to_bytes()),
        evidence: ev.clone(),
    }
}

struct HttpRequest {
    method: String,
    path: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

async fn run_blob_service(cfg: BlobServiceConfig) {
    if let Err(err) = fs::create_dir_all(&cfg.base_dir) {
        eprintln!("blob dir init error: {err}");
        return;
    }
    let _retention_days = cfg.retention_days;
    if let Some(publish_cfg) = cfg.da_publish.clone() {
        let cfg_clone = cfg.clone();
        tokio::spawn(async move {
            da_publisher_loop(cfg_clone, publish_cfg).await;
        });
    }
    let listener = match TcpListener::bind(cfg.listen).await {
        Ok(l) => l,
        Err(err) => {
            eprintln!("failed to bind blob listener {}: {err}", cfg.listen);
            return;
        }
    };
    println!("QSYS|mod=BLOB|evt=LISTEN|addr={}", cfg.listen);
    let limiter = Arc::new(Semaphore::new(cfg.max_concurrency));
    loop {
        match listener.accept().await {
            Ok((mut stream, _addr)) => {
                let permit = match limiter.clone().acquire_owned().await {
                    Ok(permit) => permit,
                    Err(_) => {
                        eprintln!("blob accept error: limiter closed");
                        continue;
                    }
                };
                let cfg_clone = cfg.clone();
                tokio::spawn(async move {
                    let _permit = permit;
                    if let Err(err) = handle_blob_connection(&mut stream, cfg_clone).await {
                        eprintln!("blob connection error: {err}");
                    }
                });
            }
            Err(err) => {
                eprintln!("blob accept error: {err}");
                break;
            }
        }
    }
}

fn parse_env_flag(value: &str) -> bool {
    matches!(
        value.to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn da_publish_config_from_env() -> Option<DaPublishConfig> {
    let endpoint = env::var("PH_DA_ENDPOINT").ok()?;
    let provider = env::var("PH_DA_PROVIDER").unwrap_or_else(|_| "generic".to_string());
    let auth_token = env::var("PH_DA_AUTH_TOKEN").ok();
    let timeout_ms = env::var("PH_DA_TIMEOUT_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(10_000);
    let publish_interval_ms = env::var("PH_DA_PUBLISH_INTERVAL_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(5_000);
    let retry_backoff_ms = env::var("PH_DA_RETRY_BACKOFF_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(60_000);
    let inline = env::var("PH_DA_PUBLISH_INLINE")
        .ok()
        .map(|v| parse_env_flag(&v))
        .unwrap_or(false);
    let prune_after_publish = env::var("PH_DA_PRUNE_AFTER_PUBLISH")
        .ok()
        .map(|v| parse_env_flag(&v))
        .unwrap_or(false);
    Some(DaPublishConfig {
        provider,
        endpoint,
        auth_token,
        timeout: Duration::from_millis(timeout_ms),
        publish_interval: Duration::from_millis(publish_interval_ms),
        inline,
        prune_after_publish,
        retry_backoff: Duration::from_millis(retry_backoff_ms),
    })
}

fn append_da_outbox(base: &Path, record: &DaOutboxRecord) -> Result<(), String> {
    let path = base.join("da_outbox.jsonl");
    let payload = serde_json::to_vec(record).map_err(|e| format!("da outbox encode: {e}"))?;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| format!("da outbox open: {e}"))?;
    file.write_all(&payload)
        .and_then(|_| file.write_all(b"\n"))
        .map_err(|e| format!("da outbox write: {e}"))?;
    Ok(())
}

async fn da_publisher_loop(cfg: BlobServiceConfig, publish: DaPublishConfig) {
    let client = match Client::builder().timeout(publish.timeout).build() {
        Ok(c) => c,
        Err(err) => {
            eprintln!("da publisher client error: {err}");
            return;
        }
    };
    loop {
        if let Err(err) = process_da_outbox(&cfg, &publish, &client).await {
            eprintln!("da publisher error: {err}");
        }
        time::sleep(publish.publish_interval).await;
    }
}

async fn process_da_outbox(
    cfg: &BlobServiceConfig,
    publish: &DaPublishConfig,
    client: &Client,
) -> Result<(), String> {
    let path = cfg.base_dir.join("da_outbox.jsonl");
    if !path.exists() {
        return Ok(());
    }
    let contents = fs::read_to_string(&path).map_err(|e| format!("da outbox read: {e}"))?;
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let record: DaOutboxRecord =
            serde_json::from_str(line).map_err(|e| format!("da outbox decode: {e}"))?;
        let meta_opt = load_blob_meta(&cfg.base_dir, &record.namespace, &record.hash)
            .map_err(|e| e.to_string())?;
        let mut meta = match meta_opt {
            Some(m) => m,
            None => continue,
        };
        if let Some(receipt) = meta.da_receipt.as_ref() {
            if receipt.status == "ok" {
                continue;
            }
            if receipt.status == "error" {
                let elapsed = now_millis().saturating_sub(receipt.updated_ms);
                if elapsed < publish.retry_backoff.as_millis() as u64 {
                    continue;
                }
            }
        }
        match publish_da_commitment(client, publish, &record).await {
            Ok(receipt) => {
                meta.da_receipt = Some(receipt);
                save_blob_meta(&cfg.base_dir, &meta).map_err(|e| e.to_string())?;
                if publish.prune_after_publish {
                    prune_blob_payload(&cfg.base_dir, &meta);
                }
                let _ = append_da_published(&cfg.base_dir, &record);
            }
            Err(err) => {
                meta.da_receipt = Some(DaReceipt {
                    provider: publish.provider.clone(),
                    commitment: Some(record.share_root.clone()),
                    tx_hash: None,
                    height: None,
                    status: "error".to_string(),
                    updated_ms: now_millis(),
                    response: None,
                    last_error: Some(err.clone()),
                });
                save_blob_meta(&cfg.base_dir, &meta).map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(())
}

fn append_da_published(base: &Path, record: &DaOutboxRecord) -> Result<(), String> {
    let path = base.join("da_published.jsonl");
    let payload = serde_json::to_vec(record).map_err(|e| format!("da published encode: {e}"))?;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|e| format!("da published open: {e}"))?;
    file.write_all(&payload)
        .and_then(|_| file.write_all(b"\n"))
        .map_err(|e| format!("da published write: {e}"))?;
    Ok(())
}

async fn publish_da_commitment(
    client: &Client,
    publish: &DaPublishConfig,
    record: &DaOutboxRecord,
) -> Result<DaReceipt, String> {
    #[derive(Serialize)]
    struct DaPayload<'a> {
        provider: &'a str,
        namespace: &'a str,
        blob_hash: &'a str,
        share_root: &'a str,
        #[serde(skip_serializing_if = "Option::is_none")]
        pedersen_root: Option<&'a str>,
        size: u64,
        data_shards: u8,
        parity_shards: u8,
        ts: u64,
    }
    let payload = DaPayload {
        provider: &publish.provider,
        namespace: &record.namespace,
        blob_hash: &record.hash,
        share_root: &record.share_root,
        pedersen_root: record.pedersen_root.as_deref(),
        size: record.size,
        data_shards: record.data_shards,
        parity_shards: record.parity_shards,
        ts: record.ts,
    };
    let mut req = client.post(&publish.endpoint).json(&payload);
    if let Some(token) = publish.auth_token.as_deref() {
        req = req.bearer_auth(token);
    }
    let resp = req
        .send()
        .await
        .map_err(|e| format!("da publish request failed: {e}"))?;
    let status = resp.status();
    let text = resp
        .text()
        .await
        .map_err(|e| format!("da publish read failed: {e}"))?;
    if !status.is_success() {
        return Err(format!(
            "da publish failed: status={} body={}",
            status, text
        ));
    }
    let parsed: Option<serde_json::Value> = serde_json::from_str(&text).ok();
    let tx_hash = parsed
        .as_ref()
        .and_then(|v| {
            v.get("tx_hash")
                .or_else(|| v.get("transaction_hash"))
                .or_else(|| v.get("hash"))
        })
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let height = parsed
        .as_ref()
        .and_then(|v| v.get("height").or_else(|| v.get("block_height")))
        .and_then(|v| v.as_u64());
    let commitment = parsed
        .as_ref()
        .and_then(|v| v.get("commitment").or_else(|| v.get("share_root")))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| Some(record.share_root.clone()));
    Ok(DaReceipt {
        provider: publish.provider.clone(),
        commitment,
        tx_hash,
        height,
        status: "ok".to_string(),
        updated_ms: now_millis(),
        response: parsed,
        last_error: None,
    })
}

fn prune_blob_payload(base: &Path, meta: &BlobMeta) {
    let (_, blob_path, share_dir) = blob_paths(base, &meta.namespace, &meta.hash);
    let _ = fs::remove_file(&blob_path);
    let _ = fs::remove_dir_all(&share_dir);
}

async fn handle_blob_connection(stream: &mut TcpStream, cfg: BlobServiceConfig) -> io::Result<()> {
    let max_body = cfg.max_bytes.unwrap_or(DEFAULT_MAX_REQUEST_BYTES);
    let req = match read_http_request(stream, MAX_HEADER_BYTES, max_body, cfg.request_timeout).await
    {
        Ok(r) => r,
        Err(err) => {
            let resp = build_response(
                "400 Bad Request",
                format!("{{\"error\":\"{err}\"}}"),
                "application/json",
            );
            let _ = stream.write_all(&resp).await;
            let _ = stream.shutdown().await;
            return Ok(());
        }
    };
    if let Some(token) = cfg.auth_token.as_deref() {
        let mut ok = false;
        if let Some(auth) = req.headers.get("authorization") {
            if let Some(bearer) = auth.strip_prefix("Bearer ") {
                ok = bearer == token;
            }
        }
        if let Some(key) = req.headers.get("x-api-key") {
            ok = ok || key == token;
        }
        if !ok {
            let resp = build_response(
                "401 Unauthorized",
                "{\"error\":\"unauthorized\"}".to_string(),
                "application/json",
            );
            let _ = stream.write_all(&resp).await;
            let _ = stream.shutdown().await;
            return Ok(());
        }
    }
    let (status, body, content_type) = match req.method.as_str() {
        "GET" if req.path == "/healthz" => (
            "200 OK".to_string(),
            format!(
                "{{\"status\":\"ok\",\"network\":\"{}\",\"version\":\"{}\"}}",
                NETWORK_ID,
                env!("CARGO_PKG_VERSION")
            ),
            "application/json".to_string(),
        ),
        "POST" if req.path.starts_with("/submit_blob") => {
            match handle_submit_blob(&req, &cfg).await {
                Ok(json) => ("200 OK".to_string(), json, "application/json".to_string()),
                Err(err) => (
                    "400 Bad Request".to_string(),
                    format!("{{\"error\":\"{err}\"}}"),
                    "application/json".to_string(),
                ),
            }
        }
        "GET" if req.path.starts_with("/commitment/") => {
            let json = handle_commitment(&req, &cfg).unwrap_or_else(|e| e);
            if json.starts_with("{") {
                ("200 OK".to_string(), json, "application/json".to_string())
            } else {
                ("404 Not Found".to_string(), json, "text/plain".to_string())
            }
        }
        "GET" if req.path.starts_with("/sample/") => {
            let (code, body) = handle_sample(&req, &cfg).unwrap_or((404, "Not Found".to_string()));
            let status = match code {
                200 => "200 OK",
                400 => "400 Bad Request",
                _ => "404 Not Found",
            };
            (
                status.to_string(),
                body,
                if code == 200 {
                    "application/json"
                } else {
                    "text/plain"
                }
                .to_string(),
            )
        }
        "GET" if req.path.starts_with("/prove_storage/") => {
            let (code, body) =
                handle_prove_storage(&req, &cfg).unwrap_or((404, "Not Found".to_string()));
            let status = match code {
                200 => "200 OK",
                400 => "400 Bad Request",
                _ => "404 Not Found",
            };
            (
                status.to_string(),
                body,
                if code == 200 {
                    "application/json"
                } else {
                    "text/plain"
                }
                .to_string(),
            )
        }
        "POST" if req.path.starts_with("/rollup_settle") => {
            match handle_rollup_settle(&req, &cfg) {
                Ok(json) => ("200 OK".to_string(), json, "application/json".to_string()),
                Err(err) => (
                    "400 Bad Request".to_string(),
                    format!("{{\"error\":\"{err}\"}}"),
                    "application/json".to_string(),
                ),
            }
        }
        _ => (
            "404 Not Found".to_string(),
            "Not Found".to_string(),
            "text/plain".to_string(),
        ),
    };

    let resp = build_response(&status, body, &content_type);
    stream.write_all(&resp).await?;
    stream.shutdown().await
}

async fn read_http_request(
    stream: &mut TcpStream,
    max_header_bytes: usize,
    max_body_bytes: usize,
    timeout: Duration,
) -> io::Result<HttpRequest> {
    let mut buf = Vec::new();
    let mut header_end = None;
    loop {
        let mut tmp = [0u8; 1024];
        let n = time::timeout(timeout, stream.read(&mut tmp))
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "read timeout"))??;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&tmp[..n]);
        if buf.len() > max_header_bytes && header_end.is_none() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "header too large",
            ));
        }
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            header_end = Some(pos + 4);
            break;
        }
    }
    let end = header_end
        .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "malformed request"))?;
    let header_str = str::from_utf8(&buf[..end])
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid header"))?;
    let mut lines = header_str.split("\r\n").filter(|l| !l.is_empty());
    let request_line = lines
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing request line"))?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let path = parts.next().unwrap_or("").to_string();
    let mut headers = HashMap::new();
    for line in lines {
        if let Some((name, value)) = line.split_once(":") {
            headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
        }
    }
    let content_len: usize = headers
        .get("content-length")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    if content_len > max_body_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "content-length exceeds limit",
        ));
    }
    let mut body = if end < buf.len() {
        buf[end..].to_vec()
    } else {
        Vec::new()
    };
    while body.len() < content_len {
        let remaining = content_len - body.len();
        let mut tmp = vec![0u8; remaining.min(8192)];
        let n = time::timeout(timeout, stream.read(&mut tmp))
            .await
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "read timeout"))??;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&tmp[..n]);
    }
    if body.len() < content_len {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "incomplete request body",
        ));
    }
    Ok(HttpRequest {
        method,
        path,
        headers,
        body,
    })
}

fn build_response(status: &str, body: String, content_type: &str) -> Vec<u8> {
    format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nContent-Type: {content_type}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .into_bytes()
}

fn sanitize_token(token: &str) -> Option<String> {
    if token
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        Some(token.to_string())
    } else {
        None
    }
}

fn namespace_rule(cfg: &BlobServiceConfig, namespace: &str) -> Option<NamespaceRule> {
    cfg.policies
        .as_ref()
        .and_then(|m| m.get(namespace).cloned())
}

fn token_mode_is_native(mode: &str) -> bool {
    let trimmed = mode.trim();
    trimmed.eq_ignore_ascii_case("native") || trimmed.to_ascii_lowercase().starts_with("native://")
}

fn token_mode_enabled(cfg: &BlobServiceConfig) -> bool {
    cfg.token_mode_contract
        .as_deref()
        .map(|mode| !mode.trim().is_empty())
        .unwrap_or(false)
}

fn token_mode_requires_oracle(cfg: &BlobServiceConfig) -> bool {
    cfg.token_mode_contract
        .as_deref()
        .map(|mode| !token_mode_is_native(mode))
        .unwrap_or(false)
}

fn pubkey_b64_to_migration_address(pk_b64: &str) -> Result<String, String> {
    type Blake2b256 = blake2::Blake2b<U32>;
    let decoded = BASE64
        .decode(pk_b64.as_bytes())
        .map_err(|e| format!("publisher key decode failed: {e}"))?;
    let mut hasher = Blake2b256::new();
    hasher.update(b"mfenx-migration-address-v1");
    hasher.update(decoded);
    let digest: [u8; 32] = hasher.finalize().into();
    Ok(format!("0x{}", hex::encode(&digest[12..])))
}

fn parse_hex_u128(input: &str) -> Result<u128, String> {
    let raw = input.strip_prefix("0x").unwrap_or(input);
    u128::from_str_radix(raw, 16).map_err(|e| format!("invalid hex quantity: {e}"))
}

async fn token_oracle_balance_sufficient(
    cfg: &BlobServiceConfig,
    payer_pk_b64: &str,
    required_amount: u64,
) -> Result<bool, String> {
    let rpc = cfg
        .token_oracle_rpc
        .as_ref()
        .ok_or_else(|| "token oracle rpc not configured".to_string())?;
    let account = pubkey_b64_to_migration_address(payer_pk_b64)?;
    let payload = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1u64,
        "method": "eth_getBalance",
        "params": [account, "latest"]
    });
    let client = Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("token oracle client error: {e}"))?;
    let resp = client
        .post(rpc)
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("token oracle request failed: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("token oracle status {status}: {body}"));
    }
    let value: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("token oracle decode failed: {e}"))?;
    let result_hex = value
        .get("result")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "token oracle missing result".to_string())?;
    let balance = parse_hex_u128(result_hex)?;
    Ok(balance >= required_amount as u128)
}

fn queue_token_burn_intent(
    registry_path: &Option<PathBuf>,
    token_mode_contract: &Option<String>,
    token_oracle_rpc: &Option<String>,
    pk_b64: &str,
    reason: &str,
) {
    let (Some(contract), Some(reg_path)) = (token_mode_contract, registry_path) else {
        return;
    };
    let outbox = reg_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("token_burn_outbox.jsonl");
    let account = match pubkey_b64_to_migration_address(pk_b64) {
        Ok(a) => a,
        Err(err) => {
            eprintln!("token burn mapping failed for {pk_b64}: {err}");
            return;
        }
    };
    let payload = serde_json::json!({
        "schema": "mfenx.powerhouse.token-burn-intent.v1",
        "token_contract": contract,
        "token_oracle": token_oracle_rpc,
        "account": account,
        "pubkey_b64": pk_b64,
        "reason": reason,
        "ts": now_millis(),
    });
    if let Some(parent) = outbox.parent() {
        let _ = fs::create_dir_all(parent);
    }
    match serde_json::to_string(&payload) {
        Ok(line) => {
            let line = format!("{line}\n");
            if let Err(err) = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&outbox)
                .and_then(|mut f| std::io::Write::write_all(&mut f, line.as_bytes()))
            {
                eprintln!("failed to append token burn intent: {err}");
            }
        }
        Err(err) => eprintln!("failed to encode token burn intent: {err}"),
    }
}

fn read_shares(
    base: &Path,
    namespace: &str,
    hash: &str,
    total: usize,
) -> Result<Vec<Vec<u8>>, NetworkError> {
    let (_, _, share_dir) = blob_paths(base, namespace, hash);
    let mut shares = Vec::with_capacity(total);
    for idx in 0..total {
        let path = share_dir.join(format!("{idx}.share"));
        let data = fs::read(&path)
            .map_err(|err| NetworkError::Io(format!("failed to read {}: {err}", path.display())))?;
        shares.push(data);
    }
    Ok(shares)
}

fn share_hashes(shares: &[Vec<u8>]) -> Vec<[u8; 32]> {
    shares
        .iter()
        .map(|s| {
            let mut hasher = Sha256::new();
            hasher.update(s);
            hasher.finalize().into()
        })
        .collect()
}

async fn handle_submit_blob(req: &HttpRequest, cfg: &BlobServiceConfig) -> Result<String, String> {
    if crate::net::refresh_migration_mode_from_env() {
        return Err("migration freeze active: blob ingestion disabled".to_string());
    }
    let namespace = req
        .headers
        .get("x-namespace")
        .and_then(|v| sanitize_token(v))
        .unwrap_or_else(|| "default".to_string());
    let fee: Option<u64> = req.headers.get("x-fee").and_then(|v| v.parse().ok());
    let publisher_pk = req.headers.get("x-publisher").cloned();
    let publisher_sig = req.headers.get("x-publisher-sig").cloned();
    if let Some(rule) = namespace_rule(cfg, &namespace) {
        if let Some(max_per_min) = rule.max_per_min {
            if max_per_min > 0 {
                let mut limits = cfg.rate_limits.lock().await;
                let entry = limits.entry(namespace.clone()).or_insert(RateState {
                    window_start: Instant::now(),
                    count: 0,
                });
                if entry.window_start.elapsed() >= Duration::from_secs(60) {
                    entry.window_start = Instant::now();
                    entry.count = 0;
                }
                if entry.count >= max_per_min {
                    return Err("rate limit exceeded".into());
                }
                entry.count += 1;
            }
        }
        if let Some(max) = rule.max_bytes {
            if req.body.len() > max {
                return Err(format!("blob exceeds max_bytes for namespace {namespace}"));
            }
        }
        if let Some(min_fee) = rule.min_fee {
            if fee.unwrap_or(0) < min_fee {
                return Err("fee below namespace minimum".into());
            }
        }
    }
    if let Some(max) = cfg.max_bytes {
        if req.body.len() > max {
            return Err("blob exceeds max_bytes".into());
        }
    }
    let blob = BlobJson::from_bytes(&namespace, &req.body);
    let data_shards = 4u8;
    let parity_shards = 2u8;
    let (shares, commitment) = encode_shares(&req.body, data_shards, parity_shards)
        .map_err(|e| format!("encode error: {e}"))?;

    let (meta_path, blob_path, share_dir) = blob_paths(&cfg.base_dir, &namespace, &blob.hash);
    if let Some(parent) = meta_path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("create dirs: {err}"))?;
    }
    fs::write(&blob_path, &req.body).map_err(|err| format!("write blob: {err}"))?;
    fs::create_dir_all(&share_dir).map_err(|err| format!("create share dir: {err}"))?;
    for (idx, share) in shares.iter().enumerate() {
        let path = share_dir.join(format!("{idx}.share"));
        fs::write(&path, share).map_err(|err| format!("write share: {err}"))?;
    }

    let sig = sign_payload(&cfg.signing, commitment.share_root.as_bytes());
    let operator_att = StoredAttestation {
        pk: cfg.verifying_b64.clone(),
        sig: encode_signature_base64(&sig),
    };
    let mut attestations = vec![operator_att];
    if let (Some(att_pk), Some(att_sig)) = (
        req.headers.get("x-attestation-pk"),
        req.headers.get("x-attestation-sig"),
    ) {
        if verify_signature_base64(att_pk, commitment.share_root.as_bytes(), att_sig).is_ok() {
            attestations.push(StoredAttestation {
                pk: att_pk.clone(),
                sig: att_sig.clone(),
            });
        }
    }
    let mut meta = BlobMeta {
        namespace: namespace.clone(),
        hash: blob.hash.clone(),
        size: blob.size,
        data_shards,
        parity_shards,
        share_root: commitment.share_root.clone(),
        pedersen_root: Some(commitment.pedersen_root.clone()),
        attestations,
        publisher_pk,
        da_receipt: None,
    };
    if let Some(pk) = meta.publisher_pk.as_ref() {
        if let Some(sig) = publisher_sig.as_ref() {
            verify_signature_base64(pk, commitment.share_root.as_bytes(), sig)
                .map_err(|_| "invalid x-publisher-sig signature".to_string())?;
        } else {
            return Err("missing x-publisher-sig header for publisher".into());
        }
    }
    save_blob_meta(&cfg.base_dir, &meta).map_err(|err| err.to_string())?;
    if let Some(da_cfg) = cfg.da_publish.clone() {
        let record = DaOutboxRecord {
            namespace: meta.namespace.clone(),
            hash: meta.hash.clone(),
            share_root: meta.share_root.clone(),
            pedersen_root: meta.pedersen_root.clone(),
            size: meta.size,
            data_shards: meta.data_shards,
            parity_shards: meta.parity_shards,
            ts: now_millis(),
        };
        if let Err(err) = append_da_outbox(&cfg.base_dir, &record) {
            eprintln!("da outbox append error: {err}");
        }
        if da_cfg.inline {
            if let Ok(client) = Client::builder().timeout(da_cfg.timeout).build() {
                match publish_da_commitment(&client, &da_cfg, &record).await {
                    Ok(receipt) => {
                        meta.da_receipt = Some(receipt);
                        let _ = save_blob_meta(&cfg.base_dir, &meta);
                        if da_cfg.prune_after_publish {
                            prune_blob_payload(&cfg.base_dir, &meta);
                        }
                    }
                    Err(err) => {
                        meta.da_receipt = Some(DaReceipt {
                            provider: da_cfg.provider.clone(),
                            commitment: Some(record.share_root.clone()),
                            tx_hash: None,
                            height: None,
                            status: "error".to_string(),
                            updated_ms: now_millis(),
                            response: None,
                            last_error: Some(err),
                        });
                        let _ = save_blob_meta(&cfg.base_dir, &meta);
                    }
                }
            }
        }
    }
    // Fees and rewards (split between operator and attestors by stake).
    if let (Some(path), Some(amount)) = (&cfg.stake_registry_path, fee) {
        let payer = meta
            .publisher_pk
            .clone()
            .unwrap_or_else(|| cfg.verifying_b64.clone());
        match StakeRegistry::load(path) {
            Ok(mut reg) => {
                let mut settled_via_registry = true;
                if let Err(debit_err) = reg.debit_fee(&payer, amount) {
                    if token_mode_enabled(cfg) {
                        if token_mode_requires_oracle(cfg) {
                            let covered = token_oracle_balance_sufficient(cfg, &payer, amount)
                                .await
                                .map_err(|err| format!("token oracle check failed: {err}"))?;
                            if covered {
                                settled_via_registry = false;
                            } else {
                                return Err(format!(
                                    "fee debit failed and oracle balance insufficient: {debit_err}"
                                ));
                            }
                        } else {
                            return Err(format!("fee debit failed in native mode: {debit_err}"));
                        }
                    } else {
                        return Err(format!("fee debit failed: {debit_err}"));
                    }
                }
                if settled_via_registry {
                    let ns_rule = namespace_rule(cfg, &namespace).unwrap_or_default();
                    let op_bps = ns_rule.operator_reward_bps.unwrap_or(5000) as u64;
                    let operator_cut = amount.saturating_mul(op_bps).saturating_div(10_000);
                    let attestor_pool = amount.saturating_sub(operator_cut);
                    reg.credit_reward(&cfg.verifying_b64, operator_cut);
                    let attestors = meta.attestations.clone();
                    let total_weight: u64 =
                        attestors.iter().filter_map(|a| reg.stake_for(&a.pk)).sum();
                    if total_weight == 0 {
                        reg.credit_reward(&cfg.verifying_b64, attestor_pool);
                    } else {
                        for att in attestors {
                            if let Some(w) = reg.stake_for(&att.pk) {
                                let share = attestor_pool.saturating_mul(w) / total_weight;
                                if share > 0 {
                                    reg.credit_reward(&att.pk, share);
                                }
                            }
                        }
                    }
                }
                reg.save(path)
                    .map_err(|err| format!("failed to persist stake registry: {err}"))?;
            }
            Err(err) => return Err(format!("failed to load stake registry: {err}")),
        }
    }

    let da_status = meta
        .da_receipt
        .as_ref()
        .map(|r| r.status.clone())
        .unwrap_or_else(|| {
            if cfg.da_publish.is_some() {
                "queued".to_string()
            } else {
                "disabled".to_string()
            }
        });
    let response = serde_json::json!({
        "status": "ok",
        "size": blob.size,
        "hash": blob.hash,
        "share_root": meta.share_root,
        "pedersen_root": meta.pedersen_root,
        "data_shards": data_shards,
        "parity_shards": parity_shards,
        "da_status": da_status,
    });
    Ok(response.to_string())
}

fn handle_rollup_settle(req: &HttpRequest, cfg: &BlobServiceConfig) -> Result<String, String> {
    #[derive(Deserialize)]
    struct RollupSettleRequest {
        namespace: String,
        share_root: String,
        #[serde(default)]
        pedersen_root: Option<String>,
        payer_pk: String,
        #[serde(default)]
        operator_pk: Option<String>,
        #[serde(default)]
        attesters: Option<Vec<String>>,
        fee: u64,
        #[serde(default)]
        mode: Option<String>,
        #[serde(default)]
        proof_b64: Option<String>,
        #[serde(default)]
        public_inputs_b64: Option<String>,
        #[serde(default)]
        merkle_path_b64: Option<String>,
    }
    let req_body: RollupSettleRequest =
        serde_json::from_slice(&req.body).map_err(|e| format!("decode error: {e}"))?;
    let registry_path = cfg
        .stake_registry_path
        .as_ref()
        .ok_or_else(|| "stake registry not configured".to_string())?;
    let pedersen_root = req_body
        .pedersen_root
        .clone()
        .unwrap_or_else(|| req_body.share_root.clone());
    let commitment = RollupCommitment {
        namespace: req_body.namespace.clone(),
        share_root: req_body.share_root.clone(),
        pedersen_root: Some(pedersen_root),
        settlement_slot: None,
    };
    let operator_pk = req_body
        .operator_pk
        .clone()
        .unwrap_or_else(|| req_body.payer_pk.clone());
    let attesters = req_body.attesters.unwrap_or_default();
    let mode = req_body.mode.unwrap_or_else(|| "optimistic".to_string());

    let zk_proof = if let (Some(p_b64), Some(pi_b64), Some(mp_b64)) = (
        req_body.proof_b64.as_ref(),
        req_body.public_inputs_b64.as_ref(),
        req_body.merkle_path_b64.as_ref(),
    ) {
        let proof = BASE64
            .decode(p_b64.as_bytes())
            .map_err(|e| format!("proof decode: {e}"))?;
        let public_inputs = BASE64
            .decode(pi_b64.as_bytes())
            .map_err(|e| format!("public inputs decode: {e}"))?;
        let merkle_path = BASE64
            .decode(mp_b64.as_bytes())
            .map_err(|e| format!("merkle path decode: {e}"))?;
        ZkRollupProof {
            proof,
            public_inputs,
            merkle_path,
        }
    } else {
        ZkRollupProof {
            proof: Vec::new(),
            public_inputs: Vec::new(),
            merkle_path: Vec::new(),
        }
    };

    let mode_enum = if mode == "zk" {
        RollupSettlementMode::Zk(zk_proof)
    } else {
        RollupSettlementMode::Optimistic(Vec::new())
    };

    match settle_rollup_with_rewards(
        registry_path,
        commitment.clone(),
        &req_body.payer_pk,
        &operator_pk,
        &attesters,
        req_body.fee,
        mode_enum,
    ) {
        Ok(receipt) => Ok(serde_json::to_string(&serde_json::json!({
            "status": "ok",
            "payer": receipt.payer,
            "fee": receipt.fee,
            "commitment": receipt.commitment.share_root
        }))
        .unwrap_or_else(|_| "{}".to_string())),
        Err(fault) => {
            let outbox = cfg.base_dir.join("evidence_outbox.jsonl");
            append_rollup_fault_evidence(&outbox, &fault);
            Err(format!("rollup fault: {}", fault.reason))
        }
    }
}

fn handle_commitment(req: &HttpRequest, cfg: &BlobServiceConfig) -> Result<String, String> {
    let parts: Vec<&str> = req.path.split('/').collect();
    if parts.len() < 4 {
        return Err("Not Found".into());
    }
    let namespace = sanitize_token(parts[2]).ok_or_else(|| "Not Found".to_string())?;
    let hash = sanitize_token(parts[3]).ok_or_else(|| "Not Found".to_string())?;
    let meta = load_blob_meta(&cfg.base_dir, &namespace, &hash)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Not Found".to_string())?;
    let attestations: Vec<_> = meta
        .attestations
        .iter()
        .map(|a| serde_json::json!({"pk": a.pk, "sig": a.sig}))
        .collect();
    let body = serde_json::json!({
        "namespace": meta.namespace,
        "hash": meta.hash,
        "size": meta.size,
        "share_root": meta.share_root,
        "pedersen_root": meta.pedersen_root,
        "data_shards": meta.data_shards,
        "parity_shards": meta.parity_shards,
        "shares": (meta.data_shards as usize + meta.parity_shards as usize),
        "attestations": attestations,
        "publisher_pk": meta.publisher_pk,
        "da_receipt": meta.da_receipt,
    });
    Ok(body.to_string())
}

fn parse_count(query: Option<&str>) -> usize {
    query
        .and_then(|q| {
            q.split('&').find_map(|kv| {
                let mut parts = kv.split('=');
                if parts.next() == Some("count") {
                    parts.next().and_then(|v| v.parse().ok())
                } else {
                    None
                }
            })
        })
        .unwrap_or(1)
}

fn pick_slash_target(meta: &BlobMeta) -> Option<String> {
    meta.publisher_pk
        .clone()
        .or_else(|| meta.attestations.first().map(|a| a.pk.clone()))
}

fn handle_sample(
    req: &HttpRequest,
    cfg: &BlobServiceConfig,
) -> Result<(u16, String), (u16, String)> {
    let parts: Vec<&str> = req.path.split('?').collect();
    let path_part = parts[0];
    let query_part = if parts.len() > 1 {
        Some(parts[1])
    } else {
        None
    };
    let segs: Vec<&str> = path_part.split('/').collect();
    if segs.len() < 4 {
        return Err((404, "Not Found".into()));
    }
    let namespace = sanitize_token(segs[2]).ok_or((404, "Not Found".into()))?;
    let hash = sanitize_token(segs[3]).ok_or((404, "Not Found".into()))?;
    let meta = load_blob_meta(&cfg.base_dir, &namespace, &hash)
        .map_err(|e| (400, e.to_string()))?
        .ok_or((404, "Not Found".into()))?;
    let total = meta.data_shards as usize + meta.parity_shards as usize;
    let shares = match read_shares(&cfg.base_dir, &namespace, &hash, total) {
        Ok(s) => s,
        Err(_err) => {
            if let Some(pk) = meta.publisher_pk.as_ref() {
                record_slash_with_registry(
                    &cfg.membership_policy,
                    &cfg.stake_registry_path,
                    &cfg.token_mode_contract,
                    &cfg.token_oracle_rpc,
                    pk,
                    "blob-missing",
                );
            }
            let ev_path = cfg.base_dir.join("evidence_outbox.jsonl");
            let ev = availability::build_missing_share_evidence(&namespace, &hash, 0);
            append_availability_evidence(&ev_path, &ev);
            return Err((404, "Not Found".into()));
        }
    };
    let share_hashes = share_hashes(&shares);
    let root_hex = hex::encode(crate::merkle_root(&share_hashes));
    if root_hex != meta.share_root {
        let ev_path = cfg.base_dir.join("evidence_outbox.jsonl");
        let ev = AvailabilityEvidence {
            namespace: namespace.clone(),
            blob_hash: hash.clone(),
            idx: 0,
            share: None,
            reason: "share_root_mismatch".into(),
        };
        append_availability_evidence(&ev_path, &ev);
        return Err((400, "share_root mismatch".into()));
    }
    let pedersen_root = availability::pedersen_merkle_root(&share_hashes);
    let pedersen_root_hex = hex::encode(pedersen_root);
    if let Some(stored) = &meta.pedersen_root {
        if &pedersen_root_hex != stored {
            let ev_path = cfg.base_dir.join("evidence_outbox.jsonl");
            let ev = AvailabilityEvidence {
                namespace: namespace.clone(),
                blob_hash: hash.clone(),
                idx: 0,
                share: None,
                reason: "pedersen_root_mismatch".into(),
            };
            append_availability_evidence(&ev_path, &ev);
            return Err((400, "pedersen_root mismatch".into()));
        }
    }
    let mut indices: Vec<usize> = (0..shares.len()).collect();
    let count = parse_count(query_part);
    indices.truncate(count.min(indices.len()));
    let mut sampled = Vec::new();
    for idx in indices {
        let proof = build_merkle_proof(&share_hashes, idx).ok_or((400, "bad index".into()))?;
        let pedersen_proof =
            availability::pedersen_share_proof(&share_hashes, idx).map_err(|e| (400, e))?;
        sampled.push(serde_json::json!({
            "idx": idx,
            "data": BASE64.encode(&shares[idx]),
            "leaf": hex::encode(share_hashes[idx]),
            "proof": proof.path.iter().map(|n| serde_json::json!({"left": n.left, "sibling": hex::encode(n.sibling)})).collect::<Vec<_>>(),
            "pedersen_proof": pedersen_proof.path.iter().map(|n| serde_json::json!({"left": n.left, "hash": hex::encode(n.sibling)})).collect::<Vec<_>>(),
        }));
    }
    let attestations: Vec<_> = meta
        .attestations
        .iter()
        .map(|a| serde_json::json!({"pk": a.pk, "sig": a.sig}))
        .collect();
    let body = serde_json::json!({
        "namespace": meta.namespace,
        "hash": meta.hash,
        "size": meta.size,
        "share_root": meta.share_root,
        "pedersen_root": meta.pedersen_root,
        "data_shards": meta.data_shards,
        "parity_shards": meta.parity_shards,
        "shares": sampled,
        "attestations": attestations,
    });
    Ok((200, body.to_string()))
}

fn handle_prove_storage(
    req: &HttpRequest,
    cfg: &BlobServiceConfig,
) -> Result<(u16, String), (u16, String)> {
    let evidence_log = cfg.base_dir.join("evidence.jsonl");
    let evidence_outbox = cfg.base_dir.join("evidence_outbox.jsonl");
    let segs: Vec<&str> = req.path.split('/').collect();
    if segs.len() < 5 {
        return Err((404, "Not Found".into()));
    }
    let namespace = sanitize_token(segs[2]).ok_or((404, "Not Found".into()))?;
    let hash = sanitize_token(segs[3]).ok_or((404, "Not Found".into()))?;
    let idx: usize = segs[4].parse().map_err(|_| (400, "invalid index".into()))?;
    let meta = load_blob_meta(&cfg.base_dir, &namespace, &hash)
        .map_err(|e| (400, e.to_string()))?
        .ok_or((404, "Not Found".into()))?;
    let total = meta.data_shards as usize + meta.parity_shards as usize;
    let shares = match read_shares(&cfg.base_dir, &namespace, &hash, total) {
        Ok(s) => s,
        Err(_err) => {
            if let Some(pk) = pick_slash_target(&meta) {
                record_slash_with_registry(
                    &cfg.membership_policy,
                    &cfg.stake_registry_path,
                    &cfg.token_mode_contract,
                    &cfg.token_oracle_rpc,
                    &pk,
                    "blob-missing",
                );
                append_evidence(
                    &evidence_log,
                    &meta.namespace,
                    &meta.hash,
                    &pk,
                    "blob-missing",
                );
                let ev = availability::build_missing_share_evidence(&namespace, &hash, idx);
                append_availability_evidence(&evidence_outbox, &ev);
            }
            return Err((404, "Not Found".into()));
        }
    };
    if idx >= shares.len() {
        return Err((404, "Not Found".into()));
    }
    let share_hashes = share_hashes(&shares);
    let proof = build_merkle_proof(&share_hashes, idx).ok_or((400, "bad index".into()))?;
    let body = serde_json::json!({
        "namespace": meta.namespace,
        "hash": meta.hash,
        "idx": idx,
        "share": BASE64.encode(&shares[idx]),
        "share_root": meta.share_root,
        "leaf": hex::encode(share_hashes[idx]),
        "proof": proof.path.iter().map(|n| serde_json::json!({"left": n.left, "sibling": hex::encode(n.sibling)})).collect::<Vec<_>>(),
    });
    Ok((200, body.to_string()))
}

struct PayloadCache {
    seen: HashSet<[u8; 32]>,
    order: VecDeque<[u8; 32]>,
    metrics: Arc<Metrics>,
}

impl PayloadCache {
    fn new(metrics: Arc<Metrics>) -> Self {
        Self {
            seen: HashSet::new(),
            order: VecDeque::new(),
            metrics,
        }
    }

    fn insert(&mut self, digest: [u8; 32]) -> bool {
        if self.seen.contains(&digest) {
            return false;
        }
        self.seen.insert(digest);
        self.order.push_back(digest);
        if self.order.len() > SEEN_CACHE_LIMIT {
            if let Some(old) = self.order.pop_front() {
                self.seen.remove(&old);
                self.metrics.inc_lrucache_evictions();
            }
        }
        true
    }
}

#[derive(Default)]
struct Metrics {
    anchors_received_total: AtomicU64,
    anchors_verified_total: AtomicU64,
    invalid_envelopes_total: AtomicU64,
    lrucache_evictions_total: AtomicU64,
    finality_events_total: AtomicU64,
    gossipsub_rejects_total: AtomicU64,
}

impl Metrics {
    fn inc_anchors_received(&self) {
        self.anchors_received_total.fetch_add(1, Ordering::Relaxed);
    }

    fn inc_anchors_verified(&self) {
        self.anchors_verified_total.fetch_add(1, Ordering::Relaxed);
    }

    fn inc_invalid_envelopes(&self) {
        self.invalid_envelopes_total.fetch_add(1, Ordering::Relaxed);
    }

    fn inc_lrucache_evictions(&self) {
        self.lrucache_evictions_total
            .fetch_add(1, Ordering::Relaxed);
    }

    fn inc_finality_events(&self) {
        self.finality_events_total.fetch_add(1, Ordering::Relaxed);
    }

    fn inc_gossipsub_rejects(&self) {
        self.gossipsub_rejects_total.fetch_add(1, Ordering::Relaxed);
    }

    fn render(&self) -> String {
        format!(
            "# TYPE anchors_received_total counter\nanchors_received_total {}\n\
# TYPE anchors_verified_total counter\nanchors_verified_total {}\n\
# TYPE invalid_envelopes_total counter\ninvalid_envelopes_total {}\n\
# TYPE lrucache_evictions_total counter\nlrucache_evictions_total {}\n\
# TYPE finality_events_total counter\nfinality_events_total {}\n\
# TYPE gossipsub_rejects_total counter\ngossipsub_rejects_total {}\n",
            self.anchors_received_total.load(Ordering::Relaxed),
            self.anchors_verified_total.load(Ordering::Relaxed),
            self.invalid_envelopes_total.load(Ordering::Relaxed),
            self.lrucache_evictions_total.load(Ordering::Relaxed),
            self.finality_events_total.load(Ordering::Relaxed),
            self.gossipsub_rejects_total.load(Ordering::Relaxed),
        )
    }
}

/// Errors surfaced by the networking runtime.
#[derive(Debug)]
pub enum NetworkError {
    /// Ledger anchor could not be produced or verified.
    Anchor(String),
    /// JSON or base64 codec failure.
    Codec(String),
    /// Filesystem interaction failure.
    Io(String),
    /// Key derivation or signature failure.
    Key(String),
    /// Underlying libp2p API returned an error.
    Libp2p(String),
    /// Evidence sender not permitted.
    Policy(String),
}

impl std::fmt::Display for NetworkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Anchor(msg) => write!(f, "anchor error: {msg}"),
            Self::Codec(msg) => write!(f, "codec error: {msg}"),
            Self::Io(msg) => write!(f, "I/O error: {msg}"),
            Self::Key(msg) => write!(f, "key error: {msg}"),
            Self::Libp2p(msg) => write!(f, "libp2p error: {msg}"),
            Self::Policy(msg) => write!(f, "policy error: {msg}"),
        }
    }
}

impl std::error::Error for NetworkError {}

impl From<AnchorCodecError> for NetworkError {
    fn from(err: AnchorCodecError) -> Self {
        Self::Codec(err.to_string())
    }
}

impl From<KeyError> for NetworkError {
    fn from(err: KeyError) -> Self {
        Self::Key(err.to_string())
    }
}

impl From<io::Error> for NetworkError {
    fn from(err: io::Error) -> Self {
        Self::Io(err.to_string())
    }
}

#[derive(NetworkBehaviour)]
struct JrocBehaviour {
    pub gossipsub: gossipsub::Behaviour,
    pub identify: identify::Behaviour,
    pub kademlia: kad::Behaviour<MemoryStore>,
}

/// Starts the JULIAN Protocol libp2p node and continues until Ctrl+C.
///
/// The swarm listens on `cfg.listen_addr`, optionally dials bootstrap peers,
/// recomputes anchors at `cfg.broadcast_interval`, and emits gossip messages
/// for every new anchor. Incoming envelopes are verified and reconciled with
/// the local ledger according to `cfg.quorum`.
pub async fn run_network(cfg: NetConfig) -> Result<(), NetworkError> {
    crate::net::refresh_migration_mode_from_env();
    let local_key_bytes = cfg.key_material.verifying.to_bytes();
    if !policy_permits(cfg.membership_policy.as_ref(), &local_key_bytes) {
        return Err(NetworkError::Key(
            "local key not permitted by identity policy".to_string(),
        ));
    }
    fs::create_dir_all(&cfg.log_dir).map_err(|err| {
        NetworkError::Io(format!(
            "failed to create log dir {}: {err}",
            cfg.log_dir.display()
        ))
    })?;
    if let Some(blob_dir) = cfg.blob_dir.as_ref() {
        fs::create_dir_all(blob_dir).map_err(|err| {
            NetworkError::Io(format!(
                "failed to create blob dir {}: {err}",
                blob_dir.display()
            ))
        })?;
    }
    let mut swarm = build_swarm(&cfg)?;
    Swarm::listen_on(&mut swarm, cfg.listen_addr.clone())
        .map_err(|err| NetworkError::Libp2p(format!("{err:?}")))?;
    let mut bootstrap_peers = 0usize;
    for addr in &cfg.bootstraps {
        if let Some(peer_id) = extract_peer_id(addr) {
            swarm
                .behaviour_mut()
                .kademlia
                .add_address(&peer_id, addr.clone());
            swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
            bootstrap_peers += 1;
        }
        if let Err(err) = Swarm::dial(&mut swarm, addr.clone()) {
            eprintln!("dial {addr} failed: {err}");
        }
    }
    if bootstrap_peers > 0 {
        match swarm.behaviour_mut().kademlia.bootstrap() {
            Ok(_) => println!("QSYS|mod=NET|evt=KAD_BOOTSTRAP|peers={bootstrap_peers}"),
            Err(err) => eprintln!("kademlia bootstrap failed: {err:?}"),
        }
    }

    let mut ticker = time::interval(cfg.broadcast_interval);
    ticker.set_missed_tick_behavior(time::MissedTickBehavior::Delay);

    let metrics = cfg.metrics.clone();
    if let Some(addr) = cfg.metrics_addr {
        let metrics_clone = metrics.clone();
        tokio::spawn(async move {
            if let Err(err) = run_metrics_server(addr, metrics_clone).await {
                eprintln!("metrics server error: {err}");
            }
        });
        println!("QSYS|mod=METRICS|evt=LISTEN|addr={addr}");
    }

    if let (Some(blob_dir), Some(blob_listen)) = (cfg.blob_dir.clone(), cfg.blob_listen) {
        let blob_cfg = BlobServiceConfig {
            base_dir: blob_dir,
            listen: blob_listen,
            max_bytes: cfg.max_blob_bytes,
            retention_days: cfg.blob_retention_days,
            policies: cfg.blob_policies.clone(),
            signing: cfg.key_material.signing.clone(),
            verifying_b64: encode_public_key_base64(&cfg.key_material.verifying),
            stake_registry_path: cfg.stake_registry_path.clone(),
            token_mode_contract: cfg.token_mode_contract.clone(),
            token_oracle_rpc: cfg.token_oracle_rpc.clone(),
            membership_policy: cfg.membership_policy.clone(),
            auth_token: cfg.blob_auth_token.clone(),
            max_concurrency: cfg.blob_max_concurrency,
            request_timeout: cfg.blob_request_timeout,
            rate_limits: Arc::new(Mutex::new(HashMap::new())),
            da_publish: da_publish_config_from_env(),
        };
        tokio::spawn(run_blob_service(blob_cfg));
    }

    let mut seen_payloads = PayloadCache::new(metrics.clone());
    let mut invalid_counters: HashMap<libp2p::PeerId, usize> = HashMap::new();
    let mut last_payload = Vec::new();
    let mut last_publish: Option<Instant> = None;
    let mut broadcast_counter: u64 = 0;
    let mut bft_state = BftState::new(cfg.bft_round_ms);
    let mut anchor_votes: HashMap<[u8; 32], (Instant, HashMap<Vec<u8>, LedgerAnchor>)> =
        HashMap::new();

    let local_peer = cfg.key_material.libp2p.public().to_peer_id();

    println!(
        "QSYS|mod=NET|evt=LISTEN|node={} peer={} addr={} topic={}",
        cfg.node_id,
        local_peer,
        cfg.listen_addr,
        cfg.anchor_topic.hash()
    );

    loop {
        select! {
            _ = ticker.tick() => {
                if cfg.bft_enabled {
                    if let Err(err) = bft_tick(
                        &mut swarm,
                        &cfg,
                        &mut bft_state,
                        &mut last_payload,
                        &mut last_publish,
                        &mut broadcast_counter,
                        &metrics,
                    )
                    .await
                    {
                        metrics.inc_gossipsub_rejects();
                        eprintln!("bft tick error: {err}");
                    }
                } else {
                    if let Err(err) = broadcast_local_anchor(
                        &mut swarm,
                        &cfg,
                        &mut last_payload,
                        &mut last_publish,
                        &mut broadcast_counter,
                        &metrics,
                    )
                    .await
                    {
                        metrics.inc_gossipsub_rejects();
                        eprintln!("broadcast error: {err}");
                    }
                }
                if let Err(err) = broadcast_evidence(&mut swarm, &cfg) {
                    eprintln!("evidence broadcast error: {err}");
                }
            }
            event = swarm.select_next_some() => {
                if let Err(err) = handle_event(
                    event,
                    &mut swarm,
                    &cfg,
                    &mut seen_payloads,
                    &mut invalid_counters,
                    &mut bft_state,
                    &mut anchor_votes,
                    &metrics
                ).await {
                    eprintln!("network error: {err}");
                }
            }
            _ = signal::ctrl_c() => {
                println!("Power-House node shutting down");
                return Ok(());
            }
        }
    }
}

fn build_swarm(cfg: &NetConfig) -> Result<Swarm<JrocBehaviour>, NetworkError> {
    let identity = cfg.key_material.libp2p.clone();

    let builder = SwarmBuilder::with_existing_identity(identity)
        .with_tokio()
        .with_tcp(
            tcp::Config::default(),
            noise::Config::new,
            yamux::Config::default,
        )
        .map_err(|err| NetworkError::Libp2p(format!("{err:?}")))?
        .with_behaviour(|key| {
            build_behaviour(key, &cfg.bridge_topics).map_err(|err| {
                let boxed: Box<dyn std::error::Error + Send + Sync> = Box::new(err);
                boxed
            })
        })
        .map_err(|err| NetworkError::Libp2p(format!("{err:?}")))?
        .with_swarm_config(|cfg| cfg.with_idle_connection_timeout(Duration::from_secs(60)));

    Ok(builder.build())
}

fn build_behaviour(
    key: &identity::Keypair,
    topics: &[IdentTopic],
) -> Result<JrocBehaviour, NetworkError> {
    let peer_id = key.public().to_peer_id();

    let gossipsub_config = gossipsub::ConfigBuilder::default()
        .validation_mode(ValidationMode::Strict)
        .message_id_fn(|message: &gossipsub::Message| {
            let mut hasher = Sha256::new();
            hasher.update(&message.data);
            gossipsub::MessageId::from(hasher.finalize().to_vec())
        })
        .build()
        .map_err(|err| NetworkError::Libp2p(format!("{err:?}")))?;

    let mut gossipsub =
        gossipsub::Behaviour::new(MessageAuthenticity::Signed(key.clone()), gossipsub_config)
            .map_err(|err| NetworkError::Libp2p(format!("{err:?}")))?;
    for topic in topics {
        gossipsub
            .subscribe(topic)
            .map_err(|err| NetworkError::Libp2p(format!("{err:?}")))?;
    }
    gossipsub
        .subscribe(&TOPIC_VOTES)
        .map_err(|err| NetworkError::Libp2p(format!("{err:?}")))?;

    let identify_config = identify::Config::new("mfenx-powerhouse/1.0.0".into(), key.public())
        .with_push_listen_addr_updates(true);
    let identify = identify::Behaviour::new(identify_config);

    let store = MemoryStore::new(peer_id);
    let kademlia = kad::Behaviour::with_config(peer_id, store, kad::Config::default());

    Ok(JrocBehaviour {
        gossipsub,
        identify,
        kademlia,
    })
}

fn evidence_outbox(cfg: &NetConfig) -> Option<PathBuf> {
    cfg.blob_dir
        .as_ref()
        .map(|d| d.join("evidence_outbox.jsonl"))
}

fn broadcast_evidence(
    swarm: &mut Swarm<JrocBehaviour>,
    cfg: &NetConfig,
) -> Result<(), NetworkError> {
    let Some(path) = evidence_outbox(cfg) else {
        return Ok(());
    };
    let contents = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(err) => {
            if err.kind() == io::ErrorKind::NotFound {
                return Ok(());
            }
            return Err(NetworkError::Io(err.to_string()));
        }
    };
    if contents.trim().is_empty() {
        return Ok(());
    }
    let lines: Vec<&str> = contents.lines().filter(|l| !l.trim().is_empty()).collect();
    if lines.is_empty() {
        return Ok(());
    }
    fs::write(&path, "").map_err(|e| NetworkError::Io(e.to_string()))?;
    for line in lines {
        if let Ok(record) = serde_json::from_str::<EvidenceRecord>(line) {
            let env = sign_evidence(&cfg.key_material.signing, &record);
            if let Ok(msg) = serde_json::to_vec(&env) {
                let _ = swarm
                    .behaviour_mut()
                    .gossipsub
                    .publish(TOPIC_EVIDENCE.clone(), msg);
            }
        } else if let Ok(av) = serde_json::from_str::<AvailabilityEvidence>(line) {
            let msg = serde_json::to_vec(&av).map_err(|e| NetworkError::Codec(e.to_string()))?;
            let _ = swarm
                .behaviour_mut()
                .gossipsub
                .publish(TOPIC_EVIDENCE.clone(), msg);
        } else if let Ok(rf) = serde_json::from_str::<RollupFaultEvidence>(line) {
            let msg = serde_json::to_vec(&rf).map_err(|e| NetworkError::Codec(e.to_string()))?;
            let _ = swarm
                .behaviour_mut()
                .gossipsub
                .publish(TOPIC_EVIDENCE.clone(), msg);
        }
    }
    Ok(())
}

fn extract_peer_id(addr: &Multiaddr) -> Option<PeerId> {
    addr.iter().find_map(|proto| match proto {
        Protocol::P2p(peer_id) => Some(peer_id),
        _ => None,
    })
}

fn handle_evidence_message(cfg: &NetConfig, data: &[u8]) -> Result<(), NetworkError> {
    // Try to parse signed evidence envelope first.
    if let Ok(env) = serde_json::from_slice::<EvidenceEnvelope>(data) {
        let payload =
            serde_json::to_vec(&env.evidence).map_err(|e| NetworkError::Codec(e.to_string()))?;
        verify_signature_base64(&env.public_key, &payload, &env.signature)?;
        // Basic validation: ensure peer is permitted by policy.
        let vk = decode_public_key_base64(&env.public_key)
            .map_err(|e| NetworkError::Codec(e.to_string()))?;
        if !policy_permits(cfg.membership_policy.as_ref(), &vk.to_bytes()) {
            return Err(NetworkError::Policy("evidence sender not permitted".into()));
        }
        if let Some(path) = cfg.blob_dir.as_ref().map(|d| d.join("evidence.jsonl")) {
            append_evidence(
                &path,
                &env.evidence.namespace,
                &env.evidence.blob_hash,
                &env.evidence.pk,
                &env.evidence.reason,
            );
        }
        record_slash_with_registry(
            &cfg.membership_policy,
            &cfg.stake_registry_path,
            &cfg.token_mode_contract,
            &cfg.token_oracle_rpc,
            &env.evidence.pk,
            &env.evidence.reason,
        );
        return Ok(());
    }
    // Fallback: raw availability or rollup evidence.
    if let Ok(av) = serde_json::from_slice::<AvailabilityEvidence>(data) {
        if let Some(path) = cfg.blob_dir.as_ref().map(|d| d.join("evidence.jsonl")) {
            append_availability_evidence(&path, &av);
        }
        if let Some(pk) = pick_slash_target(&BlobMeta {
            namespace: av.namespace.clone(),
            hash: av.blob_hash.clone(),
            size: 0,
            data_shards: 0,
            parity_shards: 0,
            share_root: String::new(),
            pedersen_root: None,
            attestations: Vec::new(),
            publisher_pk: None,
            da_receipt: None,
        }) {
            record_slash_with_registry(
                &cfg.membership_policy,
                &cfg.stake_registry_path,
                &cfg.token_mode_contract,
                &cfg.token_oracle_rpc,
                &pk,
                "availability-fault",
            );
        }
        return Ok(());
    }
    if let Ok(rf) = serde_json::from_slice::<RollupFaultEvidence>(data) {
        if let Some(path) = cfg.blob_dir.as_ref().map(|d| d.join("evidence.jsonl")) {
            append_availability_evidence(
                &path,
                &AvailabilityEvidence {
                    namespace: rf.namespace.clone(),
                    blob_hash: rf.commitment.clone(),
                    idx: 0,
                    share: rf.payload.clone(),
                    reason: rf.reason.clone(),
                },
            );
        }
        if let Some(pk) = pick_slash_target(&BlobMeta {
            namespace: rf.namespace.clone(),
            hash: rf.commitment.clone(),
            size: 0,
            data_shards: 0,
            parity_shards: 0,
            share_root: String::new(),
            pedersen_root: None,
            attestations: Vec::new(),
            publisher_pk: None,
            da_receipt: None,
        }) {
            record_slash_with_registry(
                &cfg.membership_policy,
                &cfg.stake_registry_path,
                &cfg.token_mode_contract,
                &cfg.token_oracle_rpc,
                &pk,
                "rollup-fault",
            );
        }
        return Ok(());
    }
    Ok(())
}

fn handle_vote_message(
    cfg: &NetConfig,
    bft_state: &mut BftState,
    data: &[u8],
) -> Result<(), NetworkError> {
    let vote: AnchorVoteJson =
        serde_json::from_slice(data).map_err(|err| NetworkError::Codec(err.to_string()))?;
    vote.validate()?;
    let payload = vote_payload_bytes(vote.round, &vote.anchor_hash);
    verify_signature_base64(&vote.public_key, &payload, &vote.signature)?;
    let remote_verifying = decode_public_key_base64(&vote.public_key)
        .map_err(|err| NetworkError::Codec(err.to_string()))?;
    let remote_key_bytes = remote_verifying.to_bytes();
    if !policy_permits(cfg.membership_policy.as_ref(), &remote_key_bytes) {
        return Ok(());
    }
    bft_state.maybe_advance(cfg.bft_round_ms);
    if vote.round != bft_state.round {
        return Ok(());
    }
    let total = bft_state.record_vote(&vote.anchor_hash, &remote_key_bytes);
    println!("QSYS|mod=BFT|evt=VOTE|round={} votes={}", vote.round, total);
    Ok(())
}

fn build_anchor_payload(cfg: &NetConfig) -> Result<(AnchorJson, Vec<u8>, usize), NetworkError> {
    let ledger = load_anchor_from_logs(&cfg.log_dir)?;
    let timestamp_ms = now_millis();
    let anchor_json = AnchorJson::from_ledger(
        cfg.node_id.clone(),
        cfg.quorum,
        &ledger,
        timestamp_ms,
        latest_da_commitments(&cfg.blob_dir),
        evidence_root(&cfg.blob_dir),
    )?;
    let payload =
        serde_json::to_vec(&anchor_json).map_err(|err| NetworkError::Codec(err.to_string()))?;
    Ok((anchor_json, payload, ledger.entries.len()))
}

async fn publish_anchor_payload(
    swarm: &mut Swarm<JrocBehaviour>,
    cfg: &NetConfig,
    anchor_json: AnchorJson,
    payload: Vec<u8>,
    entries_len: usize,
    last_payload: &mut Vec<u8>,
    last_publish: &mut Option<Instant>,
    broadcast_counter: &mut u64,
    metrics: &Arc<Metrics>,
) -> Result<(), NetworkError> {
    if *last_payload == payload {
        return Ok(());
    }
    if let Some(prev) = last_publish {
        if prev.elapsed() < cfg.broadcast_interval {
            return Ok(());
        }
    }
    let signature = sign_payload(&cfg.key_material.signing, &payload);
    let signature_b64 = encode_signature_base64(&signature);
    let envelope = AnchorEnvelope {
        schema: SCHEMA_ENVELOPE.to_string(),
        schema_version: ENVELOPE_SCHEMA_VERSION,
        public_key: encode_public_key_base64(&cfg.key_material.verifying),
        node_id: cfg.node_id.clone(),
        payload: BASE64.encode(&payload),
        signature: signature_b64.clone(),
    };
    let message =
        serde_json::to_vec(&envelope).map_err(|err| NetworkError::Codec(err.to_string()))?;
    let message_clone = message.clone();
    match swarm
        .behaviour_mut()
        .gossipsub
        .publish(cfg.anchor_topic.clone(), message)
    {
        Ok(_) => {
            NO_GOSSIP_PEERS_LOGGED.store(false, Ordering::Relaxed);
        }
        Err(PublishError::InsufficientPeers) => {
            if !NO_GOSSIP_PEERS_LOGGED.swap(true, Ordering::Relaxed) {
                println!("QSYS|mod=ANCHOR|evt=STANDBY|reason=awaiting_peers");
            }
            return Ok(());
        }
        Err(PublishError::Duplicate) => {
            return Ok(());
        }
        Err(err) => {
            metrics.inc_gossipsub_rejects();
            return Err(NetworkError::Libp2p(err.to_string()));
        }
    }
    bridge_anchor_message(
        swarm,
        cfg,
        &cfg.anchor_topic.hash(),
        &message_clone,
        metrics,
    );
    *last_payload = payload;
    *last_publish = Some(Instant::now());
    println!("QSYS|mod=ANCHOR|evt=BROADCAST|entries={}", entries_len);
    if let Some(interval) = cfg.checkpoint_interval {
        if interval > 0 {
            *broadcast_counter = broadcast_counter.saturating_add(1);
            if *broadcast_counter % interval == 0 {
                let checkpoint = AnchorCheckpoint::new(
                    *broadcast_counter,
                    anchor_json.clone(),
                    vec![CheckpointSignature {
                        node_id: cfg.node_id.clone(),
                        public_key: encode_public_key_base64(&cfg.key_material.verifying),
                        signature: signature_b64,
                    }],
                    latest_log_cutoff(&cfg.log_dir),
                );
                if let Err(err) = write_checkpoint(&cfg.log_dir.join("checkpoints"), &checkpoint) {
                    eprintln!("checkpoint write failed: {err}");
                } else {
                    println!(
                        "QSYS|mod=CHECKPOINT|evt=RECORDED|epoch={} entries={}",
                        checkpoint.epoch, entries_len
                    );
                }
            }
        }
    }
    Ok(())
}

async fn broadcast_local_anchor(
    swarm: &mut Swarm<JrocBehaviour>,
    cfg: &NetConfig,
    last_payload: &mut Vec<u8>,
    last_publish: &mut Option<Instant>,
    broadcast_counter: &mut u64,
    metrics: &Arc<Metrics>,
) -> Result<(), NetworkError> {
    if !policy_permits(
        cfg.membership_policy.as_ref(),
        &cfg.key_material.verifying.to_bytes(),
    ) {
        return Err(NetworkError::Key(
            "local key not permitted by identity policy".to_string(),
        ));
    }
    let (anchor_json, payload, entries_len) = build_anchor_payload(cfg)?;
    publish_anchor_payload(
        swarm,
        cfg,
        anchor_json,
        payload,
        entries_len,
        last_payload,
        last_publish,
        broadcast_counter,
        metrics,
    )
    .await
}

async fn broadcast_anchor_vote(
    swarm: &mut Swarm<JrocBehaviour>,
    cfg: &NetConfig,
    round: u64,
    anchor_hash: &str,
    metrics: &Arc<Metrics>,
) -> Result<(), NetworkError> {
    let payload = vote_payload_bytes(round, anchor_hash);
    let signature = sign_payload(&cfg.key_material.signing, &payload);
    let signature_b64 = encode_signature_base64(&signature);
    let vote = AnchorVoteJson {
        schema: SCHEMA_VOTE.to_string(),
        network: NETWORK_ID.to_string(),
        round,
        anchor_hash: anchor_hash.to_string(),
        public_key: encode_public_key_base64(&cfg.key_material.verifying),
        signature: signature_b64,
    };
    let message = serde_json::to_vec(&vote).map_err(|err| NetworkError::Codec(err.to_string()))?;
    match swarm
        .behaviour_mut()
        .gossipsub
        .publish(TOPIC_VOTES.clone(), message)
    {
        Ok(_) => Ok(()),
        Err(PublishError::InsufficientPeers) => Ok(()),
        Err(PublishError::Duplicate) => Ok(()),
        Err(err) => {
            metrics.inc_gossipsub_rejects();
            Err(NetworkError::Libp2p(err.to_string()))
        }
    }
}

async fn bft_tick(
    swarm: &mut Swarm<JrocBehaviour>,
    cfg: &NetConfig,
    bft_state: &mut BftState,
    last_payload: &mut Vec<u8>,
    last_publish: &mut Option<Instant>,
    broadcast_counter: &mut u64,
    metrics: &Arc<Metrics>,
) -> Result<(), NetworkError> {
    if !policy_permits(
        cfg.membership_policy.as_ref(),
        &cfg.key_material.verifying.to_bytes(),
    ) {
        return Err(NetworkError::Key(
            "local key not permitted by identity policy".to_string(),
        ));
    }
    bft_state.maybe_advance(cfg.bft_round_ms);
    let round = bft_state.round;
    let (anchor_json, payload, entries_len) = build_anchor_payload(cfg)?;
    let anchor_hash = anchor_json
        .fold_digest
        .clone()
        .unwrap_or_else(|| anchor_payload_hash(&payload));

    broadcast_anchor_vote(swarm, cfg, round, &anchor_hash, metrics).await?;
    let local_key = cfg.key_material.verifying.to_bytes();
    let votes = bft_state.record_vote(&anchor_hash, &local_key);

    if votes >= cfg.quorum {
        publish_anchor_payload(
            swarm,
            cfg,
            anchor_json,
            payload,
            entries_len,
            last_payload,
            last_publish,
            broadcast_counter,
            metrics,
        )
        .await?;
        println!("QSYS|mod=BFT|evt=QUORUM|round={} votes={}", round, votes);
    } else {
        println!(
            "QSYS|mod=BFT|evt=WAITING|round={} votes={}/{}",
            round, votes, cfg.quorum
        );
    }
    Ok(())
}

async fn handle_event(
    event: SwarmEvent<JrocBehaviourEvent>,
    swarm: &mut Swarm<JrocBehaviour>,
    cfg: &NetConfig,
    seen_payloads: &mut PayloadCache,
    invalid_counters: &mut HashMap<libp2p::PeerId, usize>,
    bft_state: &mut BftState,
    anchor_votes: &mut HashMap<[u8; 32], (Instant, HashMap<Vec<u8>, LedgerAnchor>)>,
    metrics: &Arc<Metrics>,
) -> Result<(), NetworkError> {
    #[allow(clippy::collapsible_match, clippy::single_match)]
    match event {
        SwarmEvent::NewListenAddr { address, .. } => {
            println!("QSYS|mod=NET|evt=LISTEN|addr={address}");
        }
        SwarmEvent::Behaviour(JrocBehaviourEvent::Gossipsub(event)) => match event {
            gossipsub::Event::Message {
                propagation_source,
                message,
                ..
            } => {
                if message.topic == TOPIC_EVIDENCE.hash() {
                    handle_evidence_message(cfg, &message.data)?;
                    return Ok(());
                }
                if message.topic == TOPIC_VOTES.hash() {
                    if cfg.bft_enabled {
                        handle_vote_message(cfg, bft_state, &message.data)?;
                    }
                    return Ok(());
                }
                if !is_anchor_topic(cfg, &message.topic) {
                    return Ok(());
                }
                metrics.inc_anchors_received();
                if message.data.len() > MAX_ENVELOPE_BYTES {
                    metrics.inc_gossipsub_rejects();
                    record_invalid(invalid_counters, propagation_source, metrics);
                    return Ok(());
                }
                let digest = sha256_digest(&message.data);
                if !seen_payloads.insert(digest) {
                    metrics.inc_gossipsub_rejects();
                    return Ok(());
                }
                let envelope: AnchorEnvelope = serde_json::from_slice(&message.data)
                    .map_err(|err| NetworkError::Codec(err.to_string()))?;
                envelope.validate()?;
                let payload = BASE64
                    .decode(envelope.payload.as_bytes())
                    .map_err(|err| NetworkError::Codec(err.to_string()))?;
                if payload.len() > MAX_ENVELOPE_BYTES {
                    metrics.inc_gossipsub_rejects();
                    record_invalid(invalid_counters, propagation_source, metrics);
                    return Ok(());
                }
                verify_signature_base64(&envelope.public_key, &payload, &envelope.signature)?;
                let remote_verifying = decode_public_key_base64(&envelope.public_key)
                    .map_err(|err| NetworkError::Codec(err.to_string()))?;
                let remote_key_bytes = remote_verifying.to_bytes();
                if !policy_permits(cfg.membership_policy.as_ref(), &remote_key_bytes) {
                    metrics.inc_gossipsub_rejects();
                    record_invalid(invalid_counters, propagation_source, metrics);
                    println!(
                        "rejecting peer {}: identity not permitted by policy",
                        envelope.node_id
                    );
                    return Ok(());
                }
                let payload_str = std::str::from_utf8(&payload)
                    .map_err(|err| NetworkError::Codec(err.to_string()))?;
                let anchor_json = AnchorJson::from_json_str(payload_str)
                    .map_err(|err| NetworkError::Codec(err.to_string()))?;
                if anchor_json.network != NETWORK_ID {
                    metrics.inc_gossipsub_rejects();
                    record_invalid(invalid_counters, propagation_source, metrics);
                    return Ok(());
                }
                if anchor_json.entries.len() > MAX_ANCHOR_ENTRIES {
                    metrics.inc_gossipsub_rejects();
                    record_invalid(invalid_counters, propagation_source, metrics);
                    return Ok(());
                }
                // DA gating: require commitments only after non-genesis entries exist,
                // then verify share roots + attestation QC; require persisted QC.
                if anchor_json.da_commitments.is_empty() {
                    if anchor_json.entries.len() > 1 {
                        metrics.inc_gossipsub_rejects();
                        println!(
                            "rejecting peer {}: missing DA commitments in anchor (entries={})",
                            envelope.node_id,
                            anchor_json.entries.len()
                        );
                        return Ok(());
                    }
                } else if let Some(blob_dir) = cfg.blob_dir.as_ref() {
                    for da in &anchor_json.da_commitments {
                        let meta = load_blob_meta(blob_dir, &da.namespace, &da.blob_hash)
                            .ok()
                            .flatten();
                        let Some(meta) = meta else {
                            metrics.inc_gossipsub_rejects();
                            println!(
                                "rejecting peer {}: missing blob {} in {}",
                                envelope.node_id, da.blob_hash, da.namespace
                            );
                            return Ok(());
                        };
                        if meta.share_root != da.share_root {
                            metrics.inc_gossipsub_rejects();
                            println!(
                                "rejecting peer {}: share_root mismatch for {}",
                                envelope.node_id, da.blob_hash
                            );
                            return Ok(());
                        }
                        if let (Some(meta_p), Some(da_p)) = (&meta.pedersen_root, &da.pedersen_root)
                        {
                            if meta_p != da_p {
                                metrics.inc_gossipsub_rejects();
                                println!(
                                    "rejecting peer {}: pedersen_root mismatch for {}",
                                    envelope.node_id, da.blob_hash
                                );
                                return Ok(());
                            }
                        }
                        if da.pedersen_root.is_none() {
                            metrics.inc_gossipsub_rejects();
                            println!(
                                "rejecting peer {}: pedersen_root missing for {}",
                                envelope.node_id, da.blob_hash
                            );
                            return Ok(());
                        }
                        let attestations: Vec<_> = meta
                            .attestations
                            .iter()
                            .map(|a| a.to_attestation(&meta.share_root, &meta.pedersen_root))
                            .collect();
                        let qc =
                            aggregate_attestations(&attestations, cfg.attestation_quorum, |pk| {
                                lookup_stake(cfg, pk)
                            })
                            .map_err(|e| NetworkError::Codec(e.to_string()))?;
                        if !qc.quorum_reached {
                            metrics.inc_gossipsub_rejects();
                            println!(
                                "rejecting peer {}: DA quorum not met for {}",
                                envelope.node_id, da.blob_hash
                            );
                            return Ok(());
                        }
                        // Persist QC for evidence and rollup settlement.
                        if let Some(log_dir) = cfg.blob_dir.as_ref() {
                            let qc_path = log_dir
                                .join(&da.namespace)
                                .join(format!("{}.qc", da.blob_hash));
                            if let Some(parent) = qc_path.parent() {
                                let _ = fs::create_dir_all(parent);
                            }
                            if let Ok(bytes) = serde_json::to_vec(&qc) {
                                let _ = fs::write(&qc_path, bytes);
                            }
                            // Reward attesters (best-effort).
                            if let Some(path) = &cfg.stake_registry_path {
                                if let Ok(mut reg) = StakeRegistry::load(path) {
                                    for signer in &qc.signers {
                                        reg.credit_reward(signer, 1);
                                    }
                                    let _ = reg.save(path);
                                }
                            }
                        }
                        // Require QC file to exist (stake-weighted gating).
                        if let Some(log_dir) = cfg.blob_dir.as_ref() {
                            let qc_path = log_dir
                                .join(&da.namespace)
                                .join(format!("{}.qc", da.blob_hash));
                            if !qc_path.exists() {
                                metrics.inc_gossipsub_rejects();
                                println!(
                                    "rejecting peer {}: missing QC for {}",
                                    envelope.node_id, da.blob_hash
                                );
                                return Ok(());
                            }
                        }
                    }
                }
                let remote_anchor = anchor_json.clone().into_ledger()?;
                let local_anchor = load_anchor_from_logs(&cfg.log_dir)?;
                let local_key_bytes = cfg.key_material.verifying.to_bytes();

                let remote_digest = anchor_digest(&remote_anchor);
                let local_digest = anchor_digest(&local_anchor);
                if remote_digest != local_digest {
                    println!(
                        "anchor divergence with peer {}: digest mismatch",
                        envelope.node_id
                    );
                    if let Err(slash_err) = cfg.membership_policy.record_slash(&remote_verifying) {
                        eprintln!(
                            "failed to record slash for {}: {}",
                            envelope.node_id, slash_err
                        );
                    } else {
                        println!(
                            "peer {} marked as slashed due to conflicting anchor",
                            envelope.node_id
                        );
                    }
                    return Ok(());
                }
                bridge_anchor_message(swarm, cfg, &message.topic, &message.data, metrics);

                if anchor_votes.len() > 64 {
                    let ttl = Duration::from_secs(300);
                    anchor_votes.retain(|_, (ts, _)| ts.elapsed() < ttl);
                }

                let now = Instant::now();
                let entry = anchor_votes
                    .entry(remote_digest)
                    .or_insert_with(|| (now, HashMap::new()));
                entry.0 = now;
                entry
                    .1
                    .entry(local_key_bytes.to_vec())
                    .or_insert_with(|| local_anchor.clone());
                entry
                    .1
                    .entry(remote_key_bytes.to_vec())
                    .or_insert_with(|| remote_anchor.clone());

                if entry.1.len() >= cfg.quorum {
                    let votes: Vec<AnchorVote<'_>> = entry
                        .1
                        .iter()
                        .map(|(key, anchor)| AnchorVote {
                            anchor,
                            public_key: key,
                        })
                        .collect();
                    match crate::reconcile_anchors_with_quorum(&votes, cfg.quorum) {
                        Ok(()) => {
                            metrics.inc_anchors_verified();
                            metrics.inc_finality_events();
                            println!(
                                "QSYS|mod=QUORUM|evt=FINALIZED|peer={} entries={}",
                                envelope.node_id,
                                remote_anchor.entries.len()
                            );
                            anchor_votes.remove(&remote_digest);
                        }
                        Err(err) => {
                            println!("anchor divergence with peer {}: {}", envelope.node_id, err);
                            if let Err(slash_err) =
                                cfg.membership_policy.record_slash(&remote_verifying)
                            {
                                eprintln!(
                                    "failed to record slash for {}: {}",
                                    envelope.node_id, slash_err
                                );
                            } else {
                                println!(
                                    "peer {} marked as slashed due to conflicting anchor",
                                    envelope.node_id
                                );
                            }
                        }
                    }
                }
            }
            _ => {}
        },
        _ => {}
    }
    Ok(())
}

fn is_anchor_topic(cfg: &NetConfig, topic: &gossipsub::TopicHash) -> bool {
    cfg.bridge_topics
        .iter()
        .any(|candidate| candidate.hash() == *topic)
}

fn bridge_anchor_message(
    swarm: &mut Swarm<JrocBehaviour>,
    cfg: &NetConfig,
    origin: &gossipsub::TopicHash,
    message: &[u8],
    metrics: &Arc<Metrics>,
) {
    if cfg.bridge_topics.len() <= 1 {
        return;
    }
    for topic in &cfg.bridge_topics {
        if topic.hash() == *origin {
            continue;
        }
        match swarm
            .behaviour_mut()
            .gossipsub
            .publish(topic.clone(), message.to_vec())
        {
            Ok(_) => {}
            Err(PublishError::InsufficientPeers) => {}
            Err(PublishError::Duplicate) => {}
            Err(err) => {
                metrics.inc_gossipsub_rejects();
                eprintln!("bridge publish error: {err}");
            }
        }
    }
}

fn sha256_digest(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let mut out = [0u8; 32];
    out.copy_from_slice(&hasher.finalize());
    out
}

fn record_invalid(
    map: &mut HashMap<libp2p::PeerId, usize>,
    peer: libp2p::PeerId,
    metrics: &Arc<Metrics>,
) {
    let entry = map.entry(peer).or_insert(0);
    *entry += 1;
    metrics.inc_invalid_envelopes();
    if *entry >= INVALID_THRESHOLD {
        println!("peer {peer} exceeded invalid envelope threshold");
        *entry = 0;
    }
}

async fn run_metrics_server(addr: SocketAddr, metrics: Arc<Metrics>) -> io::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    loop {
        let (mut stream, _) = listener.accept().await?;
        let metrics = metrics.clone();
        tokio::spawn(async move {
            if let Err(err) = respond_with_metrics(&mut stream, metrics).await {
                eprintln!("metrics connection error: {err}");
            }
        });
    }
}

async fn respond_with_metrics(
    stream: &mut tokio::net::TcpStream,
    metrics: Arc<Metrics>,
) -> io::Result<()> {
    let mut buf = [0u8; 1024];
    let mut read = 0usize;
    loop {
        if read == buf.len() {
            break;
        }
        let n = stream.read(&mut buf[read..]).await?;
        if n == 0 {
            break;
        }
        read += n;
        if read >= 4 && &buf[read - 4..read] == b"\r\n\r\n" {
            break;
        }
    }

    let request_line = std::str::from_utf8(&buf[..read]).unwrap_or("");
    let path = request_line
        .lines()
        .next()
        .and_then(|line| {
            let mut parts = line.split_whitespace();
            let _method = parts.next()?;
            let path = parts.next()?;
            Some(path)
        })
        .unwrap_or("/metrics");

    if path != "/" && path != "/metrics" {
        let response = b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n";
        stream.write_all(response).await?;
        stream.shutdown().await?;
        return Ok(());
    }

    let body = metrics.render();
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/plain; version=0.0.4\r\nContent-Length: {}\r\n\r\n{}",
        body.len(),
        body
    );
    stream.write_all(response.as_bytes()).await?;
    stream.shutdown().await
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredAttestation {
    pk: String,
    sig: String,
}

impl StoredAttestation {
    fn to_attestation(&self, share_root: &str, pedersen_root: &Option<String>) -> Attestation {
        Attestation {
            share_root: share_root.to_string(),
            pedersen_root: pedersen_root.clone(),
            public_key: self.pk.clone(),
            signature: self.sig.clone(),
            ts: Some(now_millis()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BlobMeta {
    namespace: String,
    hash: String,
    size: u64,
    data_shards: u8,
    parity_shards: u8,
    share_root: String,
    #[serde(default)]
    pedersen_root: Option<String>,
    #[serde(default)]
    attestations: Vec<StoredAttestation>,
    #[serde(default)]
    publisher_pk: Option<String>,
    #[serde(default)]
    da_receipt: Option<DaReceipt>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DaReceipt {
    provider: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    commitment: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tx_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    height: Option<u64>,
    status: String,
    updated_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    response: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DaOutboxRecord {
    namespace: String,
    hash: String,
    share_root: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pedersen_root: Option<String>,
    size: u64,
    data_shards: u8,
    parity_shards: u8,
    ts: u64,
}

fn blob_paths(base: &Path, namespace: &str, hash: &str) -> (PathBuf, PathBuf, PathBuf) {
    let ns_dir = base.join(namespace);
    let meta_path = ns_dir.join(format!("{hash}.meta"));
    let blob_path = ns_dir.join(format!("{hash}.blob"));
    let share_dir = ns_dir.join(hash).join("shares");
    (meta_path, blob_path, share_dir)
}

fn load_blob_meta(
    base: &Path,
    namespace: &str,
    hash: &str,
) -> Result<Option<BlobMeta>, NetworkError> {
    let (meta_path, _blob_path, _share_dir) = blob_paths(base, namespace, hash);
    if !meta_path.exists() {
        return Ok(None);
    }
    let bytes = fs::read(&meta_path).map_err(|err| {
        NetworkError::Io(format!("failed to read {}: {err}", meta_path.display()))
    })?;
    let meta: BlobMeta = serde_json::from_slice(&bytes).map_err(|err| {
        NetworkError::Codec(format!("invalid blob meta {}: {err}", meta_path.display()))
    })?;
    Ok(Some(meta))
}

fn save_blob_meta(base: &Path, meta: &BlobMeta) -> Result<(), NetworkError> {
    let (meta_path, _, _) = blob_paths(base, &meta.namespace, &meta.hash);
    if let Some(parent) = meta_path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            NetworkError::Io(format!("failed to create {}: {err}", parent.display()))
        })?;
    }
    let data = serde_json::to_vec_pretty(meta)
        .map_err(|err| NetworkError::Codec(format!("meta encode error: {err}")))?;
    fs::write(&meta_path, data)
        .map_err(|err| NetworkError::Io(format!("failed to write {}: {err}", meta_path.display())))
}

fn evidence_root(blob_dir: &Option<PathBuf>) -> Option<String> {
    let dir = blob_dir.as_ref()?;
    let path = dir.join("evidence.jsonl");
    let contents = std::fs::read_to_string(&path).ok()?;
    let mut leaves = Vec::new();
    for line in contents.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let mut hasher = Sha256::new();
        hasher.update(line.as_bytes());
        leaves.push(hasher.finalize().into());
    }
    if leaves.is_empty() {
        return None;
    }
    Some(hex::encode(crate::merkle_root(&leaves)))
}

fn latest_da_commitments(blob_dir: &Option<PathBuf>) -> Vec<DaCommitmentJson> {
    let mut map: HashMap<String, (SystemTime, DaCommitmentJson)> = HashMap::new();
    let min_age_secs = std::env::var("PH_DA_COMMITMENT_MIN_AGE_SECS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);
    let min_age = Duration::from_secs(min_age_secs);
    let Some(dir) = blob_dir.as_ref() else {
        return Vec::new();
    };
    let Ok(ns_entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    for ns in ns_entries.flatten() {
        let ns_path = ns.path();
        let Some(_ns_name) = ns_path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !ns_path.is_dir() {
            continue;
        }
        let Ok(meta_entries) = fs::read_dir(&ns_path) else {
            continue;
        };
        for entry in meta_entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("meta") {
                continue;
            }
            let meta_bytes = match fs::read(&path) {
                Ok(bytes) => bytes,
                Err(_) => continue,
            };
            let meta: BlobMeta = match serde_json::from_slice(&meta_bytes) {
                Ok(m) => m,
                Err(_) => continue,
            };
            let mtime = entry
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            if min_age_secs > 0 {
                if let Ok(age) = SystemTime::now().duration_since(mtime) {
                    if age < min_age {
                        continue;
                    }
                }
            }
            let (da_provider, da_commitment, da_height, da_status) = meta
                .da_receipt
                .as_ref()
                .map(|r| {
                    (
                        Some(r.provider.clone()),
                        r.commitment.clone().or(r.tx_hash.clone()),
                        r.height,
                        Some(r.status.clone()),
                    )
                })
                .unwrap_or((None, None, None, None));
            let dc = DaCommitmentJson {
                namespace: meta.namespace.clone(),
                blob_hash: meta.hash.clone(),
                share_root: meta.share_root.clone(),
                pedersen_root: meta.pedersen_root.clone(),
                da_provider,
                da_commitment,
                da_height,
                da_status,
                attestation_qc: None,
            };
            match map.get(&meta.namespace) {
                Some((seen_time, _)) if *seen_time >= mtime => {}
                _ => {
                    map.insert(meta.namespace.clone(), (mtime, dc));
                }
            }
        }
    }
    map.into_values().map(|v| v.1).collect()
}

fn lookup_stake(cfg: &NetConfig, pk_b64: &str) -> Option<u64> {
    let vk = decode_public_key_base64(pk_b64).ok()?;
    if let Some(weight) = cfg.membership_policy.stake_for(&vk) {
        return Some(weight);
    }
    if let Some(path) = &cfg.stake_registry_path {
        if let Ok(reg) = StakeRegistry::load(path) {
            if let Some(w) = reg.stake_for(pk_b64) {
                return Some(w);
            }
        }
    }
    Some(1)
}

fn append_evidence(path: &Path, namespace: &str, blob_hash: &str, pk: &str, reason: &str) {
    let record = serde_json::json!({
        "namespace": namespace,
        "blob_hash": blob_hash,
        "pk": pk,
        "reason": reason,
        "ts": now_millis(),
    });
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let line = format!("{}\n", record);
    if let Err(err) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut f| std::io::Write::write_all(&mut f, line.as_bytes()))
    {
        eprintln!("failed to write evidence: {err}");
    }
}

fn append_availability_evidence(path: &Path, ev: &AvailabilityEvidence) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    match serde_json::to_string(ev) {
        Ok(line) => {
            let line = format!("{}\n", line);
            if let Err(err) = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .and_then(|mut f| std::io::Write::write_all(&mut f, line.as_bytes()))
            {
                eprintln!("failed to write availability evidence: {err}");
            }
        }
        Err(err) => eprintln!("failed to encode availability evidence: {err}"),
    }
}

fn append_rollup_fault_evidence(path: &Path, ev: &RollupFaultEvidence) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    match serde_json::to_string(ev) {
        Ok(line) => {
            let line = format!("{}\n", line);
            if let Err(err) = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .and_then(|mut f| std::io::Write::write_all(&mut f, line.as_bytes()))
            {
                eprintln!("failed to write rollup fault evidence: {err}");
            }
        }
        Err(err) => eprintln!("failed to encode rollup fault evidence: {err}"),
    }
}

fn record_slash_with_registry(
    policy: &Arc<dyn MembershipPolicy>,
    registry_path: &Option<PathBuf>,
    token_mode_contract: &Option<String>,
    token_oracle_rpc: &Option<String>,
    pk_b64: &str,
    reason: &str,
) {
    if let Ok(vk) = decode_public_key_base64(pk_b64) {
        if let Err(err) = policy.record_slash(&vk) {
            eprintln!("failed to record slash in policy: {err}");
        }
    }
    if let Some(path) = registry_path {
        match StakeRegistry::load(path) {
            Ok(mut reg) => {
                reg.slash(pk_b64);
                if let Err(err) = reg.save(path) {
                    eprintln!("failed to persist stake registry: {err}");
                } else {
                    println!("slash recorded for {pk_b64} ({reason})");
                }
            }
            Err(err) => eprintln!("failed to load stake registry for slashing: {err}"),
        }
    }
    queue_token_burn_intent(
        registry_path,
        token_mode_contract,
        token_oracle_rpc,
        pk_b64,
        reason,
    );
}
fn load_anchor_from_logs(path: &Path) -> Result<LedgerAnchor, NetworkError> {
    let mut cutoff: Option<String> = None;
    let mut anchor_from_checkpoint = false;
    let anchor = match load_latest_checkpoint(path) {
        Ok(Some(checkpoint)) => {
            anchor_from_checkpoint = true;
            let (anchor, cp_cutoff) = checkpoint
                .clone()
                .into_ledger()
                .map_err(|err| NetworkError::Anchor(err.to_string()))?;
            cutoff = cp_cutoff;
            anchor
        }
        Ok(None) => julian_genesis_anchor(),
        Err(err) => return Err(NetworkError::Anchor(err.to_string())),
    };
    let mut entries = anchor.entries;
    let mut metadata = anchor.metadata;
    if !anchor_from_checkpoint {
        metadata.challenge_mode = None;
        metadata.fold_digest = None;
    }
    metadata
        .crate_version
        .get_or_insert_with(|| env!("CARGO_PKG_VERSION").to_string());
    let mut files: Vec<PathBuf> = std::fs::read_dir(path)
        .map_err(|err| NetworkError::Io(format!("failed to read {}: {err}", path.display())))?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|p| p.is_file() && is_ledger_file(p))
        .collect();
    files.sort();
    for file in files {
        if let Some(ref cutoff_name) = cutoff {
            if let Some(name) = file.file_name().and_then(|n| n.to_str()) {
                if name <= cutoff_name.as_str() {
                    continue;
                }
            }
        }
        let parsed = parse_log_file(&file).map_err(NetworkError::Anchor)?;
        if let Some(mode) = parsed.metadata.challenge_mode {
            match &mut metadata.challenge_mode {
                None => metadata.challenge_mode = Some(mode),
                Some(existing) if existing != &mode => {
                    return Err(NetworkError::Anchor(format!(
                        "{} challenge_mode {} conflicts with existing {}",
                        file.display(),
                        mode,
                        existing
                    )));
                }
                _ => {}
            }
        }
        if let Some(digest) = parsed.metadata.fold_digest {
            if let Some(existing) = &metadata.fold_digest {
                if existing != &digest && anchor_from_checkpoint {
                    return Err(NetworkError::Anchor(format!(
                        "{} fold_digest conflicts with existing value",
                        file.display()
                    )));
                }
            }
            metadata.fold_digest = Some(digest);
        }
        let entry_hashes = vec![parsed.digest];
        entries.push(EntryAnchor {
            statement: parsed.statement,
            merkle_root: merkle_root(&entry_hashes),
            hashes: entry_hashes,
        });
    }
    if entries.is_empty() {
        entries = julian_genesis_anchor().entries;
    }
    if let Some(digest) = read_fold_digest_hint(path).map_err(NetworkError::Anchor)? {
        if let Some(existing) = &metadata.fold_digest {
            if existing != &digest && anchor_from_checkpoint {
                return Err(NetworkError::Anchor(
                    "fold_digest hint conflicts with checkpoint metadata".to_string(),
                ));
            }
        }
        metadata.fold_digest = Some(digest);
    }
    let mut anchor = LedgerAnchor { entries, metadata };
    if anchor.metadata.fold_digest.is_none() {
        anchor.metadata.fold_digest = Some(compute_fold_digest(&anchor));
    }
    Ok(anchor)
}

fn is_ledger_file(path: &Path) -> bool {
    match path.file_name().and_then(|n| n.to_str()) {
        Some(name) => name.starts_with("ledger_") && name.ends_with(".txt"),
        None => false,
    }
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn current_round(bft_round_ms: u64) -> u64 {
    let round_ms = bft_round_ms.max(1);
    now_millis() / round_ms
}

fn anchor_payload_hash(payload: &[u8]) -> String {
    hex::encode(sha256_digest(payload))
}

fn vote_payload_bytes(round: u64, anchor_hash: &str) -> Vec<u8> {
    format!("{NETWORK_ID}:{round}:{anchor_hash}").into_bytes()
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcript_digest;
    use std::fs;
    use std::sync::atomic::Ordering;
    use std::time::SystemTime;

    fn temp_path(name: &str) -> PathBuf {
        let mut base = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        base.push(format!("{}_{}", name, nanos));
        base
    }

    #[test]
    fn payload_cache_rejects_duplicates() {
        let metrics = Arc::new(Metrics::default());
        let mut cache = PayloadCache::new(metrics.clone());
        let digest = [1u8; 32];
        assert!(cache.insert(digest));
        assert!(!cache.insert(digest));
    }

    #[test]
    fn payload_cache_eviction_tracks_metric() {
        let metrics = Arc::new(Metrics::default());
        let mut cache = PayloadCache::new(metrics.clone());
        for i in 0..(SEEN_CACHE_LIMIT + 1) {
            let mut digest = [0u8; 32];
            digest[..8].copy_from_slice(&(i as u64).to_le_bytes());
            cache.insert(digest);
        }
        assert!(metrics.lrucache_evictions_total.load(Ordering::Relaxed) >= 1);
    }

    #[test]
    fn identical_logs_yield_identical_anchors() {
        let dir = temp_path("mfenx_powerhouse_logs");
        fs::create_dir_all(&dir).unwrap();
        let log_path = dir.join("ledger_0000.txt");
        let challenges = vec![1, 2, 3];
        let round_sums = vec![4, 5, 6];
        let final_value = 7;
        let hash = transcript_digest(&challenges, &round_sums, final_value);
        let content = format!(
            "statement:Test Statement\ntranscript:{}\nround_sums:{}\nfinal:{}\nhash:{}\n",
            challenges
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(" "),
            round_sums
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(" "),
            final_value,
            crate::transcript_digest_to_hex(&hash)
        );
        fs::write(&log_path, content).unwrap();

        let anchor_a = load_anchor_from_logs(&dir).unwrap();
        let anchor_b = load_anchor_from_logs(&dir).unwrap();
        assert_eq!(anchor_a.entries, anchor_b.entries);

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn tampered_log_is_rejected() {
        let dir = temp_path("mfenx_powerhouse_logs_tampered");
        fs::create_dir_all(&dir).unwrap();
        let log_path = dir.join("ledger_0000.txt");
        let content = "statement:Demo\ntranscript:1\nround_sums:2\nfinal:3\nhash:999\n";
        fs::write(&log_path, content).unwrap();
        let result = load_anchor_from_logs(&dir);
        assert!(result.is_err());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn token_mode_native_identifiers_are_detected() {
        assert!(token_mode_is_native("native"));
        assert!(token_mode_is_native("NATIVE"));
        assert!(token_mode_is_native("native://julian"));
        assert!(token_mode_is_native("NATIVE://JULIAN"));
        assert!(!token_mode_is_native(
            "0x0000000000000000000000000000000000000001"
        ));
    }
}
