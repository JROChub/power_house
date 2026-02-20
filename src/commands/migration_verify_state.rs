#![cfg(feature = "net")]

use crate::net::StakeRegistry;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Options for validating native migration claims/state consistency.
#[derive(Debug, Clone)]
pub struct VerifyStateOptions {
    /// Require all claims in the artifact to appear in apply-state.
    pub require_complete: bool,
    /// Require registry balances to be at least minted totals per account.
    pub enforce_balance_floor: bool,
}

/// Verification result summary for migration state.
#[derive(Debug, Clone)]
pub struct VerifyStateSummary {
    /// Total claims in artifact.
    pub claim_count: usize,
    /// Applied claims discovered in state file.
    pub applied_count: usize,
    /// Claim IDs present in artifact but missing from state.
    pub missing_count: usize,
    /// Claim IDs present in state but not in artifact.
    pub unknown_count: usize,
    /// Total minted amount represented by applied claims.
    pub applied_total_mint: String,
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

#[derive(Debug, Deserialize)]
struct ApplyState {
    applied_claim_ids: Vec<String>,
}

/// Verify migration claims/state consistency and optional registry balance floors.
pub fn run_verify_state(
    registry_path: &str,
    claims_path: &str,
    state_path: &str,
    opts: &VerifyStateOptions,
) -> Result<VerifyStateSummary, String> {
    let claims_bytes = std::fs::read(Path::new(claims_path))
        .map_err(|err| format!("failed to read claims {claims_path}: {err}"))?;
    let claims: ClaimsArtifact = serde_json::from_slice(&claims_bytes)
        .map_err(|err| format!("invalid claims artifact {claims_path}: {err}"))?;

    if !claims.claim_mode.eq_ignore_ascii_case("native") {
        return Err(format!(
            "verify-state supports native claims only (found '{}')",
            claims.claim_mode
        ));
    }

    let state_bytes = std::fs::read(Path::new(state_path))
        .map_err(|err| format!("failed to read apply state {state_path}: {err}"))?;
    let state: ApplyState = serde_json::from_slice(&state_bytes)
        .map_err(|err| format!("invalid apply state {state_path}: {err}"))?;

    let registry = StakeRegistry::load(Path::new(registry_path))
        .map_err(|err| format!("failed to load registry {registry_path}: {err}"))?;

    let mut by_id: HashMap<String, (String, u128)> = HashMap::new();
    for claim in &claims.claims {
        if claim.account != claim.pubkey_b64 {
            return Err(format!(
                "native claim account mismatch for claim {} (account='{}', pubkey='{}')",
                claim.claim_id, claim.account, claim.pubkey_b64
            ));
        }
        if by_id.contains_key(&claim.claim_id) {
            return Err(format!(
                "duplicate claim_id in claims artifact: {}",
                claim.claim_id
            ));
        }
        let mint = claim
            .mint_amount
            .parse::<u128>()
            .map_err(|err| format!("invalid mint_amount for claim {}: {err}", claim.claim_id))?;
        by_id.insert(claim.claim_id.clone(), (claim.pubkey_b64.clone(), mint));
    }

    let mut seen_state = HashSet::new();
    let mut unknown_count = 0usize;
    let mut applied_count = 0usize;
    let mut applied_total_mint: u128 = 0;
    let mut minted_by_pk: HashMap<String, u128> = HashMap::new();

    for claim_id in state.applied_claim_ids {
        if !seen_state.insert(claim_id.clone()) {
            continue;
        }
        if let Some((pk, mint)) = by_id.get(&claim_id) {
            applied_count += 1;
            applied_total_mint = applied_total_mint.saturating_add(*mint);
            let entry = minted_by_pk.entry(pk.clone()).or_insert(0);
            *entry = entry.saturating_add(*mint);
        } else {
            unknown_count += 1;
        }
    }

    let claim_count = by_id.len();
    let missing_count = claim_count.saturating_sub(applied_count);

    if opts.require_complete && missing_count > 0 {
        return Err(format!(
            "migration apply state incomplete: {missing_count} claim(s) missing"
        ));
    }

    if unknown_count > 0 {
        return Err(format!(
            "migration apply state contains {unknown_count} unknown claim id(s)"
        ));
    }

    if opts.enforce_balance_floor {
        for (pk, minted) in minted_by_pk {
            if minted > u64::MAX as u128 {
                return Err(format!("minted amount overflow for account {pk}"));
            }
            let balance = registry.account(&pk).map(|acct| acct.balance).unwrap_or(0);
            if balance < minted as u64 {
                return Err(format!(
                    "registry balance floor failed for {pk}: balance={} minted={}",
                    balance, minted
                ));
            }
        }
    }

    Ok(VerifyStateSummary {
        claim_count,
        applied_count,
        missing_count,
        unknown_count,
        applied_total_mint: applied_total_mint.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::{run_verify_state, VerifyStateOptions};
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
    fn verify_state_passes_complete_native() {
        let registry = temp_path("verify_registry.json");
        let claims = temp_path("verify_claims.json");
        let state = temp_path("verify_state.json");

        let registry_payload = json!({
            "accounts": {
                "pk1": {"balance": 11, "stake": 0, "slashed": false},
                "pk2": {"balance": 20, "stake": 0, "slashed": false}
            }
        });
        let claims_payload = json!({
            "claim_mode": "native",
            "claims": [
                {"pubkey_b64":"pk1","account":"pk1","claim_id":"c1","mint_amount":"10"},
                {"pubkey_b64":"pk2","account":"pk2","claim_id":"c2","mint_amount":"20"}
            ]
        });
        let state_payload = json!({"applied_claim_ids":["c1","c2"]});

        fs::write(&registry, serde_json::to_vec(&registry_payload).unwrap()).unwrap();
        fs::write(&claims, serde_json::to_vec(&claims_payload).unwrap()).unwrap();
        fs::write(&state, serde_json::to_vec(&state_payload).unwrap()).unwrap();

        let summary = run_verify_state(
            registry.to_str().unwrap(),
            claims.to_str().unwrap(),
            state.to_str().unwrap(),
            &VerifyStateOptions {
                require_complete: true,
                enforce_balance_floor: true,
            },
        )
        .unwrap();
        assert_eq!(summary.claim_count, 2);
        assert_eq!(summary.applied_count, 2);
        assert_eq!(summary.missing_count, 0);
        assert_eq!(summary.unknown_count, 0);

        let _ = fs::remove_file(registry);
        let _ = fs::remove_file(claims);
        let _ = fs::remove_file(state);
    }

    #[test]
    fn verify_state_rejects_unknown_ids() {
        let registry = temp_path("verify_registry_bad.json");
        let claims = temp_path("verify_claims_bad.json");
        let state = temp_path("verify_state_bad.json");

        fs::write(&registry, b"{\"accounts\":{}}" as &[u8]).unwrap();
        fs::write(
            &claims,
            serde_json::to_vec(&json!({
                "claim_mode":"native",
                "claims":[{"pubkey_b64":"pk1","account":"pk1","claim_id":"c1","mint_amount":"1"}]
            }))
            .unwrap(),
        )
        .unwrap();
        fs::write(
            &state,
            serde_json::to_vec(&json!({"applied_claim_ids":["c1","unknown"]})).unwrap(),
        )
        .unwrap();

        let err = run_verify_state(
            registry.to_str().unwrap(),
            claims.to_str().unwrap(),
            state.to_str().unwrap(),
            &VerifyStateOptions {
                require_complete: false,
                enforce_balance_floor: false,
            },
        )
        .err()
        .unwrap();
        assert!(err.contains("unknown claim id"));

        let _ = fs::remove_file(registry);
        let _ = fs::remove_file(claims);
        let _ = fs::remove_file(state);
    }
}
