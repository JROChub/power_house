//! Zero-knowledge proof profiles for SFCS VM executions.
//!
//! This module contains opt-in private proof profiles for SFCS VM execution.
//!
//! The narrow private-add profile proves a two-instruction no-overflow add
//! relation. The general private-VM profile commits arbitrary supported RV32I
//! private inputs, trace digests, execution-fractal digests, and constraint
//! coverage without embedding the raw witness in `.pha`.

use super::{
    constraints::{SfcsVmConstraintError, SfcsVmConstraintProof},
    digest_json,
    vm::{SfcsVmInputs, SfcsVmProgram, SfcsVmPublicOutputs},
};
use crate::provenance::{PhaArtifact, PhaError};
use ark_ec::{AffineRepr, CurveGroup, Group};
use ark_ed_on_bn254::{EdwardsAffine, EdwardsProjective, Fr};
use ark_ff::PrimeField;
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize, SerializationError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

/// Draft `.pha` protocol for the first SFCS ZK VM profile.
pub const SFCS_ZK_PRIVATE_ADD_PROTOCOL_V1_DRAFT: &str = "power-house/sfcs-zk-private-add/v1-draft";
/// Draft `.pha` protocol for the general private SFCS VM proof profile.
pub const SFCS_ZK_PRIVATE_VM_PROTOCOL_V1_DRAFT: &str = "power-house/sfcs-zk-private-vm/v1-draft";

const ZK_POINT_DOMAIN: &[u8] = b"power-house:sfcs-zk:v1-draft:pedersen-bases\0";
const ZK_PROOF_DOMAIN: &[u8] = b"power-house:sfcs-zk:v1-draft:private-add-proof\0";
const ZK_PRIVATE_VM_PROOF_DOMAIN: &[u8] = b"power-house:sfcs-zk:v1-draft:private-vm-proof\0";
const ZK_PRIVATE_VM_SECRET_DOMAIN: &[u8] = b"power-house:sfcs-zk:v1-draft:private-vm-secret\0";
const ZK_PRIVATE_VM_CHALLENGE_DOMAIN: &[u8] =
    b"power-house:sfcs-zk:v1-draft:private-vm-challenge\0";
const ZK_PRIVATE_VM_BLINDING_DOMAIN: &[u8] = b"power-house:sfcs-zk:v1-draft:private-vm-blinding\0";
const ZK_PRIVATE_VM_NONCE_DOMAIN: &[u8] = b"power-house:sfcs-zk:v1-draft:private-vm-nonce\0";
const ZK_CHALLENGE_DOMAIN: &[u8] = b"power-house:sfcs-zk:v1-draft:challenge\0";
const ZK_NONCE_DOMAIN: &[u8] = b"power-house:sfcs-zk:v1-draft:nonce\0";
const ZK_SCALAR_PREFIX: &str = "fr:";
const ZK_POINT_PREFIX: &str = "edwards:";

/// Statement proven by the private add profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcsZkPrivateAddStatement {
    /// Profile schema.
    pub schema: String,
    /// Program digest of the verified `add; ecall` RV32I program.
    pub program_digest: String,
    /// Left private source register.
    pub lhs_register: u8,
    /// Right private source register.
    pub rhs_register: u8,
    /// Public destination register.
    pub output_register: u8,
    /// Public output value.
    pub output_value: u32,
    /// Pedersen commitment to the left private value.
    pub lhs_commitment: String,
    /// Pedersen commitment to the right private value.
    pub rhs_commitment: String,
}

/// Private witness used by the prover. This is never embedded into `.pha`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SfcsZkPrivateAddWitness {
    /// Left private value.
    pub lhs_value: u32,
    /// Right private value.
    pub rhs_value: u32,
    /// Left commitment blinding seed.
    pub lhs_blinding_seed: [u8; 32],
    /// Right commitment blinding seed.
    pub rhs_blinding_seed: [u8; 32],
}

/// Non-interactive private add proof.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcsZkPrivateAddProof {
    /// Statement.
    pub statement: SfcsZkPrivateAddStatement,
    /// Commitment to the relation blinding witness.
    pub relation_commitment: String,
    /// Schnorr nonce commitment.
    pub nonce_commitment: String,
    /// Fiat-Shamir challenge scalar.
    pub challenge: String,
    /// Schnorr response scalar.
    pub response_blinding: String,
    /// Domain-separated proof digest.
    pub proof_digest: String,
}

/// Statement proven by the general private VM profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcsZkPrivateVmStatement {
    /// Profile schema.
    pub schema: String,
    /// Program digest of the verified RV32I program.
    pub program_digest: String,
    /// Public outputs selected by the private execution witness.
    pub public_outputs: SfcsVmPublicOutputs,
    /// Number of executed VM instructions.
    pub steps: usize,
    /// Number of covered transition checks.
    pub transition_checks: u64,
    /// Number of covered register range checks.
    pub register_range_checks: u64,
    /// Number of covered memory range checks.
    pub memory_range_checks: u64,
    /// Number of covered memory consistency checks.
    pub memory_consistency_checks: u64,
    /// Number of covered branch checks.
    pub branch_checks: u64,
    /// Pedersen commitments to private execution digests.
    pub commitments: BTreeMap<String, String>,
}

/// Private witness used by the general private VM profile.
///
/// The witness carries the private initial VM inputs and a prover-side
/// blinding seed. It is never embedded into `.pha`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SfcsZkPrivateVmWitness {
    /// Private VM inputs.
    pub inputs: SfcsVmInputs,
    /// Master commitment/proof blinding seed.
    pub blinding_seed: [u8; 32],
}

/// Schnorr proof of knowledge for one committed private execution digest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcsZkPrivateVmOpeningProof {
    /// Commitment label.
    pub label: String,
    /// Nonce commitment.
    pub nonce_commitment: String,
    /// Response for the committed digest scalar.
    pub response_value: String,
    /// Response for the commitment blinding.
    pub response_blinding: String,
}

/// Non-interactive private VM proof.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcsZkPrivateVmProof {
    /// Statement.
    pub statement: SfcsZkPrivateVmStatement,
    /// Fiat-Shamir challenge scalar.
    pub challenge: String,
    /// Opening proofs for every committed private execution digest.
    pub opening_proofs: Vec<SfcsZkPrivateVmOpeningProof>,
    /// Domain-separated proof digest.
    pub proof_digest: String,
}

impl SfcsZkPrivateAddProof {
    /// Proves a private no-overflow RV32I add relation.
    pub fn prove(
        program: &SfcsVmProgram,
        lhs_register: u8,
        rhs_register: u8,
        output_register: u8,
        witness: SfcsZkPrivateAddWitness,
    ) -> Result<Self, SfcsZkError> {
        verify_private_add_program(program, lhs_register, rhs_register, output_register)?;
        let output_value = witness
            .lhs_value
            .checked_add(witness.rhs_value)
            .ok_or_else(|| {
                SfcsZkError::InvalidWitness(
                    "private add profile requires no u32 overflow".to_string(),
                )
            })?;
        let lhs_blinding = scalar_from_seed("lhs-blinding", &witness.lhs_blinding_seed);
        let rhs_blinding = scalar_from_seed("rhs-blinding", &witness.rhs_blinding_seed);
        let lhs_commitment = pedersen_commit(witness.lhs_value, lhs_blinding);
        let rhs_commitment = pedersen_commit(witness.rhs_value, rhs_blinding);
        let statement = SfcsZkPrivateAddStatement {
            schema: SFCS_ZK_PRIVATE_ADD_PROTOCOL_V1_DRAFT.to_string(),
            program_digest: program.program_digest()?,
            lhs_register,
            rhs_register,
            output_register,
            output_value,
            lhs_commitment: point_to_hex(&lhs_commitment)?,
            rhs_commitment: point_to_hex(&rhs_commitment)?,
        };
        let output_point = value_base().mul_bigint(Fr::from(output_value).into_bigint());
        let relation_commitment = lhs_commitment + rhs_commitment - output_point;
        let relation_blinding = lhs_blinding + rhs_blinding;
        let nonce = derive_nonce(&statement, &relation_blinding)?;
        let nonce_commitment = blinding_base().mul_bigint(nonce.into_bigint());
        let challenge = derive_challenge(&statement, &relation_commitment, &nonce_commitment)?;
        let response_blinding = nonce + challenge * relation_blinding;
        let mut proof = Self {
            statement,
            relation_commitment: point_to_hex(&relation_commitment)?,
            nonce_commitment: point_to_hex(&nonce_commitment)?,
            challenge: scalar_to_hex(&challenge)?,
            response_blinding: scalar_to_hex(&response_blinding)?,
            proof_digest: String::new(),
        };
        proof.proof_digest = digest_json(ZK_PROOF_DOMAIN, &proof.preimage())?;
        Ok(proof)
    }

    /// Verifies the private add proof.
    pub fn verify(&self, program: &SfcsVmProgram) -> Result<(), SfcsZkError> {
        verify_private_add_program(
            program,
            self.statement.lhs_register,
            self.statement.rhs_register,
            self.statement.output_register,
        )?;
        if self.statement.schema != SFCS_ZK_PRIVATE_ADD_PROTOCOL_V1_DRAFT {
            return Err(SfcsZkError::UnsupportedSchema(
                self.statement.schema.clone(),
            ));
        }
        if self.statement.program_digest != program.program_digest()? {
            return Err(SfcsZkError::InvalidProof(
                "program digest does not match proof statement".to_string(),
            ));
        }
        let lhs_commitment = point_from_hex(&self.statement.lhs_commitment)?;
        let rhs_commitment = point_from_hex(&self.statement.rhs_commitment)?;
        let relation_commitment = point_from_hex(&self.relation_commitment)?;
        let nonce_commitment = point_from_hex(&self.nonce_commitment)?;
        let challenge = scalar_from_hex(&self.challenge)?;
        let response = scalar_from_hex(&self.response_blinding)?;
        let output_point =
            value_base().mul_bigint(Fr::from(self.statement.output_value).into_bigint());
        let expected_relation = lhs_commitment + rhs_commitment - output_point;
        if relation_commitment != expected_relation {
            return Err(SfcsZkError::InvalidProof(
                "relation commitment does not match commitments and public output".to_string(),
            ));
        }
        let expected_challenge =
            derive_challenge(&self.statement, &relation_commitment, &nonce_commitment)?;
        if challenge != expected_challenge {
            return Err(SfcsZkError::InvalidProof(
                "Fiat-Shamir challenge mismatch".to_string(),
            ));
        }
        let left = blinding_base().mul_bigint(response.into_bigint());
        let right = nonce_commitment + relation_commitment.mul_bigint(challenge.into_bigint());
        if left != right {
            return Err(SfcsZkError::InvalidProof(
                "Schnorr response does not verify".to_string(),
            ));
        }
        let expected_digest = digest_json(ZK_PROOF_DOMAIN, &self.preimage())?;
        if self.proof_digest != expected_digest {
            return Err(SfcsZkError::InvalidProof(
                "proof digest does not match proof body".to_string(),
            ));
        }
        Ok(())
    }

    /// Commits the private proof as ordinary `.pha` core data.
    pub fn to_pha_artifact(
        &self,
        label: impl Into<String>,
        program: &SfcsVmProgram,
    ) -> Result<PhaArtifact, SfcsZkError> {
        self.verify(program)?;
        PhaArtifact::new(
            serde_json::json!({
                "producer": "power_house_sfcs_zk",
                "label": label.into(),
                "profile": SFCS_ZK_PRIVATE_ADD_PROTOCOL_V1_DRAFT,
                "program_digest": self.statement.program_digest,
                "proof_digest": self.proof_digest,
            }),
            SFCS_ZK_PRIVATE_ADD_PROTOCOL_V1_DRAFT,
            serde_json::json!({
                "profile": SFCS_ZK_PRIVATE_ADD_PROTOCOL_V1_DRAFT,
                "program_digest": self.statement.program_digest,
                "output_register": self.statement.output_register,
                "output_value": self.statement.output_value,
                "lhs_commitment": self.statement.lhs_commitment,
                "rhs_commitment": self.statement.rhs_commitment,
                "proof_digest": self.proof_digest,
            }),
            serde_json::json!({
                "program": program,
                "proof": self,
            }),
        )
        .map_err(SfcsZkError::Pha)
    }

    fn preimage(&self) -> serde_json::Value {
        serde_json::json!({
            "statement": self.statement,
            "relation_commitment": self.relation_commitment,
            "nonce_commitment": self.nonce_commitment,
            "challenge": self.challenge,
            "response_blinding": self.response_blinding,
        })
    }
}

impl SfcsZkPrivateVmProof {
    /// Proves private execution of any RV32I program supported by the SFCS VM.
    ///
    /// The raw private inputs and trace are consumed by the prover and are not
    /// embedded into the resulting proof or `.pha` artifact. The public
    /// statement exposes only the program digest, selected public outputs,
    /// coverage counters, and commitments to private execution digests.
    pub fn prove(
        program: &SfcsVmProgram,
        witness: SfcsZkPrivateVmWitness,
    ) -> Result<Self, SfcsZkError> {
        witness.inputs.verify()?;
        let trace = program.execute(&witness.inputs)?;
        let execution_fractal = trace.to_fractal_graph()?;
        let execution_fractal_digest = execution_fractal.fractal_digest()?;
        let constraints = SfcsVmConstraintProof::prove(program, &witness.inputs)?;
        let secrets = BTreeMap::from([
            ("input_digest".to_string(), trace.input_digest.clone()),
            ("trace_digest".to_string(), trace.trace_digest.clone()),
            (
                "execution_fractal_digest".to_string(),
                execution_fractal_digest,
            ),
            (
                "final_state_digest".to_string(),
                trace.final_state_digest.clone(),
            ),
            (
                "final_memory_digest".to_string(),
                trace.final_memory_digest.clone(),
            ),
            (
                "constraint_proof_digest".to_string(),
                constraints.proof_digest.clone(),
            ),
        ]);
        let mut commitments = BTreeMap::new();
        let mut openings = BTreeMap::new();
        for (label, digest) in &secrets {
            let value = private_vm_secret_scalar(label, digest);
            let blinding = private_vm_blinding_scalar(label, &witness.blinding_seed);
            commitments.insert(
                label.clone(),
                point_to_hex(&commit_secret(value, blinding))?,
            );
            openings.insert(label.clone(), (value, blinding));
        }
        let statement = SfcsZkPrivateVmStatement {
            schema: SFCS_ZK_PRIVATE_VM_PROTOCOL_V1_DRAFT.to_string(),
            program_digest: trace.program_digest,
            public_outputs: trace.public_outputs,
            steps: trace.steps.len(),
            transition_checks: constraints.transition_checks,
            register_range_checks: constraints.register_range_checks,
            memory_range_checks: constraints.memory_range_checks,
            memory_consistency_checks: constraints.memory_consistency_checks,
            branch_checks: constraints.branch_checks,
            commitments,
        };
        let nonce_commitments = statement
            .commitments
            .keys()
            .map(|label| {
                let nonce_value = private_vm_nonce_scalar(label, "value", &witness.blinding_seed);
                let nonce_blinding =
                    private_vm_nonce_scalar(label, "blinding", &witness.blinding_seed);
                Ok((
                    label.clone(),
                    nonce_value,
                    nonce_blinding,
                    point_to_hex(&commit_secret(nonce_value, nonce_blinding))?,
                ))
            })
            .collect::<Result<Vec<_>, SfcsZkError>>()?;
        let challenge = derive_private_vm_challenge(&statement, &nonce_commitments)?;
        let mut opening_proofs = Vec::new();
        for (label, nonce_value, nonce_blinding, nonce_commitment) in nonce_commitments {
            let (value, blinding) = openings.get(&label).ok_or_else(|| {
                SfcsZkError::InvalidProof(format!("missing opening for commitment {label}"))
            })?;
            opening_proofs.push(SfcsZkPrivateVmOpeningProof {
                label,
                nonce_commitment,
                response_value: scalar_to_hex(&(nonce_value + challenge * value))?,
                response_blinding: scalar_to_hex(&(nonce_blinding + challenge * blinding))?,
            });
        }
        let mut proof = Self {
            statement,
            challenge: scalar_to_hex(&challenge)?,
            opening_proofs,
            proof_digest: String::new(),
        };
        proof.proof_digest = digest_json(ZK_PRIVATE_VM_PROOF_DOMAIN, &proof.preimage())?;
        Ok(proof)
    }

    /// Verifies the private VM proof without private inputs or trace data.
    pub fn verify(&self, program: &SfcsVmProgram) -> Result<(), SfcsZkError> {
        program.verify()?;
        if self.statement.schema != SFCS_ZK_PRIVATE_VM_PROTOCOL_V1_DRAFT {
            return Err(SfcsZkError::UnsupportedSchema(
                self.statement.schema.clone(),
            ));
        }
        if self.statement.program_digest != program.program_digest()? {
            return Err(SfcsZkError::InvalidProof(
                "program digest does not match private VM statement".to_string(),
            ));
        }
        if self.statement.steps == 0 {
            return Err(SfcsZkError::InvalidProof(
                "private VM proof must cover at least one step".to_string(),
            ));
        }
        if self.statement.transition_checks != self.statement.steps as u64 {
            return Err(SfcsZkError::InvalidProof(
                "transition coverage does not match step count".to_string(),
            ));
        }
        if self.statement.register_range_checks != (self.statement.steps as u64).saturating_mul(32)
        {
            return Err(SfcsZkError::InvalidProof(
                "register range coverage does not match step count".to_string(),
            ));
        }
        let expected_labels = private_vm_commitment_labels();
        if self
            .statement
            .commitments
            .keys()
            .cloned()
            .collect::<Vec<_>>()
            != expected_labels
        {
            return Err(SfcsZkError::InvalidProof(
                "private VM commitment labels are incomplete or non-canonical".to_string(),
            ));
        }
        if self.opening_proofs.len() != expected_labels.len() {
            return Err(SfcsZkError::InvalidProof(
                "private VM opening proof count does not match commitments".to_string(),
            ));
        }
        let nonce_commitments = self
            .opening_proofs
            .iter()
            .map(|proof| {
                Ok((
                    proof.label.clone(),
                    Fr::from(0_u64),
                    Fr::from(0_u64),
                    proof.nonce_commitment.clone(),
                ))
            })
            .collect::<Result<Vec<_>, SfcsZkError>>()?;
        let challenge = scalar_from_hex(&self.challenge)?;
        let expected_challenge = derive_private_vm_challenge(&self.statement, &nonce_commitments)?;
        if challenge != expected_challenge {
            return Err(SfcsZkError::InvalidProof(
                "private VM Fiat-Shamir challenge mismatch".to_string(),
            ));
        }
        let mut seen = BTreeMap::new();
        for proof in &self.opening_proofs {
            if seen.insert(proof.label.clone(), ()).is_some() {
                return Err(SfcsZkError::InvalidProof(format!(
                    "duplicate opening proof for {}",
                    proof.label
                )));
            }
            let commitment_hex = self
                .statement
                .commitments
                .get(&proof.label)
                .ok_or_else(|| {
                    SfcsZkError::InvalidProof(format!(
                        "opening proof references unknown commitment {}",
                        proof.label
                    ))
                })?;
            let commitment = point_from_hex(commitment_hex)?;
            let nonce_commitment = point_from_hex(&proof.nonce_commitment)?;
            let response_value = scalar_from_hex(&proof.response_value)?;
            let response_blinding = scalar_from_hex(&proof.response_blinding)?;
            let left = commit_secret(response_value, response_blinding);
            let right = nonce_commitment + commitment.mul_bigint(challenge.into_bigint());
            if left != right {
                return Err(SfcsZkError::InvalidProof(format!(
                    "private VM opening proof for {} does not verify",
                    proof.label
                )));
            }
        }
        let expected_digest = digest_json(ZK_PRIVATE_VM_PROOF_DOMAIN, &self.preimage())?;
        if self.proof_digest != expected_digest {
            return Err(SfcsZkError::InvalidProof(
                "private VM proof digest does not match proof body".to_string(),
            ));
        }
        Ok(())
    }

    /// Commits the private VM proof as ordinary `.pha` core data.
    pub fn to_pha_artifact(
        &self,
        label: impl Into<String>,
        program: &SfcsVmProgram,
    ) -> Result<PhaArtifact, SfcsZkError> {
        self.verify(program)?;
        PhaArtifact::new(
            serde_json::json!({
                "producer": "power_house_sfcs_zk",
                "label": label.into(),
                "profile": SFCS_ZK_PRIVATE_VM_PROTOCOL_V1_DRAFT,
                "program_digest": self.statement.program_digest,
                "proof_digest": self.proof_digest,
            }),
            SFCS_ZK_PRIVATE_VM_PROTOCOL_V1_DRAFT,
            serde_json::json!({
                "profile": SFCS_ZK_PRIVATE_VM_PROTOCOL_V1_DRAFT,
                "program_digest": self.statement.program_digest,
                "public_outputs": self.statement.public_outputs,
                "steps": self.statement.steps,
                "transition_checks": self.statement.transition_checks,
                "register_range_checks": self.statement.register_range_checks,
                "memory_range_checks": self.statement.memory_range_checks,
                "memory_consistency_checks": self.statement.memory_consistency_checks,
                "branch_checks": self.statement.branch_checks,
                "commitments": self.statement.commitments,
                "proof_digest": self.proof_digest,
            }),
            serde_json::json!({
                "program": program,
                "proof": self,
            }),
        )
        .map_err(SfcsZkError::Pha)
    }

    fn preimage(&self) -> serde_json::Value {
        serde_json::json!({
            "statement": self.statement,
            "challenge": self.challenge,
            "opening_proofs": self.opening_proofs,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct SfcsZkPrivateAddEmbedding {
    program: SfcsVmProgram,
    proof: SfcsZkPrivateAddProof,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct SfcsZkPrivateVmEmbedding {
    program: SfcsVmProgram,
    proof: SfcsZkPrivateVmProof,
}

/// Verifies a `.pha` artifact carrying the private add proof profile.
pub fn verify_private_add_embedding(
    artifact: &PhaArtifact,
) -> Result<SfcsZkPrivateAddProof, SfcsZkError> {
    artifact.verify().map_err(SfcsZkError::Pha)?;
    if artifact.embedded_proof.protocol != SFCS_ZK_PRIVATE_ADD_PROTOCOL_V1_DRAFT {
        return Err(SfcsZkError::InvalidEmbedding(
            "embedded proof protocol is not SFCS ZK private add".to_string(),
        ));
    }
    let embedding: SfcsZkPrivateAddEmbedding =
        serde_json::from_value(artifact.embedded_proof.proof.clone())?;
    embedding.proof.verify(&embedding.program)?;
    for (field, expected) in [
        ("program_digest", &embedding.proof.statement.program_digest),
        ("proof_digest", &embedding.proof.proof_digest),
    ] {
        let found = artifact
            .provenance
            .get(field)
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| SfcsZkError::InvalidEmbedding(format!("missing provenance {field}")))?;
        if found != expected {
            return Err(SfcsZkError::InvalidEmbedding(format!(
                "provenance {field} does not match proof"
            )));
        }
        let public = artifact
            .embedded_proof
            .public_inputs
            .get(field)
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| SfcsZkError::InvalidEmbedding(format!("missing public {field}")))?;
        if public != expected {
            return Err(SfcsZkError::InvalidEmbedding(format!(
                "public {field} does not match proof"
            )));
        }
    }
    if artifact.embedded_proof.public_inputs.get("output_value")
        != Some(&serde_json::json!(embedding.proof.statement.output_value))
    {
        return Err(SfcsZkError::InvalidEmbedding(
            "public output_value does not match proof".to_string(),
        ));
    }
    Ok(embedding.proof)
}

/// Verifies a `.pha` artifact carrying the general private VM proof profile.
pub fn verify_private_vm_embedding(
    artifact: &PhaArtifact,
) -> Result<SfcsZkPrivateVmProof, SfcsZkError> {
    artifact.verify().map_err(SfcsZkError::Pha)?;
    if artifact.embedded_proof.protocol != SFCS_ZK_PRIVATE_VM_PROTOCOL_V1_DRAFT {
        return Err(SfcsZkError::InvalidEmbedding(
            "embedded proof protocol is not SFCS ZK private VM".to_string(),
        ));
    }
    let embedding: SfcsZkPrivateVmEmbedding =
        serde_json::from_value(artifact.embedded_proof.proof.clone())?;
    embedding.proof.verify(&embedding.program)?;
    for (field, expected) in [
        ("program_digest", &embedding.proof.statement.program_digest),
        ("proof_digest", &embedding.proof.proof_digest),
    ] {
        let found = artifact
            .provenance
            .get(field)
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| SfcsZkError::InvalidEmbedding(format!("missing provenance {field}")))?;
        if found != expected {
            return Err(SfcsZkError::InvalidEmbedding(format!(
                "provenance {field} does not match proof"
            )));
        }
        let public = artifact
            .embedded_proof
            .public_inputs
            .get(field)
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| SfcsZkError::InvalidEmbedding(format!("missing public {field}")))?;
        if public != expected {
            return Err(SfcsZkError::InvalidEmbedding(format!(
                "public {field} does not match proof"
            )));
        }
    }
    for field in [
        "public_outputs",
        "steps",
        "transition_checks",
        "register_range_checks",
        "memory_range_checks",
        "memory_consistency_checks",
        "branch_checks",
        "commitments",
    ] {
        let public = artifact
            .embedded_proof
            .public_inputs
            .get(field)
            .ok_or_else(|| SfcsZkError::InvalidEmbedding(format!("missing public {field}")))?;
        let expected = match field {
            "public_outputs" => serde_json::to_value(&embedding.proof.statement.public_outputs)?,
            "steps" => serde_json::json!(embedding.proof.statement.steps),
            "transition_checks" => {
                serde_json::json!(embedding.proof.statement.transition_checks)
            }
            "register_range_checks" => {
                serde_json::json!(embedding.proof.statement.register_range_checks)
            }
            "memory_range_checks" => {
                serde_json::json!(embedding.proof.statement.memory_range_checks)
            }
            "memory_consistency_checks" => {
                serde_json::json!(embedding.proof.statement.memory_consistency_checks)
            }
            "branch_checks" => serde_json::json!(embedding.proof.statement.branch_checks),
            "commitments" => serde_json::to_value(&embedding.proof.statement.commitments)?,
            _ => unreachable!(),
        };
        if public != &expected {
            return Err(SfcsZkError::InvalidEmbedding(format!(
                "public {field} does not match proof"
            )));
        }
    }
    Ok(embedding.proof)
}

/// Verifies the exact VM program shape supported by the first private profile.
pub fn verify_private_add_program(
    program: &SfcsVmProgram,
    lhs_register: u8,
    rhs_register: u8,
    output_register: u8,
) -> Result<(), SfcsZkError> {
    program.verify()?;
    validate_register(lhs_register)?;
    validate_register(rhs_register)?;
    validate_register(output_register)?;
    let expected_add = encode_rv32_add(output_register, lhs_register, rhs_register);
    if program.entry_pc != 0
        || program.instructions.as_slice() != [expected_add, 0x0000_0073]
        || program.max_steps < 2
    {
        return Err(SfcsZkError::InvalidProgram(
            "private add profile requires exactly add rd,rs1,rs2; ecall".to_string(),
        ));
    }
    Ok(())
}

/// Encodes a RV32I `add rd, rs1, rs2` instruction.
pub fn encode_rv32_add(rd: u8, rs1: u8, rs2: u8) -> u32 {
    ((rs2 as u32) << 20) | ((rs1 as u32) << 15) | ((rd as u32) << 7) | 0x33
}

fn validate_register(register: u8) -> Result<(), SfcsZkError> {
    if register > 31 {
        return Err(SfcsZkError::InvalidProgram(format!(
            "register x{register} is outside RV32I range"
        )));
    }
    Ok(())
}

fn value_base() -> EdwardsProjective {
    EdwardsProjective::generator()
}

fn blinding_base() -> EdwardsProjective {
    value_base().mul_bigint(scalar_from_seed("blinding-base", ZK_POINT_DOMAIN).into_bigint())
}

fn pedersen_commit(value: u32, blinding: Fr) -> EdwardsProjective {
    value_base().mul_bigint(Fr::from(value).into_bigint())
        + blinding_base().mul_bigint(blinding.into_bigint())
}

fn commit_secret(value: Fr, blinding: Fr) -> EdwardsProjective {
    value_base().mul_bigint(value.into_bigint())
        + blinding_base().mul_bigint(blinding.into_bigint())
}

fn private_vm_commitment_labels() -> Vec<String> {
    [
        "constraint_proof_digest",
        "execution_fractal_digest",
        "final_memory_digest",
        "final_state_digest",
        "input_digest",
        "trace_digest",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

fn private_vm_secret_scalar(label: &str, digest: &str) -> Fr {
    let mut hasher = Sha256::new();
    hasher.update(ZK_PRIVATE_VM_SECRET_DOMAIN);
    hasher.update(label.as_bytes());
    hasher.update(digest.as_bytes());
    Fr::from_le_bytes_mod_order(&hasher.finalize())
}

fn private_vm_blinding_scalar(label: &str, seed: &[u8; 32]) -> Fr {
    let mut hasher = Sha256::new();
    hasher.update(ZK_PRIVATE_VM_BLINDING_DOMAIN);
    hasher.update(label.as_bytes());
    hasher.update(seed);
    Fr::from_le_bytes_mod_order(&hasher.finalize())
}

fn private_vm_nonce_scalar(label: &str, component: &str, seed: &[u8; 32]) -> Fr {
    let mut hasher = Sha256::new();
    hasher.update(ZK_PRIVATE_VM_NONCE_DOMAIN);
    hasher.update(label.as_bytes());
    hasher.update(component.as_bytes());
    hasher.update(seed);
    Fr::from_le_bytes_mod_order(&hasher.finalize())
}

fn scalar_from_seed(label: &str, seed: &[u8]) -> Fr {
    let mut hasher = Sha256::new();
    hasher.update(ZK_POINT_DOMAIN);
    hasher.update(label.as_bytes());
    hasher.update(seed);
    Fr::from_le_bytes_mod_order(&hasher.finalize())
}

fn derive_nonce(
    statement: &SfcsZkPrivateAddStatement,
    relation_blinding: &Fr,
) -> Result<Fr, SfcsZkError> {
    let mut hasher = Sha256::new();
    hasher.update(ZK_NONCE_DOMAIN);
    hasher.update(serde_json::to_vec(statement)?);
    hasher.update(scalar_to_bytes(relation_blinding)?);
    Ok(Fr::from_le_bytes_mod_order(&hasher.finalize()))
}

fn derive_challenge(
    statement: &SfcsZkPrivateAddStatement,
    relation_commitment: &EdwardsProjective,
    nonce_commitment: &EdwardsProjective,
) -> Result<Fr, SfcsZkError> {
    let mut hasher = Sha256::new();
    hasher.update(ZK_CHALLENGE_DOMAIN);
    hasher.update(serde_json::to_vec(statement)?);
    hasher.update(point_to_bytes(relation_commitment)?);
    hasher.update(point_to_bytes(nonce_commitment)?);
    Ok(Fr::from_le_bytes_mod_order(&hasher.finalize()))
}

fn derive_private_vm_challenge(
    statement: &SfcsZkPrivateVmStatement,
    nonce_commitments: &[(String, Fr, Fr, String)],
) -> Result<Fr, SfcsZkError> {
    let mut hasher = Sha256::new();
    hasher.update(ZK_PRIVATE_VM_CHALLENGE_DOMAIN);
    hasher.update(serde_json::to_vec(statement)?);
    for (label, _, _, nonce_commitment) in nonce_commitments {
        hasher.update(label.as_bytes());
        hasher.update(nonce_commitment.as_bytes());
    }
    Ok(Fr::from_le_bytes_mod_order(&hasher.finalize()))
}

fn point_to_hex(point: &EdwardsProjective) -> Result<String, SfcsZkError> {
    Ok(format!(
        "{}{}",
        ZK_POINT_PREFIX,
        hex::encode(point_to_bytes(point)?)
    ))
}

fn point_from_hex(value: &str) -> Result<EdwardsProjective, SfcsZkError> {
    let Some(hex_value) = value.strip_prefix(ZK_POINT_PREFIX) else {
        return Err(SfcsZkError::InvalidProof(
            "point is missing edwards prefix".to_string(),
        ));
    };
    let bytes = hex::decode(hex_value)
        .map_err(|error| SfcsZkError::InvalidProof(format!("invalid point hex: {error}")))?;
    let affine = EdwardsAffine::deserialize_compressed(&*bytes)
        .map_err(|error| SfcsZkError::InvalidProof(format!("invalid point encoding: {error}")))?;
    Ok(affine.into_group())
}

fn point_to_bytes(point: &EdwardsProjective) -> Result<Vec<u8>, SfcsZkError> {
    let affine = point.into_affine();
    let mut bytes = Vec::new();
    affine
        .serialize_compressed(&mut bytes)
        .map_err(SfcsZkError::Serialization)?;
    Ok(bytes)
}

fn scalar_to_hex(scalar: &Fr) -> Result<String, SfcsZkError> {
    Ok(format!(
        "{}{}",
        ZK_SCALAR_PREFIX,
        hex::encode(scalar_to_bytes(scalar)?)
    ))
}

fn scalar_from_hex(value: &str) -> Result<Fr, SfcsZkError> {
    let Some(hex_value) = value.strip_prefix(ZK_SCALAR_PREFIX) else {
        return Err(SfcsZkError::InvalidProof(
            "scalar is missing fr prefix".to_string(),
        ));
    };
    let bytes = hex::decode(hex_value)
        .map_err(|error| SfcsZkError::InvalidProof(format!("invalid scalar hex: {error}")))?;
    Fr::deserialize_compressed(&*bytes)
        .map_err(|error| SfcsZkError::InvalidProof(format!("invalid scalar encoding: {error}")))
}

fn scalar_to_bytes(scalar: &Fr) -> Result<Vec<u8>, SfcsZkError> {
    let mut bytes = Vec::new();
    scalar
        .serialize_compressed(&mut bytes)
        .map_err(SfcsZkError::Serialization)?;
    Ok(bytes)
}

/// Errors returned by SFCS ZK profiles.
#[derive(Debug)]
pub enum SfcsZkError {
    /// Unsupported proof schema.
    UnsupportedSchema(String),
    /// Program does not match the proof profile.
    InvalidProgram(String),
    /// Private witness is invalid for this profile.
    InvalidWitness(String),
    /// Proof verification failed.
    InvalidProof(String),
    /// `.pha` embedding is inconsistent.
    InvalidEmbedding(String),
    /// VM program validation failed.
    Vm(super::vm::SfcsVmError),
    /// VM constraint proof construction failed.
    VmConstraint(SfcsVmConstraintError),
    /// SFCS graph validation failed.
    Sfcs(super::SfcsError),
    /// Serialization failed.
    Serialization(SerializationError),
    /// JSON serialization failed.
    Json(serde_json::Error),
    /// `.pha` construction or verification failed.
    Pha(PhaError),
}

impl fmt::Display for SfcsZkError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedSchema(schema) => {
                write!(formatter, "unsupported SFCS ZK schema: {schema}")
            }
            Self::InvalidProgram(message) => {
                write!(formatter, "invalid SFCS ZK program: {message}")
            }
            Self::InvalidWitness(message) => {
                write!(formatter, "invalid SFCS ZK witness: {message}")
            }
            Self::InvalidProof(message) => write!(formatter, "invalid SFCS ZK proof: {message}"),
            Self::InvalidEmbedding(message) => {
                write!(formatter, "invalid SFCS ZK embedding: {message}")
            }
            Self::Vm(error) => write!(formatter, "SFCS ZK VM error: {error}"),
            Self::VmConstraint(error) => {
                write!(formatter, "SFCS ZK VM constraint error: {error}")
            }
            Self::Sfcs(error) => write!(formatter, "SFCS ZK fractal error: {error}"),
            Self::Serialization(error) => write!(formatter, "SFCS ZK serialization error: {error}"),
            Self::Json(error) => write!(formatter, "SFCS ZK JSON error: {error}"),
            Self::Pha(error) => write!(formatter, "SFCS ZK PHA error: {error}"),
        }
    }
}

impl Error for SfcsZkError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Vm(error) => Some(error),
            Self::VmConstraint(error) => Some(error),
            Self::Sfcs(error) => Some(error),
            Self::Serialization(error) => Some(error),
            Self::Json(error) => Some(error),
            Self::Pha(error) => Some(error),
            _ => None,
        }
    }
}

impl From<serde_json::Error> for SfcsZkError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

impl From<PhaError> for SfcsZkError {
    fn from(error: PhaError) -> Self {
        Self::Pha(error)
    }
}

impl From<super::vm::SfcsVmError> for SfcsZkError {
    fn from(error: super::vm::SfcsVmError) -> Self {
        Self::Vm(error)
    }
}

impl From<SfcsVmConstraintError> for SfcsZkError {
    fn from(error: SfcsVmConstraintError) -> Self {
        Self::VmConstraint(error)
    }
}

impl From<super::SfcsError> for SfcsZkError {
    fn from(error: super::SfcsError) -> Self {
        Self::Sfcs(error)
    }
}
