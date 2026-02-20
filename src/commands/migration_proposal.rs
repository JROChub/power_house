#![cfg(feature = "net")]

use crate::net::{AnchorJson, MigrationAnchor, MigrationProposal};
use crate::{
    compute_fold_digest, julian_genesis_anchor, parse_log_file, read_fold_digest_hint, EntryAnchor,
    LedgerAnchor,
};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Parameters for generating a migration proposal artifact.
#[derive(Debug, Clone)]
pub struct ProposeMigrationOptions {
    /// Snapshot height selected for migration cutover.
    pub snapshot_height: u64,
    /// Token identifier (for example `native://julian`).
    pub token_contract: String,
    /// Stake-to-token conversion ratio.
    pub conversion_ratio: u64,
    /// Treasury mint amount applied at cutover.
    pub treasury_mint: u64,
    /// Log directory used to build an anchor context.
    pub log_dir: String,
    /// Node ID embedded in generated anchor JSON.
    pub node_id: String,
    /// Quorum threshold embedded in generated anchor JSON.
    pub quorum: usize,
    /// Optional output path for the encoded artifact.
    pub output: Option<String>,
}

#[derive(Debug, Serialize)]
struct MigrationProposalArtifact {
    migration_anchor: MigrationAnchor,
    anchor_json: AnchorJson,
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn is_ledger_file(path: &Path) -> bool {
    match path.file_name().and_then(|n| n.to_str()) {
        Some(name) => name.starts_with("ledger_") && name.ends_with(".txt"),
        None => false,
    }
}

fn load_anchor_from_logs(path: &Path) -> Result<LedgerAnchor, String> {
    let mut cutoff: Option<String> = None;
    let mut anchor_from_checkpoint = false;
    let anchor = match crate::net::load_latest_checkpoint(path) {
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
            merkle_root: crate::merkle_root(&entry_hashes),
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

/// Build and optionally persist a migration proposal artifact.
///
/// Returns the encoded JSON artifact payload.
pub fn run_propose_migration(opts: &ProposeMigrationOptions) -> Result<String, String> {
    let proposal = MigrationProposal {
        snapshot_height: opts.snapshot_height,
        token_contract: opts.token_contract.clone(),
        conversion_ratio: if opts.conversion_ratio == 0 {
            1
        } else {
            opts.conversion_ratio
        },
        treasury_mint: opts.treasury_mint,
    };

    let migration_anchor = proposal
        .to_anchor_payload()
        .map_err(|err| format!("failed to build migration payload: {err}"))?;
    let proposal_digest = crate::transcript_digest_from_hex(&migration_anchor.proposal_hash)
        .map_err(|err| format!("invalid proposal hash: {err}"))?;

    let mut ledger = load_anchor_from_logs(Path::new(&opts.log_dir))?;
    ledger.entries.push(EntryAnchor {
        statement: migration_anchor.statement.clone(),
        merkle_root: crate::merkle_root(&[proposal_digest]),
        hashes: vec![proposal_digest],
    });
    ledger.metadata.fold_digest = Some(compute_fold_digest(&ledger));
    ledger
        .metadata
        .crate_version
        .get_or_insert_with(|| env!("CARGO_PKG_VERSION").to_string());

    let anchor_json = AnchorJson::from_ledger(
        opts.node_id.clone(),
        opts.quorum,
        &ledger,
        now_millis(),
        Vec::new(),
        None,
    )
    .map_err(|err| format!("anchor conversion failed: {err}"))?;

    let artifact = MigrationProposalArtifact {
        migration_anchor,
        anchor_json,
    };
    let encoded = serde_json::to_string_pretty(&artifact)
        .map_err(|err| format!("failed to encode migration proposal artifact: {err}"))?;

    if let Some(path) = &opts.output {
        let path_obj = Path::new(path);
        if let Some(parent) = path_obj.parent() {
            let _ = fs::create_dir_all(parent);
        }
        fs::write(path_obj, &encoded)
            .map_err(|err| format!("failed to write {}: {err}", path_obj.display()))?;
    }

    Ok(encoded)
}
