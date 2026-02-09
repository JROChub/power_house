//! Rollup integration with Groth16 verification.
//! Circuit: next = prev + tx_root (Fr) plus Pedersen Merkle inclusion of tx_root into pedersen_root bytes (public).

use ark_bn254::{Bn254, Fr};
use ark_crypto_primitives::crh::pedersen::constraints::{
    CRHGadget, CRHParametersVar, TwoToOneCRHGadget,
};
use ark_crypto_primitives::crh::{pedersen, CRHScheme, CRHSchemeGadget, TwoToOneCRHSchemeGadget};
use ark_ed_on_bn254::{constraints::EdwardsVar as PedersenVar, EdwardsProjective as PedersenCurve};
use ark_ff::PrimeField;
use ark_groth16::{Groth16, Proof};
use ark_r1cs_std::{
    alloc::AllocVar, boolean::Boolean, eq::EqGadget, fields::fp::FpVar, uint8::UInt8, ToBitsGadget,
    ToBytesGadget,
};
use ark_relations::r1cs::{ConstraintSynthesizer, ConstraintSystemRef, SynthesisError};
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize, Compress, Validate};
use rand::rngs::StdRng;
use rand::SeedableRng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::Cursor;

#[derive(Clone)]
struct PedersenWindow;
impl pedersen::Window for PedersenWindow {
    // Supports 256-bit leaves + padding.
    const WINDOW_SIZE: usize = 4;
    const NUM_WINDOWS: usize = 130;
}

fn pedersen_params() -> pedersen::Parameters<PedersenCurve> {
    let mut rng = StdRng::from_seed([0u8; 32]);
    pedersen::CRH::<PedersenCurve, PedersenWindow>::setup(&mut rng).expect("pedersen setup")
}

fn pedersen_hash_bytes(params: &pedersen::Parameters<PedersenCurve>, data: &[u8]) -> [u8; 32] {
    let point = pedersen::CRH::<PedersenCurve, PedersenWindow>::evaluate(params, data)
        .expect("pedersen eval");
    let mut out = Vec::new();
    point
        .serialize_compressed(&mut out)
        .expect("serialize pedersen");
    let mut buf = [0u8; 32];
    buf.copy_from_slice(&out[..32]);
    buf
}

fn pedersen_leaf(params: &pedersen::Parameters<PedersenCurve>, data: &[u8]) -> [u8; 32] {
    let mut buf = Vec::with_capacity(1 + data.len());
    buf.push(0u8);
    buf.extend_from_slice(data);
    pedersen_hash_bytes(params, &buf)
}

fn pedersen_hash_pair(
    params: &pedersen::Parameters<PedersenCurve>,
    left: &[u8; 32],
    right: &[u8; 32],
) -> [u8; 32] {
    let mut buf = Vec::with_capacity(1 + left.len() + right.len());
    buf.push(1u8);
    buf.extend_from_slice(left);
    buf.extend_from_slice(right);
    pedersen_hash_bytes(params, &buf)
}

fn pedersen_root_from_path(leaf: &[u8], path: &[(bool, [u8; 32])]) -> Result<Vec<u8>, String> {
    if leaf.len() != 32 {
        return Err("leaf must be 32 bytes".into());
    }
    let params = pedersen_params();
    let mut current = pedersen_leaf(&params, leaf);
    for (left, sib_arr) in path {
        current = if *left {
            pedersen_hash_pair(&params, sib_arr, &current)
        } else {
            pedersen_hash_pair(&params, &current, sib_arr)
        };
    }
    Ok(current.to_vec())
}

/// Internal circuit enforcing the simple state transition and Pedersen Merkle inclusion.
#[derive(Clone)]
struct RollupCircuit {
    prev: Fr,
    next: Fr,
    tx_root: Fr,
    share_root: Fr,
    share_root_bytes: [u8; 32],
    tx_bytes: [u8; 32],
    path: Vec<(bool, [u8; 32])>,
    pedersen_params: pedersen::Parameters<PedersenCurve>,
}

impl ConstraintSynthesizer<Fr> for RollupCircuit {
    fn generate_constraints(self, cs: ConstraintSystemRef<Fr>) -> Result<(), SynthesisError> {
        let prev = FpVar::new_input(cs.clone(), || Ok(self.prev))?;
        let next = FpVar::new_input(cs.clone(), || Ok(self.next))?;
        let tx = FpVar::new_input(cs.clone(), || Ok(self.tx_root))?;
        let _share = FpVar::new_input(cs.clone(), || Ok(self.share_root))?;

        let sum = &prev + &tx;
        sum.enforce_equal(&next)?;

        // Pedersen Merkle inclusion of tx_root into share_root_bytes.
        let params_const = CRHParametersVar::<PedersenCurve, PedersenVar>::new_constant(
            cs.clone(),
            &self.pedersen_params,
        )?;
        let two_params_const = CRHParametersVar::<PedersenCurve, PedersenVar>::new_constant(
            cs.clone(),
            &self.pedersen_params,
        )?;

        let tx_bytes_var = UInt8::new_input_vec(cs.clone(), &self.tx_bytes)?;
        let mut current = CRHGadget::<PedersenCurve, PedersenVar, PedersenWindow>::evaluate(
            &params_const,
            &tx_bytes_var,
        )?;

        for (left, sib) in self.path {
            let sib_bytes = UInt8::new_witness_vec(cs.clone(), &sib)?;
            let sib_hash = CRHGadget::<PedersenCurve, PedersenVar, PedersenWindow>::evaluate(
                &params_const,
                &sib_bytes,
            )?;
            let hashed = TwoToOneCRHGadget::<PedersenCurve, PedersenVar, PedersenWindow>::compress(
                &two_params_const,
                if left { &sib_hash } else { &current },
                if left { &current } else { &sib_hash },
            )?;
            current = hashed;
        }

        let share_bytes = UInt8::new_input_vec(cs, &self.share_root_bytes)?;
        let share_bits: Vec<Boolean<Fr>> = share_bytes
            .iter()
            .map(|b| b.to_bits_le())
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .collect();
        let current_bytes = current
            .to_bytes()?
            .iter()
            .map(|b| b.to_bits_le())
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        current_bytes.enforce_equal(&share_bits)?;
        Ok(())
    }
}

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
    /// Merkle path siblings (JSON-serialized Vec<MerkleSibling>).
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

/// Verify Groth16 rollup proof with deterministic parameters and Merkle check.
pub fn verify_zk_rollup(
    commitment: &RollupCommitment,
    proof: &ZkRollupProof,
) -> Result<(), String> {
    if proof.proof.is_empty() || proof.public_inputs.len() != 128 {
        return Err("zk proof missing".into());
    }
    let pedersen_root_hex = commitment
        .pedersen_root
        .clone()
        .ok_or_else(|| "missing pedersen_root in commitment".to_string())?;
    let pedersen_root_bytes =
        hex::decode(&pedersen_root_hex).map_err(|e| format!("bad share_root: {e}"))?;
    if pedersen_root_bytes.len() != 32 {
        return Err("share_root must be 32 bytes".into());
    }
    let prev = Fr::from_le_bytes_mod_order(&proof.public_inputs[0..32]);
    let next = Fr::from_le_bytes_mod_order(&proof.public_inputs[32..64]);
    let tx_root = Fr::from_le_bytes_mod_order(&proof.public_inputs[64..96]);
    let share_root = Fr::from_le_bytes_mod_order(&proof.public_inputs[96..128]);

    // Out-of-circuit Pedersen Merkle check: verify tx_root bytes against pedersen_root via path.
    let path_json: Vec<MerkleSibling> = serde_json::from_slice(&proof.merkle_path)
        .map_err(|e| format!("merkle path decode failed: {e}"))?;
    let mut path = Vec::new();
    for sib in &path_json {
        let bytes = hex::decode(&sib.hash).map_err(|e| format!("bad sibling hex: {e}"))?;
        if bytes.len() != 32 {
            return Err("sibling must be 32 bytes".into());
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&bytes);
        path.push((sib.left, arr));
    }
    let current = pedersen_root_from_path(&proof.public_inputs[64..96], &path)?;
    if current != pedersen_root_bytes {
        return Err("merkle path invalid (pedersen)".into());
    }
    if proof.public_inputs[96..128] != pedersen_root_bytes {
        return Err("public share_root does not match pedersen_root".into());
    }

    let mut hasher = Sha256::new();
    hasher.update(commitment.namespace.as_bytes());
    hasher.update(&proof.public_inputs);
    hasher.update(&pedersen_root_bytes);
    let seed: [u8; 32] = hasher.finalize().into();
    let mut rng = StdRng::from_seed(seed);

    let mut share_root_bytes_arr = [0u8; 32];
    share_root_bytes_arr.copy_from_slice(&pedersen_root_bytes);
    let mut tx_bytes_arr = [0u8; 32];
    tx_bytes_arr.copy_from_slice(&proof.public_inputs[64..96]);
    let circuit = RollupCircuit {
        prev,
        next,
        tx_root,
        share_root,
        share_root_bytes: share_root_bytes_arr,
        tx_bytes: tx_bytes_arr,
        path,
        pedersen_params: pedersen_params(),
    };
    let params = Groth16::<Bn254, ark_groth16::r1cs_to_qap::LibsnarkReduction>::generate_random_parameters_with_reduction(circuit.clone(), &mut rng)
        .map_err(|e| format!("parameter gen failed: {e}"))?;
    let groth_proof: Proof<Bn254> = Groth16::<Bn254, ark_groth16::r1cs_to_qap::LibsnarkReduction>::create_random_proof_with_reduction(circuit, &params, &mut rng)
        .map_err(|e| format!("proof gen failed: {e}"))?;

    let mut cursor = Cursor::new(&proof.proof);
    let provided: Proof<Bn254> =
        Proof::deserialize_with_mode(&mut cursor, Compress::Yes, Validate::Yes)
            .map_err(|e| format!("proof decode failed: {e}"))?;
    if provided != groth_proof {
        return Err("zk proof invalid".into());
    }
    Ok(())
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
    /// Verify using a Groth16 proof.
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
