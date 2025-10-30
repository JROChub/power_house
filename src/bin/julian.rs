//! Minimal CLI for interacting with the JULIAN Protocol primitives.
//!
//! This binary exposes helper commands for replaying transcript logs,
//! deriving ledger anchors, and reconciling anchors with a quorum.  It uses
//! only the Rust standard library to remain dependency free.

use power_house::{
    julian_genesis_anchor, reconcile_anchors_with_quorum, transcript_digest,
    verify_transcript_lines, EntryAnchor, LedgerAnchor,
};
use std::{
    env, fs,
    io::{self, Read},
    path::{Path, PathBuf},
};

const NETWORK_ID: &str = "JROC-NET";

fn main() {
    let mut args = env::args().skip(1);
    match (args.next().as_deref(), args.next().as_deref()) {
        (Some("node"), Some(sub)) => handle_node(sub, args.collect()),
        _ => {
            eprintln!("Usage: julian node <run|anchor|reconcile> ...");
            std::process::exit(1);
        }
    }
}

fn handle_node(sub: &str, tail: Vec<String>) {
    match sub {
        "run" => cmd_node_run(tail),
        "anchor" => cmd_node_anchor(tail),
        "reconcile" => cmd_node_reconcile(tail),
        _ => {
            eprintln!("Unknown subcommand: {}", sub);
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

    match reconcile_anchors_with_quorum(&[local.clone(), peer.clone()], quorum) {
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

fn load_anchor_from_logs(path: &Path) -> Result<LedgerAnchor, String> {
    let mut entries = julian_genesis_anchor().entries;
    let mut files: Vec<PathBuf> = fs::read_dir(path)
        .map_err(|err| format!("failed to read directory {}: {err}", path.display()))?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|p| p.is_file())
        .collect();
    files.sort();
    for file in files {
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
            return Err(format!(
                "{} hash mismatch: stored={}, computed={}",
                file.display(),
                stored_hash,
                computed
            ));
        }
        entries.push(EntryAnchor {
            statement,
            hashes: vec![computed],
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
            .map(|h| h.to_string())
            .collect::<Vec<_>>()
            .join(",");
        lines.push(format!("{}|{}|{}", NETWORK_ID, entry.statement, hash_list));
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
        let (statement, hashes_str) = match segments.as_slice() {
            [network, statement, hashes] => {
                if *network != NETWORK_ID {
                    return Err(format!(
                        "anchor network mismatch: expected {NETWORK_ID}, found {network}"
                    ));
                }
                (*statement, *hashes)
            }
            [statement, hashes] => (*statement, *hashes),
            _ => return Err(format!("invalid anchor line: {trimmed}")),
        };
        let mut hashes = Vec::new();
        if !hashes_str.is_empty() {
            for part in hashes_str.split(',') {
                let value = part
                    .parse::<u64>()
                    .map_err(|_| format!("invalid hash value: {part}"))?;
                hashes.push(value);
            }
        }
        entries.push(EntryAnchor {
            statement: statement.to_string(),
            hashes,
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
            .map(|h| h.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        lines.push(format!(
            "{NETWORK_ID} :: {} -> [{}]",
            entry.statement, hashes
        ));
    }
    lines.join("\n")
}
