#![cfg(feature = "net")]

use crate::net::StakeRegistry;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const APPLY_STATE_SCHEMA: &str = "mfenx.powerhouse.migration-apply-state.v1";

/// Options for applying native migration claims into the stake registry.
#[derive(Debug, Clone)]
pub struct ApplyClaimsOptions {
    /// Optional path to the apply-state file that tracks idempotency.
    pub state_path: Option<String>,
    /// Dry-run mode computes the result without mutating registry/state files.
    pub dry_run: bool,
}

/// Summary returned after claim application.
#[derive(Debug, Clone)]
pub struct ApplyClaimsSummary {
    /// Number of claims applied during this run.
    pub applied: usize,
    /// Number of claims skipped because they were already applied.
    pub skipped: usize,
    /// Aggregate minted amount for newly-applied claims.
    pub total_mint_amount: String,
    /// Resolved state file path.
    pub state_path: String,
}

#[derive(Debug, Deserialize)]
struct ClaimsArtifact {
    claim_mode: String,
    claims: Vec<ClaimEntry>,
}

#[derive(Debug, Deserialize)]
struct ClaimEntry {
    pubkey_b64: String,
    account: String,
    claim_id: String,
    mint_amount: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ApplyState {
    schema: String,
    updated_at_ms: u64,
    applied_claim_ids: Vec<String>,
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn resolve_state_path(registry_path: &Path, explicit: Option<&str>) -> PathBuf {
    if let Some(path) = explicit {
        return PathBuf::from(path);
    }
    let default_name = registry_path
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|stem| format!("{stem}.migration_apply_state.json"))
        .unwrap_or_else(|| "migration_apply_state.json".to_string());
    registry_path.with_file_name(default_name)
}

fn load_apply_state(path: &Path) -> Result<ApplyState, String> {
    if !path.exists() {
        return Ok(ApplyState {
            schema: APPLY_STATE_SCHEMA.to_string(),
            updated_at_ms: now_millis(),
            applied_claim_ids: Vec::new(),
        });
    }
    let bytes = std::fs::read(path)
        .map_err(|err| format!("failed to read apply state {}: {err}", path.display()))?;
    let mut state: ApplyState = serde_json::from_slice(&bytes)
        .map_err(|err| format!("invalid apply state {}: {err}", path.display()))?;
    if state.schema.trim().is_empty() {
        state.schema = APPLY_STATE_SCHEMA.to_string();
    }
    Ok(state)
}

fn save_apply_state(path: &Path, state: &ApplyState) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
    }
    let bytes = serde_json::to_vec_pretty(state)
        .map_err(|err| format!("failed to encode apply state: {err}"))?;
    std::fs::write(path, bytes)
        .map_err(|err| format!("failed to write apply state {}: {err}", path.display()))
}

/// Applies native claim artifacts into the stake registry with idempotent state tracking.
///
/// Only artifacts with `claim_mode == "native"` are accepted.
pub fn run_apply_claims(
    registry_path: &str,
    claims_path: &str,
    opts: &ApplyClaimsOptions,
) -> Result<ApplyClaimsSummary, String> {
    let registry_path = Path::new(registry_path);
    let claims_path = Path::new(claims_path);
    let state_path = resolve_state_path(registry_path, opts.state_path.as_deref());

    let claims_bytes = std::fs::read(claims_path)
        .map_err(|err| format!("failed to read claims {}: {err}", claims_path.display()))?;
    let artifact: ClaimsArtifact = serde_json::from_slice(&claims_bytes)
        .map_err(|err| format!("invalid claims artifact {}: {err}", claims_path.display()))?;

    if !artifact.claim_mode.eq_ignore_ascii_case("native") {
        return Err(format!(
            "claims artifact mode '{}' is not supported for native apply (expected 'native')",
            artifact.claim_mode
        ));
    }

    let mut state = load_apply_state(&state_path)?;
    let mut applied_set = state
        .applied_claim_ids
        .iter()
        .cloned()
        .collect::<HashSet<String>>();

    let mut registry = StakeRegistry::load(registry_path)?;

    let mut applied = 0usize;
    let mut skipped = 0usize;
    let mut total_mint_amount: u128 = 0;

    for claim in artifact.claims {
        if claim.account != claim.pubkey_b64 {
            return Err(format!(
                "native claim account mismatch for claim_id {} (account='{}', pubkey='{}')",
                claim.claim_id, claim.account, claim.pubkey_b64
            ));
        }

        let mint_amount = claim
            .mint_amount
            .parse::<u128>()
            .map_err(|err| format!("invalid mint_amount for claim {}: {err}", claim.claim_id))?;
        if mint_amount > u64::MAX as u128 {
            return Err(format!(
                "mint_amount overflow for claim {}: {} > u64::MAX",
                claim.claim_id, mint_amount
            ));
        }

        if !applied_set.insert(claim.claim_id.clone()) {
            skipped += 1;
            continue;
        }

        registry.fund_balance(&claim.pubkey_b64, mint_amount as u64);
        applied += 1;
        total_mint_amount = total_mint_amount.saturating_add(mint_amount);
    }

    if !opts.dry_run {
        registry.save(registry_path)?;
        let mut applied_claim_ids = applied_set.into_iter().collect::<Vec<_>>();
        applied_claim_ids.sort();
        state.schema = APPLY_STATE_SCHEMA.to_string();
        state.updated_at_ms = now_millis();
        state.applied_claim_ids = applied_claim_ids;
        save_apply_state(&state_path, &state)?;
    }

    Ok(ApplyClaimsSummary {
        applied,
        skipped,
        total_mint_amount: total_mint_amount.to_string(),
        state_path: state_path.display().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::{run_apply_claims, ApplyClaimsOptions};
    use crate::net::StakeRegistry;
    use serde_json::json;
    use std::fs;

    fn temp_path(name: &str) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        p.push(format!("{name}_{ts}"));
        p
    }

    #[test]
    fn apply_native_claims_is_idempotent() {
        let registry = temp_path("registry_apply_native.json");
        let claims = temp_path("claims_apply_native.json");
        let state = temp_path("apply_state_native.json");

        let registry_payload = json!({
            "accounts": {
                "aKey": {"balance": 1, "stake": 0, "slashed": false}
            }
        });
        fs::write(&registry, serde_json::to_vec(&registry_payload).unwrap()).unwrap();

        let claims_payload = json!({
            "claim_mode": "native",
            "claims": [
                {
                    "pubkey_b64": "aKey",
                    "account": "aKey",
                    "claim_id": "c1",
                    "mint_amount": "10"
                },
                {
                    "pubkey_b64": "bKey",
                    "account": "bKey",
                    "claim_id": "c2",
                    "mint_amount": "20"
                }
            ]
        });
        fs::write(&claims, serde_json::to_vec(&claims_payload).unwrap()).unwrap();

        let opts = ApplyClaimsOptions {
            state_path: Some(state.display().to_string()),
            dry_run: false,
        };

        let first =
            run_apply_claims(registry.to_str().unwrap(), claims.to_str().unwrap(), &opts).unwrap();
        assert_eq!(first.applied, 2);
        assert_eq!(first.skipped, 0);
        assert_eq!(first.total_mint_amount, "30");

        let reg = StakeRegistry::load(&registry).unwrap();
        assert_eq!(reg.account("aKey").unwrap().balance, 11);
        assert_eq!(reg.account("bKey").unwrap().balance, 20);

        let second =
            run_apply_claims(registry.to_str().unwrap(), claims.to_str().unwrap(), &opts).unwrap();
        assert_eq!(second.applied, 0);
        assert_eq!(second.skipped, 2);
        assert_eq!(second.total_mint_amount, "0");

        let reg_after = StakeRegistry::load(&registry).unwrap();
        assert_eq!(reg_after.account("aKey").unwrap().balance, 11);
        assert_eq!(reg_after.account("bKey").unwrap().balance, 20);

        let _ = fs::remove_file(registry);
        let _ = fs::remove_file(claims);
        let _ = fs::remove_file(state);
    }

    #[test]
    fn reject_non_native_claims() {
        let registry = temp_path("registry_apply_erc20.json");
        let claims = temp_path("claims_apply_erc20.json");

        let registry_payload = json!({"accounts": {}});
        fs::write(&registry, serde_json::to_vec(&registry_payload).unwrap()).unwrap();

        let claims_payload = json!({
            "claim_mode": "erc20",
            "claims": []
        });
        fs::write(&claims, serde_json::to_vec(&claims_payload).unwrap()).unwrap();

        let opts = ApplyClaimsOptions {
            state_path: None,
            dry_run: false,
        };
        let err = run_apply_claims(registry.to_str().unwrap(), claims.to_str().unwrap(), &opts)
            .err()
            .unwrap();
        assert!(err.contains("expected 'native'"));

        let _ = fs::remove_file(registry);
        let _ = fs::remove_file(claims);
    }
}
