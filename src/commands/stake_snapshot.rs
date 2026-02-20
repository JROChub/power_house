#![cfg(feature = "net")]

use crate::net::{AnchorJson, StakeRegistry};
use crate::{
    compute_fold_digest, julian_genesis_anchor, merkle_root, AnchorMetadata, EntryAnchor,
    LedgerAnchor,
};
use blake2::digest::{consts::U32, Digest};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// One deterministic stake record included in a migration snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StakeSnapshotEntry {
    /// Base64 ed25519 public key.
    pub pubkey_b64: String,
    /// Spendable balance from the stake registry.
    pub balance: u64,
    /// Bonded stake from the stake registry.
    pub stake: u64,
    /// Whether this account is slashed.
    pub slashed: bool,
    /// BLAKE2b-256 hash over the canonical entry payload.
    pub leaf_hash: String,
}

/// Persisted migration snapshot artifact.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StakeSnapshotArtifact {
    /// Snapshot height selected by governance.
    pub snapshot_height: u64,
    /// Source registry path.
    pub registry_path: String,
    /// Millisecond timestamp used while producing the artifact.
    pub generated_at_ms: u64,
    /// Merkle root over canonical entry leaves.
    pub merkle_root: String,
    /// Deterministically ordered snapshot entries.
    pub entries: Vec<StakeSnapshotEntry>,
    /// Anchor JSON generated using existing net anchor schema.
    pub migration_anchor: AnchorJson,
}

type Blake2b256 = blake2::Blake2b<U32>;

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn leaf_digest(height: u64, pk_b64: &str, balance: u64, stake: u64, slashed: bool) -> [u8; 32] {
    let mut hasher = Blake2b256::new();
    hasher.update(b"migration-snapshot-entry-v1");
    hasher.update(height.to_be_bytes());
    hasher.update(pk_b64.as_bytes());
    hasher.update([0u8]);
    hasher.update(balance.to_be_bytes());
    hasher.update(stake.to_be_bytes());
    hasher.update([u8::from(slashed)]);
    hasher.finalize().into()
}

/// Build a deterministic stake snapshot artifact and return its Merkle root.
///
/// The artifact is anchored using the same `AnchorJson::from_ledger` flow used by
/// `julian net anchor`, and persisted to `output`.
pub fn run_snapshot(registry_path: &str, height: u64, output: &str) -> Result<String, String> {
    let registry = StakeRegistry::load(Path::new(registry_path))?;

    let mut ordered = registry
        .accounts()
        .iter()
        .map(|(pk, acct)| (pk.clone(), acct.clone()))
        .collect::<Vec<_>>();
    ordered.sort_by(|a, b| a.0.cmp(&b.0));

    let mut leaves = Vec::with_capacity(ordered.len());
    let mut entries = Vec::with_capacity(ordered.len());

    for (pk, acct) in ordered {
        let digest = leaf_digest(height, &pk, acct.balance, acct.stake, acct.slashed);
        leaves.push(digest);
        entries.push(StakeSnapshotEntry {
            pubkey_b64: pk,
            balance: acct.balance,
            stake: acct.stake,
            slashed: acct.slashed,
            leaf_hash: hex::encode(digest),
        });
    }

    let merkle = merkle_root(&leaves);
    let statement = format!("migration.snapshot.height.{height}");
    let ledger_entry = EntryAnchor {
        statement,
        merkle_root: merkle,
        hashes: leaves,
    };

    let mut entries_for_anchor = julian_genesis_anchor().entries;
    entries_for_anchor.push(ledger_entry);
    let mut ledger = LedgerAnchor {
        entries: entries_for_anchor,
        metadata: AnchorMetadata {
            challenge_mode: Some("migration".to_string()),
            fold_digest: None,
            crate_version: Some(env!("CARGO_PKG_VERSION").to_string()),
        },
    };
    ledger.metadata.fold_digest = Some(compute_fold_digest(&ledger));

    let node_id = std::env::var("PH_MIGRATION_NODE_ID")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| "migration-snapshot".to_string());
    let quorum = std::env::var("PH_MIGRATION_QUORUM")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(1);

    let migration_anchor =
        AnchorJson::from_ledger(node_id, quorum, &ledger, now_millis(), Vec::new(), None)
            .map_err(|e| format!("failed to anchor snapshot: {e}"))?;

    let artifact = StakeSnapshotArtifact {
        snapshot_height: height,
        registry_path: registry_path.to_string(),
        generated_at_ms: now_millis(),
        merkle_root: hex::encode(merkle),
        entries,
        migration_anchor,
    };

    let bytes = serde_json::to_vec_pretty(&artifact)
        .map_err(|e| format!("failed to encode snapshot artifact: {e}"))?;
    let output_path = Path::new(output);
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create {}: {e}", parent.display()))?;
    }
    std::fs::write(output_path, bytes)
        .map_err(|e| format!("failed to write {}: {e}", output_path.display()))?;

    Ok(hex::encode(merkle))
}

#[cfg(test)]
mod tests {
    use super::run_snapshot;
    use serde_json::json;
    use std::fs;

    fn temp_path(name: &str) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        p.push(format!("{name}_{ts}"));
        p
    }

    #[test]
    fn snapshot_root_is_deterministic_for_registry_key_order() {
        let reg_a = temp_path("reg_a.json");
        let reg_b = temp_path("reg_b.json");
        let out_a = temp_path("snap_a.json");
        let out_b = temp_path("snap_b.json");

        let payload_a = json!({
            "accounts": {
                "zKey": {"balance": 5, "stake": 7, "slashed": false},
                "aKey": {"balance": 9, "stake": 3, "slashed": true}
            }
        });
        let payload_b = json!({
            "accounts": {
                "aKey": {"balance": 9, "stake": 3, "slashed": true},
                "zKey": {"balance": 5, "stake": 7, "slashed": false}
            }
        });

        fs::write(&reg_a, serde_json::to_vec(&payload_a).unwrap()).unwrap();
        fs::write(&reg_b, serde_json::to_vec(&payload_b).unwrap()).unwrap();

        let root_a = run_snapshot(reg_a.to_str().unwrap(), 42, out_a.to_str().unwrap()).unwrap();
        let root_b = run_snapshot(reg_b.to_str().unwrap(), 42, out_b.to_str().unwrap()).unwrap();

        assert_eq!(root_a, root_b);

        let _ = fs::remove_file(reg_a);
        let _ = fs::remove_file(reg_b);
        let _ = fs::remove_file(out_a);
        let _ = fs::remove_file(out_b);
    }
}
