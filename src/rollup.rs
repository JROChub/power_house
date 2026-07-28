//! Rollup settlement integration.
//!
//! The former deterministic Groth16 envelope regenerated setup and proof
//! material inside the verifier. That did not establish an independent
//! verifier-key boundary and is rejected fail-closed. Optimistic settlement
//! remains available while a versioned proof profile with a pinned verifier
//! key is designed and reviewed.

use serde::{Deserialize, Serialize};

/// Commitment linking a rollup batch to a DA blob.
#[derive(Debug, Clone)]
pub struct RollupCommitment {
    /// Namespace of the DA blob.
    pub namespace: String,
    /// Hex-encoded share root of the DA blob.
    pub share_root: String,
    /// Optional Pedersen share root of the DA blob.
    pub pedersen_root: Option<String>,
    /// Optional L1 settlement identifier.
    pub settlement_slot: Option<String>,
}

/// Merkle path element (hex-encoded sibling) for out-of-circuit verification.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MerkleSibling {
    /// true if this sibling hash is on the left.
    pub left: bool,
    /// Hex-encoded sibling hash bytes.
    pub hash: String,
}

/// ZK rollup proof payload.
#[derive(Debug, Clone)]
pub struct ZkRollupProof {
    /// Serialized Groth16 proof bytes.
    pub proof: Vec<u8>,
    /// Public inputs: prev||next||tx_root||share_root (4 x 32 bytes LE).
    pub public_inputs: Vec<u8>,
    /// Merkle path siblings (JSON-serialized `Vec<MerkleSibling>`).
    pub merkle_path: Vec<u8>,
}

/// Fault evidence used for optimistic mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimisticFault {
    /// Description of the fault.
    pub description: String,
    /// Optional evidence payload.
    pub evidence: Vec<u8>,
}

/// Rollup fault evidence (used for slashing/settlement).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollupFaultEvidence {
    /// Namespace of the DA blob.
    pub namespace: String,
    /// Commitment hash or identifier.
    pub commitment: String,
    /// Reason for the fault.
    pub reason: String,
    /// Optional payload (hex/base64).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload: Option<String>,
}

/// Build rollup fault evidence for downstream slashing/settlement.
#[cfg(feature = "net")]
pub fn build_rollup_fault(
    commitment: &RollupCommitment,
    reason: &str,
    payload: Option<String>,
) -> RollupFaultEvidence {
    RollupFaultEvidence {
        namespace: commitment.namespace.clone(),
        commitment: commitment
            .settlement_slot
            .clone()
            .unwrap_or_else(|| commitment.share_root.clone()),
        reason: reason.to_string(),
        payload,
    }
}

/// Rejects the retired deterministic Groth16 rollup envelope.
///
/// The v0.3 profile regenerated setup and proof bytes from public request
/// material. It is not accepted as a cryptographic settlement boundary.
pub fn verify_zk_rollup(
    _commitment: &RollupCommitment,
    _proof: &ZkRollupProof,
) -> Result<(), String> {
    Err(
        "legacy deterministic Groth16 rollup profile is retired: no pinned independent verifier key"
            .to_string(),
    )
}

/// Verify optimistic rollup faults (rejects if any).
pub fn verify_optimistic_rollup(
    commitment: &RollupCommitment,
    faults: &[OptimisticFault],
) -> Result<(), String> {
    if commitment.share_root.is_empty() {
        return Err("missing share_root".into());
    }
    if faults.is_empty() {
        Ok(())
    } else {
        Err("optimistic fault raised".into())
    }
}

/// Settle a rollup fee, returning fault evidence on failure.
#[cfg(feature = "net")]
pub fn settle_rollup_with_fault(
    registry_path: &std::path::Path,
    commitment: RollupCommitment,
    payer_pk: &str,
    fee: u64,
    mode: RollupSettlementMode,
) -> Result<SettlementReceipt, RollupFaultEvidence> {
    match &mode {
        RollupSettlementMode::Zk(proof) => {
            if let Err(err) = verify_zk_rollup(&commitment, proof) {
                return Err(build_rollup_fault(&commitment, &err, None));
            }
        }
        RollupSettlementMode::Optimistic(faults) => {
            if let Err(err) = verify_optimistic_rollup(&commitment, faults) {
                return Err(build_rollup_fault(&commitment, &err, None));
            }
        }
        RollupSettlementMode::Fault(ev) => return Err(ev.clone()),
    }
    let cloned = commitment.clone();
    settle_rollup(registry_path, commitment, payer_pk, fee)
        .map_err(|e| build_rollup_fault(&cloned, &e, None))
}

/// Settle a rollup with fee rewards split between operator and attesters.
#[cfg(feature = "net")]
pub fn settle_rollup_with_rewards(
    registry_path: &std::path::Path,
    commitment: RollupCommitment,
    payer_pk: &str,
    operator_pk: &str,
    attesters: &[String],
    fee: u64,
    mode: RollupSettlementMode,
) -> Result<SettlementReceipt, RollupFaultEvidence> {
    let receipt = settle_rollup_with_fault(registry_path, commitment, payer_pk, fee, mode)?;
    let mut reg = crate::net::stake_registry::StakeRegistry::load(registry_path).map_err(|e| {
        build_rollup_fault(&receipt.commitment, &format!("load registry: {e}"), None)
    })?;
    let operator_share = fee.div_ceil(2);
    reg.credit_reward(operator_pk, operator_share);
    if !attesters.is_empty() {
        let per = (fee.saturating_sub(operator_share)) / attesters.len() as u64;
        for a in attesters {
            reg.credit_reward(a, per);
        }
    }
    reg.save(registry_path).map_err(|e| {
        build_rollup_fault(&receipt.commitment, &format!("persist registry: {e}"), None)
    })?;
    Ok(receipt)
}

/// Receipt returned after settling a rollup fee.
#[derive(Debug, Clone)]
pub struct SettlementReceipt {
    /// Commitment bound to the settlement.
    pub commitment: RollupCommitment,
    /// Fee payer public key.
    pub payer: String,
    /// Fee amount debited.
    pub fee: u64,
    /// Optional fault evidence if settlement rejected.
    #[allow(dead_code)]
    pub fault: Option<RollupFaultEvidence>,
}

/// Settle a rollup fee by debiting the stake registry.
#[cfg(feature = "net")]
pub fn settle_rollup(
    registry_path: &std::path::Path,
    commitment: RollupCommitment,
    payer_pk: &str,
    fee: u64,
) -> Result<SettlementReceipt, String> {
    let mut reg = crate::net::stake_registry::StakeRegistry::load(registry_path)?;
    reg.debit_fee(payer_pk, fee)?;
    reg.save(registry_path)?;
    Ok(SettlementReceipt {
        commitment,
        payer: payer_pk.to_string(),
        fee,
        fault: None,
    })
}

/// Rollup settlement verification mode.
#[derive(Debug, Clone)]
pub enum RollupSettlementMode {
    /// Retired deterministic Groth16 envelope; always rejected fail-closed.
    Zk(ZkRollupProof),
    /// Optimistic mode with fault evidence list.
    Optimistic(Vec<OptimisticFault>),
    /// Invalid: attach rollup fault evidence.
    Fault(RollupFaultEvidence),
}

/// Verify a rollup (zk or optimistic) then settle fees.
#[cfg(feature = "net")]
pub fn settle_rollup_verified(
    registry_path: &std::path::Path,
    commitment: RollupCommitment,
    payer_pk: &str,
    fee: u64,
    mode: RollupSettlementMode,
) -> Result<SettlementReceipt, String> {
    match &mode {
        RollupSettlementMode::Zk(proof) => verify_zk_rollup(&commitment, proof)?,
        RollupSettlementMode::Optimistic(faults) => verify_optimistic_rollup(&commitment, faults)?,
        RollupSettlementMode::Fault(ev) => return Err(format!("rollup fault: {}", ev.reason)),
    }
    settle_rollup(registry_path, commitment, payer_pk, fee)
}
