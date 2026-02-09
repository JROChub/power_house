#![cfg(feature = "net")]

use crate::net::schema::AnchorJson;
use crate::{merkle_root, LedgerAnchor};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

const CHECKPOINT_SCHEMA: &str = "mfenx.powerhouse.checkpoint.v1";

/// Serialized snapshot describing a quorum-approved anchor state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnchorCheckpoint {
    /// Checkpoint schema identifier (`mfenx.powerhouse.checkpoint.v1`).
    pub schema: String,
    /// Monotonic epoch or broadcast counter for this checkpoint.
    pub epoch: u64,
    /// Anchor JSON describing the ledger state.
    pub anchor: AnchorJson,
    /// Validator signatures attesting to the anchor.
    pub signatures: Vec<CheckpointSignature>,
    /// Optional highest ledger log filename included in the snapshot.
    pub log_cutoff: Option<String>,
}

/// Signature material contributed by a validator in the checkpoint set.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointSignature {
    /// Logical node identifier producing the signature.
    pub node_id: String,
    /// Base64-encoded ed25519 public key used to sign.
    pub public_key: String,
    /// Base64-encoded ed25519 signature over the anchor payload.
    pub signature: String,
}

impl AnchorCheckpoint {
    /// Constructs a checkpoint wrapper for the provided anchor and signatures.
    pub fn new(
        epoch: u64,
        anchor: AnchorJson,
        signatures: Vec<CheckpointSignature>,
        log_cutoff: Option<String>,
    ) -> Self {
        Self {
            schema: CHECKPOINT_SCHEMA.to_string(),
            epoch,
            anchor,
            signatures,
            log_cutoff,
        }
    }

    /// Converts the checkpoint back into a ledger anchor plus optional log cutoff marker.
    pub fn into_ledger(self) -> Result<(LedgerAnchor, Option<String>), CheckpointError> {
        if self.schema != CHECKPOINT_SCHEMA {
            return Err(CheckpointError::InvalidSchema(self.schema));
        }
        let ledger = self
            .anchor
            .clone()
            .into_ledger()
            .map_err(|err| CheckpointError::InvalidAnchor(err.to_string()))?;
        Ok((ledger, self.log_cutoff))
    }
}

/// Errors that may occur while handling checkpoints.
#[derive(Debug, Clone)]
pub enum CheckpointError {
    /// Underlying I/O error while reading or writing files.
    Io(String),
    /// The checkpoint schema tag was unexpected.
    InvalidSchema(String),
    /// The embedded anchor failed validation.
    InvalidAnchor(String),
}

impl std::fmt::Display for CheckpointError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "checkpoint I/O error: {err}"),
            Self::InvalidSchema(schema) => write!(f, "invalid checkpoint schema: {schema}"),
            Self::InvalidAnchor(err) => write!(f, "invalid checkpoint anchor: {err}"),
        }
    }
}

impl std::error::Error for CheckpointError {}

/// Writes a checkpoint JSON document to the provided directory.
pub fn write_checkpoint(
    dir: &Path,
    checkpoint: &AnchorCheckpoint,
) -> Result<PathBuf, CheckpointError> {
    fs::create_dir_all(dir).map_err(|err| CheckpointError::Io(err.to_string()))?;
    let path = dir.join(format!("checkpoint_{}.json", checkpoint.epoch));
    let tmp_path = dir.join(format!("checkpoint_{}.json.tmp", checkpoint.epoch));
    let contents = serde_json::to_string_pretty(checkpoint)
        .map_err(|err| CheckpointError::Io(err.to_string()))?;
    fs::write(&tmp_path, contents).map_err(|err| CheckpointError::Io(err.to_string()))?;
    fs::rename(&tmp_path, &path).map_err(|err| CheckpointError::Io(err.to_string()))?;
    Ok(path)
}

/// Returns the checkpoint with the highest epoch if one exists.
pub fn load_latest_checkpoint(dir: &Path) -> Result<Option<AnchorCheckpoint>, CheckpointError> {
    let path = dir.join("checkpoints");
    let entries = match fs::read_dir(&path) {
        Ok(entries) => entries,
        Err(_) => return Ok(None),
    };
    let mut best: Option<(u64, PathBuf)> = None;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
            if let Some(epoch_str) = stem.strip_prefix("checkpoint_") {
                if let Ok(epoch) = epoch_str.parse::<u64>() {
                    if best
                        .as_ref()
                        .map(|(best_epoch, _)| epoch > *best_epoch)
                        .unwrap_or(true)
                    {
                        best = Some((epoch, path.clone()));
                    }
                }
            }
        }
    }
    if let Some((_, path)) = best {
        let contents =
            fs::read_to_string(&path).map_err(|err| CheckpointError::Io(err.to_string()))?;
        let checkpoint: AnchorCheckpoint =
            serde_json::from_str(&contents).map_err(|err| CheckpointError::Io(err.to_string()))?;
        Ok(Some(checkpoint))
    } else {
        Ok(None)
    }
}

/// Determines the lexicographically greatest `ledger_*.txt` file in `log_dir`.
pub fn latest_log_cutoff(log_dir: &Path) -> Option<String> {
    let mut best: Option<String> = None;
    if let Ok(entries) = fs::read_dir(log_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with("ledger_") {
                    if best.as_ref().map(|b| name > b.as_str()).unwrap_or(true) {
                        best = Some(name.to_string());
                    }
                }
            }
        }
    }
    best
}

/// Computes a tree hash across all transcript digests in `anchor`.
pub fn anchor_hasher(anchor: &LedgerAnchor) -> [u8; 32] {
    merkle_root(
        &anchor
            .entries
            .iter()
            .flat_map(|entry| entry.hashes.clone())
            .collect::<Vec<_>>(),
    )
}
