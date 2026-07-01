//! Minimal CLI for interacting with the JULIAN Protocol primitives.
//!
//! This binary exposes helper commands for replaying transcript logs,
//! deriving ledger anchors, and reconciling anchors with a quorum using the
//! crate's domain-separated hashing and signature utilities.

#[cfg(feature = "net")]
use power_house::commands::{
    migration_apply_claims::{run_apply_claims, ApplyClaimsOptions},
    migration_burn_executor::{run_execute_burn_intents, ExecuteBurnOptions},
    migration_claims::{run_build_claims, BuildClaimsOptions},
    migration_finalize::{run_finalize_migration, FinalizeMigrationOptions},
    migration_proposal::{run_propose_migration, ProposeMigrationOptions},
    migration_verify_state::{run_verify_state, VerifyStateOptions},
    stake_snapshot::run_snapshot,
};
#[cfg(feature = "net")]
use power_house::net::{
    decode_public_key_base64, encrypt_identity_base64, load_encrypted_identity,
    load_or_derive_keypair, refresh_migration_mode_from_env, run_network, verify_signature_base64,
    AnchorEnvelope, AnchorJson, Ed25519KeySource, MembershipPolicy, MultisigPolicy, NamespaceRule,
    NetConfig, ObserverRegistration, ObserverRegistry, StakePolicy, StakeRegistry, StaticPolicy,
    ValidatorRegistration, ValidatorRegistry, OBSERVER_REGISTRY_SCHEMA, VALIDATOR_REGISTRY_SCHEMA,
};
use power_house::provenance::{ExternalProofAttachment, PhaArtifact, Rootprint};
#[cfg(feature = "sfcs")]
use power_house::{
    compile_llvm_ir_source, compile_public_rust_source, compile_wasm_stack_source,
    verify_sfcs_execution_embedding, verify_sfcs_pha_embedding,
    verify_sfcs_vm_constraint_embedding, verify_sfcs_vm_execution_embedding, SfcsCompilerError,
    SfcsError, SfcsGraph, SfcsVmConstraintError, SfcsVmConstraintProof, SfcsVmError, SfcsVmInputs,
    SfcsVmProgram,
};
#[cfg(feature = "sfcs-zk")]
use power_house::{
    compile_private_add_source, verify_sfcs_zk_private_add_embedding,
    verify_sfcs_zk_private_vm_embedding, SfcsZkError, SfcsZkPrivateAddProof,
    SfcsZkPrivateAddWitness, SfcsZkPrivateVmProof, SfcsZkPrivateVmWitness,
};
use power_house::{
    compute_fold_digest, identity::Identity, julian_genesis_anchor, parse_log_file,
    read_fold_digest_hint, reconcile_anchors_with_quorum, AnchorMetadata, AnchorVote,
    ChallengeSuite, EntryAnchor, Field, GeneralSumProof, LedgerAnchor, MemoryCapsule,
    MemoryCapsuleBuilder, MemoryError, MemoryVerificationPolicy, ObservatorySidecar, ProofStats,
};
#[cfg(feature = "sfcs")]
use std::collections::BTreeMap;
#[cfg(feature = "net")]
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener, TcpStream, UdpSocket};
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
use std::io::Write;

#[cfg(feature = "net")]
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
#[cfg(feature = "net")]
use libp2p::Multiaddr;
#[cfg(feature = "net")]
use rand::RngCore;
#[cfg(feature = "net")]
use rpassword::prompt_password;
#[cfg(feature = "net")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "net")]
use std::collections::{HashMap, HashSet};

const NETWORK_ID: &str = "MFENX-POWERHOUSE";
#[cfg(feature = "net")]
const DEFAULT_OBSERVER_BOOTSTRAPS: &[&str] = &[
    "/ip4/159.203.109.128/tcp/7002/p2p/12D3KooWMCyR9gXPXCGAMNCVJDKbisohRRq8oaTHNiR91HZ67cSR",
    "/ip4/64.23.182.213/tcp/7002/p2p/12D3KooWGEHbPAQ9ZVB9Uqg1j8CnsNqKvS2xmAe5cmT4w3idUtmQ",
    "/ip4/164.92.150.22/tcp/7002/p2p/12D3KooWFNv4sZfDKypMeWqRetghHxXzkhPTc4PvynDZKSETJqd8",
];

fn fatal(message: &str) -> ! {
    eprintln!("{message}");
    std::process::exit(1);
}

fn fatal_code(code: i32, message: &str) -> ! {
    eprintln!("{message}");
    std::process::exit(code);
}

fn print_cli_help() {
    println!("Power-House JULIAN {}", env!("CARGO_PKG_VERSION"));
    println!("Usage: julian <command> [options]");
    println!();
    println!("Core commands:");
    println!("  identity         Create, branch, merge, replay, and verify identities");
    println!("  rootprint        Navigate, fork, merge, and verify Power House provenance");
    println!("  memory           Create, verify, replay, challenge, and export proof memory");
    #[cfg(feature = "sfcs")]
    println!("  sfcs             Parse, execute, and verify SFCS computational fractals");
    println!("  node             Replay logs, derive anchors, and verify Merkle proofs");
    println!("  scale_sumcheck   Benchmark streaming sum-check verification");
    println!();
    println!("Optional external integration:");
    println!("  attach-external-proof  Attach non-core proof data to a .pha artifact");
    println!("  observatory      Verify non-core semantic sidecars against Rootprint");
    #[cfg(feature = "net")]
    {
        println!();
        println!("Network commands:");
        println!("  net, network     Start and inspect the peer-to-peer network");
        println!("  stake            Manage the stake registry and migration claims");
        println!("  governance       Build governance proposals");
        println!("  migration        Finalize and verify migrations");
        println!("  rollup           Settle rollup requests");
        println!("  keygen           Create an encrypted network identity");
        println!("  key-info         Inspect a network identity without exposing its secret");
        println!("  observer         Diagnose, set up, register, and package public observers");
        println!("  validator-registry  Sign, assemble, and verify validator registrations");
        println!("  observer-registry   Sign, assemble, and verify public observer registrations");
    }
    println!();
    println!("Use 'julian <command> --help' for command details.");
}

fn print_node_help() {
    println!("Usage: julian node <run|anchor|reconcile|prove|verify-proof> ...");
    println!("  run <node_id> <log_dir> <output_anchor>");
    println!("  anchor <log_dir>");
    println!("  reconcile <log_dir> <peer_anchor> <quorum>");
    println!("  prove <log_dir> <entry_index> <leaf_index> [output.json]");
    println!("  verify-proof <anchor_file> <proof_file>");
}

fn print_scale_help() {
    println!("Usage: julian scale_sumcheck [--vars <N>]");
    println!("  Runs deterministic streaming sum-check benchmarks through N variables.");
}

fn print_rootprint_help() {
    println!("Usage: julian rootprint <init|navigate|fork|merge|verify|equivalent> ...");
    println!("  init <artifact.pha> --label <name> --output <rootprint.json>");
    println!("  navigate <rootprint.json> <branch-selector> [--artifact-output <file>]");
    println!("  fork <rootprint.json> <parent> <artifact.pha> --label <name> [--output <file>]");
    println!(
        "  merge <rootprint.json> <left> <right> <artifact.pha> --label <name> [--output <file>]"
    );
    println!("  verify <rootprint.json>");
    println!("  equivalent <rootprint.json> <left> <right>");
    println!();
    println!("Rootprint commands verify Power House core data only.");
}

fn print_identity_help() {
    println!("Usage: julian identity <create|fork|merge|verify|replay|equivalent> ...");
    println!("  create <artifact.pha> --label <name> --identity-output <identity.json> \\");
    println!("         --rootprint-output <rootprint.json> [--artifact-output <bound.pha>]");
    println!("  fork <identity.json> <rootprint.json> <artifact.pha> --label <name> \\");
    println!("       --identity-output <identity.json> [--rootprint-output <rootprint.json>]");
    println!("  merge <left.identity.json> <right.identity.json> <rootprint.json> \\");
    println!("        <artifact.pha> --label <name> --identity-output <identity.json> \\");
    println!("        [--rootprint-output <rootprint.json>]");
    println!("  verify <identity.json> <rootprint.json>");
    println!("  replay <identity.json> <rootprint.json> [--output <state.json>]");
    println!("  equivalent <left.identity.json> <right.identity.json> <rootprint.json>");
    println!();
    println!("Identity verification is deterministic and requires no network access.");
}

fn print_attach_external_proof_help() {
    println!("Usage: julian attach-external-proof <artifact.pha> [options]");
    println!("  --id <id>                 Attachment identifier");
    println!("  --proof-system <name>     External proof system identifier");
    println!("  --payload <json-file>     Opaque external proof payload");
    println!("  --verifier-hint <value>   Optional verifier hint");
    println!("  --metadata <json-file>    Optional non-core metadata");
    println!("  --output <artifact.pha>   Output path; defaults to the input artifact");
    println!();
    println!("This optional command never changes the Power House core fingerprint.");
}

fn print_observatory_help() {
    println!("Usage: julian observatory <verify> ...");
    println!("  verify <rootprint.json> <observatory-sidecar.json>");
    println!();
    println!("Rootprint core verification completes before the optional sidecar is checked.");
    println!("Semantic packets never alter Power House proof identity or validity.");
}

fn print_memory_help() {
    println!(
        "Usage: julian memory <create|verify|replay|challenge|inspect|explain-boundary|export> ..."
    );
    println!("  create --pha <main.pha> --rootprint <proof.rootprint.json> \\");
    println!("         [--sidecar <proof.observatory.json>] [--output <capsule.phm>]");
    println!("  verify <capsule.phm> [--policy strict|inspect] [--report <report.json>]");
    println!("  replay <capsule.phm> [--report <replay.json>]");
    println!("  challenge <capsule.phm> --all [--report <challenge.json>]");
    println!("  inspect <capsule.phm> [--summary]");
    println!("  explain-boundary <capsule.phm>");
    println!("  export <capsule.phm> --format directory --output <dir>");
    println!();
    println!("Memory verification runs offline and verifies core truth before semantic bindings.");
}

#[cfg(feature = "sfcs")]
fn print_sfcs_help() {
    println!("Usage: julian sfcs <source|eval|inspect|verify-pha|vm-run|verify-vm-pha> ...");
    println!("  source <source.sfcs> --output <graph.json>");
    println!("  rust-public <source.rs> --graph-output <graph.json> \\");
    println!("       [--semantic-output <packet.json>] [--artifact-output <graph.pha>] \\");
    println!("       [--report <report.json>] [--label <name>]");
    println!("  llvm-ir <source.ll> --graph-output <graph.json> \\");
    println!("       [--semantic-output <packet.json>] [--artifact-output <graph.pha>] \\");
    println!("       [--report <report.json>] [--label <name>]");
    println!("  wasm-stack <source.wasmstack> --graph-output <graph.json> \\");
    println!("       [--semantic-output <packet.json>] [--artifact-output <graph.pha>] \\");
    println!("       [--report <report.json>] [--label <name>]");
    println!("  eval <source.sfcs> --input <name=value> [--input <name=value>] \\");
    println!("       [--report <report.json>] [--graph-output <graph.json>] \\");
    println!("       [--artifact-output <exec.pha>] [--label <name>]");
    println!("  inspect <graph.json>");
    println!("  verify-pha <artifact.pha>");
    println!("  vm-run <program.json> --inputs <inputs.json> \\");
    println!("       [--report <report.json>] [--artifact-output <vm.pha>] [--label <name>]");
    println!("  verify-vm-pha <artifact.pha>");
    println!("  vm-constraints <program.json> --inputs <inputs.json> \\");
    println!(
        "       [--report <report.json>] [--artifact-output <constraints.pha>] [--label <name>]"
    );
    println!("  verify-vm-constraints-pha <artifact.pha>");
    #[cfg(feature = "sfcs-zk")]
    {
        println!("  rust-private-add <source.rs> --lhs-value <u32> --rhs-value <u32> \\");
        println!("       --lhs-blinding <64-hex> --rhs-blinding <64-hex> \\");
        println!(
            "       [--artifact-output <zk.pha>] [--rootprint-output <proof.rootprint.json>] \\"
        );
        println!(
            "       [--sidecar-output <proof.observatory.json>] [--capsule-output <proof.phm>] \\"
        );
        println!("       [--report <report.json>] [--label <name>]");
        println!("  zk-private-add <program.json> --lhs-register <N> --rhs-register <N> \\");
        println!("       --output-register <N> --lhs-value <u32> --rhs-value <u32> \\");
        println!("       --lhs-blinding <64-hex> --rhs-blinding <64-hex> \\");
        println!("       [--report <report.json>] [--artifact-output <zk.pha>] [--label <name>]");
        println!("  zk-private-vm <program.json> --witness <witness.json> \\");
        println!("       [--report <report.json>] [--artifact-output <zk.pha>] [--label <name>]");
        println!("  verify-zk-pha <artifact.pha>");
    }
    println!();
    println!("SFCS commands are offline and do not alter Rootprint or .pha identity rules.");
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
fn print_migration_help() {
    println!("Usage: julian migration <finalize|verify-state|execute-burn-intents> ...");
    println!("  finalize --registry <file> --height <N> --log-dir <dir> --output-dir <dir>");
    println!(
        "           [--token-contract <id>] [--conversion-ratio <u64>] [--treasury-mint <u64>]"
    );
    println!("           [--amount-source stake|balance|total] [--include-slashed]");
    println!("           [--claim-id-salt <text>] [--node-id <id>] [--quorum <N>]");
    println!("           [--apply-state <file>] [--allow-unfrozen] [--force]");
    println!("  verify-state --registry <file> --claims <file> --state <file>");
    println!("               [--require-complete] [--skip-balance-floor]");
    println!(
        "  execute-burn-intents --registry <file> [--outbox <file>] [--state <file>] [--dry-run]"
    );
}

#[cfg(feature = "net")]
fn print_net_help() {
    println!("Usage: julian net <start|anchor|verify-envelope> ...");
    println!("  start --node-id <id> --log-dir <dir> --listen <multiaddr> [flags]");
    println!("        [--evm-rpc-listen <host:port>] [--evm-chain-id <u64>]");
    println!("  anchor --log-dir <dir> [--node-id <id>] [--quorum <N>]");
    println!("         (compat: julian net anchor <log_dir>)");
    println!("  verify-envelope --file <anchor.json> --log-dir <dir> [--quorum <N>]");
}

#[cfg(feature = "net")]
fn print_net_start_help() {
    println!("Usage: julian net start --node-id <id> --log-dir <dir> --listen <multiaddr> [flags]");
    println!();
    println!("Identity and peers:");
    println!("  --key <spec>                     Seed, file, or key specification");
    println!("  --identity <file>                Encrypted identity file");
    println!("  --bootstrap <multiaddr>          Bootstrap peer; repeatable");
    println!("  --bootnodes <csv>                Comma-separated bootstrap peers");
    println!();
    println!("Consensus and gossip:");
    println!("  --quorum <N>                     Anchor finality quorum");
    println!("  --attestation-quorum <N>         Blob attestation quorum");
    println!("  --broadcast-interval <ms>        Anchor broadcast interval");
    println!("  --checkpoint-interval <N>        Checkpoint interval");
    println!("  --anchor-topic <topic>           Explicit anchor gossip topic");
    println!("  --gossip-shard <name>            Select a derived shard topic");
    println!("  --gossip-bridge-topics <csv>     Additional bridge topics");
    println!("  --bft                            Enable BFT finality rounds");
    println!("  --bft-round-ms <ms>              BFT round duration");
    println!();
    println!("Policy, storage, and runtime:");
    println!("  --policy <file>                  Membership policy");
    println!("  --policy-allowlist <file>        Static peer allowlist");
    println!("  --metrics <host:port>            Prometheus listener");
    println!("  --blob-dir <dir>                 Blob data directory");
    println!("  --blob-listen <host:port>        Blob HTTP listener");
    println!("  --blob-policy <file>             Namespace policy file");
    println!("  --blob-auth-token <token>        Blob API bearer/API key");
    println!("  --max-blob-bytes <N>             Maximum submitted blob size");
    println!("  --blob-retention-days <N>        Blob retention period");
    println!("  --blob-max-concurrency <N>       Concurrent blob requests");
    println!("  --blob-request-timeout-ms <ms>   Blob request timeout");
    println!("  --tokio-threads <N>              Async worker thread count");
    println!("  --token-mode <native|TOKEN_ID>   Settlement token mode");
    println!("  --token-oracle <RPC_URL>         Token oracle endpoint");
    println!("  --evm-chain-id <u64>             Enable native-chain finality");
    println!("  --evm-rpc-listen <host:port>     Serve finalized wallet JSON-RPC");
}

#[cfg(feature = "net")]
fn print_rollup_help() {
    println!("Usage: julian rollup <settle|settle-file> ...");
    println!("  settle <registry.json> <namespace> <share_root> <payer_b64> <fee> [options]");
    println!("  settle-file <registry.json> <request.json> [--outbox <path>]");
}

#[cfg(feature = "net")]
fn print_keygen_help() {
    println!("Usage: julian keygen [key-spec] [--out <identity-file>]");
    println!("  Creates an encrypted Ed25519 identity and prints its public key.");
}

#[cfg(feature = "net")]
fn print_key_info_help() {
    println!("Usage: julian key-info <key-spec> [--json]");
    println!("  Prints the Ed25519 public key and libp2p peer ID for a key source.");
}

#[cfg(feature = "net")]
fn print_observer_help() {
    println!("Usage: julian observer <doctor|setup|register|submit|status> ...");
    println!("  Diagnose, set up, register, and submit public observers.");
    println!("  doctor   Diagnose local identity, ports, metrics, NAT, and external reachability");
    println!("  setup    Create a local key if needed, print the node command, and write registration JSON");
    println!("  register Write a signed observer registration using safe observer defaults");
    println!("  submit   Validate and upload a signed registration for admission");
    println!("  status   Read live admission status using a tracking ID");
    println!();
    println!("Common options:");
    println!("  --node-id <id>             Observer node id (default: mynode)");
    println!(
        "  --key <key>                Node key path/spec (default: $HOME/.powerhouse/node.key)"
    );
    println!("  --operator <name>          Operator name (default: node id)");
    println!("  --region <id>              Region label (default: self-hosted)");
    println!("  --public-host <host>       Public IPv4/DNS; auto-detected when possible");
    println!("  --p2p-port <port>          Public p2p port (default: 7001)");
    println!("  --metrics-port <port>      Public metrics port (default: 9102)");
    println!("  --output <file>            Registration/submission output path");
    println!("  --probe-url <url>          External probe endpoint (default: https://rpc.mfenx.com/observer-probe)");
    println!("  --intake-url <url>         Admission endpoint (default: https://rpc.mfenx.com/observer-registrations)");
    println!("  --no-probe                 Skip the production-side external probe");
    println!("  --no-upload                Validate/package without submitting");
    println!("  --json                     Machine-readable doctor/submit output");
    println!();
    println!("Observer setup includes the default public observer bootnodes on TCP 7002.");
}

#[cfg(feature = "net")]
fn print_validator_registry_help() {
    println!("Usage: julian validator-registry <register|create|assemble|verify> ...");
    println!("  register --node-id <id> --public-host <host>");
    println!("           [--key <key>] [--operator <name>] [--region <id>]");
    println!("           [--p2p-port <port>] [--metrics-port <port>]");
    println!("           [--p2p-address <multiaddr>] [--metrics-url <url>]");
    println!("           [--system-metrics-url <url>] [--policy <allowlist.json>]");
    println!("           [--registry <existing.json>] [--registry-output <registry.json>]");
    println!("           [--chain-id <id>] [--output <file>]");
    println!("  create --key <key> --node-id <id> --operator <name> --region <id>");
    println!("         --p2p-address <multiaddr> --metrics-url <url>");
    println!("         [--system-metrics-url <url>] [--chain-id <id>]");
    println!("         [--issued-at <unix>] [--valid-until <unix>] --output <file>");
    println!("  assemble --registration <file>... --policy <allowlist.json>");
    println!("           [--chain-id <id>] --output <registry.json>");
    println!("  verify <registry.json> --policy <allowlist.json> [--now <unix>] [--json]");
    println!();
    println!("Registrations are signed by the validator identity and do not alter consensus.");
}

#[cfg(feature = "net")]
fn print_observer_registry_help() {
    println!("Usage: julian observer-registry <register|create|assemble|verify> ...");
    println!("  register --node-id <id> --public-host <host>");
    println!("           [--key <key>] [--operator <name>] [--region <id>]");
    println!("           [--p2p-port <port>] [--metrics-port <port>]");
    println!("           [--p2p-address <multiaddr>] [--metrics-url <url>]");
    println!("           [--system-metrics-url <url>] [--registry <existing.json>]");
    println!("           [--registry-output <registry.json>] [--chain-id <id>] [--output <file>]");
    println!("  create --key <key> --node-id <id> --operator <name> --region <id>");
    println!("         --p2p-address <multiaddr> --metrics-url <url>");
    println!("         [--system-metrics-url <url>] [--chain-id <id>]");
    println!("         [--issued-at <unix>] [--valid-until <unix>] --output <file>");
    println!("  assemble --registration <file>... [--chain-id <id>] --output <registry.json>");
    println!("  verify <registry.json> [--now <unix>] [--json]");
    println!();
    println!("Observer records are signed identity declarations and never count as validators.");
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
        None | Some("-h") | Some("--help") | Some("help") => print_cli_help(),
        Some("-V") | Some("--version") | Some("version") => {
            println!("julian {}", env!("CARGO_PKG_VERSION"));
        }
        Some("node") => {
            if let Some(sub) = args.next() {
                handle_node(&sub, args.collect());
            } else {
                print_node_help();
            }
        }
        Some("scale_sumcheck") => {
            cmd_scale_sumcheck(args.collect());
        }
        Some("rootprint") => {
            if let Some(sub) = args.next() {
                handle_rootprint(&sub, args.collect());
            } else {
                print_rootprint_help();
            }
        }
        Some("memory") => {
            if let Some(sub) = args.next() {
                handle_memory(&sub, args.collect());
            } else {
                print_memory_help();
            }
        }
        #[cfg(feature = "sfcs")]
        Some("sfcs") => {
            if let Some(sub) = args.next() {
                handle_sfcs(&sub, args.collect());
            } else {
                print_sfcs_help();
            }
        }
        Some("identity") => {
            if let Some(sub) = args.next() {
                handle_identity(&sub, args.collect());
            } else {
                print_identity_help();
            }
        }
        Some("attach-external-proof") => {
            cmd_attach_external_proof(args.collect());
        }
        Some("observatory") => {
            if let Some(sub) = args.next() {
                handle_observatory(&sub, args.collect());
            } else {
                print_observatory_help();
            }
        }
        #[cfg(feature = "net")]
        Some("keygen") => {
            cmd_keygen(args.collect());
        }
        #[cfg(feature = "net")]
        Some("key-info") => {
            cmd_key_info(args.collect());
        }
        #[cfg(feature = "net")]
        Some("observer") => {
            if let Some(sub) = args.next() {
                handle_observer(&sub, args.collect());
            } else {
                print_observer_help();
            }
        }
        #[cfg(feature = "net")]
        Some("validator-registry") => {
            if let Some(sub) = args.next() {
                handle_validator_registry(&sub, args.collect());
            } else {
                print_validator_registry_help();
            }
        }
        #[cfg(feature = "net")]
        Some("observer-registry") => {
            if let Some(sub) = args.next() {
                handle_observer_registry(&sub, args.collect());
            } else {
                print_observer_registry_help();
            }
        }
        #[cfg(feature = "net")]
        Some("net") | Some("network") => {
            if let Some(sub) = args.next() {
                handle_net(&sub, args.collect());
            } else {
                print_net_help();
            }
        }
        #[cfg(feature = "net")]
        Some("stake") => {
            if let Some(sub) = args.next() {
                handle_stake(&sub, args.collect());
            } else {
                print_stake_help();
            }
        }
        #[cfg(feature = "net")]
        Some("governance") => {
            if let Some(sub) = args.next() {
                handle_governance(&sub, args.collect());
            } else {
                print_governance_help();
            }
        }
        #[cfg(feature = "net")]
        Some("migration") => {
            if let Some(sub) = args.next() {
                handle_migration(&sub, args.collect());
            } else {
                print_migration_help();
            }
        }
        #[cfg(feature = "net")]
        Some("rollup") => {
            if let Some(sub) = args.next() {
                handle_rollup(&sub, args.collect());
            } else {
                print_rollup_help();
            }
        }
        _ => {
            eprintln!("Unknown command: {}", command.unwrap_or_default());
            eprintln!("Run 'julian --help' for usage.");
            std::process::exit(1);
        }
    }
}

fn handle_rootprint(sub: &str, tail: Vec<String>) {
    match sub {
        "-h" | "--help" => print_rootprint_help(),
        "init" => cmd_rootprint_init(tail),
        "navigate" => cmd_rootprint_navigate(tail),
        "fork" => cmd_rootprint_fork(tail),
        "merge" => cmd_rootprint_merge(tail),
        "verify" => cmd_rootprint_verify(tail),
        "equivalent" => cmd_rootprint_equivalent(tail),
        _ => fatal(&format!("unknown rootprint subcommand: {sub}")),
    }
}

fn handle_identity(sub: &str, tail: Vec<String>) {
    match sub {
        "-h" | "--help" => print_identity_help(),
        "create" => cmd_identity_create(tail),
        "fork" => cmd_identity_fork(tail),
        "merge" => cmd_identity_merge(tail),
        "verify" => cmd_identity_verify(tail),
        "replay" => cmd_identity_replay(tail),
        "equivalent" => cmd_identity_equivalent(tail),
        _ => fatal(&format!("unknown identity subcommand: {sub}")),
    }
}

fn handle_observatory(sub: &str, tail: Vec<String>) {
    match sub {
        "-h" | "--help" => print_observatory_help(),
        "verify" => cmd_observatory_verify(tail),
        _ => fatal(&format!("unknown observatory subcommand: {sub}")),
    }
}

fn handle_memory(sub: &str, tail: Vec<String>) {
    match sub {
        "-h" | "--help" => print_memory_help(),
        "create" => cmd_memory_create(tail),
        "verify" => cmd_memory_verify(tail),
        "replay" => cmd_memory_replay(tail),
        "challenge" => cmd_memory_challenge(tail),
        "inspect" => cmd_memory_inspect(tail),
        "explain-boundary" => cmd_memory_explain_boundary(tail),
        "export" => cmd_memory_export(tail),
        _ => fatal(&format!("unknown memory subcommand: {sub}")),
    }
}

#[cfg(feature = "sfcs")]
fn handle_sfcs(sub: &str, tail: Vec<String>) {
    match sub {
        "-h" | "--help" => print_sfcs_help(),
        "source" => cmd_sfcs_source(tail),
        "rust-public" => cmd_sfcs_rust_public(tail),
        "llvm-ir" => cmd_sfcs_llvm_ir(tail),
        "wasm-stack" => cmd_sfcs_wasm_stack(tail),
        "eval" => cmd_sfcs_eval(tail),
        "inspect" => cmd_sfcs_inspect(tail),
        "verify-pha" => cmd_sfcs_verify_pha(tail),
        "vm-run" => cmd_sfcs_vm_run(tail),
        "verify-vm-pha" => cmd_sfcs_verify_vm_pha(tail),
        "vm-constraints" => cmd_sfcs_vm_constraints(tail),
        "verify-vm-constraints-pha" => cmd_sfcs_verify_vm_constraints_pha(tail),
        #[cfg(feature = "sfcs-zk")]
        "rust-private-add" => cmd_sfcs_rust_private_add(tail),
        #[cfg(feature = "sfcs-zk")]
        "zk-private-add" => cmd_sfcs_zk_private_add(tail),
        #[cfg(feature = "sfcs-zk")]
        "zk-private-vm" => cmd_sfcs_zk_private_vm(tail),
        #[cfg(feature = "sfcs-zk")]
        "verify-zk-pha" => cmd_sfcs_verify_zk_pha(tail),
        _ => fatal(&format!("unknown sfcs subcommand: {sub}")),
    }
}

fn read_pha(path: &Path) -> PhaArtifact {
    let contents = fs::read_to_string(path)
        .unwrap_or_else(|err| fatal(&format!("failed to read {}: {err}", path.display())));
    serde_json::from_str(&contents)
        .unwrap_or_else(|err| fatal(&format!("invalid .pha JSON in {}: {err}", path.display())))
}

fn read_rootprint(path: &Path) -> Rootprint {
    let contents = fs::read_to_string(path)
        .unwrap_or_else(|err| fatal(&format!("failed to read {}: {err}", path.display())));
    serde_json::from_str(&contents).unwrap_or_else(|err| {
        fatal(&format!(
            "invalid Rootprint JSON in {}: {err}",
            path.display()
        ))
    })
}

fn read_identity(path: &Path) -> Identity {
    let contents = fs::read_to_string(path)
        .unwrap_or_else(|err| fatal(&format!("failed to read {}: {err}", path.display())));
    serde_json::from_str(&contents).unwrap_or_else(|err| {
        fatal(&format!(
            "invalid identity JSON in {}: {err}",
            path.display()
        ))
    })
}

fn read_observatory_sidecar(path: &Path) -> ObservatorySidecar {
    let contents = fs::read_to_string(path)
        .unwrap_or_else(|err| fatal(&format!("failed to read {}: {err}", path.display())));
    serde_json::from_str(&contents).unwrap_or_else(|err| {
        fatal(&format!(
            "invalid Observatory sidecar JSON in {}: {err}",
            path.display()
        ))
    })
}

fn read_memory_capsule(path: &Path, policy: &MemoryVerificationPolicy) -> MemoryCapsule {
    MemoryCapsule::from_path(path, policy).unwrap_or_else(|err| {
        fatal(&format!(
            "invalid Memory Capsule in {}: {err}",
            path.display()
        ))
    })
}

fn read_json_value(path: &Path) -> serde_json::Value {
    let contents = fs::read_to_string(path)
        .unwrap_or_else(|err| fatal(&format!("failed to read {}: {err}", path.display())));
    serde_json::from_str(&contents)
        .unwrap_or_else(|err| fatal(&format!("invalid JSON in {}: {err}", path.display())))
}

#[cfg(feature = "sfcs")]
fn read_sfcs_graph(path: &Path) -> SfcsGraph {
    let contents = fs::read(path)
        .unwrap_or_else(|err| fatal(&format!("failed to read {}: {err}", path.display())));
    SfcsGraph::from_slice(&contents)
        .unwrap_or_else(|err| fatal(&format!("invalid SFCS graph in {}: {err}", path.display())))
}

#[cfg(feature = "sfcs")]
fn read_sfcs_source(path: &Path) -> SfcsGraph {
    let contents = fs::read_to_string(path)
        .unwrap_or_else(|err| fatal(&format!("failed to read {}: {err}", path.display())));
    SfcsGraph::from_source(&contents).unwrap_or_else(|err| {
        fatal(&format!(
            "failed to parse SFCS source {}: {err}",
            path.display()
        ))
    })
}

#[cfg(feature = "sfcs")]
fn read_sfcs_vm_program(path: &Path) -> SfcsVmProgram {
    let contents = fs::read_to_string(path)
        .unwrap_or_else(|err| fatal(&format!("failed to read {}: {err}", path.display())));
    let program: SfcsVmProgram = serde_json::from_str(&contents).unwrap_or_else(|err| {
        fatal(&format!(
            "invalid SFCS VM program JSON in {}: {err}",
            path.display()
        ))
    });
    program
        .verify()
        .unwrap_or_else(|err| fatal(&format!("invalid SFCS VM program: {err}")));
    program
}

#[cfg(feature = "sfcs")]
fn read_sfcs_vm_inputs(path: &Path) -> SfcsVmInputs {
    let contents = fs::read_to_string(path)
        .unwrap_or_else(|err| fatal(&format!("failed to read {}: {err}", path.display())));
    let inputs: SfcsVmInputs = serde_json::from_str(&contents).unwrap_or_else(|err| {
        fatal(&format!(
            "invalid SFCS VM inputs JSON in {}: {err}",
            path.display()
        ))
    });
    inputs
        .verify()
        .unwrap_or_else(|err| fatal(&format!("invalid SFCS VM inputs: {err}")));
    inputs
}

#[cfg(feature = "sfcs-zk")]
fn read_sfcs_private_vm_witness(path: &Path) -> SfcsZkPrivateVmWitness {
    let contents = fs::read_to_string(path)
        .unwrap_or_else(|err| fatal(&format!("failed to read {}: {err}", path.display())));
    let value: serde_json::Value = serde_json::from_str(&contents).unwrap_or_else(|err| {
        fatal(&format!(
            "invalid SFCS private VM witness JSON in {}: {err}",
            path.display()
        ))
    });
    let inputs_value = value
        .get("inputs")
        .cloned()
        .unwrap_or_else(|| fatal("private VM witness requires `inputs`"));
    let inputs: SfcsVmInputs = serde_json::from_value(inputs_value)
        .unwrap_or_else(|err| fatal(&format!("invalid private VM witness inputs: {err}")));
    inputs
        .verify()
        .unwrap_or_else(|err| fatal(&format!("invalid private VM witness inputs: {err}")));
    let blinding_hex = value
        .get("blinding_seed_hex")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_else(|| fatal("private VM witness requires `blinding_seed_hex`"));
    SfcsZkPrivateVmWitness {
        inputs,
        blinding_seed: parse_seed_hex(blinding_hex, "blinding_seed_hex"),
    }
}

fn write_json<T: serde::Serialize>(path: &Path, value: &T) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap_or_else(|err| {
            fatal(&format!(
                "failed to create output directory {}: {err}",
                parent.display()
            ))
        });
    }
    let mut bytes = serde_json::to_vec_pretty(value)
        .unwrap_or_else(|err| fatal(&format!("failed to encode JSON: {err}")));
    bytes.push(b'\n');
    fs::write(path, bytes)
        .unwrap_or_else(|err| fatal(&format!("failed to write {}: {err}", path.display())));
}

fn take_option(iter: &mut impl Iterator<Item = String>, name: &str) -> String {
    iter.next()
        .unwrap_or_else(|| fatal(&format!("{name} expects a value")))
}

#[cfg(feature = "sfcs")]
fn parse_sfcs_input(value: &str) -> (String, i64) {
    let Some((name, raw_value)) = value.split_once('=') else {
        fatal("--input expects name=value");
    };
    if name.is_empty() {
        fatal("--input name cannot be empty");
    }
    let parsed = raw_value
        .parse::<i64>()
        .unwrap_or_else(|err| fatal(&format!("invalid SFCS input value {raw_value}: {err}")));
    (name.to_string(), parsed)
}

#[cfg(feature = "sfcs")]
fn sfcs_exit_for_error(error: &SfcsError) -> i32 {
    match error {
        SfcsError::Canonical(_) | SfcsError::InvalidProgram(_) => 2,
        SfcsError::UnsupportedSchema(_) => 3,
        SfcsError::InvalidEmbedding(_)
        | SfcsError::InvalidDigest(_)
        | SfcsError::InvalidGraph(_)
        | SfcsError::CycleDetected(_)
        | SfcsError::UnknownNode(_)
        | SfcsError::MissingInput(_)
        | SfcsError::UnsupportedEvaluation(_)
        | SfcsError::Execution(_)
        | SfcsError::DuplicateNode(_)
        | SfcsError::InvalidId(_) => 1,
        SfcsError::Json(_) | SfcsError::Pha(_) => 5,
    }
}

#[cfg(feature = "sfcs")]
fn sfcs_vm_exit_for_error(error: &SfcsVmError) -> i32 {
    match error {
        SfcsVmError::InvalidProgram(_) | SfcsVmError::InvalidInput(_) => 2,
        SfcsVmError::UnsupportedSchema(_) => 3,
        SfcsVmError::InvalidDigest(_)
        | SfcsVmError::InvalidEmbedding(_)
        | SfcsVmError::Sfcs(_)
        | SfcsVmError::Execution(_) => 1,
        SfcsVmError::Json(_) | SfcsVmError::Pha(_) => 5,
    }
}

#[cfg(feature = "sfcs")]
fn sfcs_vm_constraint_exit_for_error(error: &SfcsVmConstraintError) -> i32 {
    match error {
        SfcsVmConstraintError::UnsupportedSchema(_) => 3,
        SfcsVmConstraintError::InvalidProof(_) | SfcsVmConstraintError::InvalidEmbedding(_) => 1,
        SfcsVmConstraintError::Vm(_) | SfcsVmConstraintError::Sfcs(_) => 1,
        SfcsVmConstraintError::Json(_) | SfcsVmConstraintError::Pha(_) => 5,
    }
}

#[cfg(feature = "sfcs-zk")]
fn sfcs_zk_exit_for_error(error: &SfcsZkError) -> i32 {
    match error {
        SfcsZkError::InvalidProgram(_) | SfcsZkError::InvalidWitness(_) => 2,
        SfcsZkError::UnsupportedSchema(_) => 3,
        SfcsZkError::InvalidProof(_) | SfcsZkError::InvalidEmbedding(_) => 1,
        SfcsZkError::Vm(_) | SfcsZkError::VmConstraint(_) | SfcsZkError::Sfcs(_) => 1,
        SfcsZkError::Serialization(_) | SfcsZkError::Json(_) | SfcsZkError::Pha(_) => 5,
    }
}

#[cfg(feature = "sfcs")]
fn sfcs_compiler_exit_for_error(error: &SfcsCompilerError) -> i32 {
    match error {
        SfcsCompilerError::InvalidSource(_) => 2,
        SfcsCompilerError::Sfcs(_) => 1,
        SfcsCompilerError::Vm(_) => 1,
        SfcsCompilerError::Memory(_) => 5,
    }
}

#[cfg(feature = "sfcs-zk")]
fn parse_register(value: &str, name: &str) -> u8 {
    let register = value
        .parse::<u8>()
        .unwrap_or_else(|err| fatal(&format!("{name} expects a register number: {err}")));
    if register > 31 {
        fatal(&format!("{name} must be in 0..=31"));
    }
    register
}

#[cfg(feature = "sfcs-zk")]
fn parse_u32_arg(value: &str, name: &str) -> u32 {
    value
        .parse::<u32>()
        .unwrap_or_else(|err| fatal(&format!("{name} expects a u32 value: {err}")))
}

#[cfg(feature = "sfcs-zk")]
fn parse_seed_hex(value: &str, name: &str) -> [u8; 32] {
    let bytes = hex::decode(value).unwrap_or_else(|err| fatal(&format!("{name}: bad hex: {err}")));
    if bytes.len() != 32 {
        fatal(&format!(
            "{name} expects exactly 32 bytes / 64 hex characters"
        ));
    }
    let mut seed = [0_u8; 32];
    seed.copy_from_slice(&bytes);
    seed
}

#[cfg(feature = "sfcs")]
fn cmd_sfcs_source(args: Vec<String>) {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_sfcs_help();
        return;
    }
    let mut source_path = None;
    let mut output = None;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--output" => output = Some(PathBuf::from(take_option(&mut iter, "--output"))),
            value if source_path.is_none() => source_path = Some(PathBuf::from(value)),
            other => fatal(&format!("unknown argument: {other}")),
        }
    }
    let source_path = source_path.unwrap_or_else(|| fatal("sfcs source requires <source.sfcs>"));
    let output = output.unwrap_or_else(|| fatal("--output is required"));
    let graph = read_sfcs_source(&source_path);
    let digest = graph
        .fractal_digest()
        .unwrap_or_else(|err| fatal(&format!("SFCS graph digest failed: {err}")));
    write_json(&output, &graph);
    println!("SFCS SOURCE");
    println!("graph_digest: {digest}");
    println!("nodes: {}", graph.nodes.len());
    println!("outputs: {}", graph.outputs.len());
    println!("graph: {}", output.display());
}

#[cfg(feature = "sfcs")]
fn cmd_sfcs_rust_public(args: Vec<String>) {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_sfcs_help();
        return;
    }
    let mut source_path = None;
    let mut graph_output = None;
    let mut semantic_output = None;
    let mut artifact_output = None;
    let mut report_path = None;
    let mut label = "sfcs-rust-public".to_string();
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--graph-output" => {
                graph_output = Some(PathBuf::from(take_option(&mut iter, "--graph-output")))
            }
            "--semantic-output" => {
                semantic_output = Some(PathBuf::from(take_option(&mut iter, "--semantic-output")))
            }
            "--artifact-output" => {
                artifact_output = Some(PathBuf::from(take_option(&mut iter, "--artifact-output")))
            }
            "--report" => report_path = Some(PathBuf::from(take_option(&mut iter, "--report"))),
            "--label" => label = take_option(&mut iter, "--label"),
            value if source_path.is_none() => source_path = Some(PathBuf::from(value)),
            other => fatal(&format!("unknown argument: {other}")),
        }
    }
    let source_path = source_path.unwrap_or_else(|| fatal("sfcs rust-public requires <source.rs>"));
    let graph_output = graph_output.unwrap_or_else(|| fatal("--graph-output is required"));
    let source = fs::read_to_string(&source_path)
        .unwrap_or_else(|err| fatal(&format!("failed to read {}: {err}", source_path.display())));
    let compiled = compile_public_rust_source(&source).unwrap_or_else(|err| {
        fatal_code(
            sfcs_compiler_exit_for_error(&err),
            &format!("SFCS public Rust compilation failed: {err}"),
        )
    });
    let graph_digest = compiled
        .graph_digest()
        .unwrap_or_else(|err| fatal(&format!("SFCS public Rust graph digest failed: {err}")));
    write_json(&graph_output, &compiled.graph);
    if let Some(path) = semantic_output {
        write_json(&path, &compiled.semantic_packet);
    }
    if let Some(path) = artifact_output {
        let artifact = compiled
            .graph
            .to_pha_artifact(&label)
            .unwrap_or_else(|err| fatal(&format!("SFCS public Rust .pha failed: {err}")));
        write_json(&path, &artifact);
    }
    let report = serde_json::json!({
        "schema": "power-house/sfcs-rust-public-cli-report/v1",
        "source": source_path.display().to_string(),
        "compiler_schema": compiled.schema,
        "source_digest": compiled.source_digest,
        "function_name": compiled.function_name,
        "parameters": compiled.parameters,
        "return_type": compiled.return_type,
        "graph_digest": graph_digest,
        "graph_nodes": compiled.graph.nodes.len(),
        "graph_outputs": compiled.graph.outputs,
        "semantic_packet_digest": compiled.semantic_packet["packet_digest"],
    });
    if let Some(path) = report_path {
        write_json(&path, &report);
    }
    println!("SFCS RUST PUBLIC");
    println!(
        "source_digest: {}",
        report["source_digest"].as_str().unwrap_or("")
    );
    println!("graph_digest: {graph_digest}");
    println!("nodes: {}", report["graph_nodes"].as_u64().unwrap_or(0));
    println!(
        "semantic_packet_digest: {}",
        report["semantic_packet_digest"].as_str().unwrap_or("")
    );
}

#[cfg(feature = "sfcs")]
fn cmd_sfcs_llvm_ir(args: Vec<String>) {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_sfcs_help();
        return;
    }
    let mut source_path = None;
    let mut graph_output = None;
    let mut semantic_output = None;
    let mut artifact_output = None;
    let mut report_path = None;
    let mut label = "sfcs-llvm-ir".to_string();
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--graph-output" => {
                graph_output = Some(PathBuf::from(take_option(&mut iter, "--graph-output")))
            }
            "--semantic-output" => {
                semantic_output = Some(PathBuf::from(take_option(&mut iter, "--semantic-output")))
            }
            "--artifact-output" => {
                artifact_output = Some(PathBuf::from(take_option(&mut iter, "--artifact-output")))
            }
            "--report" => report_path = Some(PathBuf::from(take_option(&mut iter, "--report"))),
            "--label" => label = take_option(&mut iter, "--label"),
            value if source_path.is_none() => source_path = Some(PathBuf::from(value)),
            other => fatal(&format!("unknown argument: {other}")),
        }
    }
    let source_path = source_path.unwrap_or_else(|| fatal("sfcs llvm-ir requires <source.ll>"));
    let graph_output = graph_output.unwrap_or_else(|| fatal("--graph-output is required"));
    let source = fs::read_to_string(&source_path)
        .unwrap_or_else(|err| fatal(&format!("failed to read {}: {err}", source_path.display())));
    let compiled = compile_llvm_ir_source(&source).unwrap_or_else(|err| {
        fatal_code(
            sfcs_compiler_exit_for_error(&err),
            &format!("SFCS LLVM IR compilation failed: {err}"),
        )
    });
    let graph_digest = compiled
        .graph_digest()
        .unwrap_or_else(|err| fatal(&format!("SFCS LLVM IR graph digest failed: {err}")));
    write_json(&graph_output, &compiled.graph);
    if let Some(path) = semantic_output {
        write_json(&path, &compiled.semantic_packet);
    }
    if let Some(path) = artifact_output {
        let artifact = compiled
            .graph
            .to_pha_artifact(&label)
            .unwrap_or_else(|err| fatal(&format!("SFCS LLVM IR .pha failed: {err}")));
        write_json(&path, &artifact);
    }
    let report = serde_json::json!({
        "schema": "power-house/sfcs-llvm-ir-cli-report/v1",
        "source": source_path.display().to_string(),
        "compiler_schema": compiled.schema,
        "source_digest": compiled.source_digest,
        "function_name": compiled.function_name,
        "parameters": compiled.parameters,
        "graph_digest": graph_digest,
        "graph_nodes": compiled.graph.nodes.len(),
        "graph_outputs": compiled.graph.outputs,
        "semantic_packet_digest": compiled.semantic_packet["packet_digest"],
    });
    if let Some(path) = report_path {
        write_json(&path, &report);
    }
    println!("SFCS LLVM IR");
    println!(
        "source_digest: {}",
        report["source_digest"].as_str().unwrap_or("")
    );
    println!("graph_digest: {graph_digest}");
    println!("nodes: {}", report["graph_nodes"].as_u64().unwrap_or(0));
    println!(
        "semantic_packet_digest: {}",
        report["semantic_packet_digest"].as_str().unwrap_or("")
    );
}

#[cfg(feature = "sfcs")]
fn cmd_sfcs_wasm_stack(args: Vec<String>) {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_sfcs_help();
        return;
    }
    let mut source_path = None;
    let mut graph_output = None;
    let mut semantic_output = None;
    let mut artifact_output = None;
    let mut report_path = None;
    let mut label = "sfcs-wasm-stack".to_string();
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--graph-output" => {
                graph_output = Some(PathBuf::from(take_option(&mut iter, "--graph-output")))
            }
            "--semantic-output" => {
                semantic_output = Some(PathBuf::from(take_option(&mut iter, "--semantic-output")))
            }
            "--artifact-output" => {
                artifact_output = Some(PathBuf::from(take_option(&mut iter, "--artifact-output")))
            }
            "--report" => report_path = Some(PathBuf::from(take_option(&mut iter, "--report"))),
            "--label" => label = take_option(&mut iter, "--label"),
            value if source_path.is_none() => source_path = Some(PathBuf::from(value)),
            other => fatal(&format!("unknown argument: {other}")),
        }
    }
    let source_path =
        source_path.unwrap_or_else(|| fatal("sfcs wasm-stack requires <source.wasmstack>"));
    let graph_output = graph_output.unwrap_or_else(|| fatal("--graph-output is required"));
    let source = fs::read_to_string(&source_path)
        .unwrap_or_else(|err| fatal(&format!("failed to read {}: {err}", source_path.display())));
    let compiled = compile_wasm_stack_source(&source).unwrap_or_else(|err| {
        fatal_code(
            sfcs_compiler_exit_for_error(&err),
            &format!("SFCS WASM stack compilation failed: {err}"),
        )
    });
    let graph_digest = compiled
        .graph_digest()
        .unwrap_or_else(|err| fatal(&format!("SFCS WASM stack graph digest failed: {err}")));
    write_json(&graph_output, &compiled.graph);
    if let Some(path) = semantic_output {
        write_json(&path, &compiled.semantic_packet);
    }
    if let Some(path) = artifact_output {
        let artifact = compiled
            .graph
            .to_pha_artifact(&label)
            .unwrap_or_else(|err| fatal(&format!("SFCS WASM stack .pha failed: {err}")));
        write_json(&path, &artifact);
    }
    let report = serde_json::json!({
        "schema": "power-house/sfcs-wasm-stack-cli-report/v1",
        "source": source_path.display().to_string(),
        "compiler_schema": compiled.schema,
        "source_digest": compiled.source_digest,
        "parameters": compiled.parameters,
        "graph_digest": graph_digest,
        "graph_nodes": compiled.graph.nodes.len(),
        "graph_outputs": compiled.graph.outputs,
        "semantic_packet_digest": compiled.semantic_packet["packet_digest"],
    });
    if let Some(path) = report_path {
        write_json(&path, &report);
    }
    println!("SFCS WASM STACK");
    println!(
        "source_digest: {}",
        report["source_digest"].as_str().unwrap_or("")
    );
    println!("graph_digest: {graph_digest}");
    println!("nodes: {}", report["graph_nodes"].as_u64().unwrap_or(0));
    println!(
        "semantic_packet_digest: {}",
        report["semantic_packet_digest"].as_str().unwrap_or("")
    );
}

#[cfg(feature = "sfcs")]
fn cmd_sfcs_eval(args: Vec<String>) {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_sfcs_help();
        return;
    }
    let mut source_path = None;
    let mut inputs = BTreeMap::<String, i64>::new();
    let mut report_path = None;
    let mut graph_output = None;
    let mut artifact_output = None;
    let mut label = "sfcs-execution".to_string();
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--input" => {
                let (name, value) = parse_sfcs_input(&take_option(&mut iter, "--input"));
                if inputs.insert(name.clone(), value).is_some() {
                    fatal(&format!("duplicate SFCS input: {name}"));
                }
            }
            "--report" => report_path = Some(PathBuf::from(take_option(&mut iter, "--report"))),
            "--graph-output" => {
                graph_output = Some(PathBuf::from(take_option(&mut iter, "--graph-output")))
            }
            "--artifact-output" => {
                artifact_output = Some(PathBuf::from(take_option(&mut iter, "--artifact-output")))
            }
            "--label" => label = take_option(&mut iter, "--label"),
            value if source_path.is_none() => source_path = Some(PathBuf::from(value)),
            other => fatal(&format!("unknown argument: {other}")),
        }
    }
    let source_path = source_path.unwrap_or_else(|| fatal("sfcs eval requires <source.sfcs>"));
    let graph = read_sfcs_source(&source_path);
    if let Some(path) = graph_output {
        write_json(&path, &graph);
    }
    let trace = graph.execution_trace(&inputs).unwrap_or_else(|err| {
        fatal_code(
            sfcs_exit_for_error(&err),
            &format!("SFCS execution failed: {err}"),
        )
    });
    let synthesis = graph.synthesis_plan().unwrap_or_else(|err| {
        fatal_code(
            sfcs_exit_for_error(&err),
            &format!("SFCS synthesis failed: {err}"),
        )
    });
    let report = serde_json::json!({
        "schema": "power-house/sfcs-cli-report/v1",
        "source": source_path,
        "graph_digest": trace.graph_digest,
        "trace_digest": trace.trace_digest,
        "synthesis_digest": synthesis.synthesis_digest,
        "embedding_invariant_digest": synthesis.embedding_invariant_digest,
        "input_digest": trace.input_digest,
        "output_digest": trace.output_digest,
        "outputs": trace.outputs,
        "trace_steps": trace.steps.len(),
        "fast_path_regions": synthesis.fast_path_regions,
        "dense_regions": synthesis.dense_regions,
        "dense_nodes": synthesis.dense_nodes,
        "fast_path_workload_digest": synthesis.fast_path_workload_digest,
    });
    if let Some(path) = report_path {
        write_json(&path, &report);
    }
    if let Some(path) = artifact_output {
        let artifact = graph
            .to_execution_pha_artifact(&label, &inputs)
            .unwrap_or_else(|err| fatal(&format!("SFCS .pha embedding failed: {err}")));
        write_json(&path, &artifact);
    }
    println!("SFCS EVAL");
    println!(
        "graph_digest: {}",
        report["graph_digest"].as_str().unwrap_or("")
    );
    println!(
        "trace_digest: {}",
        report["trace_digest"].as_str().unwrap_or("")
    );
    println!(
        "synthesis_digest: {}",
        report["synthesis_digest"].as_str().unwrap_or("")
    );
    for (name, value) in report["outputs"].as_object().into_iter().flatten() {
        println!("output {name}={value}");
    }
}

#[cfg(feature = "sfcs")]
fn cmd_sfcs_inspect(args: Vec<String>) {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_sfcs_help();
        return;
    }
    if args.len() != 1 {
        fatal("sfcs inspect requires <graph.json>");
    }
    let graph = read_sfcs_graph(Path::new(&args[0]));
    let digest = graph
        .fractal_digest()
        .unwrap_or_else(|err| fatal(&format!("SFCS graph digest failed: {err}")));
    let discovery = graph
        .discover_structure()
        .unwrap_or_else(|err| fatal(&format!("SFCS discovery failed: {err}")));
    println!("SFCS INSPECT");
    println!("graph_digest: {digest}");
    println!("nodes: {}", discovery.node_count);
    println!("fast_path_nodes: {}", discovery.fast_path_nodes.len());
    println!("dense_nodes: {}", discovery.dense_nodes.len());
    println!("fast_path_regions: {}", discovery.fast_path_regions);
    println!("dense_regions: {}", discovery.dense_regions);
}

#[cfg(feature = "sfcs")]
fn cmd_sfcs_verify_pha(args: Vec<String>) {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_sfcs_help();
        return;
    }
    if args.len() != 1 {
        fatal("sfcs verify-pha requires <artifact.pha>");
    }
    let artifact = read_pha(Path::new(&args[0]));
    match verify_sfcs_execution_embedding(&artifact) {
        Ok(report) => {
            println!("SFCS EXECUTION PHA VALID");
            println!("graph_digest: {}", report.graph_digest);
            println!("trace_digest: {}", report.trace_digest);
            println!("synthesis_digest: {}", report.synthesis_digest);
            println!("output_digest: {}", report.output_digest);
        }
        Err(execution_error) => match verify_sfcs_pha_embedding(&artifact) {
            Ok(report) => {
                println!("SFCS GRAPH PHA VALID");
                println!("graph_digest: {}", report.graph_digest);
                println!("fast_path_nodes: {}", report.fast_path_nodes);
                println!("dense_nodes: {}", report.dense_nodes);
            }
            Err(graph_error) => fatal_code(
                sfcs_exit_for_error(&graph_error),
                &format!(
                    "SFCS .pha verification failed: execution={execution_error}; graph={graph_error}"
                ),
            ),
        },
    }
}

#[cfg(feature = "sfcs")]
fn cmd_sfcs_vm_run(args: Vec<String>) {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_sfcs_help();
        return;
    }
    let mut program_path = None;
    let mut inputs_path = None;
    let mut report_path = None;
    let mut artifact_output = None;
    let mut label = "sfcs-vm-execution".to_string();
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--inputs" => inputs_path = Some(PathBuf::from(take_option(&mut iter, "--inputs"))),
            "--report" => report_path = Some(PathBuf::from(take_option(&mut iter, "--report"))),
            "--artifact-output" => {
                artifact_output = Some(PathBuf::from(take_option(&mut iter, "--artifact-output")))
            }
            "--label" => label = take_option(&mut iter, "--label"),
            value if program_path.is_none() => program_path = Some(PathBuf::from(value)),
            other => fatal(&format!("unknown argument: {other}")),
        }
    }
    let program_path = program_path.unwrap_or_else(|| fatal("sfcs vm-run requires <program.json>"));
    let inputs_path = inputs_path.unwrap_or_else(|| fatal("--inputs is required"));
    let program = read_sfcs_vm_program(&program_path);
    let inputs = read_sfcs_vm_inputs(&inputs_path);
    let trace = program.execute(&inputs).unwrap_or_else(|err| {
        fatal_code(
            sfcs_vm_exit_for_error(&err),
            &format!("SFCS VM execution failed: {err}"),
        )
    });
    let execution_fractal = trace.to_fractal_graph().unwrap_or_else(|err| {
        fatal_code(
            sfcs_vm_exit_for_error(&err),
            &format!("SFCS VM execution fractal failed: {err}"),
        )
    });
    let execution_fractal_digest = execution_fractal
        .fractal_digest()
        .unwrap_or_else(|err| fatal(&format!("SFCS VM execution fractal digest failed: {err}")));
    let report = serde_json::json!({
        "schema": "power-house/sfcs-vm-cli-report/v1",
        "program": program_path.display().to_string(),
        "inputs": inputs_path.display().to_string(),
        "architecture": trace.architecture,
        "program_digest": trace.program_digest,
        "input_digest": trace.input_digest,
        "trace_digest": trace.trace_digest,
        "execution_fractal_digest": execution_fractal_digest,
        "initial_state_digest": trace.initial_state_digest,
        "final_state_digest": trace.final_state_digest,
        "final_memory_digest": trace.final_memory_digest,
        "final_pc": trace.final_pc,
        "steps": trace.steps.len(),
        "public_outputs": trace.public_outputs,
    });
    if let Some(path) = report_path {
        write_json(&path, &report);
    }
    if let Some(path) = artifact_output {
        let artifact = program
            .to_execution_pha_artifact(&label, &inputs)
            .unwrap_or_else(|err| fatal(&format!("SFCS VM .pha embedding failed: {err}")));
        write_json(&path, &artifact);
    }
    println!("SFCS VM RUN");
    println!(
        "program_digest: {}",
        report["program_digest"].as_str().unwrap_or("")
    );
    println!(
        "trace_digest: {}",
        report["trace_digest"].as_str().unwrap_or("")
    );
    println!(
        "final_state_digest: {}",
        report["final_state_digest"].as_str().unwrap_or("")
    );
    println!(
        "execution_fractal_digest: {}",
        report["execution_fractal_digest"].as_str().unwrap_or("")
    );
    println!("steps: {}", report["steps"].as_u64().unwrap_or(0));
}

#[cfg(feature = "sfcs")]
fn cmd_sfcs_verify_vm_pha(args: Vec<String>) {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_sfcs_help();
        return;
    }
    if args.len() != 1 {
        fatal("sfcs verify-vm-pha requires <artifact.pha>");
    }
    let artifact = read_pha(Path::new(&args[0]));
    match verify_sfcs_vm_execution_embedding(&artifact) {
        Ok(report) => {
            println!("SFCS VM EXECUTION PHA VALID");
            println!("program_digest: {}", report.program_digest);
            println!("trace_digest: {}", report.trace_digest);
            println!(
                "execution_fractal_digest: {}",
                report.execution_fractal_digest
            );
            println!("final_state_digest: {}", report.final_state_digest);
            println!("steps: {}", report.steps);
        }
        Err(error) => fatal_code(
            sfcs_vm_exit_for_error(&error),
            &format!("SFCS VM .pha verification failed: {error}"),
        ),
    }
}

#[cfg(feature = "sfcs")]
fn cmd_sfcs_vm_constraints(args: Vec<String>) {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_sfcs_help();
        return;
    }
    let mut program_path = None;
    let mut inputs_path = None;
    let mut report_path = None;
    let mut artifact_output = None;
    let mut label = "sfcs-vm-constraints".to_string();
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--inputs" => inputs_path = Some(PathBuf::from(take_option(&mut iter, "--inputs"))),
            "--report" => report_path = Some(PathBuf::from(take_option(&mut iter, "--report"))),
            "--artifact-output" => {
                artifact_output = Some(PathBuf::from(take_option(&mut iter, "--artifact-output")))
            }
            "--label" => label = take_option(&mut iter, "--label"),
            value if program_path.is_none() => program_path = Some(PathBuf::from(value)),
            other => fatal(&format!("unknown argument: {other}")),
        }
    }
    let program_path =
        program_path.unwrap_or_else(|| fatal("sfcs vm-constraints requires <program.json>"));
    let inputs_path = inputs_path.unwrap_or_else(|| fatal("--inputs is required"));
    let program = read_sfcs_vm_program(&program_path);
    let inputs = read_sfcs_vm_inputs(&inputs_path);
    let proof = SfcsVmConstraintProof::prove(&program, &inputs).unwrap_or_else(|err| {
        fatal_code(
            sfcs_vm_constraint_exit_for_error(&err),
            &format!("SFCS VM constraint proof failed: {err}"),
        )
    });
    let report = serde_json::json!({
        "schema": "power-house/sfcs-vm-constraints-cli-report/v1",
        "program": program_path.display().to_string(),
        "inputs": inputs_path.display().to_string(),
        "profile": power_house::SFCS_VM_CONSTRAINT_PROTOCOL_V1_DRAFT,
        "program_digest": proof.program_digest,
        "input_digest": proof.input_digest,
        "trace_digest": proof.trace_digest,
        "execution_fractal_digest": proof.execution_fractal_digest,
        "final_state_digest": proof.final_state_digest,
        "final_memory_digest": proof.final_memory_digest,
        "proof_digest": proof.proof_digest,
        "steps": proof.steps,
        "transition_checks": proof.transition_checks,
        "register_range_checks": proof.register_range_checks,
        "memory_range_checks": proof.memory_range_checks,
        "memory_consistency_checks": proof.memory_consistency_checks,
        "branch_checks": proof.branch_checks,
    });
    if let Some(path) = report_path {
        write_json(&path, &report);
    }
    if let Some(path) = artifact_output {
        let artifact = proof
            .to_pha_artifact(&label, &program, &inputs)
            .unwrap_or_else(|err| fatal(&format!("SFCS VM constraint .pha failed: {err}")));
        write_json(&path, &artifact);
    }
    println!("SFCS VM CONSTRAINTS");
    println!(
        "program_digest: {}",
        report["program_digest"].as_str().unwrap_or("")
    );
    println!(
        "trace_digest: {}",
        report["trace_digest"].as_str().unwrap_or("")
    );
    println!(
        "proof_digest: {}",
        report["proof_digest"].as_str().unwrap_or("")
    );
    println!(
        "transition_checks: {}",
        report["transition_checks"].as_u64().unwrap_or(0)
    );
    println!(
        "memory_consistency_checks: {}",
        report["memory_consistency_checks"].as_u64().unwrap_or(0)
    );
}

#[cfg(feature = "sfcs")]
fn cmd_sfcs_verify_vm_constraints_pha(args: Vec<String>) {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_sfcs_help();
        return;
    }
    if args.len() != 1 {
        fatal("sfcs verify-vm-constraints-pha requires <artifact.pha>");
    }
    let artifact = read_pha(Path::new(&args[0]));
    match verify_sfcs_vm_constraint_embedding(&artifact) {
        Ok(proof) => {
            println!("SFCS VM CONSTRAINT PHA VALID");
            println!("program_digest: {}", proof.program_digest);
            println!("trace_digest: {}", proof.trace_digest);
            println!("proof_digest: {}", proof.proof_digest);
            println!("transition_checks: {}", proof.transition_checks);
            println!(
                "memory_consistency_checks: {}",
                proof.memory_consistency_checks
            );
        }
        Err(error) => fatal_code(
            sfcs_vm_constraint_exit_for_error(&error),
            &format!("SFCS VM constraint .pha verification failed: {error}"),
        ),
    }
}

#[cfg(feature = "sfcs-zk")]
fn cmd_sfcs_rust_private_add(args: Vec<String>) {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_sfcs_help();
        return;
    }
    let mut source_path = None;
    let mut lhs_value = None;
    let mut rhs_value = None;
    let mut lhs_blinding_seed = None;
    let mut rhs_blinding_seed = None;
    let mut report_path = None;
    let mut artifact_output = None;
    let mut rootprint_output = None;
    let mut sidecar_output = None;
    let mut capsule_output = None;
    let mut label = "sfcs-rust-private-add".to_string();
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--lhs-value" => {
                lhs_value = Some(parse_u32_arg(
                    &take_option(&mut iter, "--lhs-value"),
                    "--lhs-value",
                ))
            }
            "--rhs-value" => {
                rhs_value = Some(parse_u32_arg(
                    &take_option(&mut iter, "--rhs-value"),
                    "--rhs-value",
                ))
            }
            "--lhs-blinding" => {
                lhs_blinding_seed = Some(parse_seed_hex(
                    &take_option(&mut iter, "--lhs-blinding"),
                    "--lhs-blinding",
                ))
            }
            "--rhs-blinding" => {
                rhs_blinding_seed = Some(parse_seed_hex(
                    &take_option(&mut iter, "--rhs-blinding"),
                    "--rhs-blinding",
                ))
            }
            "--report" => report_path = Some(PathBuf::from(take_option(&mut iter, "--report"))),
            "--artifact-output" => {
                artifact_output = Some(PathBuf::from(take_option(&mut iter, "--artifact-output")))
            }
            "--rootprint-output" => {
                rootprint_output = Some(PathBuf::from(take_option(&mut iter, "--rootprint-output")))
            }
            "--sidecar-output" => {
                sidecar_output = Some(PathBuf::from(take_option(&mut iter, "--sidecar-output")))
            }
            "--capsule-output" => {
                capsule_output = Some(PathBuf::from(take_option(&mut iter, "--capsule-output")))
            }
            "--label" => label = take_option(&mut iter, "--label"),
            value if source_path.is_none() => source_path = Some(PathBuf::from(value)),
            other => fatal(&format!("unknown argument: {other}")),
        }
    }
    let source_path =
        source_path.unwrap_or_else(|| fatal("sfcs rust-private-add requires <source.rs>"));
    let source = fs::read_to_string(&source_path)
        .unwrap_or_else(|err| fatal(&format!("failed to read {}: {err}", source_path.display())));
    let compiled = compile_private_add_source(&source).unwrap_or_else(|err| {
        fatal_code(
            sfcs_compiler_exit_for_error(&err),
            &format!("SFCS Rust-subset compilation failed: {err}"),
        )
    });
    let proof = SfcsZkPrivateAddProof::prove(
        &compiled.program,
        compiled.lhs_register,
        compiled.rhs_register,
        compiled.output_register,
        SfcsZkPrivateAddWitness {
            lhs_value: lhs_value.unwrap_or_else(|| fatal("--lhs-value is required")),
            rhs_value: rhs_value.unwrap_or_else(|| fatal("--rhs-value is required")),
            lhs_blinding_seed: lhs_blinding_seed
                .unwrap_or_else(|| fatal("--lhs-blinding is required")),
            rhs_blinding_seed: rhs_blinding_seed
                .unwrap_or_else(|| fatal("--rhs-blinding is required")),
        },
    )
    .unwrap_or_else(|err| {
        fatal_code(
            sfcs_zk_exit_for_error(&err),
            &format!("SFCS ZK private-add proof failed: {err}"),
        )
    });
    proof.verify(&compiled.program).unwrap_or_else(|err| {
        fatal_code(
            sfcs_zk_exit_for_error(&err),
            &format!("SFCS ZK private-add verification failed: {err}"),
        )
    });
    let artifact = proof
        .to_pha_artifact(&label, &compiled.program)
        .unwrap_or_else(|err| fatal(&format!("SFCS ZK .pha embedding failed: {err}")));
    let rootprint = Rootprint::new(&label, artifact.clone())
        .unwrap_or_else(|err| fatal(&format!("Rootprint creation failed: {err}")));
    let sidecar_nodes = BTreeMap::from([(rootprint.root_branch.clone(), compiled.semantic_packet)]);
    let sidecar = ObservatorySidecar::new(&rootprint, sidecar_nodes)
        .unwrap_or_else(|err| fatal(&format!("Observatory sidecar creation failed: {err}")));
    sidecar
        .verify(&rootprint)
        .unwrap_or_else(|err| fatal(&format!("Observatory sidecar verification failed: {err}")));
    let packet = sidecar
        .nodes
        .get(&rootprint.root_branch)
        .cloned()
        .unwrap_or_else(|| fatal("compiler sidecar packet missing root branch"));
    let packet_schema = packet
        .get("schema")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("slbit/viz-packet/v3")
        .to_string();
    let packet_id = packet
        .get("packet_id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("sfcs-rust-private-add")
        .to_string();
    let capsule = MemoryCapsuleBuilder::new(&label)
        .producer("mfenx", env!("CARGO_PKG_VERSION"))
        .with_pha(artifact.clone())
        .with_rootprint(rootprint.clone())
        .with_replay_required()
        .with_sidecar(sidecar.clone())
        .with_semantic_packet(
            packet_schema,
            packet_id,
            &rootprint.root_branch,
            &sidecar.rootprint_state_fingerprint,
            "claim_view",
            packet,
        )
        .unwrap_or_else(|err| fatal(&format!("semantic packet binding failed: {err}")))
        .with_challenge_suite(ChallengeSuite::standard())
        .build()
        .unwrap_or_else(|err| fatal(&format!("Memory Capsule creation failed: {err}")));
    let memory_report = capsule
        .verify(MemoryVerificationPolicy::strict())
        .unwrap_or_else(|err| fatal(&format!("Memory Capsule verification failed: {err}")));

    if let Some(path) = artifact_output {
        write_json(&path, &artifact);
    }
    if let Some(path) = rootprint_output {
        write_json(&path, &rootprint);
    }
    if let Some(path) = sidecar_output {
        write_json(&path, &sidecar);
    }
    if let Some(path) = capsule_output {
        capsule
            .write_canonical(&path)
            .unwrap_or_else(|err| fatal(&format!("failed to write capsule: {err}")));
    }
    let report = serde_json::json!({
        "schema": "power-house/sfcs-rust-private-add-cli-report/v1",
        "source": source_path.display().to_string(),
        "compiler_schema": compiled.schema,
        "source_digest": compiled.source_digest,
        "program_digest": proof.statement.program_digest,
        "zk_profile": power_house::SFCS_ZK_PRIVATE_ADD_PROTOCOL_V1_DRAFT,
        "proof_digest": proof.proof_digest,
        "pha_fingerprint": artifact.phx_fingerprint,
        "rootprint_root": rootprint.root_branch,
        "rootprint_replay_fingerprint": sidecar.rootprint_state_fingerprint,
        "sidecar_sha256": sidecar.sidecar_sha256,
        "capsule_digest": capsule.header.capsule_digest,
        "memory_core_valid": memory_report.core_valid,
        "memory_rootprint_valid": memory_report.rootprint_valid,
        "memory_replay_valid": memory_report.replay_valid,
        "memory_sidecar_valid": memory_report.sidecar_valid,
        "memory_semantic_valid": memory_report.semantic_valid,
        "output_register": proof.statement.output_register,
        "output_value": proof.statement.output_value,
        "lhs_commitment": proof.statement.lhs_commitment,
        "rhs_commitment": proof.statement.rhs_commitment,
        "truth_boundary": "semantic packet data is non-core and cannot alter .pha or Rootprint proof identity"
    });
    if let Some(path) = report_path {
        write_json(&path, &report);
    }
    println!("SFCS RUST PRIVATE ADD");
    println!(
        "source_digest: {}",
        report["source_digest"].as_str().unwrap_or("")
    );
    println!(
        "program_digest: {}",
        report["program_digest"].as_str().unwrap_or("")
    );
    println!(
        "proof_digest: {}",
        report["proof_digest"].as_str().unwrap_or("")
    );
    println!(
        "rootprint_replay_fingerprint: {}",
        report["rootprint_replay_fingerprint"]
            .as_str()
            .unwrap_or("")
    );
    println!(
        "capsule_digest: {}",
        report["capsule_digest"].as_str().unwrap_or("")
    );
    println!(
        "output x{}={}",
        proof.statement.output_register, proof.statement.output_value
    );
    println!("truth_boundary: semantic packet data is non-core");
}

#[cfg(feature = "sfcs-zk")]
fn cmd_sfcs_zk_private_add(args: Vec<String>) {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_sfcs_help();
        return;
    }
    let mut program_path = None;
    let mut lhs_register = None;
    let mut rhs_register = None;
    let mut output_register = None;
    let mut lhs_value = None;
    let mut rhs_value = None;
    let mut lhs_blinding_seed = None;
    let mut rhs_blinding_seed = None;
    let mut report_path = None;
    let mut artifact_output = None;
    let mut label = "sfcs-zk-private-add".to_string();
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--lhs-register" => {
                lhs_register = Some(parse_register(
                    &take_option(&mut iter, "--lhs-register"),
                    "--lhs-register",
                ))
            }
            "--rhs-register" => {
                rhs_register = Some(parse_register(
                    &take_option(&mut iter, "--rhs-register"),
                    "--rhs-register",
                ))
            }
            "--output-register" => {
                output_register = Some(parse_register(
                    &take_option(&mut iter, "--output-register"),
                    "--output-register",
                ))
            }
            "--lhs-value" => {
                lhs_value = Some(parse_u32_arg(
                    &take_option(&mut iter, "--lhs-value"),
                    "--lhs-value",
                ))
            }
            "--rhs-value" => {
                rhs_value = Some(parse_u32_arg(
                    &take_option(&mut iter, "--rhs-value"),
                    "--rhs-value",
                ))
            }
            "--lhs-blinding" => {
                lhs_blinding_seed = Some(parse_seed_hex(
                    &take_option(&mut iter, "--lhs-blinding"),
                    "--lhs-blinding",
                ))
            }
            "--rhs-blinding" => {
                rhs_blinding_seed = Some(parse_seed_hex(
                    &take_option(&mut iter, "--rhs-blinding"),
                    "--rhs-blinding",
                ))
            }
            "--report" => report_path = Some(PathBuf::from(take_option(&mut iter, "--report"))),
            "--artifact-output" => {
                artifact_output = Some(PathBuf::from(take_option(&mut iter, "--artifact-output")))
            }
            "--label" => label = take_option(&mut iter, "--label"),
            value if program_path.is_none() => program_path = Some(PathBuf::from(value)),
            other => fatal(&format!("unknown argument: {other}")),
        }
    }
    let program_path =
        program_path.unwrap_or_else(|| fatal("sfcs zk-private-add requires <program.json>"));
    let program = read_sfcs_vm_program(&program_path);
    let proof = SfcsZkPrivateAddProof::prove(
        &program,
        lhs_register.unwrap_or_else(|| fatal("--lhs-register is required")),
        rhs_register.unwrap_or_else(|| fatal("--rhs-register is required")),
        output_register.unwrap_or_else(|| fatal("--output-register is required")),
        SfcsZkPrivateAddWitness {
            lhs_value: lhs_value.unwrap_or_else(|| fatal("--lhs-value is required")),
            rhs_value: rhs_value.unwrap_or_else(|| fatal("--rhs-value is required")),
            lhs_blinding_seed: lhs_blinding_seed
                .unwrap_or_else(|| fatal("--lhs-blinding is required")),
            rhs_blinding_seed: rhs_blinding_seed
                .unwrap_or_else(|| fatal("--rhs-blinding is required")),
        },
    )
    .unwrap_or_else(|err| {
        fatal_code(
            sfcs_zk_exit_for_error(&err),
            &format!("SFCS ZK private-add proof failed: {err}"),
        )
    });
    let report = serde_json::json!({
        "schema": "power-house/sfcs-zk-private-add-cli-report/v1",
        "program": program_path.display().to_string(),
        "profile": power_house::SFCS_ZK_PRIVATE_ADD_PROTOCOL_V1_DRAFT,
        "program_digest": proof.statement.program_digest,
        "output_register": proof.statement.output_register,
        "output_value": proof.statement.output_value,
        "lhs_commitment": proof.statement.lhs_commitment,
        "rhs_commitment": proof.statement.rhs_commitment,
        "proof_digest": proof.proof_digest,
    });
    if let Some(path) = report_path {
        write_json(&path, &report);
    }
    if let Some(path) = artifact_output {
        let artifact = proof
            .to_pha_artifact(&label, &program)
            .unwrap_or_else(|err| fatal(&format!("SFCS ZK .pha embedding failed: {err}")));
        write_json(&path, &artifact);
    }
    println!("SFCS ZK PRIVATE ADD");
    println!(
        "program_digest: {}",
        report["program_digest"].as_str().unwrap_or("")
    );
    println!(
        "proof_digest: {}",
        report["proof_digest"].as_str().unwrap_or("")
    );
    println!(
        "output x{}={}",
        proof.statement.output_register, proof.statement.output_value
    );
}

#[cfg(feature = "sfcs-zk")]
fn cmd_sfcs_zk_private_vm(args: Vec<String>) {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_sfcs_help();
        return;
    }
    let mut program_path = None;
    let mut witness_path = None;
    let mut report_path = None;
    let mut artifact_output = None;
    let mut label = "sfcs-zk-private-vm".to_string();
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--witness" => witness_path = Some(PathBuf::from(take_option(&mut iter, "--witness"))),
            "--report" => report_path = Some(PathBuf::from(take_option(&mut iter, "--report"))),
            "--artifact-output" => {
                artifact_output = Some(PathBuf::from(take_option(&mut iter, "--artifact-output")))
            }
            "--label" => label = take_option(&mut iter, "--label"),
            value if program_path.is_none() => program_path = Some(PathBuf::from(value)),
            other => fatal(&format!("unknown argument: {other}")),
        }
    }
    let program_path =
        program_path.unwrap_or_else(|| fatal("sfcs zk-private-vm requires <program.json>"));
    let witness_path = witness_path.unwrap_or_else(|| fatal("--witness is required"));
    let program = read_sfcs_vm_program(&program_path);
    let witness = read_sfcs_private_vm_witness(&witness_path);
    let proof = SfcsZkPrivateVmProof::prove(&program, witness).unwrap_or_else(|err| {
        fatal_code(
            sfcs_zk_exit_for_error(&err),
            &format!("SFCS ZK private-VM proof failed: {err}"),
        )
    });
    proof.verify(&program).unwrap_or_else(|err| {
        fatal_code(
            sfcs_zk_exit_for_error(&err),
            &format!("SFCS ZK private-VM verification failed: {err}"),
        )
    });
    let report = serde_json::json!({
        "schema": "power-house/sfcs-zk-private-vm-cli-report/v1",
        "program": program_path.display().to_string(),
        "profile": power_house::SFCS_ZK_PRIVATE_VM_PROTOCOL_V1_DRAFT,
        "program_digest": proof.statement.program_digest,
        "public_outputs": proof.statement.public_outputs,
        "steps": proof.statement.steps,
        "transition_checks": proof.statement.transition_checks,
        "register_range_checks": proof.statement.register_range_checks,
        "memory_range_checks": proof.statement.memory_range_checks,
        "memory_consistency_checks": proof.statement.memory_consistency_checks,
        "branch_checks": proof.statement.branch_checks,
        "linear_relation_checks": proof.statement.linear_relation_checks,
        "zk_range_proofs": proof.statement.zk_range_proofs,
        "zk_memory_consistency_proofs": proof.statement.zk_memory_consistency_proofs,
        "zk_memory_value_proofs": proof.statement.zk_memory_value_proofs,
        "commitments": proof.statement.commitments,
        "proof_digest": proof.proof_digest,
        "private_witness_embedded": false,
    });
    if let Some(path) = report_path {
        write_json(&path, &report);
    }
    if let Some(path) = artifact_output {
        let artifact = proof
            .to_pha_artifact(&label, &program)
            .unwrap_or_else(|err| fatal(&format!("SFCS ZK private-VM .pha failed: {err}")));
        write_json(&path, &artifact);
    }
    println!("SFCS ZK PRIVATE VM");
    println!(
        "program_digest: {}",
        report["program_digest"].as_str().unwrap_or("")
    );
    println!(
        "proof_digest: {}",
        report["proof_digest"].as_str().unwrap_or("")
    );
    println!("steps: {}", report["steps"].as_u64().unwrap_or(0));
    println!(
        "transition_checks: {}",
        report["transition_checks"].as_u64().unwrap_or(0)
    );
    println!(
        "memory_consistency_checks: {}",
        report["memory_consistency_checks"].as_u64().unwrap_or(0)
    );
    println!(
        "linear_relation_checks: {}",
        report["linear_relation_checks"].as_u64().unwrap_or(0)
    );
    println!(
        "zk_range_proofs: {}",
        report["zk_range_proofs"].as_u64().unwrap_or(0)
    );
    println!(
        "zk_memory_consistency_proofs: {}",
        report["zk_memory_consistency_proofs"].as_u64().unwrap_or(0)
    );
    println!(
        "zk_memory_value_proofs: {}",
        report["zk_memory_value_proofs"].as_u64().unwrap_or(0)
    );
    println!("private_witness_embedded: false");
}

#[cfg(feature = "sfcs-zk")]
fn cmd_sfcs_verify_zk_pha(args: Vec<String>) {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_sfcs_help();
        return;
    }
    if args.len() != 1 {
        fatal("sfcs verify-zk-pha requires <artifact.pha>");
    }
    let artifact = read_pha(Path::new(&args[0]));
    match verify_sfcs_zk_private_add_embedding(&artifact) {
        Ok(proof) => {
            println!("SFCS ZK PRIVATE ADD PHA VALID");
            println!("program_digest: {}", proof.statement.program_digest);
            println!("proof_digest: {}", proof.proof_digest);
            println!(
                "public_output: x{}={}",
                proof.statement.output_register, proof.statement.output_value
            );
        }
        Err(add_error) => match verify_sfcs_zk_private_vm_embedding(&artifact) {
            Ok(proof) => {
                println!("SFCS ZK PRIVATE VM PHA VALID");
                println!("program_digest: {}", proof.statement.program_digest);
                println!("proof_digest: {}", proof.proof_digest);
                println!("steps: {}", proof.statement.steps);
                println!("transition_checks: {}", proof.statement.transition_checks);
                println!(
                    "memory_consistency_checks: {}",
                    proof.statement.memory_consistency_checks
                );
                println!(
                    "linear_relation_checks: {}",
                    proof.statement.linear_relation_checks
                );
                println!("zk_range_proofs: {}", proof.statement.zk_range_proofs);
                println!(
                    "zk_memory_consistency_proofs: {}",
                    proof.statement.zk_memory_consistency_proofs
                );
                println!(
                    "zk_memory_value_proofs: {}",
                    proof.statement.zk_memory_value_proofs
                );
                println!("private_witness_embedded: false");
            }
            Err(vm_error) => fatal_code(
                sfcs_zk_exit_for_error(&vm_error),
                &format!(
                    "SFCS ZK .pha verification failed: private-add error: {add_error}; private-vm error: {vm_error}"
                ),
            ),
        },
    }
}

fn cmd_rootprint_init(args: Vec<String>) {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_rootprint_help();
        return;
    }
    let mut artifact_path = None;
    let mut label = None;
    let mut output = None;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--label" => label = Some(take_option(&mut iter, "--label")),
            "--output" => output = Some(PathBuf::from(take_option(&mut iter, "--output"))),
            value if artifact_path.is_none() => artifact_path = Some(PathBuf::from(value)),
            other => fatal(&format!("unknown argument: {other}")),
        }
    }
    let artifact_path = artifact_path.unwrap_or_else(|| fatal("artifact.pha is required"));
    let output = output.unwrap_or_else(|| fatal("--output is required"));
    let label = label.unwrap_or_else(|| fatal("--label is required"));
    let artifact = read_pha(&artifact_path);
    let graph = Rootprint::new(label, artifact)
        .unwrap_or_else(|err| fatal(&format!("failed to create Rootprint: {err}")));
    write_json(&output, &graph);
    println!("root_branch: {}", graph.root_branch);
    println!("rootprint: {}", output.display());
}

fn cmd_rootprint_navigate(args: Vec<String>) {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_rootprint_help();
        return;
    }
    let mut positionals = Vec::new();
    let mut artifact_output = None;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--artifact-output" => {
                artifact_output = Some(PathBuf::from(take_option(&mut iter, "--artifact-output")))
            }
            value if !value.starts_with("--") => positionals.push(value.to_string()),
            other => fatal(&format!("unknown argument: {other}")),
        }
    }
    if positionals.len() != 2 {
        fatal("navigate requires <rootprint.json> <branch-selector>");
    }
    let graph = read_rootprint(Path::new(&positionals[0]));
    graph
        .verify()
        .unwrap_or_else(|err| fatal(&format!("Rootprint verification failed: {err}")));
    let branch = graph
        .navigate(&positionals[1])
        .unwrap_or_else(|err| fatal(&err.to_string()));
    if let Some(path) = artifact_output {
        write_json(&path, &branch.artifact);
    }
    println!(
        "{}",
        serde_json::to_string_pretty(branch)
            .unwrap_or_else(|err| fatal(&format!("failed to encode branch: {err}")))
    );
}

fn cmd_rootprint_fork(args: Vec<String>) {
    mutate_rootprint(args, false);
}

fn cmd_rootprint_merge(args: Vec<String>) {
    mutate_rootprint(args, true);
}

fn mutate_rootprint(args: Vec<String>, merge: bool) {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_rootprint_help();
        return;
    }
    let required = if merge { 4 } else { 3 };
    let mut positionals = Vec::new();
    let mut label = None;
    let mut output = None;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--label" => label = Some(take_option(&mut iter, "--label")),
            "--output" => output = Some(PathBuf::from(take_option(&mut iter, "--output"))),
            value if !value.starts_with("--") => positionals.push(value.to_string()),
            other => fatal(&format!("unknown argument: {other}")),
        }
    }
    if positionals.len() != required {
        if merge {
            fatal("merge requires <rootprint.json> <left> <right> <artifact.pha>");
        }
        fatal("fork requires <rootprint.json> <parent> <artifact.pha>");
    }
    let input = PathBuf::from(&positionals[0]);
    let output = output.unwrap_or_else(|| input.clone());
    let label = label.unwrap_or_else(|| fatal("--label is required"));
    let mut graph = read_rootprint(&input);
    graph
        .verify()
        .unwrap_or_else(|err| fatal(&format!("Rootprint verification failed: {err}")));
    let artifact_path = if merge {
        &positionals[3]
    } else {
        &positionals[2]
    };
    let artifact = read_pha(Path::new(artifact_path));
    let branch_id = if merge {
        graph
            .merge(&positionals[1], &positionals[2], label, artifact)
            .unwrap_or_else(|err| fatal(&format!("merge failed: {err}")))
    } else {
        graph
            .fork(&positionals[1], label, artifact)
            .unwrap_or_else(|err| fatal(&format!("fork failed: {err}")))
    };
    graph
        .verify()
        .unwrap_or_else(|err| fatal(&format!("Rootprint verification failed: {err}")));
    write_json(&output, &graph);
    println!("branch: {branch_id}");
    println!("rootprint: {}", output.display());
}

fn cmd_rootprint_verify(args: Vec<String>) {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_rootprint_help();
        return;
    }
    if args.len() != 1 {
        fatal("verify requires <rootprint.json>");
    }
    let graph = read_rootprint(Path::new(&args[0]));
    graph
        .verify()
        .unwrap_or_else(|err| fatal(&format!("Rootprint verification failed: {err}")));
    println!(
        "PASS: Rootprint core verified ({} branches, root {}).",
        graph.branches.len(),
        graph.root_branch
    );
}

fn cmd_rootprint_equivalent(args: Vec<String>) {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_rootprint_help();
        return;
    }
    if args.len() != 3 {
        fatal("equivalent requires <rootprint.json> <left> <right>");
    }
    let graph = read_rootprint(Path::new(&args[0]));
    graph
        .verify()
        .unwrap_or_else(|err| fatal(&format!("Rootprint verification failed: {err}")));
    let equivalent = graph
        .equivalent(&args[1], &args[2])
        .unwrap_or_else(|err| fatal(&err.to_string()));
    println!(
        "{}",
        if equivalent {
            "equivalent"
        } else {
            "different"
        }
    );
}

fn cmd_observatory_verify(args: Vec<String>) {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_observatory_help();
        return;
    }
    if args.len() != 2 {
        fatal("observatory verify requires <rootprint.json> <observatory-sidecar.json>");
    }
    let graph = read_rootprint(Path::new(&args[0]));
    graph
        .verify()
        .unwrap_or_else(|err| fatal(&format!("Rootprint verification failed: {err}")));
    let sidecar = read_observatory_sidecar(Path::new(&args[1]));
    sidecar
        .verify(&graph)
        .unwrap_or_else(|err| fatal(&format!("Observatory sidecar verification failed: {err}")));
    println!(
        "PASS: Rootprint core and Observatory sidecar verified ({} semantic nodes, {}).",
        sidecar.nodes.len(),
        sidecar.rootprint_state_fingerprint
    );
}

fn cmd_memory_create(args: Vec<String>) {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_memory_help();
        return;
    }
    let mut pha_path = None;
    let mut rootprint_path = None;
    let mut sidecar_path = None;
    let mut output = None;
    let mut capsule_id = "memory".to_string();
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--pha" => pha_path = Some(PathBuf::from(take_option(&mut iter, "--pha"))),
            "--rootprint" => {
                rootprint_path = Some(PathBuf::from(take_option(&mut iter, "--rootprint")))
            }
            "--sidecar" => sidecar_path = Some(PathBuf::from(take_option(&mut iter, "--sidecar"))),
            "--output" => output = Some(PathBuf::from(take_option(&mut iter, "--output"))),
            "--capsule-id" => capsule_id = take_option(&mut iter, "--capsule-id"),
            other => fatal(&format!("unknown argument: {other}")),
        }
    }
    let pha = read_pha(&pha_path.unwrap_or_else(|| fatal("--pha is required")));
    let rootprint =
        read_rootprint(&rootprint_path.unwrap_or_else(|| fatal("--rootprint is required")));
    let output = output.unwrap_or_else(|| PathBuf::from("capsule.phm"));
    let mut builder = MemoryCapsuleBuilder::new(capsule_id)
        .producer("mfenx", env!("CARGO_PKG_VERSION"))
        .with_pha(pha)
        .with_rootprint(rootprint.clone())
        .with_replay_required()
        .with_challenge_suite(ChallengeSuite::standard());
    if let Some(path) = sidecar_path {
        let sidecar = read_observatory_sidecar(&path);
        sidecar
            .verify(&rootprint)
            .unwrap_or_else(|err| fatal(&format!("sidecar verification failed: {err}")));
        for (branch_id, packet) in &sidecar.nodes {
            let packet_schema = packet
                .get("schema")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("opaque/semantic-packet");
            let packet_id = packet
                .get("packet_id")
                .or_else(|| packet.get("claim_id"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or(branch_id);
            builder = builder
                .with_semantic_packet(
                    packet_schema,
                    packet_id,
                    branch_id,
                    &sidecar.rootprint_state_fingerprint,
                    "claim_view",
                    packet.clone(),
                )
                .unwrap_or_else(|err| fatal(&format!("semantic packet binding failed: {err}")));
        }
        builder = builder.with_sidecar(sidecar);
    }
    let capsule = builder
        .build()
        .unwrap_or_else(|err| fatal(&format!("memory capsule creation failed: {err}")));
    capsule
        .write_canonical(&output)
        .unwrap_or_else(|err| fatal(&format!("failed to write capsule: {err}")));
    println!("memory_capsule: {}", output.display());
    println!(
        "capsule_digest: {}",
        capsule.header.capsule_digest.as_deref().unwrap_or("<none>")
    );
    println!("truth_boundary: semantic packets are non-core and cannot alter proof identity");
}

fn cmd_memory_verify(args: Vec<String>) {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_memory_help();
        return;
    }
    let mut positionals = Vec::new();
    let mut report_output = None;
    let mut policy = MemoryVerificationPolicy::strict();
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--policy" => {
                policy = match take_option(&mut iter, "--policy").as_str() {
                    "strict" => MemoryVerificationPolicy::strict(),
                    "inspect" => MemoryVerificationPolicy::inspect(),
                    other => fatal(&format!("unknown memory policy: {other}")),
                }
            }
            "--report" => report_output = Some(PathBuf::from(take_option(&mut iter, "--report"))),
            value if !value.starts_with("--") => positionals.push(value.to_string()),
            other => fatal(&format!("unknown argument: {other}")),
        }
    }
    if positionals.len() != 1 {
        fatal("memory verify requires <capsule.phm>");
    }
    let capsule = read_memory_capsule(Path::new(&positionals[0]), &policy);
    match capsule.verify(policy) {
        Ok(report) => {
            if let Some(path) = report_output {
                write_json(&path, &report);
            }
            println!("POWER HOUSE MEMORY VERIFY");
            println!();
            println!("capsule: {}", positionals[0]);
            println!("capsule_digest: {}", report.capsule_digest);
            println!("schema: {}", capsule.header.schema);
            println!();
            println!("CORE        {}", status_word(report.core_valid));
            println!("ROOTPRINT   {}", status_word(report.rootprint_valid));
            println!("REPLAY      {}", status_word(report.replay_valid));
            println!("SIDECAR     {}", optional_status_word(report.sidecar_valid));
            println!(
                "SEMANTIC    {}",
                optional_status_word(report.semantic_valid)
            );
            let valid_witnesses = report
                .witness_validity
                .iter()
                .filter(|item| item.valid)
                .count();
            println!(
                "WITNESSES   {} VALID / {} INVALID",
                valid_witnesses,
                report
                    .witness_validity
                    .len()
                    .saturating_sub(valid_witnesses)
            );
            println!();
            println!("truth boundary:");
            println!("  core proof identity is independent from semantic rendering");
        }
        Err(MemoryError::Rejected(trace)) => {
            if let Some(path) = report_output {
                write_json(&path, &trace);
            }
            println!("POWER HOUSE MEMORY VERIFY");
            println!();
            println!(
                "CORE        {}",
                status_word(trace.core_valid_before_failure)
            );
            println!(
                "ROOTPRINT   {}",
                status_word(trace.rootprint_valid_before_failure)
            );
            println!("{}        INVALID", trace.layer.to_uppercase());
            println!();
            println!("rejection:");
            println!("  layer: {}", trace.layer);
            println!("  code: {}", trace.code);
            println!("  reason: {}", trace.message);
            if let Some(path) = trace.json_pointer {
                println!("  path: {path}");
            }
            println!(
                "  semantic_can_affect_core: {}",
                trace.semantic_can_affect_core
            );
            fatal_code(1, "memory capsule rejected");
        }
        Err(err) => fatal(&format!("memory verification failed: {err}")),
    }
}

fn cmd_memory_replay(args: Vec<String>) {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_memory_help();
        return;
    }
    let mut positionals = Vec::new();
    let mut report_output = None;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--report" => report_output = Some(PathBuf::from(take_option(&mut iter, "--report"))),
            value if !value.starts_with("--") => positionals.push(value.to_string()),
            other => fatal(&format!("unknown argument: {other}")),
        }
    }
    if positionals.len() != 1 {
        fatal("memory replay requires <capsule.phm>");
    }
    let policy = MemoryVerificationPolicy::strict();
    let capsule = read_memory_capsule(Path::new(&positionals[0]), &policy);
    let report = capsule
        .replay()
        .unwrap_or_else(|err| fatal(&format!("memory replay failed: {err}")));
    if let Some(path) = report_output {
        write_json(&path, &report);
    }
    println!("replay_fingerprint: {}", report.replay_fingerprint);
    println!("branches: {}", report.branch_count);
    println!("replay: {}", status_word(report.replay_valid));
}

fn cmd_memory_challenge(args: Vec<String>) {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_memory_help();
        return;
    }
    let mut positionals = Vec::new();
    let mut report_output = None;
    let mut run_all = false;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--all" => run_all = true,
            "--report" => report_output = Some(PathBuf::from(take_option(&mut iter, "--report"))),
            value if !value.starts_with("--") => positionals.push(value.to_string()),
            other => fatal(&format!("unknown argument: {other}")),
        }
    }
    if positionals.len() != 1 || !run_all {
        fatal("memory challenge requires <capsule.phm> --all");
    }
    let policy = MemoryVerificationPolicy::strict();
    let capsule = read_memory_capsule(Path::new(&positionals[0]), &policy);
    let report = capsule
        .challenge_all(policy)
        .unwrap_or_else(|err| fatal(&format!("memory challenge failed: {err}")));
    if let Some(path) = report_output {
        write_json(&path, &report);
    }
    println!(
        "CHALLENGE   {}/{} EXPECTED REJECTIONS",
        report.expected_rejections, report.total
    );
    if report.mismatches > 0 {
        fatal_code(9, "memory challenge mismatch");
    }
}

fn cmd_memory_inspect(args: Vec<String>) {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_memory_help();
        return;
    }
    let path = args
        .iter()
        .find(|arg| !arg.starts_with("--"))
        .unwrap_or_else(|| fatal("memory inspect requires <capsule.phm>"));
    let policy = MemoryVerificationPolicy::inspect();
    let capsule = read_memory_capsule(Path::new(path), &policy);
    println!("capsule_id: {}", capsule.header.capsule_id);
    println!("schema: {}", capsule.header.schema);
    println!(
        "capsule_digest: {}",
        capsule
            .header
            .capsule_digest
            .as_deref()
            .unwrap_or("<missing>")
    );
    println!("branches: {}", capsule.lineage.rootprint.branches.len());
    println!(
        "semantic_packets: {}",
        capsule
            .semantics
            .as_ref()
            .map(|layer| layer.packets.len())
            .unwrap_or(0)
    );
}

fn cmd_memory_explain_boundary(args: Vec<String>) {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_memory_help();
        return;
    }
    println!("Power House verifies core artifacts, Rootprint lineage, and replay state.");
    println!("Observatory sidecars bind semantic packets to verified Rootprint replay.");
    println!("slbit packets explain meaning but do not become proof identity.");
    println!("Witnesses observe capsule digests; they do not make false claims true.");
    println!("Challenge vectors mutate copies and show where falsehood is rejected.");
}

fn cmd_memory_export(args: Vec<String>) {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_memory_help();
        return;
    }
    let mut positionals = Vec::new();
    let mut format = None;
    let mut output = None;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--format" => format = Some(take_option(&mut iter, "--format")),
            "--output" => output = Some(PathBuf::from(take_option(&mut iter, "--output"))),
            value if !value.starts_with("--") => positionals.push(value.to_string()),
            other => fatal(&format!("unknown argument: {other}")),
        }
    }
    if positionals.len() != 1 {
        fatal("memory export requires <capsule.phm>");
    }
    if format.as_deref() != Some("directory") {
        fatal("memory export currently supports --format directory");
    }
    let output = output.unwrap_or_else(|| fatal("--output is required"));
    let policy = MemoryVerificationPolicy::inspect();
    let capsule = read_memory_capsule(Path::new(&positionals[0]), &policy);
    fs::create_dir_all(&output)
        .unwrap_or_else(|err| fatal(&format!("failed to create {}: {err}", output.display())));
    write_json(&output.join("capsule.json"), &capsule);
    write_json(&output.join("core.pha"), &capsule.core.pha);
    write_json(&output.join("rootprint.json"), &capsule.lineage.rootprint);
    if let Some(sidecar) = capsule
        .semantics
        .as_ref()
        .and_then(|semantics| semantics.sidecar.as_ref())
    {
        write_json(&output.join("observatory-sidecar.json"), sidecar);
    }
    println!("memory_export: {}", output.display());
}

fn status_word(valid: bool) -> &'static str {
    if valid {
        "VALID"
    } else {
        "INVALID"
    }
}

fn optional_status_word(valid: Option<bool>) -> &'static str {
    match valid {
        Some(true) => "VALID",
        Some(false) => "INVALID",
        None => "NOT PRESENT",
    }
}

fn cmd_identity_create(args: Vec<String>) {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_identity_help();
        return;
    }
    let mut artifact_path = None;
    let mut label = None;
    let mut identity_output = None;
    let mut rootprint_output = None;
    let mut artifact_output = None;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--label" => label = Some(take_option(&mut iter, "--label")),
            "--identity-output" => {
                identity_output = Some(PathBuf::from(take_option(&mut iter, "--identity-output")))
            }
            "--rootprint-output" => {
                rootprint_output = Some(PathBuf::from(take_option(&mut iter, "--rootprint-output")))
            }
            "--artifact-output" => {
                artifact_output = Some(PathBuf::from(take_option(&mut iter, "--artifact-output")))
            }
            value if artifact_path.is_none() => artifact_path = Some(PathBuf::from(value)),
            other => fatal(&format!("unknown argument: {other}")),
        }
    }
    let artifact_path = artifact_path.unwrap_or_else(|| fatal("artifact.pha is required"));
    let label = label.unwrap_or_else(|| fatal("--label is required"));
    let identity_output = identity_output.unwrap_or_else(|| fatal("--identity-output is required"));
    let rootprint_output =
        rootprint_output.unwrap_or_else(|| fatal("--rootprint-output is required"));

    let (identity, graph) = Identity::create(label, read_pha(&artifact_path))
        .unwrap_or_else(|error| fatal(&format!("identity creation failed: {error}")));
    write_json(&identity_output, &identity);
    write_json(&rootprint_output, &graph);
    if let Some(path) = artifact_output {
        write_json(&path, identity.pha());
    }
    println!("identity: {}", identity.rootprint_id());
    println!("identity_file: {}", identity_output.display());
    println!("rootprint: {}", rootprint_output.display());
}

fn cmd_identity_fork(args: Vec<String>) {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_identity_help();
        return;
    }
    let mut positionals = Vec::new();
    let mut label = None;
    let mut identity_output = None;
    let mut rootprint_output = None;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--label" => label = Some(take_option(&mut iter, "--label")),
            "--identity-output" => {
                identity_output = Some(PathBuf::from(take_option(&mut iter, "--identity-output")))
            }
            "--rootprint-output" => {
                rootprint_output = Some(PathBuf::from(take_option(&mut iter, "--rootprint-output")))
            }
            value if !value.starts_with("--") => positionals.push(value.to_string()),
            other => fatal(&format!("unknown argument: {other}")),
        }
    }
    if positionals.len() != 3 {
        fatal("fork requires <identity.json> <rootprint.json> <artifact.pha>");
    }
    let label = label.unwrap_or_else(|| fatal("--label is required"));
    let identity_output = identity_output.unwrap_or_else(|| fatal("--identity-output is required"));
    let graph_input = PathBuf::from(&positionals[1]);
    let graph_output = rootprint_output.unwrap_or_else(|| graph_input.clone());
    let parent = read_identity(Path::new(&positionals[0]));
    let mut graph = read_rootprint(&graph_input);
    let identity = parent
        .fork(&mut graph, label, read_pha(Path::new(&positionals[2])))
        .unwrap_or_else(|error| fatal(&format!("identity fork failed: {error}")));
    write_json(&identity_output, &identity);
    write_json(&graph_output, &graph);
    println!("identity: {}", identity.rootprint_id());
    println!("identity_file: {}", identity_output.display());
    println!("rootprint: {}", graph_output.display());
}

fn cmd_identity_merge(args: Vec<String>) {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_identity_help();
        return;
    }
    let mut positionals = Vec::new();
    let mut label = None;
    let mut identity_output = None;
    let mut rootprint_output = None;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--label" => label = Some(take_option(&mut iter, "--label")),
            "--identity-output" => {
                identity_output = Some(PathBuf::from(take_option(&mut iter, "--identity-output")))
            }
            "--rootprint-output" => {
                rootprint_output = Some(PathBuf::from(take_option(&mut iter, "--rootprint-output")))
            }
            value if !value.starts_with("--") => positionals.push(value.to_string()),
            other => fatal(&format!("unknown argument: {other}")),
        }
    }
    if positionals.len() != 4 {
        fatal(
            "merge requires <left.identity.json> <right.identity.json> <rootprint.json> <artifact.pha>",
        );
    }
    let label = label.unwrap_or_else(|| fatal("--label is required"));
    let identity_output = identity_output.unwrap_or_else(|| fatal("--identity-output is required"));
    let graph_input = PathBuf::from(&positionals[2]);
    let graph_output = rootprint_output.unwrap_or_else(|| graph_input.clone());
    let left = read_identity(Path::new(&positionals[0]));
    let right = read_identity(Path::new(&positionals[1]));
    let mut graph = read_rootprint(&graph_input);
    let identity = Identity::merge(
        &left,
        &right,
        &mut graph,
        label,
        read_pha(Path::new(&positionals[3])),
    )
    .unwrap_or_else(|error| fatal(&format!("identity merge failed: {error}")));
    write_json(&identity_output, &identity);
    write_json(&graph_output, &graph);
    println!("identity: {}", identity.rootprint_id());
    println!("identity_file: {}", identity_output.display());
    println!("rootprint: {}", graph_output.display());
}

fn cmd_identity_verify(args: Vec<String>) {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_identity_help();
        return;
    }
    if args.len() != 2 {
        fatal("verify requires <identity.json> <rootprint.json>");
    }
    let identity = read_identity(Path::new(&args[0]));
    let graph = read_rootprint(Path::new(&args[1]));
    identity
        .verify(&graph)
        .unwrap_or_else(|error| fatal(&format!("identity verification failed: {error}")));
    println!(
        "PASS: identity {} verified offline.",
        identity.rootprint_id()
    );
}

fn cmd_identity_replay(args: Vec<String>) {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_identity_help();
        return;
    }
    let mut positionals = Vec::new();
    let mut output = None;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--output" => output = Some(PathBuf::from(take_option(&mut iter, "--output"))),
            value if !value.starts_with("--") => positionals.push(value.to_string()),
            other => fatal(&format!("unknown argument: {other}")),
        }
    }
    if positionals.len() != 2 {
        fatal("replay requires <identity.json> <rootprint.json>");
    }
    let identity = read_identity(Path::new(&positionals[0]));
    let graph = read_rootprint(Path::new(&positionals[1]));
    let state = identity
        .replay(&graph)
        .unwrap_or_else(|error| fatal(&format!("identity replay failed: {error}")));
    if let Some(path) = output {
        write_json(&path, &state);
        println!("replay_state: {}", path.display());
    } else {
        println!(
            "{}",
            serde_json::to_string_pretty(&state)
                .unwrap_or_else(|error| fatal(&format!("failed to encode replay state: {error}")))
        );
    }
}

fn cmd_identity_equivalent(args: Vec<String>) {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_identity_help();
        return;
    }
    if args.len() != 3 {
        fatal("equivalent requires <left.identity.json> <right.identity.json> <rootprint.json>");
    }
    let left = read_identity(Path::new(&args[0]));
    let right = read_identity(Path::new(&args[1]));
    let graph = read_rootprint(Path::new(&args[2]));
    let equivalent = left
        .equivalent(&right, &graph)
        .unwrap_or_else(|error| fatal(&format!("identity equivalence failed: {error}")));
    println!(
        "{}",
        if equivalent {
            "equivalent"
        } else {
            "different"
        }
    );
}

fn cmd_attach_external_proof(args: Vec<String>) {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_attach_external_proof_help();
        return;
    }
    let mut artifact_path = None;
    let mut id = None;
    let mut proof_system = None;
    let mut payload = None;
    let mut verifier_hint = None;
    let mut metadata = None;
    let mut output = None;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--id" => id = Some(take_option(&mut iter, "--id")),
            "--proof-system" => proof_system = Some(take_option(&mut iter, "--proof-system")),
            "--payload" => payload = Some(PathBuf::from(take_option(&mut iter, "--payload"))),
            "--verifier-hint" => verifier_hint = Some(take_option(&mut iter, "--verifier-hint")),
            "--metadata" => metadata = Some(PathBuf::from(take_option(&mut iter, "--metadata"))),
            "--output" => output = Some(PathBuf::from(take_option(&mut iter, "--output"))),
            value if artifact_path.is_none() => artifact_path = Some(PathBuf::from(value)),
            other => fatal(&format!("unknown argument: {other}")),
        }
    }
    let artifact_path = artifact_path.unwrap_or_else(|| fatal("artifact.pha is required"));
    let output = output.unwrap_or_else(|| artifact_path.clone());
    let payload_path = payload.unwrap_or_else(|| fatal("--payload is required"));
    let mut artifact = read_pha(&artifact_path);
    artifact
        .verify()
        .unwrap_or_else(|err| fatal(&format!("PHA core verification failed: {err}")));
    let original_fingerprint = artifact.phx_fingerprint.clone();
    let mut attachment = ExternalProofAttachment::new(
        id.unwrap_or_else(|| fatal("--id is required")),
        proof_system.unwrap_or_else(|| fatal("--proof-system is required")),
        read_json_value(&payload_path),
    )
    .unwrap_or_else(|err| fatal(&format!("failed to create attachment: {err}")));
    attachment.verifier_hint = verifier_hint;
    attachment.metadata = metadata.map(|path| read_json_value(&path));
    let attachments = artifact
        .embedded_proof
        .external_proof_attachments
        .get_or_insert_with(Vec::new);
    if attachments
        .iter()
        .any(|existing| existing.id == attachment.id)
    {
        fatal(&format!("attachment id already exists: {}", attachment.id));
    }
    attachments.push(attachment);
    artifact
        .verify_external_proof_attachments()
        .unwrap_or_else(|err| fatal(&format!("attachment verification failed: {err}")));
    if artifact.phx_fingerprint != original_fingerprint {
        fatal("internal error: external attachment changed the Power House fingerprint");
    }
    write_json(&output, &artifact);
    println!("phx_fingerprint: {}", artifact.phx_fingerprint);
    println!("core_unchanged: true");
    println!("artifact: {}", output.display());
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
fn handle_migration(sub: &str, tail: Vec<String>) {
    match sub {
        "-h" | "--help" => print_migration_help(),
        "finalize" => cmd_migration_finalize(tail),
        "verify-state" => cmd_migration_verify_state(tail),
        "execute-burn-intents" => cmd_migration_execute_burn_intents(tail),
        _ => {
            eprintln!("Unknown migration subcommand: {sub}");
            std::process::exit(1);
        }
    }
}

#[cfg(feature = "net")]
fn handle_rollup(sub: &str, tail: Vec<String>) {
    match sub {
        "-h" | "--help" => print_rollup_help(),
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
        "-h" | "--help" => print_node_help(),
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
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_scale_help();
        return;
    }

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
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_keygen_help();
        return;
    }

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
fn cmd_key_info(args: Vec<String>) {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_key_info_help();
        return;
    }

    let mut key_spec: Option<String> = None;
    let mut json = false;
    for arg in args {
        match arg.as_str() {
            "--json" => json = true,
            value if key_spec.is_none() => key_spec = Some(value.to_string()),
            value => fatal(&format!("unknown argument: {value}")),
        }
    }
    let key_spec = key_spec.unwrap_or_else(|| fatal("key-info requires a key specification"));
    let material = load_or_derive_keypair(&Ed25519KeySource::from_spec(Some(&key_spec)))
        .unwrap_or_else(|err| fatal(&format!("failed to load key: {err}")));
    let public_key = power_house::net::encode_public_key_base64(&material.verifying);
    let peer_id = material.libp2p.public().to_peer_id().to_string();

    if json {
        println!(
            "{}",
            serde_json::json!({
                "peer_id": peer_id,
                "public_key_b64": public_key,
            })
        );
    } else {
        println!("public_key_b64: {public_key}");
        println!("peer_id: {peer_id}");
    }
}

#[cfg(feature = "net")]
fn handle_observer(sub: &str, tail: Vec<String>) {
    match sub {
        "-h" | "--help" => print_observer_help(),
        "doctor" => cmd_observer_doctor(tail),
        "setup" => cmd_observer_setup(tail),
        "register" => cmd_observer_register(tail),
        "submit" => cmd_observer_submit(tail),
        "status" => cmd_observer_status(tail),
        _ => fatal(&format!("unknown observer subcommand: {sub}")),
    }
}

#[cfg(feature = "net")]
#[derive(Debug, Clone)]
struct ObserverCliOptions {
    node_id: String,
    operator: String,
    region: String,
    key_spec: String,
    p2p_port: u16,
    metrics_port: u16,
    public_host: Option<String>,
    probe_url: Option<String>,
    output: PathBuf,
    json: bool,
}

#[cfg(feature = "net")]
#[derive(Debug, Clone, Serialize)]
struct ObserverCheck {
    name: String,
    status: String,
    detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    fix: Option<String>,
}

#[cfg(feature = "net")]
#[derive(Debug, Clone, Serialize)]
struct ObserverDiagnosis {
    schema: String,
    node_id: String,
    key_spec: String,
    local_ip: Option<String>,
    public_host: Option<String>,
    p2p_port: u16,
    metrics_port: u16,
    peer_id: Option<String>,
    public_key_b64: Option<String>,
    checks: Vec<ObserverCheck>,
}

#[cfg(feature = "net")]
fn cmd_observer_doctor(args: Vec<String>) {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_observer_help();
        return;
    }
    let (values, switches) = parse_observer_options(
        args,
        &[
            "--node-id",
            "--key",
            "--operator",
            "--region",
            "--public-host",
            "--p2p-port",
            "--metrics-port",
            "--probe-url",
        ],
        &["--json", "--no-probe"],
    );
    let options = observer_cli_options(&values, &switches, false);
    let diagnosis = diagnose_observer(&options);
    print_observer_diagnosis(&diagnosis, options.json);
}

#[cfg(feature = "net")]
fn cmd_observer_setup(args: Vec<String>) {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_observer_help();
        return;
    }
    let (values, switches) = parse_observer_options(
        args,
        &[
            "--node-id",
            "--key",
            "--operator",
            "--region",
            "--public-host",
            "--p2p-port",
            "--metrics-port",
            "--probe-url",
            "--output",
        ],
        &["--json", "--no-probe"],
    );
    let options = observer_cli_options(&values, &switches, true);
    let created_key = ensure_default_node_key(&options.key_spec);
    let registration = observer_registration_from_options(&options);
    write_json_file(&options.output, &registration, "observer registration");
    let diagnosis = diagnose_observer(&options);
    if options.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "schema": "power-house-observer-setup-v1",
                "created_node_key": created_key.as_ref().map(|path| path.display().to_string()),
                "registration_output": options.output,
                "diagnosis": diagnosis,
                "start_command": observer_start_command(&options),
            }))
            .unwrap_or_else(|err| fatal(&format!("failed to encode setup JSON: {err}")))
        );
    } else {
        if let Some(path) = created_key {
            println!("created node key: {}", path.display());
        }
        println!("observer registration: {}", options.output.display());
        println!();
        println!("Start the observer with:");
        println!("{}", observer_start_command(&options));
        println!();
        print_observer_diagnosis(&diagnosis, false);
        println!();
        println!("Submit/check the signed JSON at https://mfenx.com/register.html");
    }
}

#[cfg(feature = "net")]
fn cmd_observer_register(args: Vec<String>) {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_observer_help();
        return;
    }
    let (values, switches) = parse_observer_options(
        args,
        &[
            "--node-id",
            "--key",
            "--operator",
            "--region",
            "--public-host",
            "--p2p-port",
            "--metrics-port",
            "--output",
        ],
        &["--json"],
    );
    let options = observer_cli_options(&values, &switches, true);
    let registration = observer_registration_from_options(&options);
    write_json_file(&options.output, &registration, "observer registration");
    if options.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&registration)
                .unwrap_or_else(|err| fatal(&format!("failed to encode registration: {err}")))
        );
    } else {
        print_registration_summary(
            "observer",
            &options.output,
            &registration.node_id,
            &registration.peer_id,
            &registration.p2p_address,
            &registration.metrics_url,
        );
        println!("next: run `julian observer doctor` or upload the signed JSON at https://mfenx.com/register.html");
    }
}

#[cfg(feature = "net")]
fn cmd_observer_submit(args: Vec<String>) {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_observer_help();
        return;
    }
    let mut input = None;
    let mut output = None;
    let mut json = false;
    let mut probe_url = Some("https://rpc.mfenx.com/observer-probe".to_string());
    let mut intake_url = Some("https://rpc.mfenx.com/observer-registrations".to_string());
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--output" => {
                output = Some(PathBuf::from(
                    iter.next()
                        .unwrap_or_else(|| fatal("--output expects a value")),
                ))
            }
            "--probe-url" => {
                probe_url = Some(
                    iter.next()
                        .unwrap_or_else(|| fatal("--probe-url expects a value")),
                )
            }
            "--intake-url" => {
                intake_url = Some(
                    iter.next()
                        .unwrap_or_else(|| fatal("--intake-url expects a value")),
                )
            }
            "--no-probe" => probe_url = None,
            "--no-upload" => intake_url = None,
            "--json" => json = true,
            value if !value.starts_with("--") && input.is_none() => {
                input = Some(PathBuf::from(value))
            }
            value => fatal(&format!("unknown argument: {value}")),
        }
    }
    let input = input.unwrap_or_else(|| fatal("submit requires <observer-registration.json>"));
    let registration = read_observer_registration_or_package(&input);
    registration
        .verify(177155, unix_seconds())
        .unwrap_or_else(|err| fatal(&format!("observer registration verification failed: {err}")));
    let probe = probe_url
        .as_deref()
        .and_then(|url| observer_probe_for_registration(url, &registration).ok());
    let admission = intake_url.as_deref().map(|url| {
        observer_intake_request("POST", url, Some(&registration))
            .unwrap_or_else(|err| fatal(&format!("observer registration submission failed: {err}")))
    });
    let package = serde_json::json!({
        "schema": "mfenx-node-registration-submission-v1",
        "created_at_unix": unix_seconds(),
        "registration_type": "observer",
        "client_side_status": "ready",
        "client_side_errors": [],
        "client_side_warnings": [],
        "probe": probe,
        "registration": registration,
        "admission": admission,
    });
    if let Some(path) = output {
        write_json_file(&path, &package, "observer submission package");
        if !json {
            println!("submission package: {}", path.display());
        }
    }
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&package)
                .unwrap_or_else(|err| fatal(&format!("failed to encode package: {err}")))
        );
    } else {
        println!("observer registration verified locally");
        if !package["probe"].is_null() {
            println!("external probe: {}", package["probe"]);
        } else {
            println!("external probe: skipped or unavailable");
        }
        if let Some(admission) = package.get("admission").filter(|value| !value.is_null()) {
            println!(
                "tracking id: {}",
                admission["tracking_id"].as_str().unwrap_or("unavailable")
            );
            println!(
                "admission status: {}",
                admission["status"].as_str().unwrap_or("unknown")
            );
            println!(
                "next: julian observer status {}",
                admission["tracking_id"].as_str().unwrap_or("<tracking-id>")
            );
        } else {
            println!("upload skipped; package is ready for https://mfenx.com/register.html");
        }
    }
}

#[cfg(feature = "net")]
fn cmd_observer_status(args: Vec<String>) {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_observer_help();
        return;
    }
    let mut tracking_id = None;
    let mut intake_url = "https://rpc.mfenx.com/observer-registrations".to_string();
    let mut json = false;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--intake-url" => {
                intake_url = iter
                    .next()
                    .unwrap_or_else(|| fatal("--intake-url expects a value"))
            }
            "--json" => json = true,
            value if !value.starts_with("--") && tracking_id.is_none() => {
                tracking_id = Some(value.to_string())
            }
            value => fatal(&format!("unknown argument: {value}")),
        }
    }
    let tracking_id = tracking_id.unwrap_or_else(|| fatal("status requires <tracking-id>"));
    let url = format!("{}/{}", intake_url.trim_end_matches('/'), tracking_id);
    let status = observer_intake_request::<serde_json::Value>("GET", &url, None)
        .unwrap_or_else(|err| fatal(&format!("observer admission status failed: {err}")));
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&status)
                .unwrap_or_else(|err| fatal(&format!("failed to encode status: {err}")))
        );
    } else {
        println!("tracking id: {tracking_id}");
        println!(
            "admission status: {}",
            status["status"].as_str().unwrap_or("unknown")
        );
        if let Some(revision) = status["registry_revision"].as_str() {
            println!("registry revision: {revision}");
        }
        if let Some(message) = status
            .pointer("/error/message")
            .and_then(|value| value.as_str())
        {
            println!("error: {message}");
        }
    }
}

#[cfg(feature = "net")]
fn parse_observer_options(
    args: Vec<String>,
    value_flags: &[&str],
    switches: &[&str],
) -> (HashMap<String, String>, HashSet<String>) {
    let mut values = HashMap::new();
    let mut present = HashSet::new();
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        if switches.contains(&arg.as_str()) {
            present.insert(arg);
            continue;
        }
        if !value_flags.contains(&arg.as_str()) {
            fatal(&format!("unknown argument: {arg}"));
        }
        let value = iter
            .next()
            .unwrap_or_else(|| fatal(&format!("{arg} expects a value")));
        if values.insert(arg.clone(), value).is_some() {
            fatal(&format!("duplicate argument: {arg}"));
        }
    }
    (values, present)
}

#[cfg(feature = "net")]
fn observer_cli_options(
    values: &HashMap<String, String>,
    switches: &HashSet<String>,
    require_public_host: bool,
) -> ObserverCliOptions {
    let node_id = values
        .get("--node-id")
        .cloned()
        .unwrap_or_else(|| "mynode".to_string());
    let operator = values
        .get("--operator")
        .cloned()
        .unwrap_or_else(|| node_id.clone());
    let region = values
        .get("--region")
        .cloned()
        .unwrap_or_else(|| "self-hosted".to_string());
    let key_spec = values
        .get("--key")
        .cloned()
        .unwrap_or_else(default_node_key_spec);
    let p2p_port = parse_port_option(values.get("--p2p-port"), 7001, "--p2p-port");
    let metrics_port = parse_port_option(values.get("--metrics-port"), 9102, "--metrics-port");
    let public_host = values
        .get("--public-host")
        .map(|host| normalize_public_host(host))
        .or_else(|| detect_public_host().ok());
    if require_public_host && public_host.is_none() {
        fatal("--public-host is required because public IP auto-detection failed");
    }
    let probe_url = if switches.contains("--no-probe") {
        None
    } else {
        Some(
            values
                .get("--probe-url")
                .cloned()
                .unwrap_or_else(|| "https://rpc.mfenx.com/observer-probe".to_string()),
        )
    };
    let output = values
        .get("--output")
        .map(PathBuf::from)
        .unwrap_or_else(|| default_registration_output(&node_id, "observer"));
    ObserverCliOptions {
        node_id,
        operator,
        region,
        key_spec,
        p2p_port,
        metrics_port,
        public_host,
        probe_url,
        output,
        json: switches.contains("--json"),
    }
}

#[cfg(feature = "net")]
fn observer_registration_from_options(options: &ObserverCliOptions) -> ObserverRegistration {
    let mut values = HashMap::new();
    values.insert("--key".to_string(), options.key_spec.clone());
    values.insert("--node-id".to_string(), options.node_id.clone());
    values.insert("--operator".to_string(), options.operator.clone());
    values.insert("--region".to_string(), options.region.clone());
    values.insert(
        "--public-host".to_string(),
        options
            .public_host
            .clone()
            .unwrap_or_else(|| fatal("public host is required")),
    );
    values.insert("--p2p-port".to_string(), options.p2p_port.to_string());
    values.insert(
        "--metrics-port".to_string(),
        options.metrics_port.to_string(),
    );
    build_observer_registration(&values)
}

#[cfg(feature = "net")]
fn diagnose_observer(options: &ObserverCliOptions) -> ObserverDiagnosis {
    let mut checks = Vec::new();
    let local_ip = local_ipv4_guess();
    let key_result = load_or_derive_keypair(&Ed25519KeySource::from_spec(Some(&options.key_spec)));
    let (peer_id, public_key_b64) = match key_result {
        Ok(material) => {
            let public_key = power_house::net::encode_public_key_base64(&material.verifying);
            let peer_id = material.libp2p.public().to_peer_id().to_string();
            checks.push(observer_check(
                "node key",
                "OK",
                format!("loaded identity {peer_id}"),
                None,
            ));
            (Some(peer_id), Some(public_key))
        }
        Err(err) => {
            checks.push(observer_check(
                "node key",
                "FAIL",
                format!("failed to load {}: {err}", options.key_spec),
                Some("run `julian observer setup` to create the default key".to_string()),
            ));
            (None, None)
        }
    };
    checks.push(observer_check(
        "local ip",
        if local_ip.is_some() { "OK" } else { "WARN" },
        local_ip
            .clone()
            .unwrap_or_else(|| "could not infer local LAN IP".to_string()),
        None,
    ));
    checks.push(port_check("p2p port", options.p2p_port));
    checks.push(port_check("metrics port", options.metrics_port));
    match fetch_local_metrics(options.metrics_port) {
        Ok(metrics) => {
            if let (Some(peer_id), Some(public_key)) = (&peer_id, &public_key_b64) {
                if metrics_contains_identity(&metrics, &options.node_id, peer_id, public_key) {
                    checks.push(observer_check(
                        "local metrics identity",
                        "OK",
                        "metrics exposes matching node_id, peer_id, public key, and chain"
                            .to_string(),
                        None,
                    ));
                } else {
                    checks.push(observer_check(
                        "local metrics identity",
                        "FAIL",
                        "metrics endpoint is reachable but does not match this node identity".to_string(),
                        Some("restart the observer with the same --node-id and --key used for registration".to_string()),
                    ));
                }
            } else {
                checks.push(observer_check(
                    "local metrics",
                    "OK",
                    "metrics endpoint is reachable".to_string(),
                    None,
                ));
            }
        }
        Err(err) => checks.push(observer_check(
            "local metrics",
            "WARN",
            format!("not reachable at 127.0.0.1:{}: {err}", options.metrics_port),
            Some(observer_start_command(options)),
        )),
    }
    if let (Some(host), Some(probe_url)) = (&options.public_host, &options.probe_url) {
        match fetch_observer_probe(probe_url, host, options.metrics_port, options.p2p_port) {
            Ok(probe) => add_probe_checks(&mut checks, &probe),
            Err(err) => checks.push(observer_check(
                "external probe",
                "WARN",
                format!("probe unavailable: {err}"),
                Some("verify from phone cellular data or retry with --probe-url".to_string()),
            )),
        }
    } else {
        checks.push(observer_check(
            "external probe",
            "WARN",
            "public host or probe URL unavailable".to_string(),
            Some(
                "pass --public-host <public-ip-or-dns> or run after internet access is available"
                    .to_string(),
            ),
        ));
    }
    ObserverDiagnosis {
        schema: "power-house-observer-doctor-v1".to_string(),
        node_id: options.node_id.clone(),
        key_spec: options.key_spec.clone(),
        local_ip,
        public_host: options.public_host.clone(),
        p2p_port: options.p2p_port,
        metrics_port: options.metrics_port,
        peer_id,
        public_key_b64,
        checks,
    }
}

#[cfg(feature = "net")]
fn observer_check(
    name: impl Into<String>,
    status: impl Into<String>,
    detail: impl Into<String>,
    fix: Option<String>,
) -> ObserverCheck {
    ObserverCheck {
        name: name.into(),
        status: status.into(),
        detail: detail.into(),
        fix,
    }
}

#[cfg(feature = "net")]
fn port_check(name: &str, port: u16) -> ObserverCheck {
    match TcpListener::bind(("0.0.0.0", port)) {
        Ok(listener) => {
            drop(listener);
            observer_check(
                name,
                "OK",
                format!("tcp/{port} is available"),
                None,
            )
        }
        Err(err) => observer_check(
            name,
            "INFO",
            format!("tcp/{port} is already in use: {err}"),
            Some("this is expected when the observer is already running; otherwise choose another port".to_string()),
        ),
    }
}

#[cfg(feature = "net")]
fn print_observer_diagnosis(diagnosis: &ObserverDiagnosis, json: bool) {
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(diagnosis)
                .unwrap_or_else(|err| fatal(&format!("failed to encode diagnosis: {err}")))
        );
        return;
    }
    println!("Power House Observer Doctor");
    println!("node_id: {}", diagnosis.node_id);
    if let Some(host) = &diagnosis.public_host {
        println!("public_host: {host}");
    }
    for check in &diagnosis.checks {
        println!("[{}] {}: {}", check.status, check.name, check.detail);
        if let Some(fix) = &check.fix {
            println!("    fix: {fix}");
        }
    }
}

#[cfg(feature = "net")]
fn ensure_default_node_key(key_spec: &str) -> Option<PathBuf> {
    if key_spec.contains("://") {
        return None;
    }
    let path = Path::new(key_spec);
    if path.exists() {
        return None;
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .unwrap_or_else(|err| fatal(&format!("failed to create {}: {err}", parent.display())));
    }
    let mut seed = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut seed);
    fs::write(path, seed)
        .unwrap_or_else(|err| fatal(&format!("failed to write key {}: {err}", path.display())));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap_or_else(|err| {
            fatal(&format!(
                "failed to set key permissions {}: {err}",
                path.display()
            ))
        });
    }
    Some(path.to_path_buf())
}

#[cfg(feature = "net")]
fn observer_start_command(options: &ObserverCliOptions) -> String {
    let bootstrap_lines = DEFAULT_OBSERVER_BOOTSTRAPS
        .iter()
        .map(|addr| format!("  --bootstrap {} \\", shell_word(addr)))
        .collect::<Vec<_>>()
        .join("\n");
    let bootstrap_section = if bootstrap_lines.is_empty() {
        String::new()
    } else {
        format!("{bootstrap_lines}\n")
    };
    format!(
        "julian net start \\\n  --node-id {} \\\n  --log-dir ./logs/{}-observer \\\n  --blob-dir ./data/{}-observer \\\n  --listen /ip4/0.0.0.0/tcp/{} \\\n{}  --key \"{}\" \\\n  --metrics 0.0.0.0:{}",
        shell_word(&options.node_id),
        options.node_id,
        options.node_id,
        options.p2p_port,
        bootstrap_section,
        options.key_spec,
        options.metrics_port
    )
}

#[cfg(feature = "net")]
fn shell_word(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | '/' | ':'))
    {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
}

#[cfg(feature = "net")]
fn local_ipv4_guess() -> Option<String> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("1.1.1.1:80").ok()?;
    match socket.local_addr().ok()?.ip() {
        IpAddr::V4(value) => Some(value.to_string()),
        IpAddr::V6(_) => None,
    }
}

#[cfg(feature = "net")]
fn detect_public_host() -> Result<String, String> {
    let text = https_get_text("https://api.ipify.org", 4)?;
    Ok(normalize_public_host(text.trim()))
}

#[cfg(feature = "net")]
fn https_get_text(url: &str, timeout_secs: u64) -> Result<String, String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| err.to_string())?;
    runtime.block_on(async {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|err| err.to_string())?;
        let response = client
            .get(url)
            .send()
            .await
            .map_err(|err| err.to_string())?
            .error_for_status()
            .map_err(|err| err.to_string())?;
        response.text().await.map_err(|err| err.to_string())
    })
}

#[cfg(feature = "net")]
fn observer_intake_request<T: Serialize>(
    method: &str,
    url: &str,
    body: Option<&T>,
) -> Result<serde_json::Value, String> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| err.to_string())?;
    runtime.block_on(async {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|err| err.to_string())?;
        let method =
            reqwest::Method::from_bytes(method.as_bytes()).map_err(|err| err.to_string())?;
        let mut request = client.request(method, url);
        if let Some(value) = body {
            request = request.json(value);
        }
        let response = request.send().await.map_err(|err| err.to_string())?;
        let status = response.status();
        let text = response.text().await.map_err(|err| err.to_string())?;
        let value: serde_json::Value =
            serde_json::from_str(&text).map_err(|err| format!("invalid intake response: {err}"))?;
        if !status.is_success() {
            let message = value
                .pointer("/error/message")
                .and_then(|item| item.as_str())
                .unwrap_or("registration intake rejected the request");
            return Err(format!("HTTP {status}: {message}"));
        }
        Ok(value)
    })
}

#[cfg(feature = "net")]
fn fetch_local_metrics(port: u16) -> Result<String, String> {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).map_err(|err| err.to_string())?;
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .map_err(|err| err.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(3)))
        .map_err(|err| err.to_string())?;
    stream
        .write_all(b"GET /metrics HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .map_err(|err| err.to_string())?;
    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .map_err(|err| err.to_string())?;
    if !response.starts_with("HTTP/1.1 200") && !response.starts_with("HTTP/1.0 200") {
        return Err(response.lines().next().unwrap_or("HTTP error").to_string());
    }
    Ok(response
        .split("\r\n\r\n")
        .nth(1)
        .unwrap_or(response.as_str())
        .to_string())
}

#[cfg(feature = "net")]
fn metrics_contains_identity(
    body: &str,
    node_id: &str,
    peer_id: &str,
    public_key_b64: &str,
) -> bool {
    body.lines().any(|line| {
        line.starts_with("powerhouse_node_identity{")
            && line.contains(&format!("node_id=\"{node_id}\""))
            && line.contains(&format!("peer_id=\"{peer_id}\""))
            && line.contains(&format!("public_key_b64=\"{public_key_b64}\""))
            && line.contains("chain_id=\"177155\"")
    })
}

#[cfg(feature = "net")]
fn fetch_observer_probe(
    probe_url: &str,
    host: &str,
    metrics_port: u16,
    p2p_port: u16,
) -> Result<serde_json::Value, String> {
    let separator = if probe_url.contains('?') { '&' } else { '?' };
    let url = format!(
        "{probe_url}{separator}host={}&metrics_port={metrics_port}&p2p_port={p2p_port}",
        percent_encode_query(host)
    );
    let text = https_get_text(&url, 8)?;
    serde_json::from_str(&text).map_err(|err| err.to_string())
}

#[cfg(feature = "net")]
fn percent_encode_query(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_') {
            encoded.push(byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

#[cfg(feature = "net")]
fn add_probe_checks(checks: &mut Vec<ObserverCheck>, probe: &serde_json::Value) {
    let metrics_ok = probe
        .pointer("/metrics/reachable")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let identity_ok = probe
        .get("registration_identity_matches")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let p2p_ok = probe
        .pointer("/p2p/reachable")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    checks.push(observer_check(
        "external metrics",
        if metrics_ok && identity_ok {
            "OK"
        } else {
            "FAIL"
        },
        if metrics_ok && identity_ok {
            "public metrics endpoint is reachable and exposes identity".to_string()
        } else {
            probe
                .pointer("/metrics/error")
                .and_then(|value| value.as_str())
                .unwrap_or("metrics identity not reachable from external probe")
                .to_string()
        },
        if metrics_ok && identity_ok {
            None
        } else {
            Some(
                "forward TCP metrics port to this machine and keep the observer running"
                    .to_string(),
            )
        },
    ));
    checks.push(observer_check(
        "external p2p",
        if p2p_ok { "OK" } else { "FAIL" },
        if p2p_ok {
            "public p2p port accepts TCP connections".to_string()
        } else {
            probe
                .pointer("/p2p/error")
                .and_then(|value| value.as_str())
                .unwrap_or("p2p port not reachable from external probe")
                .to_string()
        },
        if p2p_ok {
            None
        } else {
            Some("forward TCP p2p port to this machine".to_string())
        },
    ));
}

#[cfg(feature = "net")]
fn read_observer_registration_or_package(path: &Path) -> ObserverRegistration {
    let value: serde_json::Value = read_json_file(path, "observer registration package");
    if value.get("schema").and_then(|schema| schema.as_str())
        == Some("power-house-observer-registration-v1")
    {
        return serde_json::from_value(value)
            .unwrap_or_else(|err| fatal(&format!("invalid observer registration: {err}")));
    }
    if let Some(registration) = value.get("registration") {
        return serde_json::from_value(registration.clone()).unwrap_or_else(|err| {
            fatal(&format!("invalid packaged observer registration: {err}"))
        });
    }
    fatal("input must be an observer registration or mfenx submission package")
}

#[cfg(feature = "net")]
fn observer_probe_for_registration(
    probe_url: &str,
    registration: &ObserverRegistration,
) -> Result<serde_json::Value, String> {
    let host = metrics_host_from_url(&registration.metrics_url)?;
    let metrics_port = metrics_port_from_url(&registration.metrics_url).unwrap_or(80);
    let p2p_port = p2p_port_from_multiaddr(&registration.p2p_address).unwrap_or(7001);
    let mut probe = fetch_observer_probe(probe_url, &host, metrics_port, p2p_port)?;
    let identity = probe.pointer("/metrics/identity");
    let identity_matches = identity.is_some_and(|value| {
        value.get("node_id").and_then(|item| item.as_str()) == Some(&registration.node_id)
            && value.get("peer_id").and_then(|item| item.as_str()) == Some(&registration.peer_id)
            && value.get("public_key_b64").and_then(|item| item.as_str())
                == Some(&registration.public_key_b64)
            && value
                .get("chain_id")
                .and_then(|item| item.as_str())
                .and_then(|item| item.parse::<u64>().ok())
                == Some(registration.chain_id)
    });
    probe["registration_identity_matches"] = serde_json::Value::Bool(identity_matches);
    let probe_ok = probe
        .get("ok")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    probe["ok"] = serde_json::Value::Bool(probe_ok && identity_matches);
    Ok(probe)
}

#[cfg(feature = "net")]
fn metrics_host_from_url(url: &str) -> Result<String, String> {
    let parsed = reqwest::Url::parse(url).map_err(|err| err.to_string())?;
    parsed
        .host_str()
        .map(|host| host.to_string())
        .ok_or_else(|| "metrics URL has no host".to_string())
}

#[cfg(feature = "net")]
fn metrics_port_from_url(url: &str) -> Option<u16> {
    reqwest::Url::parse(url)
        .ok()
        .and_then(|parsed| parsed.port_or_known_default())
}

#[cfg(feature = "net")]
fn p2p_port_from_multiaddr(address: &str) -> Option<u16> {
    let mut parts = address.split('/');
    while let Some(part) = parts.next() {
        if part == "tcp" {
            return parts.next().and_then(|port| port.parse().ok());
        }
    }
    None
}

#[cfg(feature = "net")]
fn handle_validator_registry(sub: &str, tail: Vec<String>) {
    match sub {
        "-h" | "--help" => print_validator_registry_help(),
        "register" => cmd_validator_registry_register(tail),
        "create" => cmd_validator_registry_create(tail),
        "assemble" => cmd_validator_registry_assemble(tail),
        "verify" => cmd_validator_registry_verify(tail),
        _ => fatal(&format!("unknown validator-registry subcommand: {sub}")),
    }
}

#[cfg(feature = "net")]
fn cmd_validator_registry_register(args: Vec<String>) {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_validator_registry_help();
        return;
    }
    let values = parse_registry_options(
        args,
        &[
            "--key",
            "--node-id",
            "--operator",
            "--region",
            "--public-host",
            "--host",
            "--p2p-port",
            "--metrics-port",
            "--p2p-address",
            "--metrics-url",
            "--system-metrics-port",
            "--system-metrics-url",
            "--chain-id",
            "--issued-at",
            "--valid-until",
            "--output",
            "--policy",
            "--registry",
            "--registry-output",
        ],
    );
    let registration = build_validator_registration(&values);
    let output = values
        .get("--output")
        .map(PathBuf::from)
        .unwrap_or_else(|| default_registration_output(&registration.node_id, "validator"));
    write_json_file(&output, &registration, "validator registration");
    print_registration_summary(
        "validator",
        &output,
        &registration.node_id,
        &registration.peer_id,
        &registration.p2p_address,
        &registration.metrics_url,
    );

    if values.contains_key("--registry") && !values.contains_key("--registry-output") {
        fatal("--registry requires --registry-output so the assembled registry is explicit");
    }

    if let Some(registry_output) = values.get("--registry-output") {
        let policy = read_validator_policy(Path::new(
            values
                .get("--policy")
                .map(String::as_str)
                .unwrap_or_else(|| fatal("--registry-output requires --policy")),
        ));
        let mut registrations = values
            .get("--registry")
            .map(|path| read_json_file::<ValidatorRegistry>(Path::new(path), "validator registry"))
            .map(|registry| {
                if registry.chain_id != registration.chain_id {
                    fatal("existing registry chain ID differs from the new registration");
                }
                registry.registrations
            })
            .unwrap_or_default();
        registrations.retain(|entry| {
            entry.node_id != registration.node_id
                && entry.public_key_b64 != registration.public_key_b64
                && entry.peer_id != registration.peer_id
        });
        registrations.push(registration.clone());
        let registry = ValidatorRegistry {
            schema: VALIDATOR_REGISTRY_SCHEMA.to_string(),
            chain_id: registration.chain_id,
            registrations,
        };
        registry
            .verify(&policy, unix_seconds())
            .unwrap_or_else(|err| fatal(&format!("validator registry verification failed: {err}")));
        write_json_file(Path::new(registry_output), &registry, "validator registry");
        println!("validator registry: {registry_output}");
        println!(
            "verified validator identities: {}",
            registry.registrations.len()
        );
    } else {
        println!("next: submit the signed registration for policy admission and registry assembly");
    }
}

#[cfg(feature = "net")]
fn cmd_validator_registry_create(args: Vec<String>) {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_validator_registry_help();
        return;
    }
    let mut values = HashMap::new();
    let mut iter = args.into_iter();
    while let Some(flag) = iter.next() {
        let value = iter
            .next()
            .unwrap_or_else(|| fatal(&format!("{flag} expects a value")));
        if !matches!(
            flag.as_str(),
            "--key"
                | "--node-id"
                | "--operator"
                | "--region"
                | "--p2p-address"
                | "--metrics-url"
                | "--system-metrics-url"
                | "--chain-id"
                | "--issued-at"
                | "--valid-until"
                | "--output"
        ) {
            fatal(&format!("unknown argument: {flag}"));
        }
        if values.insert(flag.clone(), value).is_some() {
            fatal(&format!("duplicate argument: {flag}"));
        }
    }

    let now = unix_seconds();
    let chain_id = parse_u64_option(values.get("--chain-id"), 177155, "--chain-id");
    let issued_at = parse_u64_option(values.get("--issued-at"), now, "--issued-at");
    let valid_until = parse_u64_option(
        values.get("--valid-until"),
        issued_at.saturating_add(365 * 24 * 60 * 60),
        "--valid-until",
    );
    let key = required_option(&values, "--key");
    let material = load_or_derive_keypair(&Ed25519KeySource::from_spec(Some(key)))
        .unwrap_or_else(|err| fatal(&format!("failed to load validator key: {err}")));
    let registration = ValidatorRegistration::sign(
        chain_id,
        required_option(&values, "--node-id").to_string(),
        required_option(&values, "--operator").to_string(),
        required_option(&values, "--region").to_string(),
        required_option(&values, "--p2p-address").to_string(),
        required_option(&values, "--metrics-url").to_string(),
        values.get("--system-metrics-url").cloned(),
        issued_at,
        valid_until,
        &material,
    )
    .unwrap_or_else(|err| fatal(&format!("failed to create validator registration: {err}")));
    write_json_file(
        Path::new(required_option(&values, "--output")),
        &registration,
        "validator registration",
    );
}

#[cfg(feature = "net")]
fn cmd_validator_registry_assemble(args: Vec<String>) {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_validator_registry_help();
        return;
    }
    let mut registrations = Vec::new();
    let mut policy = None;
    let mut output = None;
    let mut chain_id = 177155;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--registration" => registrations.push(PathBuf::from(
                iter.next()
                    .unwrap_or_else(|| fatal("--registration expects a value")),
            )),
            "--policy" => {
                policy = Some(PathBuf::from(
                    iter.next()
                        .unwrap_or_else(|| fatal("--policy expects a value")),
                ))
            }
            "--output" => {
                output = Some(PathBuf::from(
                    iter.next()
                        .unwrap_or_else(|| fatal("--output expects a value")),
                ))
            }
            "--chain-id" => {
                chain_id = iter
                    .next()
                    .unwrap_or_else(|| fatal("--chain-id expects a value"))
                    .parse()
                    .unwrap_or_else(|_| fatal("invalid --chain-id"))
            }
            value => fatal(&format!("unknown argument: {value}")),
        }
    }
    if registrations.is_empty() {
        fatal("assemble requires at least one --registration");
    }
    let entries = registrations
        .iter()
        .map(|path| read_json_file::<ValidatorRegistration>(path, "validator registration"))
        .collect();
    let registry = ValidatorRegistry {
        schema: VALIDATOR_REGISTRY_SCHEMA.to_string(),
        chain_id,
        registrations: entries,
    };
    let admitted = read_validator_policy(
        policy
            .as_deref()
            .unwrap_or_else(|| fatal("assemble requires --policy")),
    );
    registry
        .verify(&admitted, unix_seconds())
        .unwrap_or_else(|err| fatal(&format!("validator registry verification failed: {err}")));
    write_json_file(
        output
            .as_deref()
            .unwrap_or_else(|| fatal("assemble requires --output")),
        &registry,
        "validator registry",
    );
}

#[cfg(feature = "net")]
fn cmd_validator_registry_verify(args: Vec<String>) {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_validator_registry_help();
        return;
    }
    let mut registry_path = None;
    let mut policy_path = None;
    let mut now = unix_seconds();
    let mut json = false;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--policy" => {
                policy_path = Some(PathBuf::from(
                    iter.next()
                        .unwrap_or_else(|| fatal("--policy expects a value")),
                ))
            }
            "--now" => {
                now = iter
                    .next()
                    .unwrap_or_else(|| fatal("--now expects a value"))
                    .parse()
                    .unwrap_or_else(|_| fatal("invalid --now"))
            }
            "--json" => json = true,
            value if registry_path.is_none() => registry_path = Some(PathBuf::from(value)),
            value => fatal(&format!("unknown argument: {value}")),
        }
    }
    let registry = read_json_file::<ValidatorRegistry>(
        registry_path
            .as_deref()
            .unwrap_or_else(|| fatal("verify requires a registry path")),
        "validator registry",
    );
    let admitted = read_validator_policy(
        policy_path
            .as_deref()
            .unwrap_or_else(|| fatal("verify requires --policy")),
    );
    registry
        .verify(&admitted, now)
        .unwrap_or_else(|err| fatal(&format!("validator registry verification failed: {err}")));
    if json {
        println!(
            "{}",
            serde_json::json!({
                "chain_id": registry.chain_id,
                "registrations": registry.registrations,
                "schema": registry.schema,
                "validators_verified": registry.registrations.len(),
                "verified": true,
            })
        );
    } else {
        println!(
            "validator registry verified: {} admitted identities on chain {}",
            registry.registrations.len(),
            registry.chain_id
        );
    }
}

#[cfg(feature = "net")]
fn handle_observer_registry(sub: &str, tail: Vec<String>) {
    match sub {
        "-h" | "--help" => print_observer_registry_help(),
        "register" => cmd_observer_registry_register(tail),
        "create" => cmd_observer_registry_create(tail),
        "assemble" => cmd_observer_registry_assemble(tail),
        "verify" => cmd_observer_registry_verify(tail),
        _ => fatal(&format!("unknown observer-registry subcommand: {sub}")),
    }
}

#[cfg(feature = "net")]
fn cmd_observer_registry_register(args: Vec<String>) {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_observer_registry_help();
        return;
    }
    let values = parse_registry_options(
        args,
        &[
            "--key",
            "--node-id",
            "--operator",
            "--region",
            "--public-host",
            "--host",
            "--p2p-port",
            "--metrics-port",
            "--p2p-address",
            "--metrics-url",
            "--system-metrics-port",
            "--system-metrics-url",
            "--chain-id",
            "--issued-at",
            "--valid-until",
            "--output",
            "--registry",
            "--registry-output",
        ],
    );
    let registration = build_observer_registration(&values);
    let output = values
        .get("--output")
        .map(PathBuf::from)
        .unwrap_or_else(|| default_registration_output(&registration.node_id, "observer"));
    write_json_file(&output, &registration, "observer registration");
    print_registration_summary(
        "observer",
        &output,
        &registration.node_id,
        &registration.peer_id,
        &registration.p2p_address,
        &registration.metrics_url,
    );

    if values.contains_key("--registry") && !values.contains_key("--registry-output") {
        fatal("--registry requires --registry-output so the assembled registry is explicit");
    }

    if let Some(registry_output) = values.get("--registry-output") {
        let mut registrations = values
            .get("--registry")
            .map(|path| read_json_file::<ObserverRegistry>(Path::new(path), "observer registry"))
            .map(|registry| {
                if registry.chain_id != registration.chain_id {
                    fatal("existing registry chain ID differs from the new registration");
                }
                registry.registrations
            })
            .unwrap_or_default();
        registrations.retain(|entry| {
            entry.node_id != registration.node_id
                && entry.public_key_b64 != registration.public_key_b64
                && entry.peer_id != registration.peer_id
        });
        registrations.push(registration.clone());
        let registry = ObserverRegistry {
            schema: OBSERVER_REGISTRY_SCHEMA.to_string(),
            chain_id: registration.chain_id,
            registrations,
        };
        registry
            .verify(unix_seconds())
            .unwrap_or_else(|err| fatal(&format!("observer registry verification failed: {err}")));
        write_json_file(Path::new(registry_output), &registry, "observer registry");
        println!("observer registry: {registry_output}");
        println!(
            "verified observer identities: {}",
            registry.registrations.len()
        );
    } else {
        println!("next: upload the signed registration on mfenx.com/register.html or send it for observer registry assembly");
    }
}

#[cfg(feature = "net")]
fn cmd_observer_registry_create(args: Vec<String>) {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_observer_registry_help();
        return;
    }
    let mut values = HashMap::new();
    let mut iter = args.into_iter();
    while let Some(flag) = iter.next() {
        let value = iter
            .next()
            .unwrap_or_else(|| fatal(&format!("{flag} expects a value")));
        if !matches!(
            flag.as_str(),
            "--key"
                | "--node-id"
                | "--operator"
                | "--region"
                | "--p2p-address"
                | "--metrics-url"
                | "--system-metrics-url"
                | "--chain-id"
                | "--issued-at"
                | "--valid-until"
                | "--output"
        ) {
            fatal(&format!("unknown argument: {flag}"));
        }
        if values.insert(flag.clone(), value).is_some() {
            fatal(&format!("duplicate argument: {flag}"));
        }
    }

    let now = unix_seconds();
    let chain_id = parse_u64_option(values.get("--chain-id"), 177155, "--chain-id");
    let issued_at = parse_u64_option(values.get("--issued-at"), now, "--issued-at");
    let valid_until = parse_u64_option(
        values.get("--valid-until"),
        issued_at.saturating_add(365 * 24 * 60 * 60),
        "--valid-until",
    );
    let key = required_option(&values, "--key");
    let material = load_or_derive_keypair(&Ed25519KeySource::from_spec(Some(key)))
        .unwrap_or_else(|err| fatal(&format!("failed to load observer key: {err}")));
    let registration = ObserverRegistration::sign(
        chain_id,
        required_option(&values, "--node-id").to_string(),
        required_option(&values, "--operator").to_string(),
        required_option(&values, "--region").to_string(),
        required_option(&values, "--p2p-address").to_string(),
        required_option(&values, "--metrics-url").to_string(),
        values.get("--system-metrics-url").cloned(),
        issued_at,
        valid_until,
        &material,
    )
    .unwrap_or_else(|err| fatal(&format!("failed to create observer registration: {err}")));
    write_json_file(
        Path::new(required_option(&values, "--output")),
        &registration,
        "observer registration",
    );
}

#[cfg(feature = "net")]
fn cmd_observer_registry_assemble(args: Vec<String>) {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_observer_registry_help();
        return;
    }
    let mut registrations = Vec::new();
    let mut output = None;
    let mut chain_id = 177155;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--registration" => registrations.push(PathBuf::from(
                iter.next()
                    .unwrap_or_else(|| fatal("--registration expects a value")),
            )),
            "--output" => {
                output = Some(PathBuf::from(
                    iter.next()
                        .unwrap_or_else(|| fatal("--output expects a value")),
                ))
            }
            "--chain-id" => {
                chain_id = iter
                    .next()
                    .unwrap_or_else(|| fatal("--chain-id expects a value"))
                    .parse()
                    .unwrap_or_else(|_| fatal("invalid --chain-id"))
            }
            value => fatal(&format!("unknown argument: {value}")),
        }
    }
    if registrations.is_empty() {
        fatal("assemble requires at least one --registration");
    }
    let entries = registrations
        .iter()
        .map(|path| read_json_file::<ObserverRegistration>(path, "observer registration"))
        .collect();
    let registry = ObserverRegistry {
        schema: OBSERVER_REGISTRY_SCHEMA.to_string(),
        chain_id,
        registrations: entries,
    };
    registry
        .verify(unix_seconds())
        .unwrap_or_else(|err| fatal(&format!("observer registry verification failed: {err}")));
    write_json_file(
        output
            .as_deref()
            .unwrap_or_else(|| fatal("assemble requires --output")),
        &registry,
        "observer registry",
    );
}

#[cfg(feature = "net")]
fn cmd_observer_registry_verify(args: Vec<String>) {
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_observer_registry_help();
        return;
    }
    let mut registry_path = None;
    let mut now = unix_seconds();
    let mut json = false;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--now" => {
                now = iter
                    .next()
                    .unwrap_or_else(|| fatal("--now expects a value"))
                    .parse()
                    .unwrap_or_else(|_| fatal("invalid --now"))
            }
            "--json" => json = true,
            value if registry_path.is_none() => registry_path = Some(PathBuf::from(value)),
            value => fatal(&format!("unknown argument: {value}")),
        }
    }
    let registry = read_json_file::<ObserverRegistry>(
        registry_path
            .as_deref()
            .unwrap_or_else(|| fatal("verify requires a registry path")),
        "observer registry",
    );
    registry
        .verify(now)
        .unwrap_or_else(|err| fatal(&format!("observer registry verification failed: {err}")));
    if json {
        println!(
            "{}",
            serde_json::json!({
                "chain_id": registry.chain_id,
                "registrations": registry.registrations,
                "schema": registry.schema,
                "observers_verified": registry.registrations.len(),
                "verified": true,
            })
        );
    } else {
        println!(
            "observer registry verified: {} signed identities on chain {}",
            registry.registrations.len(),
            registry.chain_id
        );
    }
}

#[cfg(feature = "net")]
fn parse_registry_options(args: Vec<String>, allowed: &[&str]) -> HashMap<String, String> {
    let mut values = HashMap::new();
    let mut iter = args.into_iter();
    while let Some(flag) = iter.next() {
        if !flag.starts_with("--") {
            fatal(&format!("unexpected positional argument: {flag}"));
        }
        if !allowed.contains(&flag.as_str()) {
            fatal(&format!("unknown argument: {flag}"));
        }
        let value = iter
            .next()
            .unwrap_or_else(|| fatal(&format!("{flag} expects a value")));
        if value.starts_with("--") {
            fatal(&format!("{flag} expects a value"));
        }
        if values.insert(flag.clone(), value).is_some() {
            fatal(&format!("duplicate argument: {flag}"));
        }
    }
    values
}

#[cfg(feature = "net")]
fn build_validator_registration(values: &HashMap<String, String>) -> ValidatorRegistration {
    let input = registration_input(values, "validator");
    ValidatorRegistration::sign(
        input.chain_id,
        input.node_id,
        input.operator,
        input.region,
        input.p2p_address,
        input.metrics_url,
        input.system_metrics_url,
        input.issued_at,
        input.valid_until,
        &input.material,
    )
    .unwrap_or_else(|err| fatal(&format!("failed to create validator registration: {err}")))
}

#[cfg(feature = "net")]
fn build_observer_registration(values: &HashMap<String, String>) -> ObserverRegistration {
    let input = registration_input(values, "observer");
    ObserverRegistration::sign(
        input.chain_id,
        input.node_id,
        input.operator,
        input.region,
        input.p2p_address,
        input.metrics_url,
        input.system_metrics_url,
        input.issued_at,
        input.valid_until,
        &input.material,
    )
    .unwrap_or_else(|err| fatal(&format!("failed to create observer registration: {err}")))
}

#[cfg(feature = "net")]
struct RegistrationInput {
    chain_id: u64,
    node_id: String,
    operator: String,
    region: String,
    p2p_address: String,
    metrics_url: String,
    system_metrics_url: Option<String>,
    issued_at: u64,
    valid_until: u64,
    material: power_house::net::KeyMaterial,
}

#[cfg(feature = "net")]
fn registration_input(values: &HashMap<String, String>, label: &str) -> RegistrationInput {
    let now = unix_seconds();
    let chain_id = parse_u64_option(values.get("--chain-id"), 177155, "--chain-id");
    let issued_at = parse_u64_option(values.get("--issued-at"), now, "--issued-at");
    let valid_until = parse_u64_option(
        values.get("--valid-until"),
        issued_at.saturating_add(365 * 24 * 60 * 60),
        "--valid-until",
    );
    let node_id = required_option(values, "--node-id").to_string();
    let operator = values
        .get("--operator")
        .cloned()
        .unwrap_or_else(|| node_id.clone());
    let region = values
        .get("--region")
        .cloned()
        .unwrap_or_else(|| "self-hosted".to_string());
    let key_spec = values
        .get("--key")
        .cloned()
        .unwrap_or_else(default_node_key_spec);
    let material = load_or_derive_keypair(&Ed25519KeySource::from_spec(Some(&key_spec)))
        .unwrap_or_else(|err| fatal(&format!("failed to load {label} key {key_spec}: {err}")));
    let peer_id = material.libp2p.public().to_peer_id().to_string();

    let public_host = registry_public_host(values);
    let p2p_address = values.get("--p2p-address").cloned().unwrap_or_else(|| {
        let host = public_host
            .clone()
            .unwrap_or_else(|| fatal("--public-host is required unless --p2p-address is provided"));
        build_p2p_address(
            &host,
            parse_port_option(values.get("--p2p-port"), 7001, "--p2p-port"),
            &peer_id,
        )
    });
    let metrics_url = values.get("--metrics-url").cloned().unwrap_or_else(|| {
        let host = public_host
            .as_deref()
            .unwrap_or_else(|| fatal("--public-host is required unless --metrics-url is provided"));
        build_metrics_url(
            host,
            parse_port_option(values.get("--metrics-port"), 9100, "--metrics-port"),
        )
    });
    let system_metrics_url = values.get("--system-metrics-url").cloned().or_else(|| {
        values.get("--system-metrics-port").map(|_| {
            let host = public_host.as_deref().unwrap_or_else(|| {
                fatal("--public-host is required when --system-metrics-port is used")
            });
            build_metrics_url(
                host,
                parse_port_option(
                    values.get("--system-metrics-port"),
                    9101,
                    "--system-metrics-port",
                ),
            )
        })
    });

    RegistrationInput {
        chain_id,
        node_id,
        operator,
        region,
        p2p_address,
        metrics_url,
        system_metrics_url,
        issued_at,
        valid_until,
        material,
    }
}

#[cfg(feature = "net")]
fn registry_public_host(values: &HashMap<String, String>) -> Option<String> {
    values
        .get("--public-host")
        .or_else(|| values.get("--host"))
        .map(|host| normalize_public_host(host))
}

#[cfg(feature = "net")]
fn default_node_key_spec() -> String {
    env::var("POWERHOUSE_NODE_KEY").unwrap_or_else(|_| {
        env::var("HOME")
            .map(|home| format!("{home}/.powerhouse/node.key"))
            .unwrap_or_else(|_| "$HOME/.powerhouse/node.key".to_string())
    })
}

#[cfg(feature = "net")]
fn default_registration_output(node_id: &str, kind: &str) -> PathBuf {
    PathBuf::from(format!("{node_id}.{kind}.registration.json"))
}

#[cfg(feature = "net")]
fn parse_port_option(value: Option<&String>, default: u16, name: &str) -> u16 {
    value.map_or(default, |raw| {
        let port: u16 = raw
            .parse()
            .unwrap_or_else(|_| fatal(&format!("invalid {name}")));
        if port == 0 {
            fatal(&format!("{name} must be between 1 and 65535"));
        }
        port
    })
}

#[cfg(feature = "net")]
fn normalize_public_host(host: &str) -> String {
    let trimmed = host
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .trim_end_matches('.');
    if trimmed.is_empty() {
        fatal("--public-host cannot be empty");
    }
    if trimmed.contains("://") || trimmed.contains('/') {
        fatal("--public-host must be a bare DNS name, IPv4 address, or IPv6 address");
    }
    if trimmed.parse::<Ipv4Addr>().is_ok() || trimmed.parse::<Ipv6Addr>().is_ok() {
        trimmed.to_string()
    } else {
        trimmed.to_ascii_lowercase()
    }
}

#[cfg(feature = "net")]
fn build_p2p_address(host: &str, port: u16, peer_id: &str) -> String {
    let protocol = if host.parse::<Ipv4Addr>().is_ok() {
        format!("/ip4/{host}")
    } else if host.parse::<Ipv6Addr>().is_ok() {
        format!("/ip6/{host}")
    } else {
        format!("/dns4/{host}")
    };
    format!("{protocol}/tcp/{port}/p2p/{peer_id}")
}

#[cfg(feature = "net")]
fn build_metrics_url(host: &str, port: u16) -> String {
    let host_for_url = if host.parse::<Ipv6Addr>().is_ok() {
        format!("[{host}]")
    } else {
        host.to_string()
    };
    format!("http://{host_for_url}:{port}/metrics")
}

#[cfg(feature = "net")]
fn print_registration_summary(
    kind: &str,
    output: &Path,
    node_id: &str,
    peer_id: &str,
    p2p_address: &str,
    metrics_url: &str,
) {
    println!("{kind} registration: {}", output.display());
    println!("node_id: {node_id}");
    println!("peer_id: {peer_id}");
    println!("p2p_address: {p2p_address}");
    println!("metrics_url: {metrics_url}");
}

#[cfg(feature = "net")]
fn required_option<'a>(values: &'a HashMap<String, String>, key: &str) -> &'a str {
    values
        .get(key)
        .map(String::as_str)
        .unwrap_or_else(|| fatal(&format!("{key} is required")))
}

#[cfg(feature = "net")]
fn parse_u64_option(value: Option<&String>, default: u64, name: &str) -> u64 {
    value.map_or(default, |raw| {
        raw.parse()
            .unwrap_or_else(|_| fatal(&format!("invalid {name}")))
    })
}

#[cfg(feature = "net")]
fn unix_seconds() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(feature = "net")]
fn read_json_file<T: serde::de::DeserializeOwned>(path: &Path, label: &str) -> T {
    let contents = fs::read_to_string(path)
        .unwrap_or_else(|err| fatal(&format!("failed to read {label} {}: {err}", path.display())));
    serde_json::from_str(&contents)
        .unwrap_or_else(|err| fatal(&format!("invalid {label} {}: {err}", path.display())))
}

#[cfg(feature = "net")]
fn write_json_file<T: serde::Serialize>(path: &Path, value: &T, label: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap_or_else(|err| {
            fatal(&format!(
                "failed to create {} directory {}: {err}",
                label,
                parent.display()
            ))
        });
    }
    let bytes = serde_json::to_vec_pretty(value)
        .unwrap_or_else(|err| fatal(&format!("failed to encode {label}: {err}")));
    fs::write(path, [bytes, b"\n".to_vec()].concat()).unwrap_or_else(|err| {
        fatal(&format!(
            "failed to write {label} {}: {err}",
            path.display()
        ))
    });
}

#[cfg(feature = "net")]
fn read_validator_policy(path: &Path) -> std::collections::HashSet<String> {
    #[derive(Deserialize)]
    struct ValidatorPolicy {
        allowlist: Vec<String>,
    }
    let policy: ValidatorPolicy = read_json_file(path, "validator policy");
    if policy.allowlist.is_empty() {
        fatal("validator policy allowlist cannot be empty");
    }
    policy.allowlist.into_iter().collect()
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
fn cmd_migration_finalize(args: Vec<String>) {
    if args.iter().any(|a| a == "-h" || a == "--help") {
        print_migration_help();
        return;
    }

    let mut registry_path: Option<String> = None;
    let mut snapshot_height: Option<u64> = None;
    let mut log_dir: Option<String> = None;
    let mut output_dir: Option<String> = None;
    let mut token_contract = String::from("native://julian");
    let mut conversion_ratio: u64 = 1;
    let mut treasury_mint: u64 = 0;
    let mut amount_source = String::from("total");
    let mut include_slashed = false;
    let mut claim_id_salt = String::from("mfenx-migration-claim-v1");
    let mut node_id = String::from("migration-finalize");
    let mut quorum: usize = 1;
    let mut apply_state_path: Option<String> = None;
    let mut allow_unfrozen = false;
    let mut force = false;

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
                snapshot_height = Some(raw.parse().unwrap_or_else(|_| fatal("invalid --height")));
            }
            "--log-dir" => {
                log_dir = Some(
                    iter.next()
                        .unwrap_or_else(|| fatal("--log-dir expects a value")),
                );
            }
            "--output-dir" => {
                output_dir = Some(
                    iter.next()
                        .unwrap_or_else(|| fatal("--output-dir expects a value")),
                );
            }
            "--token-contract" => {
                token_contract = iter
                    .next()
                    .unwrap_or_else(|| fatal("--token-contract expects a value"));
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
            "--amount-source" => {
                amount_source = iter
                    .next()
                    .unwrap_or_else(|| fatal("--amount-source expects a value"));
            }
            "--include-slashed" => {
                include_slashed = true;
            }
            "--claim-id-salt" => {
                claim_id_salt = iter
                    .next()
                    .unwrap_or_else(|| fatal("--claim-id-salt expects a value"));
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
            "--apply-state" => {
                apply_state_path = Some(
                    iter.next()
                        .unwrap_or_else(|| fatal("--apply-state expects a value")),
                );
            }
            "--allow-unfrozen" => {
                allow_unfrozen = true;
            }
            "--force" => {
                force = true;
            }
            other => fatal(&format!("unknown argument: {other}")),
        }
    }

    let opts = FinalizeMigrationOptions {
        registry_path: registry_path.unwrap_or_else(|| fatal("--registry is required")),
        snapshot_height: snapshot_height.unwrap_or_else(|| fatal("--height is required")),
        log_dir: log_dir.unwrap_or_else(|| fatal("--log-dir is required")),
        output_dir: output_dir.unwrap_or_else(|| fatal("--output-dir is required")),
        token_contract,
        conversion_ratio,
        treasury_mint,
        amount_source,
        include_slashed,
        claim_id_salt,
        node_id,
        quorum,
        apply_state_path,
        allow_unfrozen,
        force,
    };

    let summary = run_finalize_migration(&opts)
        .unwrap_or_else(|err| fatal(&format!("migration finalize failed: {err}")));
    println!("snapshot_root: {}", summary.snapshot_root);
    println!("claims_root: {}", summary.claims_root);
    println!("applied_claims: {}", summary.applied_claims);
    println!("skipped_claims: {}", summary.skipped_claims);
    println!("snapshot: {}", summary.snapshot_path);
    println!("claims: {}", summary.claims_path);
    println!("apply_state: {}", summary.apply_state_path);
    println!("proposal: {}", summary.proposal_path);
}

#[cfg(feature = "net")]
fn cmd_migration_verify_state(args: Vec<String>) {
    if args.iter().any(|a| a == "-h" || a == "--help") {
        print_migration_help();
        return;
    }

    let mut registry: Option<String> = None;
    let mut claims: Option<String> = None;
    let mut state: Option<String> = None;
    let mut require_complete = false;
    let mut enforce_balance_floor = true;

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
                state = Some(
                    iter.next()
                        .unwrap_or_else(|| fatal("--state expects a value")),
                );
            }
            "--require-complete" => {
                require_complete = true;
            }
            "--skip-balance-floor" => {
                enforce_balance_floor = false;
            }
            other => fatal(&format!("unknown argument: {other}")),
        }
    }

    let summary = run_verify_state(
        &registry.unwrap_or_else(|| fatal("--registry is required")),
        &claims.unwrap_or_else(|| fatal("--claims is required")),
        &state.unwrap_or_else(|| fatal("--state is required")),
        &VerifyStateOptions {
            require_complete,
            enforce_balance_floor,
        },
    )
    .unwrap_or_else(|err| fatal(&format!("migration verify-state failed: {err}")));

    println!("claim_count: {}", summary.claim_count);
    println!("applied_count: {}", summary.applied_count);
    println!("missing_count: {}", summary.missing_count);
    println!("unknown_count: {}", summary.unknown_count);
    println!("applied_total_mint: {}", summary.applied_total_mint);
}

#[cfg(feature = "net")]
fn cmd_migration_execute_burn_intents(args: Vec<String>) {
    if args.iter().any(|a| a == "-h" || a == "--help") {
        print_migration_help();
        return;
    }

    let mut registry: Option<String> = None;
    let mut outbox: Option<String> = None;
    let mut state: Option<String> = None;
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
            "--outbox" => {
                outbox = Some(
                    iter.next()
                        .unwrap_or_else(|| fatal("--outbox expects a value")),
                );
            }
            "--state" => {
                state = Some(
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
    let outbox = outbox.unwrap_or_else(|| {
        let parent = Path::new(&registry)
            .parent()
            .unwrap_or_else(|| Path::new("."));
        parent.join("token_burn_outbox.jsonl").display().to_string()
    });

    let summary = run_execute_burn_intents(
        &registry,
        &outbox,
        &ExecuteBurnOptions {
            state_path: state,
            dry_run,
        },
    )
    .unwrap_or_else(|err| fatal(&format!("migration execute-burn-intents failed: {err}")));

    println!("processed: {}", summary.processed);
    println!("skipped: {}", summary.skipped);
    println!("native_executed: {}", summary.native_executed);
    println!("unsupported_mode: {}", summary.unsupported_mode);
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

    let encoded = run_propose_migration(&ProposeMigrationOptions {
        snapshot_height,
        token_contract,
        conversion_ratio,
        treasury_mint,
        log_dir,
        node_id,
        quorum,
        output: output.clone(),
    })
    .unwrap_or_else(|err| fatal(&format!("propose-migration failed: {err}")));

    if let Some(path) = output {
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
        print_net_start_help();
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
    let mut evm_rpc_listen_spec: Option<String> = None;
    let mut evm_chain_id_spec: Option<String> = None;

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
            "--evm-rpc-listen" => {
                evm_rpc_listen_spec = Some(
                    iter.next()
                        .unwrap_or_else(|| fatal("--evm-rpc-listen expects a value")),
                );
            }
            "--evm-chain-id" => {
                evm_chain_id_spec = Some(
                    iter.next()
                        .unwrap_or_else(|| fatal("--evm-chain-id expects a value")),
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
    let evm_rpc_listen = evm_rpc_listen_spec
        .as_deref()
        .map(parse_metrics_addr)
        .unwrap_or(None);
    let evm_chain_id = evm_chain_id_spec.map(|v| {
        v.parse::<u64>()
            .unwrap_or_else(|_| fatal("invalid --evm-chain-id"))
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
        evm_rpc_listen,
        evm_chain_id,
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
