#![cfg(feature = "net")]

use crate::net::StakeRegistry;
use blake2::digest::{consts::U32, Digest};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

type Blake2b256 = blake2::Blake2b<U32>;

const EXEC_STATE_SCHEMA: &str = "mfenx.powerhouse.migration-burn-exec-state.v1";

/// Options for executing migration burn intents.
#[derive(Debug, Clone)]
pub struct ExecuteBurnOptions {
    /// Optional path to outbox state file.
    pub state_path: Option<String>,
    /// Dry-run mode computes actions without writing registry/state changes.
    pub dry_run: bool,
}

/// Summary returned after executing burn intents.
#[derive(Debug, Clone)]
pub struct ExecuteBurnSummary {
    /// Number of outbox records processed in this run.
    pub processed: usize,
    /// Number of records skipped because they were already processed.
    pub skipped: usize,
    /// Number of native intents executed as slashes.
    pub native_executed: usize,
    /// Number of non-native intents left untouched.
    pub unsupported_mode: usize,
    /// State file path used for idempotency.
    pub state_path: String,
}

#[derive(Debug, Deserialize)]
struct BurnIntent {
    #[serde(default)]
    schema: String,
    token_contract: Option<String>,
    pubkey_b64: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ExecuteState {
    schema: String,
    updated_at_ms: u64,
    processed_ids: Vec<String>,
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn token_mode_is_native(mode: &str) -> bool {
    let trimmed = mode.trim();
    trimmed.eq_ignore_ascii_case("native") || trimmed.to_ascii_lowercase().starts_with("native://")
}

fn resolve_state_path(outbox_path: &Path, explicit: Option<&str>) -> PathBuf {
    if let Some(path) = explicit {
        return PathBuf::from(path);
    }
    outbox_path.with_file_name("token_burn_exec_state.json")
}

fn intent_id(raw_line: &str) -> String {
    let mut hasher = Blake2b256::new();
    hasher.update(b"mfenx-migration-burn-intent-id-v1");
    hasher.update(raw_line.as_bytes());
    hex::encode(hasher.finalize())
}

fn load_state(path: &Path) -> Result<ExecuteState, String> {
    if !path.exists() {
        return Ok(ExecuteState {
            schema: EXEC_STATE_SCHEMA.to_string(),
            updated_at_ms: now_millis(),
            processed_ids: Vec::new(),
        });
    }
    let bytes = std::fs::read(path)
        .map_err(|err| format!("failed to read burn state {}: {err}", path.display()))?;
    let mut state: ExecuteState = serde_json::from_slice(&bytes)
        .map_err(|err| format!("invalid burn state {}: {err}", path.display()))?;
    if state.schema.trim().is_empty() {
        state.schema = EXEC_STATE_SCHEMA.to_string();
    }
    Ok(state)
}

fn save_state(path: &Path, state: &ExecuteState) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
    }
    let encoded = serde_json::to_vec_pretty(state)
        .map_err(|err| format!("failed to encode burn state: {err}"))?;
    std::fs::write(path, encoded)
        .map_err(|err| format!("failed to write burn state {}: {err}", path.display()))
}

/// Execute native burn intents by slashing corresponding stake registry accounts.
///
/// Intents are consumed idempotently using a persistent state file.
pub fn run_execute_burn_intents(
    registry_path: &str,
    outbox_path: &str,
    opts: &ExecuteBurnOptions,
) -> Result<ExecuteBurnSummary, String> {
    let registry_path = Path::new(registry_path);
    let outbox_path = Path::new(outbox_path);
    let state_path = resolve_state_path(outbox_path, opts.state_path.as_deref());

    let outbox = if outbox_path.exists() {
        std::fs::read_to_string(outbox_path)
            .map_err(|err| format!("failed to read outbox {}: {err}", outbox_path.display()))?
    } else {
        String::new()
    };

    let mut state = load_state(&state_path)?;
    let mut seen = state
        .processed_ids
        .iter()
        .cloned()
        .collect::<HashSet<String>>();

    let mut registry = StakeRegistry::load(registry_path)
        .map_err(|err| format!("failed to load registry {}: {err}", registry_path.display()))?;

    let mut processed = 0usize;
    let mut skipped = 0usize;
    let mut native_executed = 0usize;
    let mut unsupported_mode = 0usize;

    for raw in outbox.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }

        let id = intent_id(line);
        if !seen.insert(id) {
            skipped += 1;
            continue;
        }

        let intent: BurnIntent = serde_json::from_str(line)
            .map_err(|err| format!("invalid burn intent record: {err}"))?;

        if !intent.schema.is_empty() && intent.schema != "mfenx.powerhouse.token-burn-intent.v1" {
            return Err(format!("unexpected burn intent schema: {}", intent.schema));
        }

        let mode = intent.token_contract.unwrap_or_default();
        if !token_mode_is_native(&mode) {
            unsupported_mode += 1;
            processed += 1;
            continue;
        }

        let pk = intent
            .pubkey_b64
            .ok_or_else(|| "burn intent missing pubkey_b64".to_string())?;
        registry.slash(&pk);
        native_executed += 1;
        processed += 1;
    }

    if !opts.dry_run {
        registry
            .save(registry_path)
            .map_err(|err| format!("failed to save registry {}: {err}", registry_path.display()))?;
        let mut processed_ids = seen.into_iter().collect::<Vec<_>>();
        processed_ids.sort();
        state.schema = EXEC_STATE_SCHEMA.to_string();
        state.updated_at_ms = now_millis();
        state.processed_ids = processed_ids;
        save_state(&state_path, &state)?;
    }

    Ok(ExecuteBurnSummary {
        processed,
        skipped,
        native_executed,
        unsupported_mode,
        state_path: state_path.display().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::{run_execute_burn_intents, ExecuteBurnOptions};
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
    fn execute_native_burn_intents_is_idempotent() {
        let registry = temp_path("burn_registry.json");
        let outbox = temp_path("burn_outbox.jsonl");
        let state = temp_path("burn_state.json");

        let registry_payload = json!({
            "accounts": {
                "pk1": {"balance": 10, "stake": 99, "slashed": false}
            }
        });
        fs::write(&registry, serde_json::to_vec(&registry_payload).unwrap()).unwrap();

        let line = json!({
            "schema":"mfenx.powerhouse.token-burn-intent.v1",
            "token_contract":"native://julian",
            "pubkey_b64":"pk1",
            "reason":"test"
        })
        .to_string();
        fs::write(&outbox, format!("{line}\n")).unwrap();

        let opts = ExecuteBurnOptions {
            state_path: Some(state.display().to_string()),
            dry_run: false,
        };

        let first =
            run_execute_burn_intents(registry.to_str().unwrap(), outbox.to_str().unwrap(), &opts)
                .unwrap();
        assert_eq!(first.native_executed, 1);

        let reg = StakeRegistry::load(&registry).unwrap();
        assert_eq!(reg.account("pk1").unwrap().stake, 0);
        assert!(reg.account("pk1").unwrap().slashed);

        let second =
            run_execute_burn_intents(registry.to_str().unwrap(), outbox.to_str().unwrap(), &opts)
                .unwrap();
        assert_eq!(second.skipped, 1);

        let _ = fs::remove_file(registry);
        let _ = fs::remove_file(outbox);
        let _ = fs::remove_file(state);
    }
}
