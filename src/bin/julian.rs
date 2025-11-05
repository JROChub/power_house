//! Minimal CLI for interacting with the JULIAN Protocol primitives.
//!
//! This binary exposes helper commands for replaying transcript logs,
//! deriving ledger anchors, and reconciling anchors with a quorum using the
//! crate's domain-separated hashing and signature utilities.

#[cfg(feature = "net")]
use power_house::net::{
    decode_public_key_base64, load_encrypted_identity, load_or_derive_keypair, run_network,
    verify_signature_base64, AnchorEnvelope, AnchorJson, Ed25519KeySource, IdentityPolicy,
    NetConfig,
};
use power_house::{
    julian_genesis_anchor, reconcile_anchors_with_quorum, transcript_digest,
    verify_transcript_lines, AnchorVote, EntryAnchor, LedgerAnchor,
};
#[cfg(feature = "net")]
use std::net::SocketAddr;
#[cfg(feature = "net")]
use std::time::Duration;
use std::{
    env, fs,
    io::{self, Read},
    path::{Path, PathBuf},
};

#[cfg(feature = "net")]
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
#[cfg(feature = "net")]
use libp2p::Multiaddr;
#[cfg(feature = "net")]
use rpassword::prompt_password;
use serde_json;

const NETWORK_ID: &str = "JROC-NET";

fn fatal(message: &str) -> ! {
    eprintln!("{message}");
    std::process::exit(1);
}

fn main() {
    let mut args = env::args().skip(1);
    let command = args.next();
    match command.as_deref() {
        Some("node") => {
            let sub = args.next().unwrap_or_else(|| {
                eprintln!("Usage: julian node <run|anchor|reconcile> ...");
                std::process::exit(1);
            });
            handle_node(&sub, args.collect());
        }
        #[cfg(feature = "net")]
        Some("net") => {
            let sub = args.next().unwrap_or_else(|| {
                eprintln!("Usage: julian net <start|anchor|verify-envelope> ...");
                std::process::exit(1);
            });
            handle_net(&sub, args.collect());
        }
        _ => {
            eprintln!("Usage: julian <node|net> ...");
            std::process::exit(1);
        }
    }
}

fn handle_node(sub: &str, tail: Vec<String>) {
    match sub {
        "run" => cmd_node_run(tail),
        "anchor" => cmd_node_anchor(tail),
        "reconcile" => cmd_node_reconcile(tail),
        "prove" => cmd_node_prove(tail),
        "verify-proof" => cmd_node_verify_proof(tail),
        _ => {
            eprintln!("Unknown subcommand: {}", sub);
            std::process::exit(1);
        }
    }
}

#[cfg(feature = "net")]
fn handle_net(sub: &str, tail: Vec<String>) {
    match sub {
        "start" => cmd_net_start(tail),
        "anchor" => cmd_net_anchor(tail),
        "verify-envelope" => cmd_net_verify_envelope(tail),
        _ => {
            eprintln!("Unknown net subcommand: {sub}");
            std::process::exit(1);
        }
    }
}

fn cmd_node_run(args: Vec<String>) {
    if args.len() < 3 {
        eprintln!("Usage: julian node run <node_id> <log_dir> <output_anchor>");
        std::process::exit(1);
    }
    let node_id = &args[0];
    println!("{NETWORK_ID} node {node_id} starting…");
    let log_dir = Path::new(&args[1]);
    let output = Path::new(&args[2]);
    let anchor = match load_anchor_from_logs(log_dir) {
        Ok(anchor) => anchor,
        Err(err) => {
            eprintln!("error: {err}");
            std::process::exit(1);
        }
    };
    if let Err(err) = write_anchor(output, &anchor) {
        eprintln!("error writing anchor: {err}");
        std::process::exit(1);
    }
    println!(
        "{NETWORK_ID} node {node_id} anchor written to {}",
        output.display()
    );
    println!("anchor summary:\n{}", format_anchor(&anchor));
}

fn cmd_node_anchor(args: Vec<String>) {
    if args.len() != 1 {
        eprintln!("Usage: julian node anchor <log_dir>");
        std::process::exit(1);
    }
    let log_dir = Path::new(&args[0]);
    match load_anchor_from_logs(log_dir) {
        Ok(anchor) => println!("{}", format_anchor(&anchor)),
        Err(err) => {
            eprintln!("error: {err}");
            std::process::exit(1);
        }
    }
}

fn cmd_node_reconcile(args: Vec<String>) {
    if args.len() < 3 {
        eprintln!("Usage: julian node reconcile <log_dir> <peer_anchor> <quorum>");
        std::process::exit(1);
    }
    let log_dir = Path::new(&args[0]);
    let peer_path = Path::new(&args[1]);
    let quorum: usize = args[2].parse().unwrap_or_else(|_| {
        eprintln!("Invalid quorum value: {}", args[2]);
        std::process::exit(1);
    });

    let local = match load_anchor_from_logs(log_dir) {
        Ok(anchor) => anchor,
        Err(err) => {
            eprintln!("error: {err}");
            std::process::exit(1);
        }
    };
    let peer = match read_anchor(peer_path) {
        Ok(anchor) => anchor,
        Err(err) => {
            eprintln!("error reading peer anchor: {err}");
            std::process::exit(1);
        }
    };

    let votes = [
        AnchorVote {
            anchor: &local,
            public_key: b"LOCAL_OFFLINE",
        },
        AnchorVote {
            anchor: &peer,
            public_key: b"PEER_FILE",
        },
    ];
    match reconcile_anchors_with_quorum(&votes, quorum) {
        Ok(()) => {
            println!("Finality reached with quorum {quorum}.");
            println!("Local anchor:\n{}", format_anchor(&local));
            println!("Peer anchor:\n{}", format_anchor(&peer));
        }
        Err(err) => {
            eprintln!("Quorum check failed: {err}");
            std::process::exit(1);
        }
    }
}

fn cmd_node_prove(args: Vec<String>) {
    if args.len() < 3 {
        eprintln!("Usage: julian node prove <log_dir> <entry_index> <leaf_index> [output.json]");
        std::process::exit(1);
    }
    let log_dir = Path::new(&args[0]);
    let entry_index: usize = args[1]
        .parse()
        .unwrap_or_else(|_| fatal("invalid entry index"));
    let leaf_index: usize = args[2]
        .parse()
        .unwrap_or_else(|_| fatal("invalid leaf index"));
    let anchor = load_anchor_from_logs(log_dir).unwrap_or_else(|err| fatal(&format!("{err}")));
    let entry = anchor
        .entries
        .get(entry_index)
        .unwrap_or_else(|| fatal("entry index out of bounds"));
    if leaf_index >= entry.hashes.len() {
        fatal("leaf index out of bounds");
    }
    let proof = power_house::build_merkle_proof(&entry.hashes, leaf_index)
        .ok_or_else(|| "unable to build proof".to_string())
        .unwrap_or_else(|err| fatal(&err));
    if proof.root != entry.merkle_root {
        fatal("computed proof root does not match entry merkle root");
    }
    let proof_json: serde_json::Value = serde_json::from_str(&proof.to_json_string()).unwrap();
    let document = serde_json::json!({
        "entry_index": entry_index,
        "statement": entry.statement,
        "leaf_index": leaf_index,
        "leaf": power_house::transcript_digest_to_hex(&entry.hashes[leaf_index]),
        "merkle_root": power_house::transcript_digest_to_hex(&entry.merkle_root),
        "proof": proof_json
    });
    if let Some(path) = args.get(3) {
        if let Err(err) = fs::write(path, serde_json::to_string_pretty(&document).unwrap()) {
            fatal(&format!("failed to write proof: {err}"));
        }
    } else {
        println!("{}", serde_json::to_string_pretty(&document).unwrap());
    }
}

fn cmd_node_verify_proof(args: Vec<String>) {
    if args.len() != 2 {
        eprintln!("Usage: julian node verify-proof <anchor_file> <proof_file>");
        std::process::exit(1);
    }
    let anchor = read_anchor(Path::new(&args[0]))
        .unwrap_or_else(|err| fatal(&format!("failed to read anchor: {err}")));
    let proof_text = fs::read_to_string(&args[1])
        .unwrap_or_else(|err| fatal(&format!("failed to read proof file: {err}")));
    let document: serde_json::Value = serde_json::from_str(&proof_text)
        .unwrap_or_else(|err| fatal(&format!("invalid proof JSON: {err}")));
    let entry_index = document
        .get("entry_index")
        .and_then(|v| v.as_u64())
        .unwrap_or_else(|| fatal("proof missing entry_index")) as usize;
    let leaf_index = document
        .get("leaf_index")
        .and_then(|v| v.as_u64())
        .unwrap_or_else(|| fatal("proof missing leaf_index")) as usize;
    let proof_value = document
        .get("proof")
        .unwrap_or_else(|| fatal("proof missing inner proof object"));
    let proof_json = serde_json::to_string(proof_value).unwrap();
    let proof = power_house::MerkleProof::from_json_str(&proof_json)
        .unwrap_or_else(|err| fatal(&format!("invalid proof: {err}")));
    if entry_index >= anchor.entries.len() {
        fatal("entry index out of bounds");
    }
    let entry = &anchor.entries[entry_index];
    if leaf_index >= entry.hashes.len() {
        fatal("leaf index out of bounds");
    }
    if proof.root != entry.merkle_root {
        fatal("proof root does not match anchor merkle root");
    }
    if proof.leaf != entry.hashes[leaf_index] {
        fatal("proof leaf does not match anchor digest");
    }
    if !power_house::verify_merkle_proof(&proof) {
        fatal("invalid Merkle proof");
    }
    println!(
        "Proof verified for statement '{}' (entry {}, leaf {}).",
        entry.statement, entry_index, leaf_index
    );
}

#[cfg(feature = "net")]
fn cmd_net_start(args: Vec<String>) {
    let mut node_id = None;
    let mut log_dir = None;
    let mut listen = None;
    let mut bootstraps = Vec::new();
    let mut quorum: usize = 1;
    let mut broadcast_ms: u64 = 5_000;
    let mut key_spec: Option<String> = None;
    let mut identity_path: Option<String> = None;
    let mut metrics_addr_spec: Option<String> = None;
    let mut policy_allowlist_spec: Option<String> = None;
    let mut checkpoint_interval_spec: Option<String> = None;

    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--node-id" => {
                node_id = Some(
                    iter.next()
                        .unwrap_or_else(|| fatal("--node-id expects a value")),
                );
            }
            "--log-dir" => {
                log_dir = Some(
                    iter.next()
                        .unwrap_or_else(|| fatal("--log-dir expects a value")),
                );
            }
            "--listen" => {
                let value = iter
                    .next()
                    .unwrap_or_else(|| fatal("--listen expects a value"));
                let addr: Multiaddr = value
                    .parse()
                    .unwrap_or_else(|_| fatal("invalid multiaddr for --listen"));
                listen = Some(addr);
            }
            "--bootstrap" => {
                let value = iter
                    .next()
                    .unwrap_or_else(|| fatal("--bootstrap expects a value"));
                let addr: Multiaddr = value
                    .parse()
                    .unwrap_or_else(|_| fatal("invalid multiaddr for --bootstrap"));
                bootstraps.push(addr);
            }
            "--broadcast-interval" => {
                let value = iter
                    .next()
                    .unwrap_or_else(|| fatal("--broadcast-interval expects a value"));
                broadcast_ms = value
                    .parse()
                    .unwrap_or_else(|_| fatal("invalid --broadcast-interval"));
            }
            "--quorum" => {
                let value = iter
                    .next()
                    .unwrap_or_else(|| fatal("--quorum expects a value"));
                quorum = value.parse().unwrap_or_else(|_| fatal("invalid --quorum"));
            }
            "--key" => {
                key_spec = Some(
                    iter.next()
                        .unwrap_or_else(|| fatal("--key expects a value")),
                );
            }
            "--identity" => {
                identity_path = Some(
                    iter.next()
                        .unwrap_or_else(|| fatal("--identity expects a value")),
                );
            }
            "--metrics" => {
                metrics_addr_spec = Some(
                    iter.next()
                        .unwrap_or_else(|| fatal("--metrics expects a value")),
                );
            }
            "--policy-allowlist" => {
                policy_allowlist_spec = Some(
                    iter.next()
                        .unwrap_or_else(|| fatal("--policy-allowlist expects a value")),
                );
            }
            "--checkpoint-interval" => {
                checkpoint_interval_spec = Some(
                    iter.next()
                        .unwrap_or_else(|| fatal("--checkpoint-interval expects a value")),
                );
            }
            other => fatal(&format!("unknown argument: {other}")),
        }
    }

    let node_id = node_id.unwrap_or_else(|| fatal("--node-id is required"));
    let log_dir = log_dir.unwrap_or_else(|| fatal("--log-dir is required"));
    let listen_addr = listen.unwrap_or_else(|| fatal("--listen is required"));
    if identity_path.is_some() && key_spec.is_some() {
        fatal("use either --key or --identity, not both");
    }

    let key_material = if let Some(path) = identity_path {
        let passphrase = prompt_password("Identity passphrase: ")
            .unwrap_or_else(|err| fatal(&format!("failed to read passphrase: {err}")));
        match load_encrypted_identity(Path::new(&path), &passphrase) {
            Ok(material) => material,
            Err(err) => fatal(&format!("failed to load identity: {err}")),
        }
    } else {
        let key_source = Ed25519KeySource::from_spec(key_spec.as_deref());
        match load_or_derive_keypair(&key_source) {
            Ok(material) => material,
            Err(err) => fatal(&format!("failed to load key: {err}")),
        }
    };

    let metrics_addr = metrics_addr_spec
        .as_deref()
        .map(parse_metrics_addr)
        .unwrap_or(None);
    let identity_policy = match policy_allowlist_spec {
        Some(path) => match IdentityPolicy::from_allowlist_path(Path::new(&path)) {
            Ok(policy) => policy,
            Err(err) => fatal(&format!("failed to load policy: {err}")),
        },
        None => IdentityPolicy::allow_all(),
    };
    let checkpoint_interval = checkpoint_interval_spec.map(|value| {
        value
            .parse()
            .unwrap_or_else(|_| fatal("invalid --checkpoint-interval"))
    });

    let config = NetConfig::new(
        node_id,
        listen_addr,
        bootstraps,
        PathBuf::from(log_dir),
        quorum,
        Duration::from_millis(broadcast_ms),
        key_material,
        metrics_addr,
        identity_policy,
        checkpoint_interval,
    );

    let runtime = tokio::runtime::Runtime::new()
        .unwrap_or_else(|err| fatal(&format!("failed to start runtime: {err}")));
    if let Err(err) = runtime.block_on(run_network(config)) {
        fatal(&format!("network error: {err}"));
    }
}

#[cfg(feature = "net")]
fn cmd_net_anchor(args: Vec<String>) {
    let mut log_dir = None;
    let mut node_id = String::from("unknown-node");
    let mut quorum: usize = 1;

    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--log-dir" => {
                log_dir = Some(
                    iter.next()
                        .unwrap_or_else(|| fatal("--log-dir expects a value")),
                );
            }
            "--node-id" => {
                node_id = iter
                    .next()
                    .unwrap_or_else(|| fatal("--node-id expects a value"));
            }
            "--quorum" => {
                let value = iter
                    .next()
                    .unwrap_or_else(|| fatal("--quorum expects a value"));
                quorum = value.parse().unwrap_or_else(|_| fatal("invalid --quorum"));
            }
            other => fatal(&format!("unknown argument: {other}")),
        }
    }

    let log_dir = log_dir.unwrap_or_else(|| fatal("--log-dir is required"));
    let ledger = load_anchor_from_logs(Path::new(&log_dir)).unwrap_or_else(|err| fatal(&err));
    let anchor_json = AnchorJson::from_ledger(node_id, quorum, &ledger, now_millis())
        .unwrap_or_else(|err| fatal(&format!("anchor conversion failed: {err}")));
    match anchor_json.to_json_string() {
        Ok(text) => println!("{text}"),
        Err(err) => fatal(&format!("FAIL: failed to encode anchor: {err}")),
    }
}

#[cfg(feature = "net")]
fn cmd_net_verify_envelope(args: Vec<String>) {
    let mut file = None;
    let mut log_dir = None;
    let mut quorum: usize = 1;

    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--file" => {
                file = Some(
                    iter.next()
                        .unwrap_or_else(|| fatal("--file expects a value")),
                );
            }
            "--log-dir" => {
                log_dir = Some(
                    iter.next()
                        .unwrap_or_else(|| fatal("--log-dir expects a value")),
                );
            }
            "--quorum" => {
                let value = iter
                    .next()
                    .unwrap_or_else(|| fatal("--quorum expects a value"));
                quorum = value.parse().unwrap_or_else(|_| fatal("invalid --quorum"));
            }
            other => fatal(&format!("unknown argument: {other}")),
        }
    }

    let file = file.unwrap_or_else(|| fatal("--file is required"));
    let log_dir = log_dir.unwrap_or_else(|| fatal("--log-dir is required"));
    let contents = fs::read_to_string(&file)
        .unwrap_or_else(|err| fatal(&format!("FAIL: failed to read envelope: {err}")));
    let envelope: AnchorEnvelope = serde_json::from_str(&contents)
        .unwrap_or_else(|err| fatal(&format!("FAIL: invalid envelope JSON: {err}")));
    if let Err(err) = envelope.validate() {
        fatal(&format!("FAIL: invalid envelope: {err}"));
    }
    let payload = BASE64
        .decode(envelope.payload.as_bytes())
        .unwrap_or_else(|err| fatal(&format!("FAIL: payload decode failed: {err}")));
    verify_signature_base64(&envelope.public_key, &payload, &envelope.signature)
        .unwrap_or_else(|err| fatal(&format!("FAIL: signature verification failed: {err}")));
    let remote_verifying = decode_public_key_base64(&envelope.public_key)
        .unwrap_or_else(|err| fatal(&format!("FAIL: invalid public key: {err}")));
    let remote_key_bytes = remote_verifying.to_bytes();
    let payload_str = std::str::from_utf8(&payload)
        .unwrap_or_else(|err| fatal(&format!("FAIL: payload is not UTF-8: {err}")));
    let anchor_json = AnchorJson::from_json_str(payload_str)
        .unwrap_or_else(|err| fatal(&format!("FAIL: invalid anchor payload: {err}")));
    let remote_ledger = anchor_json
        .clone()
        .into_ledger()
        .unwrap_or_else(|err| fatal(&format!("FAIL: anchor decode error: {err}")));
    let local = load_anchor_from_logs(Path::new(&log_dir))
        .unwrap_or_else(|err| fatal(&format!("FAIL: {err}")));
    let votes = [
        AnchorVote {
            anchor: &local,
            public_key: b"LOCAL_OFFLINE",
        },
        AnchorVote {
            anchor: &remote_ledger,
            public_key: &remote_key_bytes,
        },
    ];
    match reconcile_anchors_with_quorum(&votes, quorum) {
        Ok(()) => println!("PASS: envelope verified and quorum satisfied."),
        Err(err) => fatal(&format!("FAIL: quorum check failed: {err}")),
    }
}

fn load_anchor_from_logs(path: &Path) -> Result<LedgerAnchor, String> {
    #[cfg(feature = "net")]
    let mut cutoff: Option<String> = None;
    #[cfg(not(feature = "net"))]
    let cutoff: Option<String> = None;
    let mut entries = {
        #[cfg(feature = "net")]
        {
            match power_house::net::load_latest_checkpoint(path) {
                Ok(Some(checkpoint)) => match checkpoint.into_ledger() {
                    Ok((anchor, cp_cutoff)) => {
                        cutoff = cp_cutoff;
                        anchor.entries
                    }
                    Err(err) => return Err(format!("checkpoint error: {err}")),
                },
                Ok(None) => julian_genesis_anchor().entries,
                Err(err) => return Err(format!("checkpoint error: {err}")),
            }
        }
        #[cfg(not(feature = "net"))]
        {
            julian_genesis_anchor().entries
        }
    };
    let mut files: Vec<PathBuf> = fs::read_dir(path)
        .map_err(|err| format!("failed to read directory {}: {err}", path.display()))?
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
        fs::File::open(&file)
            .map_err(|err| format!("failed to open {}: {err}", file.display()))?
            .read_to_string(&mut contents)
            .map_err(|err| format!("failed to read {}: {err}", file.display()))?;
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
            return Err(format!("{} missing statement prefix", file.display()));
        }
        let statement = statement_line[10..].to_string();
        verify_transcript_lines(lines.iter().map(|s| s.as_str()))
            .map_err(|err| format!("{} verification failed: {err}", file.display()))?;
        let (challenges, round_sums, final_value, stored_hash) =
            power_house::parse_transcript_record(lines.iter().map(|s| s.as_str()))
                .map_err(|err| format!("{} parse error: {err}", file.display()))?;
        let computed = transcript_digest(&challenges, &round_sums, final_value);
        if computed != stored_hash {
            let stored_hex = power_house::transcript_digest_to_hex(&stored_hash);
            let computed_hex = power_house::transcript_digest_to_hex(&computed);
            return Err(format!(
                "{} hash mismatch: stored={}, computed={}",
                file.display(),
                stored_hex,
                computed_hex
            ));
        }
        let entry_hashes = vec![computed];
        entries.push(EntryAnchor {
            statement,
            merkle_root: power_house::merkle_root(&entry_hashes),
            hashes: entry_hashes,
        });
    }
    Ok(LedgerAnchor { entries })
}

fn write_anchor(path: &Path, anchor: &LedgerAnchor) -> io::Result<()> {
    fs::write(path, anchor_to_string(anchor))
}

fn read_anchor(path: &Path) -> Result<LedgerAnchor, String> {
    let mut input = String::new();
    fs::File::open(path)
        .map_err(|err| format!("failed to open {}: {err}", path.display()))?
        .read_to_string(&mut input)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    anchor_from_string(&input)
}

fn anchor_to_string(anchor: &LedgerAnchor) -> String {
    let mut lines = Vec::new();
    for entry in &anchor.entries {
        let hash_list = entry
            .hashes
            .iter()
            .map(|h| power_house::transcript_digest_to_hex(h))
            .collect::<Vec<_>>()
            .join(",");
        lines.push(format!(
            "{}|{}|{}|root={}",
            NETWORK_ID,
            entry.statement,
            hash_list,
            power_house::transcript_digest_to_hex(&entry.merkle_root)
        ));
    }
    lines.join("\n")
}

fn anchor_from_string(input: &str) -> Result<LedgerAnchor, String> {
    let mut entries = Vec::new();
    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let segments: Vec<&str> = trimmed.split('|').collect();
        let (statement, hashes_str, root_part) = match segments.as_slice() {
            [network, statement, hashes, root] if *network == NETWORK_ID => {
                (*statement, *hashes, Some(*root))
            }
            [network, statement, hashes] if *network == NETWORK_ID => (*statement, *hashes, None),
            [statement, hashes, root] => (*statement, *hashes, Some(*root)),
            [statement, hashes] => (*statement, *hashes, None),
            _ => return Err(format!("invalid anchor line: {trimmed}")),
        };
        if segments.len() >= 3 && segments[0] != NETWORK_ID {
            // Ensure lines with an unexpected network identifier are rejected explicitly.
            if segments.len() == 4 {
                return Err(format!(
                    "anchor network mismatch: expected {NETWORK_ID}, found {}",
                    segments[0]
                ));
            }
        }
        let mut hashes = Vec::new();
        if !hashes_str.is_empty() {
            for part in hashes_str.split(',') {
                let trimmed = part.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let value = power_house::transcript_digest_from_hex(trimmed)
                    .map_err(|err| format!("invalid hash value: {trimmed}: {err}"))?;
                hashes.push(value);
            }
        }
        let merkle_root = if let Some(root_field) = root_part {
            let value = root_field
                .strip_prefix("root=")
                .ok_or_else(|| format!("invalid root field: {root_field}"))?;
            power_house::transcript_digest_from_hex(value)
                .map_err(|err| format!("invalid root digest: {err}"))?
        } else {
            power_house::merkle_root(&hashes)
        };
        entries.push(EntryAnchor {
            statement: statement.to_string(),
            hashes,
            merkle_root,
        });
    }
    if entries.is_empty() {
        entries = julian_genesis_anchor().entries;
    }
    Ok(LedgerAnchor { entries })
}

fn format_anchor(anchor: &LedgerAnchor) -> String {
    let mut lines = Vec::new();
    for entry in &anchor.entries {
        let hashes = entry
            .hashes
            .iter()
            .map(|h| power_house::transcript_digest_to_hex(h))
            .collect::<Vec<_>>()
            .join(", ");
        lines.push(format!(
            "{NETWORK_ID} :: {} -> [{}] :: root={}",
            entry.statement,
            hashes,
            power_house::transcript_digest_to_hex(&entry.merkle_root)
        ));
    }
    lines.join("\n")
}

#[cfg(feature = "net")]
fn now_millis() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(feature = "net")]
fn parse_metrics_addr(spec: &str) -> Option<SocketAddr> {
    if spec.is_empty() {
        fatal("--metrics expects a non-empty value");
    }
    if spec.eq_ignore_ascii_case("off") {
        return None;
    }
    let normalized = if spec.starts_with(':') {
        format!("0.0.0.0{}", spec)
    } else {
        spec.to_string()
    };
    match normalized.parse::<SocketAddr>() {
        Ok(addr) => Some(addr),
        Err(_) => fatal("invalid --metrics address"),
    }
}
