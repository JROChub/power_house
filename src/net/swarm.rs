#![cfg(feature = "net")]

use crate::net::sign::{
    decode_public_key_base64, encode_public_key_base64, encode_signature_base64, sign_payload,
    verify_signature_base64, KeyError, KeyMaterial,
};
use crate::net::{
    checkpoint::{
        latest_log_cutoff, load_latest_checkpoint, write_checkpoint, AnchorCheckpoint,
        CheckpointSignature,
    },
    policy::IdentityPolicy,
    schema::{
        AnchorCodecError, AnchorEnvelope, AnchorJson, ENVELOPE_SCHEMA_VERSION, NETWORK_ID,
        SCHEMA_ENVELOPE,
    },
};
use crate::{
    julian_genesis_anchor, merkle_root, transcript_digest, verify_transcript_lines, AnchorVote,
    EntryAnchor, LedgerAnchor,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use futures::StreamExt;
use libp2p::{
    gossipsub::{self, IdentTopic, MessageAuthenticity, ValidationMode},
    identify, identity,
    kad::{self, store::MemoryStore},
    noise,
    swarm::{NetworkBehaviour, Swarm, SwarmEvent},
    tcp, yamux, Multiaddr, SwarmBuilder,
};
use once_cell::sync::Lazy;
use serde_json;
use sha2::{Digest, Sha256};
use std::net::SocketAddr;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    io::{self, Read},
    path::{Path, PathBuf},
    sync::{atomic::AtomicU64, atomic::Ordering, Arc},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::{select, signal, time};

static TOPIC_ANCHORS: Lazy<IdentTopic> = Lazy::new(|| IdentTopic::new("jrocnet/anchors/v1"));
const MAX_ENVELOPE_BYTES: usize = 64 * 1024;
const MAX_ANCHOR_ENTRIES: usize = 10_000;
const SEEN_CACHE_LIMIT: usize = 2048;
const INVALID_THRESHOLD: usize = 5;

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
    /// Interval between anchor recomputation and gossip broadcasts.
    pub broadcast_interval: Duration,
    /// Signing and libp2p keys backing the node identity.
    pub key_material: KeyMaterial,
    /// Identity admission policy.
    pub identity_policy: IdentityPolicy,
    /// Optional checkpoint interval (in broadcasts).
    pub checkpoint_interval: Option<u64>,
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
        metrics_addr: Option<SocketAddr>,
        identity_policy: IdentityPolicy,
        checkpoint_interval: Option<u64>,
    ) -> Self {
        Self {
            node_id,
            listen_addr,
            bootstraps,
            log_dir,
            quorum,
            broadcast_interval,
            key_material,
            metrics: Arc::new(Metrics::default()),
            metrics_addr,
            identity_policy,
            checkpoint_interval,
        }
    }
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
}

impl std::fmt::Display for NetworkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Anchor(msg) => write!(f, "anchor error: {msg}"),
            Self::Codec(msg) => write!(f, "codec error: {msg}"),
            Self::Io(msg) => write!(f, "I/O error: {msg}"),
            Self::Key(msg) => write!(f, "key error: {msg}"),
            Self::Libp2p(msg) => write!(f, "libp2p error: {msg}"),
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
    let local_key_bytes = cfg.key_material.verifying.to_bytes();
    if !cfg.identity_policy.permits(&local_key_bytes) {
        return Err(NetworkError::Key(
            "local key not permitted by identity policy".to_string(),
        ));
    }
    let mut swarm = build_swarm(&cfg)?;
    Swarm::listen_on(&mut swarm, cfg.listen_addr.clone())
        .map_err(|err| NetworkError::Libp2p(format!("{err:?}")))?;
    for addr in &cfg.bootstraps {
        if let Err(err) = Swarm::dial(&mut swarm, addr.clone()) {
            eprintln!("dial {addr} failed: {err}");
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
        println!("metrics server listening on {addr}");
    }

    let mut seen_payloads = PayloadCache::new(metrics.clone());
    let mut invalid_counters: HashMap<libp2p::PeerId, usize> = HashMap::new();
    let mut last_payload = Vec::new();
    let mut last_publish: Option<Instant> = None;
    let mut broadcast_counter: u64 = 0;

    let local_peer = cfg.key_material.libp2p.public().to_peer_id();

    println!(
        "JROC-NET node {} (peer {}) listening on {}",
        cfg.node_id, local_peer, cfg.listen_addr
    );

    loop {
        select! {
            _ = ticker.tick() => {
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
            event = swarm.select_next_some() => {
                if let Err(err) = handle_event(event, &cfg, &mut seen_payloads, &mut invalid_counters, &metrics).await {
                    eprintln!("network error: {err}");
                }
            }
            _ = signal::ctrl_c() => {
                println!("JROC-NET node shutting down");
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
            build_behaviour(key).map_err(|err| {
                let boxed: Box<dyn std::error::Error + Send + Sync> = Box::new(err);
                boxed
            })
        })
        .map_err(|err| NetworkError::Libp2p(format!("{err:?}")))?
        .with_swarm_config(|cfg| cfg.with_idle_connection_timeout(Duration::from_secs(60)));

    Ok(builder.build())
}

fn build_behaviour(key: &identity::Keypair) -> Result<JrocBehaviour, NetworkError> {
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
    gossipsub
        .subscribe(&TOPIC_ANCHORS)
        .map_err(|err| NetworkError::Libp2p(format!("{err:?}")))?;

    let identify_config = identify::Config::new("jrocnet/1.0.0".into(), key.public())
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

async fn broadcast_local_anchor(
    swarm: &mut Swarm<JrocBehaviour>,
    cfg: &NetConfig,
    last_payload: &mut Vec<u8>,
    last_publish: &mut Option<Instant>,
    broadcast_counter: &mut u64,
    metrics: &Arc<Metrics>,
) -> Result<(), NetworkError> {
    if !cfg
        .identity_policy
        .permits(&cfg.key_material.verifying.to_bytes())
    {
        return Err(NetworkError::Key(
            "local key not permitted by identity policy".to_string(),
        ));
    }
    let ledger = load_anchor_from_logs(&cfg.log_dir)?;
    let timestamp_ms = now_millis();
    let anchor_json =
        AnchorJson::from_ledger(cfg.node_id.clone(), cfg.quorum, &ledger, timestamp_ms)?;
    let payload =
        serde_json::to_vec(&anchor_json).map_err(|err| NetworkError::Codec(err.to_string()))?;
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
    if let Err(err) = swarm
        .behaviour_mut()
        .gossipsub
        .publish(TOPIC_ANCHORS.clone(), message)
    {
        metrics.inc_gossipsub_rejects();
        return Err(NetworkError::Libp2p(err.to_string()));
    }
    *last_payload = payload;
    *last_publish = Some(Instant::now());
    println!(
        "broadcasted local anchor ({} entries)",
        ledger.entries.len()
    );
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
                        "checkpoint {} recorded (entries={})",
                        checkpoint.epoch,
                        ledger.entries.len()
                    );
                }
            }
        }
    }
    Ok(())
}

async fn handle_event(
    event: SwarmEvent<JrocBehaviourEvent>,
    cfg: &NetConfig,
    seen_payloads: &mut PayloadCache,
    invalid_counters: &mut HashMap<libp2p::PeerId, usize>,
    metrics: &Arc<Metrics>,
) -> Result<(), NetworkError> {
    match event {
        SwarmEvent::NewListenAddr { address, .. } => {
            println!("listening on {address}");
        }
        SwarmEvent::Behaviour(JrocBehaviourEvent::Gossipsub(event)) => match event {
            gossipsub::Event::Message {
                propagation_source,
                message,
                ..
            } => {
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
                if !cfg.identity_policy.permits(&remote_key_bytes) {
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
                let remote_anchor = anchor_json.clone().into_ledger()?;
                let local_anchor = load_anchor_from_logs(&cfg.log_dir)?;
                let local_key_bytes = cfg.key_material.verifying.to_bytes();
                let votes = [
                    AnchorVote {
                        anchor: &local_anchor,
                        public_key: &local_key_bytes,
                    },
                    AnchorVote {
                        anchor: &remote_anchor,
                        public_key: &remote_key_bytes,
                    },
                ];
                match crate::reconcile_anchors_with_quorum(&votes, cfg.quorum) {
                    Ok(()) => {
                        metrics.inc_anchors_verified();
                        metrics.inc_finality_events();
                        println!(
                            "finality reached with peer {} :: entries={}",
                            envelope.node_id,
                            remote_anchor.entries.len()
                        );
                    }
                    Err(err) => {
                        println!("anchor divergence with peer {}: {}", envelope.node_id, err);
                    }
                }
            }
            _ => {}
        },
        _ => {}
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
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
        let dir = temp_path("jroc_net_logs");
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
        let dir = temp_path("jroc_net_logs_tampered");
        fs::create_dir_all(&dir).unwrap();
        let log_path = dir.join("ledger_0000.txt");
        let content = "statement:Demo\ntranscript:1\nround_sums:2\nfinal:3\nhash:999\n";
        fs::write(&log_path, content).unwrap();
        let result = load_anchor_from_logs(&dir);
        assert!(result.is_err());
        fs::remove_dir_all(&dir).unwrap();
    }
}

fn load_anchor_from_logs(path: &Path) -> Result<LedgerAnchor, NetworkError> {
    let mut cutoff: Option<String> = None;
    let mut entries = match load_latest_checkpoint(path) {
        Ok(Some(checkpoint)) => {
            let (anchor, cp_cutoff) = checkpoint
                .clone()
                .into_ledger()
                .map_err(|err| NetworkError::Anchor(err.to_string()))?;
            cutoff = cp_cutoff;
            anchor.entries
        }
        Ok(None) => julian_genesis_anchor().entries,
        Err(err) => return Err(NetworkError::Anchor(err.to_string())),
    };
    let mut files: Vec<PathBuf> = std::fs::read_dir(path)
        .map_err(|err| NetworkError::Io(format!("failed to read {}: {err}", path.display())))?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|p| p.is_file())
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
        let mut contents = String::new();
        std::fs::File::open(&file)
            .map_err(|err| NetworkError::Io(format!("failed to open {}: {err}", file.display())))?
            .read_to_string(&mut contents)
            .map_err(|err| NetworkError::Io(format!("failed to read {}: {err}", file.display())))?;
        let mut lines: Vec<String> = contents
            .lines()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty())
            .collect();
        if lines.is_empty() {
            continue;
        }
        let statement_line = lines.remove(0);
        if !statement_line.starts_with("statement:") {
            return Err(NetworkError::Anchor(format!(
                "{} missing statement prefix",
                file.display()
            )));
        }
        let statement = statement_line[10..].to_string();
        verify_transcript_lines(lines.iter().map(|s| s.as_str())).map_err(|err| {
            NetworkError::Anchor(format!("{} verification failed: {err}", file.display()))
        })?;
        let (challenges, round_sums, final_value, stored_hash) =
            crate::parse_transcript_record(lines.iter().map(|s| s.as_str())).map_err(|err| {
                NetworkError::Anchor(format!("{} parse error: {err}", file.display()))
            })?;
        let computed = transcript_digest(&challenges, &round_sums, final_value);
        if computed != stored_hash {
            let stored_hex = crate::transcript_digest_to_hex(&stored_hash);
            let computed_hex = crate::transcript_digest_to_hex(&computed);
            return Err(NetworkError::Anchor(format!(
                "{} hash mismatch: stored={}, computed={}",
                file.display(),
                stored_hex,
                computed_hex
            )));
        }
        let entry_hashes = vec![computed];
        entries.push(EntryAnchor {
            statement,
            merkle_root: merkle_root(&entry_hashes),
            hashes: entry_hashes,
        });
    }
    Ok(LedgerAnchor { entries })
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
