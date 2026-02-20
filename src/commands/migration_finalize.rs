#![cfg(feature = "net")]

use crate::commands::migration_apply_claims::{run_apply_claims, ApplyClaimsOptions};
use crate::commands::migration_claims::{run_build_claims, BuildClaimsOptions};
use crate::commands::migration_proposal::{run_propose_migration, ProposeMigrationOptions};
use crate::commands::stake_snapshot::run_snapshot;

/// Options for running a deterministic end-to-end migration finalize flow.
#[derive(Debug, Clone)]
pub struct FinalizeMigrationOptions {
    /// Path to stake registry JSON.
    pub registry_path: String,
    /// Snapshot height for migration cutover.
    pub snapshot_height: u64,
    /// Ledger log directory for proposal anchoring.
    pub log_dir: String,
    /// Output directory for migration artifacts.
    pub output_dir: String,
    /// Token identifier embedded in migration proposal.
    pub token_contract: String,
    /// Stake-to-token conversion ratio.
    pub conversion_ratio: u64,
    /// Treasury mint amount for proposal metadata.
    pub treasury_mint: u64,
    /// Amount source for claims (`stake|balance|total`).
    pub amount_source: String,
    /// Include slashed accounts in claims.
    pub include_slashed: bool,
    /// Claim-ID salt for deterministic claim generation.
    pub claim_id_salt: String,
    /// Node ID embedded in proposal anchor.
    pub node_id: String,
    /// Quorum embedded in proposal anchor.
    pub quorum: usize,
    /// Optional explicit apply-state path.
    pub apply_state_path: Option<String>,
    /// Allow finalize execution when migration freeze is not enabled.
    pub allow_unfrozen: bool,
    /// Permit overwriting existing artifacts.
    pub force: bool,
}

/// Summary produced by finalize migration workflow.
#[derive(Debug, Clone)]
pub struct FinalizeMigrationSummary {
    /// Snapshot root hex.
    pub snapshot_root: String,
    /// Claims merkle root hex.
    pub claims_root: String,
    /// Number of claims newly applied.
    pub applied_claims: usize,
    /// Number of already-applied claims skipped.
    pub skipped_claims: usize,
    /// Snapshot artifact path.
    pub snapshot_path: String,
    /// Claims artifact path.
    pub claims_path: String,
    /// Apply-state path.
    pub apply_state_path: String,
    /// Migration proposal artifact path.
    pub proposal_path: String,
}

fn ensure_writable(path: &std::path::Path, force: bool) -> Result<(), String> {
    if path.exists() && !force {
        return Err(format!(
            "{} already exists; rerun with --force to overwrite",
            path.display()
        ));
    }
    Ok(())
}

/// Run full migration finalize pipeline:
/// freeze-check, snapshot, claims, apply-claims, and proposal anchor artifact.
pub fn run_finalize_migration(
    opts: &FinalizeMigrationOptions,
) -> Result<FinalizeMigrationSummary, String> {
    crate::net::refresh_migration_mode_from_env();
    if !opts.allow_unfrozen && !crate::net::migration_mode_frozen() {
        return Err(
            "migration freeze is not active (set PH_MIGRATION_MODE=freeze or use --allow-unfrozen)"
                .to_string(),
        );
    }

    let out_dir = std::path::Path::new(&opts.output_dir);
    std::fs::create_dir_all(out_dir)
        .map_err(|err| format!("failed to create output dir {}: {err}", out_dir.display()))?;

    let snapshot_path = out_dir.join("migration_snapshot.json");
    let claims_path = out_dir.join("migration_claims.json");
    let proposal_path = out_dir.join("migration_anchor.json");
    let apply_state_path = opts.apply_state_path.clone().unwrap_or_else(|| {
        out_dir
            .join("migration_apply_state.json")
            .display()
            .to_string()
    });

    ensure_writable(&snapshot_path, opts.force)?;
    ensure_writable(&claims_path, opts.force)?;
    ensure_writable(&proposal_path, opts.force)?;

    let snapshot_root = run_snapshot(
        &opts.registry_path,
        opts.snapshot_height,
        snapshot_path.to_str().unwrap_or("migration_snapshot.json"),
    )?;

    let claims_root = run_build_claims(
        snapshot_path.to_str().unwrap_or("migration_snapshot.json"),
        claims_path.to_str().unwrap_or("migration_claims.json"),
        &BuildClaimsOptions {
            amount_source: opts.amount_source.clone(),
            include_slashed: opts.include_slashed,
            conversion_ratio: if opts.conversion_ratio == 0 {
                1
            } else {
                opts.conversion_ratio
            },
            claim_id_salt: opts.claim_id_salt.clone(),
            token_contract: Some(opts.token_contract.clone()),
            snapshot_height_override: Some(opts.snapshot_height),
            claim_mode: "native".to_string(),
        },
    )?;

    let apply_summary = run_apply_claims(
        &opts.registry_path,
        claims_path.to_str().unwrap_or("migration_claims.json"),
        &ApplyClaimsOptions {
            state_path: Some(apply_state_path.clone()),
            dry_run: false,
        },
    )?;

    run_propose_migration(&ProposeMigrationOptions {
        snapshot_height: opts.snapshot_height,
        token_contract: opts.token_contract.clone(),
        conversion_ratio: if opts.conversion_ratio == 0 {
            1
        } else {
            opts.conversion_ratio
        },
        treasury_mint: opts.treasury_mint,
        log_dir: opts.log_dir.clone(),
        node_id: opts.node_id.clone(),
        quorum: opts.quorum,
        output: Some(proposal_path.display().to_string()),
    })?;

    Ok(FinalizeMigrationSummary {
        snapshot_root,
        claims_root,
        applied_claims: apply_summary.applied,
        skipped_claims: apply_summary.skipped,
        snapshot_path: snapshot_path.display().to_string(),
        claims_path: claims_path.display().to_string(),
        apply_state_path,
        proposal_path: proposal_path.display().to_string(),
    })
}
