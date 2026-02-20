//! Minimal CLI for interacting with the JULIAN Protocol primitives.
//!
//! This binary exposes helper commands for replaying transcript logs,
//! deriving ledger anchors, and reconciling anchors with a quorum using the
//! crate's domain-separated hashing and signature utilities.

#[cfg(feature = "net")]
use power_house::commands::{
    migration_apply_claims::{run_apply_claims, ApplyClaimsOptions},
    migration_claims::{run_build_claims, BuildClaimsOptions},
    stake_snapshot::run_snapshot,
};
#[cfg(feature = "net")]
use power_house::net::{
    decode_public_key_base64, encrypt_identity_base64, load_encrypted_identity,
    load_or_derive_keypair, refresh_migration_mode_from_env, run_network, verify_signature_base64,
    AnchorEnvelope, AnchorJson, Ed25519KeySource, MembershipPolicy, MigrationProposal,
    MultisigPolicy, NamespaceRule, NetConfig, StakePolicy, StakeRegistry, StaticPolicy,
};
use power_house::{
    compute_fold_digest, julian_genesis_anchor, parse_log_file, read_fold_digest_hint,
    reconcile_anchors_with_quorum, AnchorMetadata, AnchorVote, EntryAnchor, Field, GeneralSumProof,
    LedgerAnchor, ProofStats,
};
#[cfg(feature = "net")]
use std::net::SocketAddr;
#[cfg(feature = "net")]
use std::sync::Arc;
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
#[cfg(feature = "net")]
use serde::Deserialize;
#[cfg(feature = "net")]
use std::collections::HashMap;

const NETWORK_ID: &str = "MFENX-POWERHOUSE";

fn fatal(message: &str) -> ! {
    eprintln!("{message}");
    std::process::exit(1);
}

#[cfg(feature = "net")]
fn print_stake_help() {
    println!("Usage: julian stake <show|fund|bond|snapshot|claims|apply-claims|unbond|reward> ...");
    println!("  show <stake_registry.json>");
    println!("  fund <registry.json> <pubkey_b64> <amount>");
    println!("  bond <registry.json> <pubkey_b64> <amount>");
    println!("  snapshot --registry <path> --height <N> --output <file>");
    println!(
        "  claims --snapshot <file> --output <file> [--mode native|erc20] [--amount-source stake|balance|total]"
    );
    println!("  apply-claims --registry <file> --claims <file> [--state <file>] [--dry-run]");
    println!("  unbond <registry.json> <pubkey_b64> <amount>");
    println!("  reward <registry.json> <pubkey_b64> <amount>");
}

#[cfg(feature = "net")]
fn print_governance_help() {
    println!("Usage: julian governance <propose-migration> ...");
    println!("  propose-migration --snapshot-height <N> [--token-contract <id>]");
    println!("    [--conversion-ratio <u64>] [--treasury-mint <u64>]");
    println!("    --log-dir <dir> [--node-id <id>] [--quorum <N>] [--output <file>]");
}

#[cfg(feature = "net")]
fn print_net_help() {
    println!("Usage: julian net <start|anchor|verify-envelope> ...");
    println!("  start --node-id <id> --log-dir <dir> --listen <multiaddr> [flags]");
    println!("  anchor --log-dir <dir> [--node-id <id>] [--quorum <N>]");
    println!("         (compat: julian net anchor <log_dir>)");
    println!("  verify-envelope --file <anchor.json> --log-dir <dir> [--quorum <N>]");
}

#[cfg(feature = "net")]
fn append_rollup_fault(path: &Path, ev: &power_house::rollup::RollupFaultEvidence) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    match serde_json::to_string(ev) {
        Ok(line) => {
            let line = format!("{line}\n");
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
        Some("scale_sumcheck") => {
            cmd_scale_sumcheck(args.collect());
        }
        #[cfg(feature = "net")]
        Some("keygen") => {
            cmd_keygen(args.collect());
        }
        #[cfg(feature = "net")]
        Some("net") => {
            let sub = args.next().unwrap_or_else(|| {
                eprintln!("Usage: julian net <start|anchor|verify-envelope> ...");
                std::process::exit(1);
            });
            handle_net(&sub, args.collect());
        }
        #[cfg(feature = "net")]
        Some("stake") => {
            let sub = args.next().unwrap_or_else(|| {
                eprintln!("Usage: julian stake <show|fund|bond|snapshot|claims|apply-claims|unbond|reward> ...");
                std::process::exit(1);
            });
            handle_stake(&sub, args.collect());
        }
        #[cfg(feature = "net")]
        Some("governance") => {
            let sub = args.next().unwrap_or_else(|| {
                eprintln!("Usage: julian governance <propose-migration> ...");
                std::process::exit(1);
            });
            handle_governance(&sub, args.collect());
        }
        #[cfg(feature = "net")]
        Some("rollup") => {
            let sub = args.next().unwrap_or_else(|| {
                eprintln!("Usage: julian rollup <settle> ...");
                std::process::exit(1);
            });
            handle_rollup(&sub, args.collect());
        }
        _ => {
            eprintln!("Usage: julian <node|scale_sumcheck|net|stake|governance|rollup|keygen> ...");
            std::process::exit(1);
        }
    }
}

#[cfg(feature = "net")]
fn handle_stake(sub: &str, tail: Vec<String>) {
    match sub {
        "-h" | "--help" => print_stake_help(),
        "show" => cmd_stake_show(tail),
        "fund" => cmd_stake_fund(tail),
        "bond" => cmd_stake_bond(tail),
        "snapshot" => cmd_stake_snapshot(tail),
        "claims" => cmd_stake_claims(tail),
        "apply-claims" => cmd_stake_apply_claims(tail),
        "unbond" => cmd_stake_unbond(tail),
        "reward" => cmd_stake_reward(tail),
        _ => {
            eprintln!("Unknown stake subcommand: {sub}");
            std::process::exit(1);
        }
    }
}

#[cfg(feature = "net")]
fn handle_governance(sub: &str, tail: Vec<String>) {
    match sub {
        "-h" | "--help" => print_governance_help(),
        "propose-migration" => cmd_governance_propose_migration(tail),
        _ => {
            eprintln!("Unknown governance subcommand: {sub}");
            std::process::exit(1);
        }
    }
}

#[cfg(feature = "net")]
fn handle_rollup(sub: &str, tail: Vec<String>) {
    match sub {
        "settle" => cmd_rollup_settle(tail),
        "settle-file" => cmd_rollup_settle_file(tail),
        _ => {
            eprintln!("Unknown rollup subcommand: {sub}");
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

fn cmd_scale_sumcheck(args: Vec<String>) {
    let mut max_vars: Option<usize> = None;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--vars" | "--max-vars" => {
                let value = iter
                    .next()
                    .unwrap_or_else(|| fatal("--vars expects a value"));
                max_vars = Some(
                    value
                        .parse()
                        .unwrap_or_else(|_| fatal("invalid --vars value")),
                );
            }
            other => fatal(&format!("unknown argument: {other}")),
        }
    }
    run_scale_sumcheck(max_vars);
}

fn run_scale_sumcheck(max_vars: Option<usize>) {
    let field = Field::new(257);
    let default_dims = [8usize, 10, 12, 14, 16, 18];
    let dimensions: Vec<usize> = match max_vars {
        Some(m) => (8..=m).step_by(2).collect(),
        None => default_dims.to_vec(),
    };
    if dimensions.is_empty() {
        fatal("No dimensions selected; provide --vars >= 8.");
    }
    let mut rows = Vec::new();
    println!(
        "{:>5} | {:>10} | {:>10} | {:>10} | {:>12} | {:>12}",
        "vars", "2^vars", "total(ms)", "avg(ms)", "max_round(ms)", "final_eval"
    );
    println!("{}", "-".repeat(70));
    for &vars in &dimensions {
        let evaluator = make_scale_evaluator(vars, field.modulus());
        let (proof, stats) = GeneralSumProof::prove_streaming_with_stats(vars, &field, evaluator);
        let (total_ms, avg_ms, max_round_ms) = summarize_stats(&stats);
        let size = 1usize << vars;
        rows.push((
            vars,
            size,
            total_ms,
            avg_ms,
            max_round_ms,
            proof.final_evaluation,
        ));
        println!(
            "{:>5} | {:>10} | {:>10.3} | {:>10.3} | {:>12.3} | {:>12}",
            vars, size, total_ms, avg_ms, max_round_ms, proof.final_evaluation
        );
    }

    if let Ok(path) = std::env::var("POWER_HOUSE_SCALE_OUT") {
        let mut file = fs::File::create(&path).expect("create csv output");
        use std::io::Write;
        writeln!(
            file,
            "vars,size,total_ms,avg_ms,max_round_ms,final_evaluation"
        )
        .expect("write csv header");
        for (vars, size, total_ms, avg_ms, max_round_ms, final_eval) in rows {
            writeln!(
                file,
                "{vars},{size},{total_ms:.6},{avg_ms:.6},{max_round_ms:.6},{final_eval}"
            )
            .expect("write csv row");
        }
        println!("CSV exported to {path}");
    }
}

fn make_scale_evaluator(
    num_vars: usize,
    modulus: u64,
) -> impl Fn(usize) -> u64 + Send + Sync + 'static {
    move |idx: usize| {
        let mut acc = (idx as u64) % modulus;
        for bit in 0..num_vars {
            let bit_value = ((idx >> bit) & 1) as u64;
            if bit_value == 0 {
                continue;
            }
            let coef = ((bit as u64 + 3).pow(2)) % modulus;
            acc = (acc + coef) % modulus;
        }
        for bit in 0..num_vars.saturating_sub(1) {
            let a = ((idx >> bit) & 1) as u64;
            let b = ((idx >> (bit + 1)) & 1) as u64;
            if a == 0 || b == 0 {
                continue;
            }
            let coef = (17 + (bit as u64 * 5)) % modulus;
            acc = (acc + coef) % modulus;
        }
        if num_vars >= 3 {
            let a = (idx & 1) as u64;
            let b = ((idx >> 1) & 1) as u64;
            let c = ((idx >> 2) & 1) as u64;
            if a == 1 && b == 1 && c == 1 {
                acc = (acc + 29) % modulus;
            }
        }
        acc % modulus
    }
}

fn ms(duration: &std::time::Duration) -> f64 {
    duration.as_secs_f64() * 1_000.0
}

fn summarize_stats(stats: &ProofStats) -> (f64, f64, f64) {
    if stats.round_durations.is_empty() {
        return (0.0, 0.0, 0.0);
    }
    let total = ms(&stats.total_duration);
    let max = stats.round_durations.iter().map(ms).fold(0.0f64, f64::max);
    let mean = total / (stats.round_durations.len() as f64);
    (total, mean, max)
}

#[cfg(feature = "net")]
fn handle_net(sub: &str, tail: Vec<String>) {
    match sub {
        "-h" | "--help" => print_net_help(),
        "start" => cmd_net_start(tail),
        "anchor" => cmd_net_anchor(tail),
        "verify-envelope" => cmd_net_verify_envelope(tail),
        _ => {
            eprintln!("Unknown net subcommand: {sub}");
            std::process::exit(1);
        }
    }
}

#[cfg(feature = "net")]
fn cmd_keygen(args: Vec<String>) {
    let mut key_spec: Option<String> = None;
    let mut out_path: Option<PathBuf> = None;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--out" => {
                out_path = Some(PathBuf::from(
                    iter.next()
                        .unwrap_or_else(|| fatal("--out expects a value")),
                ));
            }
            value => {
                if key_spec.is_none() {
                    key_spec = Some(value.to_string());
                } else {
                    fatal(&format!("unknown argument: {value}"));
                }
            }
        }
    }

    let key_source = Ed25519KeySource::from_spec(key_spec.as_deref());
    let passphrase = prompt_password("Identity passphrase: ")
        .unwrap_or_else(|err| fatal(&format!("failed to read passphrase: {err}")));
    let material = load_or_derive_keypair(&key_source)
        .unwrap_or_else(|err| fatal(&format!("failed to derive key: {err}")));
    let encoded = encrypt_identity_base64(&material.signing, &passphrase);
    let out_path = out_path.unwrap_or_else(|| PathBuf::from("julian.identity"));
    if let Some(parent) = out_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    fs::write(&out_path, format!("{encoded}\n"))
        .unwrap_or_else(|err| fatal(&format!("failed to write identity: {err}")));
    println!(
        "public_key_b64: {}",
        power_house::net::encode_public_key_base64(&material.verifying)
    );
    println!("identity_path: {}", out_path.display());
}

#[cfg(feature = "net")]
fn load_registry(path: &Path) -> StakeRegistry {
    StakeRegistry::load(path).unwrap_or_else(|err| {
        fatal(&format!(
            "failed to load stake registry {}: {err}",
            path.display()
        ))
    })
}

#[cfg(feature = "net")]
fn save_registry(path: &Path, reg: &StakeRegistry) {
    reg.save(path).unwrap_or_else(|err| {
        fatal(&format!(
            "failed to save stake registry {}: {err}",
            path.display()
        ))
    });
}

#[cfg(feature = "net")]
fn cmd_stake_show(args: Vec<String>) {
    if args.is_empty() {
        eprintln!("Usage: julian stake show <stake_registry.json>");
        std::process::exit(1);
    }
    let path = Path::new(&args[0]);
    match StakeRegistry::load(path) {
        Ok(reg) => {
            let pretty = serde_json::to_string_pretty(&reg)
                .unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"));
            println!("{pretty}");
        }
        Err(err) => fatal(&format!(
            "failed to load stake registry {}: {err}",
            path.display()
        )),
    }
}

#[cfg(feature = "net")]
fn cmd_stake_fund(args: Vec<String>) {
    if args.len() < 3 {
        eprintln!("Usage: julian stake fund <registry.json> <pubkey_b64> <amount>");
        std::process::exit(1);
    }
    let path = Path::new(&args[0]);
    let pk = &args[1];
    let amount: u64 = args[2].parse().unwrap_or_else(|_| fatal("invalid amount"));
    let mut reg = load_registry(path);
    reg.fund_balance(pk, amount);
    save_registry(path, &reg);
    if let Some(acct) = reg.account(pk) {
        println!(
            "funded {pk} by {amount}, balance={} stake={}",
            acct.balance, acct.stake
        );
    }
}

#[cfg(feature = "net")]
fn cmd_stake_bond(args: Vec<String>) {
    refresh_migration_mode_from_env();
    if power_house::net::migration_mode_frozen() {
        fatal("migration freeze active: stake bonding is disabled");
    }
    if args.len() < 3 {
        eprintln!("Usage: julian stake bond <registry.json> <pubkey_b64> <amount>");
        std::process::exit(1);
    }
    let path = Path::new(&args[0]);
    let pk = &args[1];
    let amount: u64 = args[2].parse().unwrap_or_else(|_| fatal("invalid amount"));
    let mut reg = load_registry(path);
    reg.bond_from_balance(pk, amount)
        .unwrap_or_else(|err| fatal(&err));
    save_registry(path, &reg);
    if let Some(acct) = reg.account(pk) {
        println!(
            "bonded {amount} for {pk}, balance={} stake={}",
            acct.balance, acct.stake
        );
    }
}

#[cfg(feature = "net")]
fn cmd_stake_snapshot(args: Vec<String>) {
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("Usage: julian stake snapshot --registry <path> --height <N> --output <file>");
        return;
    }

    let mut registry_path: Option<String> = None;
    let mut height: Option<u64> = None;
    let mut output: Option<String> = None;

    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--registry" => {
                registry_path = Some(
                    iter.next()
                        .unwrap_or_else(|| fatal("--registry expects a value")),
                );
            }
            "--height" => {
                let raw = iter
                    .next()
                    .unwrap_or_else(|| fatal("--height expects a value"));
                height = Some(
                    raw.parse::<u64>()
                        .unwrap_or_else(|_| fatal("invalid --height")),
                );
            }
            "--output" => {
                output = Some(
                    iter.next()
                        .unwrap_or_else(|| fatal("--output expects a value")),
                );
            }
            other => fatal(&format!("unknown argument: {other}")),
        }
    }

    let registry_path = registry_path.unwrap_or_else(|| fatal("--registry is required"));
    let height = height.unwrap_or_else(|| fatal("--height is required"));
    let output = output.unwrap_or_else(|| fatal("--output is required"));

    let root = run_snapshot(&registry_path, height, &output)
        .unwrap_or_else(|err| fatal(&format!("snapshot failed: {err}")));
    println!("snapshot root: {root}");
    println!("artifact: {output}");
}

#[cfg(feature = "net")]
fn cmd_stake_claims(args: Vec<String>) {
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("Usage: julian stake claims --snapshot <file> --output <file> [options]");
        println!("  [--mode native|erc20]");
        println!("  [--amount-source stake|balance|total] [--include-slashed]");
        println!("  [--conversion-ratio <u64>] [--claim-id-salt <text>]");
        println!("  [--token-contract <id>] [--snapshot-height <u64>]");
        return;
    }

    let mut snapshot: Option<String> = None;
    let mut output: Option<String> = None;
    let mut claim_mode = String::from("native");
    let mut amount_source = String::from("total");
    let mut include_slashed = false;
    let mut conversion_ratio: u64 = 1;
    let mut claim_id_salt = String::from("mfenx-migration-claim-v1");
    let mut token_contract: Option<String> = None;
    let mut snapshot_height_override: Option<u64> = None;

    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--snapshot" => {
                snapshot = Some(
                    iter.next()
                        .unwrap_or_else(|| fatal("--snapshot expects a value")),
                );
            }
            "--output" => {
                output = Some(
                    iter.next()
                        .unwrap_or_else(|| fatal("--output expects a value")),
                );
            }
            "--mode" => {
                claim_mode = iter
                    .next()
                    .unwrap_or_else(|| fatal("--mode expects a value"));
            }
            "--amount-source" => {
                amount_source = iter
                    .next()
                    .unwrap_or_else(|| fatal("--amount-source expects a value"));
            }
            "--include-slashed" => {
                include_slashed = true;
            }
            "--conversion-ratio" => {
                let raw = iter
                    .next()
                    .unwrap_or_else(|| fatal("--conversion-ratio expects a value"));
                conversion_ratio = raw
                    .parse::<u64>()
                    .unwrap_or_else(|_| fatal("invalid --conversion-ratio"));
            }
            "--claim-id-salt" => {
                claim_id_salt = iter
                    .next()
                    .unwrap_or_else(|| fatal("--claim-id-salt expects a value"));
            }
            "--token-contract" => {
                token_contract = Some(
                    iter.next()
                        .unwrap_or_else(|| fatal("--token-contract expects a value")),
                );
            }
            "--snapshot-height" => {
                let raw = iter
                    .next()
                    .unwrap_or_else(|| fatal("--snapshot-height expects a value"));
                snapshot_height_override = Some(
                    raw.parse::<u64>()
                        .unwrap_or_else(|_| fatal("invalid --snapshot-height")),
                );
            }
            other => fatal(&format!("unknown argument: {other}")),
        }
    }

    let snapshot = snapshot.unwrap_or_else(|| fatal("--snapshot is required"));
    let output = output.unwrap_or_else(|| fatal("--output is required"));
    let opts = BuildClaimsOptions {
        claim_mode,
        amount_source,
        include_slashed,
        conversion_ratio,
        claim_id_salt,
        token_contract,
        snapshot_height_override,
    };

    let root = run_build_claims(&snapshot, &output, &opts)
        .unwrap_or_else(|err| fatal(&format!("claim build failed: {err}")));
    println!("claims root: {root}");
    println!("artifact: {output}");
}

#[cfg(feature = "net")]
fn cmd_stake_apply_claims(args: Vec<String>) {
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("Usage: julian stake apply-claims --registry <file> --claims <file> [options]");
        println!("  [--state <file>] [--dry-run]");
        return;
    }

    let mut registry: Option<String> = None;
    let mut claims: Option<String> = None;
    let mut state_path: Option<String> = None;
    let mut dry_run = false;

    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--registry" => {
                registry = Some(
                    iter.next()
                        .unwrap_or_else(|| fatal("--registry expects a value")),
                );
            }
            "--claims" => {
                claims = Some(
                    iter.next()
                        .unwrap_or_else(|| fatal("--claims expects a value")),
                );
            }
            "--state" => {
                state_path = Some(
                    iter.next()
                        .unwrap_or_else(|| fatal("--state expects a value")),
                );
            }
            "--dry-run" => {
                dry_run = true;
            }
            other => fatal(&format!("unknown argument: {other}")),
        }
    }

    let registry = registry.unwrap_or_else(|| fatal("--registry is required"));
    let claims = claims.unwrap_or_else(|| fatal("--claims is required"));
    let opts = ApplyClaimsOptions {
        state_path,
        dry_run,
    };

    let summary = run_apply_claims(&registry, &claims, &opts)
        .unwrap_or_else(|err| fatal(&format!("apply-claims failed: {err}")));
    println!("applied: {}", summary.applied);
    println!("skipped: {}", summary.skipped);
    println!("total_mint_amount: {}", summary.total_mint_amount);
    println!("state: {}", summary.state_path);
    if dry_run {
        println!("dry_run: true");
    }
}

#[cfg(feature = "net")]
fn cmd_governance_propose_migration(args: Vec<String>) {
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("Usage: julian governance propose-migration \\");
        println!("  --snapshot-height <N> [--token-contract <id>] \\");
        println!("  [--conversion-ratio <u64>] [--treasury-mint <u64>] \\");
        println!("  --log-dir <dir> [--node-id <id>] [--quorum <N>] [--output <file>]");
        return;
    }

    #[derive(serde::Serialize)]
    struct MigrationProposalArtifact {
        migration_anchor: power_house::net::MigrationAnchor,
        anchor_json: AnchorJson,
    }

    let mut snapshot_height: Option<u64> = None;
    let mut token_contract: Option<String> = None;
    let mut conversion_ratio: u64 = 1;
    let mut treasury_mint: u64 = 0;
    let mut log_dir: Option<String> = None;
    let mut node_id: String = "migration-governance".to_string();
    let mut quorum: usize = 1;
    let mut output: Option<String> = None;

    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--snapshot-height" => {
                let raw = iter
                    .next()
                    .unwrap_or_else(|| fatal("--snapshot-height expects a value"));
                snapshot_height = Some(
                    raw.parse::<u64>()
                        .unwrap_or_else(|_| fatal("invalid --snapshot-height")),
                );
            }
            "--token-contract" => {
                token_contract = Some(
                    iter.next()
                        .unwrap_or_else(|| fatal("--token-contract expects a value")),
                );
            }
            "--conversion-ratio" => {
                let raw = iter
                    .next()
                    .unwrap_or_else(|| fatal("--conversion-ratio expects a value"));
                conversion_ratio = raw
                    .parse::<u64>()
                    .unwrap_or_else(|_| fatal("invalid --conversion-ratio"));
            }
            "--treasury-mint" => {
                let raw = iter
                    .next()
                    .unwrap_or_else(|| fatal("--treasury-mint expects a value"));
                treasury_mint = raw
                    .parse::<u64>()
                    .unwrap_or_else(|_| fatal("invalid --treasury-mint"));
            }
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
                let raw = iter
                    .next()
                    .unwrap_or_else(|| fatal("--quorum expects a value"));
                quorum = raw
                    .parse::<usize>()
                    .unwrap_or_else(|_| fatal("invalid --quorum"));
            }
            "--output" => {
                output = Some(
                    iter.next()
                        .unwrap_or_else(|| fatal("--output expects a value")),
                );
            }
            other => fatal(&format!("unknown argument: {other}")),
        }
    }

    let snapshot_height = snapshot_height.unwrap_or_else(|| fatal("--snapshot-height is required"));
    let token_contract = token_contract
        .or_else(|| std::env::var("PH_MIGRATION_TOKEN_ID").ok())
        .unwrap_or_else(|| "native://julian".to_string());
    let log_dir = log_dir.unwrap_or_else(|| fatal("--log-dir is required"));

    let proposal = MigrationProposal {
        snapshot_height,
        token_contract,
        conversion_ratio: if conversion_ratio == 0 {
            1
        } else {
            conversion_ratio
        },
        treasury_mint,
    };
    let migration_anchor = proposal
        .to_anchor_payload()
        .unwrap_or_else(|err| fatal(&format!("failed to build migration payload: {err}")));
    let proposal_digest = power_house::transcript_digest_from_hex(&migration_anchor.proposal_hash)
        .unwrap_or_else(|err| fatal(&format!("invalid proposal hash: {err}")));

    let mut ledger = load_anchor_from_logs(Path::new(&log_dir)).unwrap_or_else(|err| fatal(&err));
    ledger.entries.push(EntryAnchor {
        statement: migration_anchor.statement.clone(),
        merkle_root: power_house::merkle_root(&[proposal_digest]),
        hashes: vec![proposal_digest],
    });
    ledger.metadata.fold_digest = Some(compute_fold_digest(&ledger));
    ledger
        .metadata
        .crate_version
        .get_or_insert_with(|| env!("CARGO_PKG_VERSION").to_string());

    let anchor_json =
        AnchorJson::from_ledger(node_id, quorum, &ledger, now_millis(), Vec::new(), None)
            .unwrap_or_else(|err| fatal(&format!("anchor conversion failed: {err}")));

    let artifact = MigrationProposalArtifact {
        migration_anchor,
        anchor_json,
    };
    let encoded = serde_json::to_string_pretty(&artifact)
        .unwrap_or_else(|err| fatal(&format!("failed to encode artifact: {err}")));

    if let Some(path) = output {
        if let Some(parent) = Path::new(&path).parent() {
            let _ = fs::create_dir_all(parent);
        }
        fs::write(&path, &encoded)
            .unwrap_or_else(|err| fatal(&format!("failed to write {}: {err}", path)));
        println!("wrote migration proposal artifact to {path}");
    } else {
        println!("{encoded}");
    }
}

#[cfg(feature = "net")]
fn cmd_stake_unbond(args: Vec<String>) {
    if args.len() < 3 {
        eprintln!("Usage: julian stake unbond <registry.json> <pubkey_b64> <amount>");
        std::process::exit(1);
    }
    let path = Path::new(&args[0]);
    let pk = &args[1];
    let amount: u64 = args[2].parse().unwrap_or_else(|_| fatal("invalid amount"));
    let mut reg = load_registry(path);
    reg.unbond(pk, amount).unwrap_or_else(|err| fatal(&err));
    save_registry(path, &reg);
    if let Some(acct) = reg.account(pk) {
        println!(
            "unbonded {amount} for {pk}, balance={} stake={}",
            acct.balance, acct.stake
        );
    }
}

#[cfg(feature = "net")]
fn cmd_stake_reward(args: Vec<String>) {
    if args.len() < 3 {
        eprintln!("Usage: julian stake reward <registry.json> <pubkey_b64> <amount>");
        std::process::exit(1);
    }
    let path = Path::new(&args[0]);
    let pk = &args[1];
    let amount: u64 = args[2].parse().unwrap_or_else(|_| fatal("invalid amount"));
    let mut reg = load_registry(path);
    reg.credit_reward(pk, amount);
    save_registry(path, &reg);
    if let Some(acct) = reg.account(pk) {
        println!(
            "rewarded {pk} by {amount}, balance={} stake={}",
            acct.balance, acct.stake
        );
    }
}

#[cfg(feature = "net")]
fn cmd_rollup_settle(args: Vec<String>) {
    if args.len() < 5 {
        eprintln!("Usage: julian rollup settle <registry.json> <namespace> <share_root> <payer_b64> <fee> [zk|optimistic] [operator_b64] [attesters_csv] [--proof file] [--public-inputs file] [--merkle-path file] [--outbox path]");
        std::process::exit(1);
    }
    let registry = Path::new(&args[0]);
    let namespace = args[1].clone();
    let share_root = args[2].clone();
    let payer = args[3].clone();
    let fee: u64 = args[4].parse().unwrap_or_else(|_| fatal("invalid fee"));

    let mut mode = "optimistic".to_string();
    let mut operator_pk: Option<String> = None;
    let mut attesters: Vec<String> = Vec::new();
    let mut proof_path: Option<String> = None;
    let mut public_inputs_path: Option<String> = None;
    let mut merkle_path_file: Option<String> = None;
    let mut outbox: Option<String> = None;

    for arg in args.iter().skip(5) {
        if arg.starts_with("--proof=") {
            proof_path = Some(arg.trim_start_matches("--proof=").to_string());
        } else if arg.starts_with("--public-inputs=") {
            public_inputs_path = Some(arg.trim_start_matches("--public-inputs=").to_string());
        } else if arg.starts_with("--merkle-path=") {
            merkle_path_file = Some(arg.trim_start_matches("--merkle-path=").to_string());
        } else if arg.starts_with("--outbox=") {
            outbox = Some(arg.trim_start_matches("--outbox=").to_string());
        } else if mode == "optimistic" && (arg == "zk" || arg == "optimistic") {
            mode = arg.clone();
        } else if operator_pk.is_none() {
            operator_pk = Some(arg.clone());
        } else if attesters.is_empty() {
            attesters = arg
                .split(',')
                .filter(|v| !v.is_empty())
                .map(|v| v.to_string())
                .collect();
        }
    }

    let operator_pk = operator_pk.unwrap_or_else(|| payer.clone());
    let commitment = power_house::rollup::RollupCommitment {
        namespace,
        share_root,
        pedersen_root: None,
        settlement_slot: None,
    };

    let zk_proof = if let (Some(pp), Some(pi_path), Some(mp_path)) =
        (proof_path, public_inputs_path, merkle_path_file)
    {
        let proof_bytes = std::fs::read(&pp).unwrap_or_else(|_| fatal("failed to read proof file"));
        let public_inputs =
            std::fs::read(&pi_path).unwrap_or_else(|_| fatal("failed to read public inputs file"));
        let merkle_path =
            std::fs::read(&mp_path).unwrap_or_else(|_| fatal("failed to read merkle path file"));
        power_house::rollup::ZkRollupProof {
            proof: proof_bytes,
            public_inputs,
            merkle_path,
        }
    } else {
        power_house::rollup::ZkRollupProof {
            proof: Vec::new(),
            public_inputs: Vec::new(),
            merkle_path: Vec::new(),
        }
    };

    let result = match mode.as_str() {
        "zk" => power_house::rollup::settle_rollup_with_rewards(
            registry,
            commitment.clone(),
            &payer,
            &operator_pk,
            &attesters,
            fee,
            power_house::rollup::RollupSettlementMode::Zk(zk_proof),
        ),
        _ => power_house::rollup::settle_rollup_with_rewards(
            registry,
            commitment.clone(),
            &payer,
            &operator_pk,
            &attesters,
            fee,
            power_house::rollup::RollupSettlementMode::Optimistic(Vec::new()),
        ),
    };
    match result {
        Ok(receipt) => {
            println!(
                "settled rollup for {payer} fee={fee} commitment={}",
                receipt.commitment.share_root
            );
        }
        Err(err) => {
            let outbox_path: PathBuf = outbox.map(PathBuf::from).unwrap_or_else(|| {
                registry
                    .parent()
                    .unwrap_or(Path::new("."))
                    .join("evidence_outbox.jsonl")
            });
            append_rollup_fault(&outbox_path, &err);
            eprintln!("fault evidence written to {}", outbox_path.display());
            fatal(&format!("settlement failed: {}", err.reason));
        }
    }
}

#[cfg(feature = "net")]
fn cmd_rollup_settle_file(args: Vec<String>) {
    if args.len() < 2 {
        eprintln!(
            "Usage: julian rollup settle-file <registry.json> <request.json> [--outbox path]"
        );
        std::process::exit(1);
    }
    #[derive(serde::Deserialize)]
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
    let registry = Path::new(&args[0]);
    let req_bytes =
        std::fs::read(&args[1]).unwrap_or_else(|_| fatal("failed to read request file"));
    let mut outbox: Option<String> = None;
    for arg in args.iter().skip(2) {
        if arg.starts_with("--outbox=") {
            outbox = Some(arg.trim_start_matches("--outbox=").to_string());
        }
    }
    let req: RollupSettleRequest =
        serde_json::from_slice(&req_bytes).unwrap_or_else(|_| fatal("invalid request JSON"));
    let pedersen_root = req
        .pedersen_root
        .clone()
        .unwrap_or_else(|| req.share_root.clone());
    let commitment = power_house::rollup::RollupCommitment {
        namespace: req.namespace.clone(),
        share_root: req.share_root.clone(),
        pedersen_root: Some(pedersen_root),
        settlement_slot: None,
    };
    let operator_pk = req
        .operator_pk
        .clone()
        .unwrap_or_else(|| req.payer_pk.clone());
    let attesters = req.attesters.clone().unwrap_or_default();
    let mode = req.mode.clone().unwrap_or_else(|| "optimistic".to_string());
    let zk_proof = if let (Some(p_b64), Some(pi_b64), Some(mp_b64)) = (
        req.proof_b64.as_ref(),
        req.public_inputs_b64.as_ref(),
        req.merkle_path_b64.as_ref(),
    ) {
        let proof = BASE64
            .decode(p_b64.as_bytes())
            .unwrap_or_else(|_| fatal("proof decode failed"));
        let public_inputs = BASE64
            .decode(pi_b64.as_bytes())
            .unwrap_or_else(|_| fatal("public inputs decode failed"));
        let merkle_path = BASE64
            .decode(mp_b64.as_bytes())
            .unwrap_or_else(|_| fatal("merkle path decode failed"));
        power_house::rollup::ZkRollupProof {
            proof,
            public_inputs,
            merkle_path,
        }
    } else {
        power_house::rollup::ZkRollupProof {
            proof: Vec::new(),
            public_inputs: Vec::new(),
            merkle_path: Vec::new(),
        }
    };
    let mode_enum = if mode == "zk" {
        power_house::rollup::RollupSettlementMode::Zk(zk_proof)
    } else {
        power_house::rollup::RollupSettlementMode::Optimistic(Vec::new())
    };
    match power_house::rollup::settle_rollup_with_rewards(
        registry,
        commitment.clone(),
        &req.payer_pk,
        &operator_pk,
        &attesters,
        req.fee,
        mode_enum,
    ) {
        Ok(receipt) => println!(
            "settled rollup fee={} commitment={}",
            receipt.fee, receipt.commitment.share_root
        ),
        Err(fault) => {
            let outbox_path: PathBuf = outbox.map(PathBuf::from).unwrap_or_else(|| {
                registry
                    .parent()
                    .unwrap_or(Path::new("."))
                    .join("evidence_outbox.jsonl")
            });
            append_rollup_fault(&outbox_path, &fault);
            eprintln!("fault evidence written to {}", outbox_path.display());
            fatal(&format!("settlement failed: {}", fault.reason));
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
    let anchor = load_anchor_from_logs(log_dir).unwrap_or_else(|err| fatal(&err.to_string()));
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
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!(
            "Usage: julian net start --node-id <id> --log-dir <dir> --listen <multiaddr> [flags]"
        );
        println!(
            "  Flags include --token-mode <native|TOKEN_ID> and optional --token-oracle <RPC_URL>."
        );
        return;
    }

    refresh_migration_mode_from_env();
    let mut node_id = None;
    let mut log_dir = None;
    let mut listen = None;
    let mut bootstraps = Vec::new();
    let mut quorum: usize = 1;
    let mut broadcast_ms: u64 = 5_000;
    let mut key_spec: Option<String> = None;
    let mut identity_path: Option<String> = None;
    let mut anchor_topic_spec: Option<String> = None;
    let mut gossip_shard_spec: Option<String> = None;
    let mut gossip_bridge_topics_spec: Option<String> = None;
    let mut bft_enabled = false;
    let mut bft_round_ms_spec: Option<String> = None;
    let mut metrics_addr_spec: Option<String> = None;
    let mut policy_allowlist_spec: Option<String> = None;
    let mut policy_spec: Option<String> = None;
    let mut checkpoint_interval_spec: Option<String> = None;
    let mut blob_dir_spec: Option<String> = None;
    let mut blob_listen_spec: Option<String> = None;
    let mut max_blob_bytes_spec: Option<String> = None;
    let mut blob_retention_days_spec: Option<String> = None;
    let mut blob_policy_spec: Option<String> = None;
    let mut blob_auth_token_spec: Option<String> = None;
    let mut blob_max_concurrency_spec: Option<String> = None;
    let mut blob_request_timeout_ms_spec: Option<String> = None;
    let mut attestation_quorum_spec: Option<String> = None;
    let mut tokio_threads_spec: Option<String> = None;
    let mut token_mode_contract_spec: Option<String> = None;
    let mut token_oracle_rpc_spec: Option<String> = None;

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
            "--bootnodes" => {
                let value = iter
                    .next()
                    .unwrap_or_else(|| fatal("--bootnodes expects a value"));
                for raw in value.split(',') {
                    let trimmed = raw.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    let addr: Multiaddr = trimmed
                        .parse()
                        .unwrap_or_else(|_| fatal("invalid multiaddr for --bootnodes"));
                    bootstraps.push(addr);
                }
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
            "--anchor-topic" => {
                anchor_topic_spec = Some(
                    iter.next()
                        .unwrap_or_else(|| fatal("--anchor-topic expects a value")),
                );
            }
            "--gossip-shard" => {
                gossip_shard_spec = Some(
                    iter.next()
                        .unwrap_or_else(|| fatal("--gossip-shard expects a value")),
                );
            }
            "--gossip-bridge-topics" => {
                gossip_bridge_topics_spec = Some(
                    iter.next()
                        .unwrap_or_else(|| fatal("--gossip-bridge-topics expects a value")),
                );
            }
            "--bft" => {
                bft_enabled = true;
            }
            "--bft-round-ms" => {
                bft_round_ms_spec = Some(
                    iter.next()
                        .unwrap_or_else(|| fatal("--bft-round-ms expects a value")),
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
            "--policy" => {
                policy_spec = Some(
                    iter.next()
                        .unwrap_or_else(|| fatal("--policy expects a value")),
                );
            }
            "--checkpoint-interval" => {
                checkpoint_interval_spec = Some(
                    iter.next()
                        .unwrap_or_else(|| fatal("--checkpoint-interval expects a value")),
                );
            }
            "--blob-dir" => {
                blob_dir_spec = Some(
                    iter.next()
                        .unwrap_or_else(|| fatal("--blob-dir expects a value")),
                );
            }
            "--blob-listen" => {
                blob_listen_spec = Some(
                    iter.next()
                        .unwrap_or_else(|| fatal("--blob-listen expects a value")),
                );
            }
            "--max-blob-bytes" => {
                max_blob_bytes_spec = Some(
                    iter.next()
                        .unwrap_or_else(|| fatal("--max-blob-bytes expects a value")),
                );
            }
            "--blob-retention-days" => {
                blob_retention_days_spec = Some(
                    iter.next()
                        .unwrap_or_else(|| fatal("--blob-retention-days expects a value")),
                );
            }
            "--blob-policy" => {
                blob_policy_spec = Some(
                    iter.next()
                        .unwrap_or_else(|| fatal("--blob-policy expects a value")),
                );
            }
            "--blob-auth-token" => {
                blob_auth_token_spec = Some(
                    iter.next()
                        .unwrap_or_else(|| fatal("--blob-auth-token expects a value")),
                );
            }
            "--blob-max-concurrency" => {
                blob_max_concurrency_spec = Some(
                    iter.next()
                        .unwrap_or_else(|| fatal("--blob-max-concurrency expects a value")),
                );
            }
            "--blob-request-timeout-ms" => {
                blob_request_timeout_ms_spec = Some(
                    iter.next()
                        .unwrap_or_else(|| fatal("--blob-request-timeout-ms expects a value")),
                );
            }
            "--attestation-quorum" => {
                attestation_quorum_spec = Some(
                    iter.next()
                        .unwrap_or_else(|| fatal("--attestation-quorum expects a value")),
                );
            }
            "--tokio-threads" => {
                tokio_threads_spec = Some(
                    iter.next()
                        .unwrap_or_else(|| fatal("--tokio-threads expects a value")),
                );
            }
            "--token-mode" => {
                token_mode_contract_spec = Some(
                    iter.next()
                        .unwrap_or_else(|| fatal("--token-mode expects a value")),
                );
            }
            "--token-oracle" => {
                token_oracle_rpc_spec = Some(
                    iter.next()
                        .unwrap_or_else(|| fatal("--token-oracle expects a value")),
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
    let membership_policy =
        load_membership_policy(policy_spec.as_deref(), policy_allowlist_spec.as_deref());
    let checkpoint_interval = checkpoint_interval_spec.map(|value| {
        value
            .parse()
            .unwrap_or_else(|_| fatal("invalid --checkpoint-interval"))
    });
    let blob_dir = blob_dir_spec.map(PathBuf::from);
    let blob_listen = blob_listen_spec
        .as_deref()
        .map(parse_metrics_addr)
        .unwrap_or(None);
    let max_blob_bytes = max_blob_bytes_spec.map(|v| {
        v.parse::<usize>()
            .unwrap_or_else(|_| fatal("invalid --max-blob-bytes"))
    });
    let blob_retention_days = blob_retention_days_spec.map(|v| {
        v.parse::<u64>()
            .unwrap_or_else(|_| fatal("invalid --blob-retention-days"))
    });
    let blob_policies = blob_policy_spec
        .as_deref()
        .map(|path| load_blob_policies(Path::new(path)));
    let blob_auth_token = blob_auth_token_spec;
    let blob_max_concurrency = blob_max_concurrency_spec.map(|v| {
        v.parse::<usize>()
            .unwrap_or_else(|_| fatal("invalid --blob-max-concurrency"))
    });
    let blob_request_timeout_ms = blob_request_timeout_ms_spec.map(|v| {
        v.parse::<u64>()
            .unwrap_or_else(|_| fatal("invalid --blob-request-timeout-ms"))
    });
    let attestation_quorum = attestation_quorum_spec.map(|v| {
        v.parse::<usize>()
            .unwrap_or_else(|_| fatal("invalid --attestation-quorum"))
    });
    let anchor_topic = anchor_topic_spec.or_else(|| {
        gossip_shard_spec.map(|shard| format!("mfenx/powerhouse/anchors/v1/shard/{shard}"))
    });
    let gossip_bridge_topics = gossip_bridge_topics_spec.as_deref().map(parse_topic_list);
    let bft_round_ms = bft_round_ms_spec.map(|v| {
        v.parse::<u64>()
            .unwrap_or_else(|_| fatal("invalid --bft-round-ms"))
    });
    let tokio_threads = tokio_threads_spec.map(|v| {
        v.parse::<usize>()
            .unwrap_or_else(|_| fatal("invalid --tokio-threads"))
    });

    let config = NetConfig::new(
        node_id,
        listen_addr,
        bootstraps,
        PathBuf::from(log_dir),
        quorum,
        Duration::from_millis(broadcast_ms),
        key_material,
        anchor_topic,
        gossip_bridge_topics,
        bft_enabled,
        bft_round_ms,
        metrics_addr,
        membership_policy.clone(),
        checkpoint_interval,
        blob_dir,
        blob_listen,
        max_blob_bytes,
        blob_retention_days,
        blob_policies,
        blob_auth_token,
        blob_max_concurrency,
        blob_request_timeout_ms,
        attestation_quorum,
        token_mode_contract_spec,
        token_oracle_rpc_spec,
    );

    let mut builder = tokio::runtime::Builder::new_multi_thread();
    builder.enable_all();
    if let Some(threads) = tokio_threads {
        builder.worker_threads(threads);
    }
    let runtime = builder
        .build()
        .unwrap_or_else(|err| fatal(&format!("failed to start runtime: {err}")));
    if let Err(err) = runtime.block_on(run_network(config)) {
        fatal(&format!("network error: {err}"));
    }
}

#[cfg(feature = "net")]
fn cmd_net_anchor(args: Vec<String>) {
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!("Usage: julian net anchor --log-dir <dir> [--node-id <id>] [--quorum <N>]");
        println!("Compat: julian net anchor <log_dir> [--node-id <id>] [--quorum <N>]");
        return;
    }

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
            other => {
                if other.starts_with("--") {
                    fatal(&format!("unknown argument: {other}"));
                }
                if log_dir.is_none() {
                    log_dir = Some(other.to_string());
                } else {
                    fatal(&format!("unexpected positional argument: {other}"));
                }
            }
        }
    }

    let log_dir = log_dir.unwrap_or_else(|| fatal("--log-dir is required"));
    let ledger = load_anchor_from_logs(Path::new(&log_dir)).unwrap_or_else(|err| fatal(&err));
    let anchor_json =
        AnchorJson::from_ledger(node_id, quorum, &ledger, now_millis(), Vec::new(), None)
            .unwrap_or_else(|err| fatal(&format!("anchor conversion failed: {err}")));
    match anchor_json.to_json_string() {
        Ok(text) => println!("{text}"),
        Err(err) => fatal(&format!("FAIL: failed to encode anchor: {err}")),
    }
}

#[cfg(feature = "net")]
fn cmd_net_verify_envelope(args: Vec<String>) {
    if args.iter().any(|a| a == "-h" || a == "--help") {
        println!(
            "Usage: julian net verify-envelope --file <anchor.json> --log-dir <dir> [--quorum <N>]"
        );
        return;
    }

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
    #[allow(unused_mut)]
    let mut anchor_from_checkpoint = false;
    let anchor = {
        #[cfg(feature = "net")]
        {
            match power_house::net::load_latest_checkpoint(path) {
                Ok(Some(checkpoint)) => {
                    anchor_from_checkpoint = true;
                    match checkpoint.into_ledger() {
                        Ok((anchor, cp_cutoff)) => {
                            cutoff = cp_cutoff;
                            anchor
                        }
                        Err(err) => return Err(format!("checkpoint error: {err}")),
                    }
                }
                Ok(None) => julian_genesis_anchor(),
                Err(err) => return Err(format!("checkpoint error: {err}")),
            }
        }
        #[cfg(not(feature = "net"))]
        {
            julian_genesis_anchor()
        }
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
    let mut files: Vec<PathBuf> = fs::read_dir(path)
        .map_err(|err| format!("failed to read directory {}: {err}", path.display()))?
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
        let parsed = parse_log_file(&file)?;
        if let Some(mode) = parsed.metadata.challenge_mode {
            match &mut metadata.challenge_mode {
                None => metadata.challenge_mode = Some(mode),
                Some(existing) if existing != &mode => {
                    return Err(format!(
                        "{} challenge_mode {} conflicts with existing {}",
                        file.display(),
                        mode,
                        existing
                    ));
                }
                _ => {}
            }
        }
        if let Some(digest) = parsed.metadata.fold_digest {
            if let Some(existing) = &metadata.fold_digest {
                if existing != &digest && anchor_from_checkpoint {
                    return Err(format!(
                        "{} fold_digest conflicts with existing value",
                        file.display()
                    ));
                }
            }
            metadata.fold_digest = Some(digest);
        }
        let entry_hashes = vec![parsed.digest];
        entries.push(EntryAnchor {
            statement: parsed.statement,
            merkle_root: power_house::merkle_root(&entry_hashes),
            hashes: entry_hashes,
        });
    }
    if entries.is_empty() {
        entries = julian_genesis_anchor().entries;
    }
    if let Some(digest) = read_fold_digest_hint(path)? {
        if let Some(existing) = &metadata.fold_digest {
            if existing != &digest && anchor_from_checkpoint {
                return Err("fold_digest hint conflicts with checkpoint metadata".to_string());
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
    if let Some(mode) = &anchor.metadata.challenge_mode {
        lines.push(format!("# challenge_mode: {mode}"));
    }
    if let Some(digest) = &anchor.metadata.fold_digest {
        lines.push(format!(
            "# fold_digest: {}",
            power_house::transcript_digest_to_hex(digest)
        ));
    }
    if let Some(version) = &anchor.metadata.crate_version {
        lines.push(format!("# crate_version: {version}"));
    }
    for entry in &anchor.entries {
        let hash_list = entry
            .hashes
            .iter()
            .map(power_house::transcript_digest_to_hex)
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
    let mut metadata = AnchorMetadata::default();
    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix('#') {
            if let Some((key, value)) = rest.trim().split_once(':') {
                let key = key.trim();
                let value = value.trim();
                match key {
                    "challenge_mode" if !value.is_empty() => {
                        metadata.challenge_mode = Some(value.to_string())
                    }
                    "fold_digest" if !value.is_empty() => {
                        metadata.fold_digest =
                            Some(power_house::transcript_digest_from_hex(value).map_err(
                                |err| format!("invalid fold_digest value {value}: {err}"),
                            )?);
                    }
                    "crate_version" if !value.is_empty() => {
                        metadata.crate_version = Some(value.to_string())
                    }
                    _ => {}
                }
            }
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
    if metadata.fold_digest.is_none() {
        let temp = LedgerAnchor {
            entries: entries.clone(),
            metadata: AnchorMetadata::default(),
        };
        metadata.fold_digest = Some(compute_fold_digest(&temp));
    }
    metadata
        .crate_version
        .get_or_insert_with(|| env!("CARGO_PKG_VERSION").to_string());
    Ok(LedgerAnchor { entries, metadata })
}

fn format_anchor(anchor: &LedgerAnchor) -> String {
    let mut lines = Vec::new();
    if let Some(mode) = &anchor.metadata.challenge_mode {
        lines.push(format!("challenge_mode: {mode}"));
    }
    if let Some(digest) = &anchor.metadata.fold_digest {
        lines.push(format!(
            "fold_digest: {}",
            power_house::transcript_digest_to_hex(digest)
        ));
    }
    if let Some(version) = &anchor.metadata.crate_version {
        lines.push(format!("crate_version: {version}"));
    }
    for entry in &anchor.entries {
        let hashes = entry
            .hashes
            .iter()
            .map(power_house::transcript_digest_to_hex)
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

#[cfg(feature = "net")]
fn parse_topic_list(spec: &str) -> Vec<String> {
    spec.split(|c: char| c == ',' || c.is_whitespace())
        .filter_map(|raw| {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        })
        .collect()
}

#[cfg(feature = "net")]
#[derive(Debug, Deserialize)]
#[serde(tag = "backend", rename_all = "kebab-case")]
enum GovernanceDescriptor {
    Static {
        allowlist: Vec<String>,
    },
    StaticFile {
        path: String,
    },
    Multisig {
        state_path: String,
    },
    Stake {
        state_path: String,
        #[serde(default)]
        min_stake: Option<u64>,
        #[serde(default)]
        slash_pct: Option<u8>,
    },
}

#[cfg(feature = "net")]
fn load_membership_policy(
    policy_spec: Option<&str>,
    allowlist_spec: Option<&str>,
) -> Arc<dyn MembershipPolicy> {
    if let Some(spec_path) = policy_spec {
        let path = Path::new(spec_path);
        let contents = fs::read_to_string(path)
            .unwrap_or_else(|err| fatal(&format!("failed to read policy {spec_path}: {err}")));
        let descriptor: GovernanceDescriptor = serde_json::from_str(&contents)
            .unwrap_or_else(|err| fatal(&format!("invalid policy descriptor {spec_path}: {err}")));
        match descriptor {
            GovernanceDescriptor::Static { allowlist } => {
                StaticPolicy::from_base64_strings(&allowlist)
                    .map(|p| Arc::new(p) as Arc<dyn MembershipPolicy>)
                    .unwrap_or_else(|err| fatal(&format!("failed to load static policy: {err}")))
            }
            GovernanceDescriptor::StaticFile { path } => {
                StaticPolicy::from_allowlist(Path::new(&path))
                    .map(|p| Arc::new(p) as Arc<dyn MembershipPolicy>)
                    .unwrap_or_else(|err| fatal(&format!("failed to load allowlist policy: {err}")))
            }
            GovernanceDescriptor::Multisig { state_path } => {
                MultisigPolicy::load(Path::new(&state_path))
                    .map(|p| Arc::new(p) as Arc<dyn MembershipPolicy>)
                    .unwrap_or_else(|err| fatal(&format!("failed to load multisig policy: {err}")))
            }
            GovernanceDescriptor::Stake {
                state_path,
                min_stake,
                slash_pct,
            } => StakePolicy::load(Path::new(&state_path), min_stake, slash_pct)
                .map(|p| Arc::new(p) as Arc<dyn MembershipPolicy>)
                .unwrap_or_else(|err| fatal(&format!("failed to load stake policy: {err}"))),
        }
    } else if let Some(path) = allowlist_spec {
        StaticPolicy::from_allowlist(Path::new(path))
            .map(|p| Arc::new(p) as Arc<dyn MembershipPolicy>)
            .unwrap_or_else(|err| fatal(&format!("failed to load allowlist policy: {err}")))
    } else {
        Arc::new(StaticPolicy::allow_all())
    }
}

#[cfg(feature = "net")]
#[derive(Debug, Deserialize, Clone)]
struct BlobPolicyFile {
    #[serde(default)]
    namespaces: HashMap<String, NamespaceRule>,
}

#[cfg(feature = "net")]
fn load_blob_policies(path: &Path) -> HashMap<String, NamespaceRule> {
    let contents = fs::read_to_string(path).unwrap_or_else(|err| {
        fatal(&format!(
            "failed to read blob policy {}: {err}",
            path.display()
        ))
    });
    let file: BlobPolicyFile = serde_json::from_str(&contents)
        .unwrap_or_else(|err| fatal(&format!("invalid blob policy {}: {err}", path.display())));
    file.namespaces
}
