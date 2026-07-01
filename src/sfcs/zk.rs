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
    vm::{SfcsVmExecutionTrace, SfcsVmInputs, SfcsVmProgram, SfcsVmPublicOutputs},
};
use crate::provenance::{PhaArtifact, PhaError};
use ark_ec::{AffineRepr, CurveGroup, Group};
use ark_ed_on_bn254::{EdwardsAffine, EdwardsProjective, Fr};
use ark_ff::{PrimeField, Zero};
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
    /// Number of ZK-checkable linear transition relations.
    pub linear_relation_checks: u64,
    /// Number of zero-knowledge 32-bit range proofs over private VM values.
    pub zk_range_proofs: u64,
    /// Number of zero-knowledge private memory consistency proofs.
    pub zk_memory_consistency_proofs: u64,
    /// Number of zero-knowledge memory access/register value binding proofs.
    pub zk_memory_value_proofs: u64,
    /// Number of zero-knowledge byte-level memory semantics proofs.
    pub zk_memory_byte_proofs: u64,
    /// Number of zero-knowledge private bitwise operation proofs.
    pub zk_bitwise_proofs: u64,
    /// Number of zero-knowledge private comparison proofs.
    pub zk_comparison_proofs: u64,
    /// Number of zero-knowledge private branch condition proofs.
    pub zk_branch_proofs: u64,
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

/// Homomorphic proof for one private linear VM transition relation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcsZkPrivateVmLinearRelationProof {
    /// VM step index.
    pub step_index: u64,
    /// Relation kind, e.g. `add` or `addi`.
    pub relation: String,
    /// Pedersen commitment to the left operand.
    pub lhs_commitment: String,
    /// Optional Pedersen commitment to the right operand.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rhs_commitment: Option<String>,
    /// Optional public constant used by immediate relations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_constant: Option<u32>,
    /// Pedersen commitment to the output value.
    pub output_commitment: String,
    /// Commitment to the zero-valued relation residual.
    pub relation_commitment: String,
    /// Schnorr nonce commitment for the relation blinding.
    pub nonce_commitment: String,
    /// Response proving the residual commitment opens with zero value.
    pub response_blinding: String,
}

/// One zero-knowledge bit proof inside a private VM range proof.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcsZkPrivateVmBitProof {
    /// Nonce commitment for the branch proving the bit is zero.
    pub zero_nonce_commitment: String,
    /// Nonce commitment for the branch proving the bit is one.
    pub one_nonce_commitment: String,
    /// Fiat-Shamir challenge share for the zero branch.
    pub zero_challenge: String,
    /// Fiat-Shamir challenge share for the one branch.
    pub one_challenge: String,
    /// Schnorr response for the zero branch.
    pub zero_response: String,
    /// Schnorr response for the one branch.
    pub one_response: String,
}

/// Zero-knowledge 32-bit range proof for one private VM value commitment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcsZkPrivateVmRangeProof {
    /// Deterministic value label, e.g. `linear:3:add:lhs`.
    pub label: String,
    /// Commitment being proven to carry a 32-bit value.
    pub value_commitment: String,
    /// Pedersen commitments to each little-endian bit.
    pub bit_commitments: Vec<String>,
    /// OR proofs that each bit commitment opens to either zero or one.
    pub bit_proofs: Vec<SfcsZkPrivateVmBitProof>,
    /// Homomorphic residual commitment tying the bits back to the value.
    pub recomposition_commitment: String,
    /// Nonce commitment for the recomposition residual proof.
    pub recomposition_nonce_commitment: String,
    /// Schnorr response for the recomposition residual blinding.
    pub recomposition_response_blinding: String,
}

/// Zero-knowledge equality proof between two private VM value commitments.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcsZkPrivateVmEqualityProof {
    /// Left commitment.
    pub left_commitment: String,
    /// Right commitment.
    pub right_commitment: String,
    /// Commitment to the zero-valued difference.
    pub difference_commitment: String,
    /// Nonce commitment for the difference blinding proof.
    pub nonce_commitment: String,
    /// Schnorr response for the difference blinding.
    pub response_blinding: String,
}

/// Zero-knowledge read-after-write consistency proof for private VM memory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcsZkPrivateVmMemoryConsistencyProof {
    /// Read step index.
    pub read_step_index: u64,
    /// Prior write step index supplying the read value.
    pub write_step_index: u64,
    /// Hidden address equality proof.
    pub address_equality: SfcsZkPrivateVmEqualityProof,
    /// Hidden value equality proof.
    pub value_equality: SfcsZkPrivateVmEqualityProof,
}

/// Zero-knowledge proof that a memory access is bound to the VM register
/// transition carrying its value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcsZkPrivateVmMemoryValueProof {
    /// Memory access step index.
    pub step_index: u64,
    /// `read` or `write`.
    pub kind: String,
    /// Hidden equality between the memory access value and the VM register
    /// value that produced or consumed it.
    pub value_equality: SfcsZkPrivateVmEqualityProof,
}

/// One branch inside a zero-knowledge selective OR proof.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcsZkPrivateVmSelectiveBranchProof {
    /// Nonce commitments, one for each constrained component in the branch.
    pub nonce_commitments: Vec<String>,
    /// Fiat-Shamir branch challenge share.
    pub challenge: String,
    /// Schnorr responses, one for each constrained component in the branch.
    pub responses: Vec<String>,
}

/// Zero-knowledge OR proof that committed components match one row of a
/// finite relation. `null` candidate entries are wildcards and are not
/// constrained for that branch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcsZkPrivateVmSelectiveProof {
    /// Stable proof label.
    pub label: String,
    /// Component commitments.
    pub commitments: Vec<String>,
    /// Candidate rows. `null` means wildcard for that component in that row.
    pub candidates: Vec<Vec<Option<u32>>>,
    /// One OR branch proof per candidate row.
    pub branches: Vec<SfcsZkPrivateVmSelectiveBranchProof>,
}

/// Zero-knowledge bitwise relation proof tied to range-proof bit commitments.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcsZkPrivateVmBitwiseProof {
    /// VM step index.
    pub step_index: u64,
    /// Operation: `and`, `or`, `xor`, `andi`, `ori`, or `xori`.
    pub operation: String,
    /// Range-proof label for lhs.
    pub lhs_range_label: String,
    /// Optional range-proof label for rhs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rhs_range_label: Option<String>,
    /// Optional public immediate value for immediate bitwise operations.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_constant: Option<u32>,
    /// Range-proof label for output.
    pub output_range_label: String,
    /// One finite-relation proof per bit.
    pub bit_proofs: Vec<SfcsZkPrivateVmSelectiveProof>,
}

/// Zero-knowledge comparison/order proof tied to range-proof commitments.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcsZkPrivateVmComparisonProof {
    /// VM step index or branch step index.
    pub step_index: u64,
    /// Relation: `slt`, `sltu`, `slti`, `sltiu`, `blt`, `bge`, `bltu`, or `bgeu`.
    pub relation: String,
    /// Whether the comparison uses signed order.
    pub signed: bool,
    /// Range-proof label for lhs.
    pub lhs_range_label: String,
    /// Optional range-proof label for rhs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rhs_range_label: Option<String>,
    /// Optional public immediate value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_constant: Option<u32>,
    /// Range-proof label for the hidden comparison result.
    pub output_range_label: String,
    /// Range-proof label for the 32-bit comparison slack.
    pub diff_range_label: String,
    /// Selective proof for the comparison predicate.
    pub relation_proof: SfcsZkPrivateVmSelectiveProof,
}

/// Zero-knowledge byte-level memory access proof.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcsZkPrivateVmMemoryByteProof {
    /// Memory access step index.
    pub step_index: u64,
    /// `read` or `write`.
    pub kind: String,
    /// Load/store mnemonic.
    pub mnemonic: String,
    /// Access width in bytes.
    pub width: u8,
    /// Byte-level read-after-write equality proofs for bytes supplied by a
    /// prior private write.
    pub byte_consistency: Vec<SfcsZkPrivateVmEqualityProof>,
    /// Proof tying the architectural register value to addressed bytes and
    /// the required sign/zero extension or low-byte extraction semantics.
    pub value_semantics: SfcsZkPrivateVmSelectiveProof,
}

/// Zero-knowledge proof for private branch conditions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcsZkPrivateVmBranchProof {
    /// Branch step index.
    pub step_index: u64,
    /// Branch mnemonic.
    pub branch: String,
    /// Public branch decision observed in the private execution trace.
    pub branch_taken: bool,
    /// Equality proof for `beq` taken and `bne` not taken.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub equality: Option<SfcsZkPrivateVmEqualityProof>,
    /// Non-equality/order proof for the remaining branch cases.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<SfcsZkPrivateVmSelectiveProof>,
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
    /// Homomorphic transition relation proofs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub linear_relation_proofs: Vec<SfcsZkPrivateVmLinearRelationProof>,
    /// Zero-knowledge u32 range proofs for VM values used by private relations.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub range_proofs: Vec<SfcsZkPrivateVmRangeProof>,
    /// Zero-knowledge memory read-after-write consistency proofs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub memory_consistency_proofs: Vec<SfcsZkPrivateVmMemoryConsistencyProof>,
    /// Zero-knowledge memory access/register value binding proofs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub memory_value_proofs: Vec<SfcsZkPrivateVmMemoryValueProof>,
    /// Zero-knowledge byte-level memory semantics proofs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub memory_byte_proofs: Vec<SfcsZkPrivateVmMemoryByteProof>,
    /// Zero-knowledge bitwise operation proofs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bitwise_proofs: Vec<SfcsZkPrivateVmBitwiseProof>,
    /// Zero-knowledge comparison/order proofs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub comparison_proofs: Vec<SfcsZkPrivateVmComparisonProof>,
    /// Zero-knowledge branch condition proofs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub branch_proofs: Vec<SfcsZkPrivateVmBranchProof>,
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
        let linear_relation_preimages =
            private_vm_linear_relation_preimages(&trace, &witness.blinding_seed)?;
        let memory_consistency_preimages =
            private_vm_memory_consistency_preimages(&trace, &witness.blinding_seed)?;
        let memory_value_preimages =
            private_vm_memory_value_preimages(&trace, &witness.blinding_seed)?;
        let memory_byte_preimages =
            private_vm_memory_byte_preimages(&trace, &witness.blinding_seed)?;
        let bitwise_preimages = private_vm_bitwise_preimages(&trace, &witness.blinding_seed)?;
        let comparison_preimages = private_vm_comparison_preimages(&trace, &witness.blinding_seed)?;
        let branch_preimages = private_vm_branch_preimages(&trace, &witness.blinding_seed)?;
        let range_proof_count = linear_relation_preimages
            .iter()
            .map(|relation| relation.range_inputs.len() as u64)
            .sum::<u64>()
            + memory_consistency_preimages
                .iter()
                .map(|proof| proof.range_inputs.len() as u64)
                .sum::<u64>()
            + memory_value_preimages
                .iter()
                .map(|proof| proof.range_inputs.len() as u64)
                .sum::<u64>()
            + memory_byte_preimages
                .iter()
                .map(|proof| proof.range_inputs.len() as u64)
                .sum::<u64>()
            + bitwise_preimages
                .iter()
                .map(|proof| proof.range_inputs.len() as u64)
                .sum::<u64>()
            + comparison_preimages
                .iter()
                .map(|proof| proof.range_inputs.len() as u64)
                .sum::<u64>()
            + branch_preimages
                .iter()
                .map(|proof| proof.range_inputs.len() as u64)
                .sum::<u64>();
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
            linear_relation_checks: linear_relation_preimages.len() as u64,
            zk_range_proofs: range_proof_count,
            zk_memory_consistency_proofs: memory_consistency_preimages.len() as u64,
            zk_memory_value_proofs: memory_value_preimages.len() as u64,
            zk_memory_byte_proofs: memory_byte_preimages.len() as u64,
            zk_bitwise_proofs: bitwise_preimages.len() as u64,
            zk_comparison_proofs: comparison_preimages.len() as u64,
            zk_branch_proofs: branch_preimages.len() as u64,
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
        let challenge = derive_private_vm_challenge(
            &statement,
            &nonce_commitments,
            &linear_relation_preimages,
        )?;
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
        let mut linear_relation_proofs = Vec::new();
        let mut range_proofs = Vec::new();
        for relation in linear_relation_preimages {
            for range_input in &relation.range_inputs {
                range_proofs.push(private_vm_range_proof(range_input, &witness.blinding_seed)?);
            }
            linear_relation_proofs.push(SfcsZkPrivateVmLinearRelationProof {
                step_index: relation.step_index,
                relation: relation.relation,
                lhs_commitment: point_to_hex(&relation.lhs_commitment)?,
                rhs_commitment: relation
                    .rhs_commitment
                    .as_ref()
                    .map(point_to_hex)
                    .transpose()?,
                public_constant: relation.public_constant,
                output_commitment: point_to_hex(&relation.output_commitment)?,
                relation_commitment: point_to_hex(&relation.relation_commitment)?,
                nonce_commitment: point_to_hex(&relation.nonce_commitment)?,
                response_blinding: scalar_to_hex(
                    &(relation.nonce_blinding + challenge * relation.relation_blinding),
                )?,
            });
        }
        let mut memory_consistency_proofs = Vec::new();
        for memory_preimage in memory_consistency_preimages {
            for range_input in &memory_preimage.range_inputs {
                range_proofs.push(private_vm_range_proof(range_input, &witness.blinding_seed)?);
            }
            memory_consistency_proofs.push(memory_preimage.proof);
        }
        let mut memory_value_proofs = Vec::new();
        for memory_preimage in memory_value_preimages {
            for range_input in &memory_preimage.range_inputs {
                range_proofs.push(private_vm_range_proof(range_input, &witness.blinding_seed)?);
            }
            memory_value_proofs.push(memory_preimage.proof);
        }
        let mut memory_byte_proofs = Vec::new();
        for memory_preimage in memory_byte_preimages {
            for range_input in &memory_preimage.range_inputs {
                range_proofs.push(private_vm_range_proof(range_input, &witness.blinding_seed)?);
            }
            memory_byte_proofs.push(memory_preimage.proof);
        }
        let mut bitwise_proofs = Vec::new();
        for bitwise_preimage in bitwise_preimages {
            for range_input in &bitwise_preimage.range_inputs {
                range_proofs.push(private_vm_range_proof(range_input, &witness.blinding_seed)?);
            }
            bitwise_proofs.push(bitwise_preimage.proof);
        }
        let mut comparison_proofs = Vec::new();
        for comparison_preimage in comparison_preimages {
            for range_input in &comparison_preimage.range_inputs {
                range_proofs.push(private_vm_range_proof(range_input, &witness.blinding_seed)?);
            }
            comparison_proofs.push(comparison_preimage.proof);
        }
        let mut branch_proofs = Vec::new();
        for branch_preimage in branch_preimages {
            for range_input in &branch_preimage.range_inputs {
                range_proofs.push(private_vm_range_proof(range_input, &witness.blinding_seed)?);
            }
            branch_proofs.push(branch_preimage.proof);
        }
        let mut proof = Self {
            statement,
            challenge: scalar_to_hex(&challenge)?,
            opening_proofs,
            linear_relation_proofs,
            range_proofs,
            memory_consistency_proofs,
            memory_value_proofs,
            memory_byte_proofs,
            bitwise_proofs,
            comparison_proofs,
            branch_proofs,
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
        if self.statement.linear_relation_checks != self.linear_relation_proofs.len() as u64 {
            return Err(SfcsZkError::InvalidProof(
                "private VM linear relation count does not match proofs".to_string(),
            ));
        }
        if self.statement.zk_range_proofs != self.range_proofs.len() as u64 {
            return Err(SfcsZkError::InvalidProof(
                "private VM range proof count does not match proofs".to_string(),
            ));
        }
        if self.statement.zk_memory_consistency_proofs
            != self.memory_consistency_proofs.len() as u64
        {
            return Err(SfcsZkError::InvalidProof(
                "private VM memory consistency proof count does not match proofs".to_string(),
            ));
        }
        if self.statement.zk_memory_value_proofs != self.memory_value_proofs.len() as u64 {
            return Err(SfcsZkError::InvalidProof(
                "private VM memory value proof count does not match proofs".to_string(),
            ));
        }
        if self.statement.zk_memory_byte_proofs != self.memory_byte_proofs.len() as u64 {
            return Err(SfcsZkError::InvalidProof(
                "private VM memory byte proof count does not match proofs".to_string(),
            ));
        }
        if self.statement.zk_bitwise_proofs != self.bitwise_proofs.len() as u64 {
            return Err(SfcsZkError::InvalidProof(
                "private VM bitwise proof count does not match proofs".to_string(),
            ));
        }
        if self.statement.zk_comparison_proofs != self.comparison_proofs.len() as u64 {
            return Err(SfcsZkError::InvalidProof(
                "private VM comparison proof count does not match proofs".to_string(),
            ));
        }
        if self.statement.zk_branch_proofs != self.branch_proofs.len() as u64 {
            return Err(SfcsZkError::InvalidProof(
                "private VM branch proof count does not match proofs".to_string(),
            ));
        }
        let mut previous_relation_step = None;
        for proof in &self.linear_relation_proofs {
            if previous_relation_step.is_some_and(|previous| proof.step_index <= previous) {
                return Err(SfcsZkError::InvalidProof(
                    "private VM linear relation proofs are not canonical".to_string(),
                ));
            }
            previous_relation_step = Some(proof.step_index);
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
        let expected_challenge = derive_private_vm_challenge_from_proof(
            &self.statement,
            &nonce_commitments,
            &self.linear_relation_proofs,
        )?;
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
        for proof in &self.linear_relation_proofs {
            proof.verify(challenge)?;
        }
        for proof in &self.range_proofs {
            proof.verify()?;
        }
        let range_map = self
            .range_proofs
            .iter()
            .map(|proof| (proof.label.as_str(), proof))
            .collect::<BTreeMap<_, _>>();
        for proof in &self.memory_consistency_proofs {
            proof.verify()?;
        }
        for proof in &self.memory_value_proofs {
            proof.verify()?;
        }
        for proof in &self.memory_byte_proofs {
            proof.verify()?;
        }
        for proof in &self.bitwise_proofs {
            proof.verify(&range_map)?;
        }
        for proof in &self.comparison_proofs {
            proof.verify(&range_map)?;
        }
        for proof in &self.branch_proofs {
            proof.verify()?;
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
                "linear_relation_checks": self.statement.linear_relation_checks,
                "zk_range_proofs": self.statement.zk_range_proofs,
                "zk_memory_consistency_proofs": self.statement.zk_memory_consistency_proofs,
                "zk_memory_value_proofs": self.statement.zk_memory_value_proofs,
                "zk_memory_byte_proofs": self.statement.zk_memory_byte_proofs,
                "zk_bitwise_proofs": self.statement.zk_bitwise_proofs,
                "zk_comparison_proofs": self.statement.zk_comparison_proofs,
                "zk_branch_proofs": self.statement.zk_branch_proofs,
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
            "linear_relation_proofs": self.linear_relation_proofs,
            "range_proofs": self.range_proofs,
            "memory_consistency_proofs": self.memory_consistency_proofs,
            "memory_value_proofs": self.memory_value_proofs,
            "memory_byte_proofs": self.memory_byte_proofs,
            "bitwise_proofs": self.bitwise_proofs,
            "comparison_proofs": self.comparison_proofs,
            "branch_proofs": self.branch_proofs,
        })
    }
}

impl SfcsZkPrivateVmLinearRelationProof {
    fn verify(&self, challenge: Fr) -> Result<(), SfcsZkError> {
        let lhs = point_from_hex(&self.lhs_commitment)?;
        let output = point_from_hex(&self.output_commitment)?;
        let relation_commitment = point_from_hex(&self.relation_commitment)?;
        let expected_relation = match self.relation.as_str() {
            "add" => {
                if self.public_constant.is_some() {
                    return Err(SfcsZkError::InvalidProof(
                        "add relation must not carry a public constant".to_string(),
                    ));
                }
                let rhs = self
                    .rhs_commitment
                    .as_ref()
                    .ok_or_else(|| {
                        SfcsZkError::InvalidProof(
                            "add relation requires rhs commitment".to_string(),
                        )
                    })
                    .and_then(|value| point_from_hex(value))?;
                output - lhs - rhs
            }
            "sub" => {
                if self.public_constant.is_some() {
                    return Err(SfcsZkError::InvalidProof(
                        "sub relation must not carry a public constant".to_string(),
                    ));
                }
                let rhs = self
                    .rhs_commitment
                    .as_ref()
                    .ok_or_else(|| {
                        SfcsZkError::InvalidProof(
                            "sub relation requires rhs commitment".to_string(),
                        )
                    })
                    .and_then(|value| point_from_hex(value))?;
                output - lhs + rhs
            }
            "addi" => {
                if self.rhs_commitment.is_some() {
                    return Err(SfcsZkError::InvalidProof(
                        "addi relation must not carry an rhs commitment".to_string(),
                    ));
                }
                let constant = self.public_constant.ok_or_else(|| {
                    SfcsZkError::InvalidProof("addi relation requires public constant".to_string())
                })?;
                output - lhs - value_base().mul_bigint(Fr::from(constant).into_bigint())
            }
            "subi" => {
                if self.rhs_commitment.is_some() {
                    return Err(SfcsZkError::InvalidProof(
                        "subi relation must not carry an rhs commitment".to_string(),
                    ));
                }
                let constant = self.public_constant.ok_or_else(|| {
                    SfcsZkError::InvalidProof("subi relation requires public constant".to_string())
                })?;
                output - lhs + value_base().mul_bigint(Fr::from(constant).into_bigint())
            }
            "scale" => {
                if self.rhs_commitment.is_some() {
                    return Err(SfcsZkError::InvalidProof(
                        "scale relation must not carry an rhs commitment".to_string(),
                    ));
                }
                let coefficient = self.public_constant.ok_or_else(|| {
                    SfcsZkError::InvalidProof(
                        "scale relation requires public coefficient".to_string(),
                    )
                })?;
                output - lhs.mul_bigint(Fr::from(coefficient).into_bigint())
            }
            other => {
                return Err(SfcsZkError::InvalidProof(format!(
                    "unsupported private VM linear relation {other}"
                )));
            }
        };
        if relation_commitment != expected_relation {
            return Err(SfcsZkError::InvalidProof(format!(
                "linear relation commitment mismatch at step {}",
                self.step_index
            )));
        }
        let nonce_commitment = point_from_hex(&self.nonce_commitment)?;
        let response = scalar_from_hex(&self.response_blinding)?;
        let left = blinding_base().mul_bigint(response.into_bigint());
        let right = nonce_commitment + relation_commitment.mul_bigint(challenge.into_bigint());
        if left != right {
            return Err(SfcsZkError::InvalidProof(format!(
                "linear relation proof failed at step {}",
                self.step_index
            )));
        }
        Ok(())
    }
}

impl SfcsZkPrivateVmRangeProof {
    fn verify(&self) -> Result<(), SfcsZkError> {
        if self.bit_commitments.len() != 32 || self.bit_proofs.len() != 32 {
            return Err(SfcsZkError::InvalidProof(format!(
                "range proof {} must contain 32 bit commitments and proofs",
                self.label
            )));
        }
        let value_commitment = point_from_hex(&self.value_commitment)?;
        let mut bit_commitments = Vec::with_capacity(32);
        for value in &self.bit_commitments {
            bit_commitments.push(point_from_hex(value)?);
        }
        for (index, (bit_commitment, proof)) in bit_commitments
            .iter()
            .zip(self.bit_proofs.iter())
            .enumerate()
        {
            proof.verify(&self.label, index, bit_commitment)?;
        }

        let mut expected_recomposition = value_commitment;
        for (index, bit_commitment) in bit_commitments.iter().enumerate() {
            let coefficient = Fr::from(1_u64 << index);
            expected_recomposition -= bit_commitment.mul_bigint(coefficient.into_bigint());
        }
        let recomposition_commitment = point_from_hex(&self.recomposition_commitment)?;
        if recomposition_commitment != expected_recomposition {
            return Err(SfcsZkError::InvalidProof(format!(
                "range proof {} recomposition commitment mismatch",
                self.label
            )));
        }
        let nonce_commitment = point_from_hex(&self.recomposition_nonce_commitment)?;
        let response = scalar_from_hex(&self.recomposition_response_blinding)?;
        let challenge = derive_range_recomposition_challenge(
            &self.label,
            &value_commitment,
            &bit_commitments,
            &recomposition_commitment,
            &nonce_commitment,
        )?;
        let left = blinding_base().mul_bigint(response.into_bigint());
        let right = nonce_commitment + recomposition_commitment.mul_bigint(challenge.into_bigint());
        if left != right {
            return Err(SfcsZkError::InvalidProof(format!(
                "range proof {} recomposition proof failed",
                self.label
            )));
        }
        Ok(())
    }
}

impl SfcsZkPrivateVmBitProof {
    fn verify(
        &self,
        label: &str,
        index: usize,
        bit_commitment: &EdwardsProjective,
    ) -> Result<(), SfcsZkError> {
        let zero_nonce = point_from_hex(&self.zero_nonce_commitment)?;
        let one_nonce = point_from_hex(&self.one_nonce_commitment)?;
        let zero_challenge = scalar_from_hex(&self.zero_challenge)?;
        let one_challenge = scalar_from_hex(&self.one_challenge)?;
        let zero_response = scalar_from_hex(&self.zero_response)?;
        let one_response = scalar_from_hex(&self.one_response)?;
        let challenge =
            derive_range_bit_challenge(label, index, bit_commitment, &zero_nonce, &one_nonce)?;
        if zero_challenge + one_challenge != challenge {
            return Err(SfcsZkError::InvalidProof(format!(
                "range bit proof {label}[{index}] challenge split mismatch"
            )));
        }
        let zero_left = blinding_base().mul_bigint(zero_response.into_bigint());
        let zero_right = zero_nonce + bit_commitment.mul_bigint(zero_challenge.into_bigint());
        if zero_left != zero_right {
            return Err(SfcsZkError::InvalidProof(format!(
                "range bit proof {label}[{index}] zero branch failed"
            )));
        }
        let one_relation = *bit_commitment - value_base();
        let one_left = blinding_base().mul_bigint(one_response.into_bigint());
        let one_right = one_nonce + one_relation.mul_bigint(one_challenge.into_bigint());
        if one_left != one_right {
            return Err(SfcsZkError::InvalidProof(format!(
                "range bit proof {label}[{index}] one branch failed"
            )));
        }
        Ok(())
    }
}

impl SfcsZkPrivateVmEqualityProof {
    fn verify(&self, label: &str) -> Result<(), SfcsZkError> {
        let left = point_from_hex(&self.left_commitment)?;
        let right = point_from_hex(&self.right_commitment)?;
        let difference = point_from_hex(&self.difference_commitment)?;
        let expected_difference = left - right;
        if difference != expected_difference {
            return Err(SfcsZkError::InvalidProof(format!(
                "equality proof {label} difference commitment mismatch"
            )));
        }
        let nonce = point_from_hex(&self.nonce_commitment)?;
        let response = scalar_from_hex(&self.response_blinding)?;
        let challenge = derive_equality_challenge(label, &left, &right, &difference, &nonce)?;
        let left_side = blinding_base().mul_bigint(response.into_bigint());
        let right_side = nonce + difference.mul_bigint(challenge.into_bigint());
        if left_side != right_side {
            return Err(SfcsZkError::InvalidProof(format!(
                "equality proof {label} failed"
            )));
        }
        Ok(())
    }
}

impl SfcsZkPrivateVmSelectiveProof {
    fn verify(&self) -> Result<(), SfcsZkError> {
        if self.commitments.is_empty() {
            return Err(SfcsZkError::InvalidProof(format!(
                "selective proof {} has no commitments",
                self.label
            )));
        }
        if self.candidates.is_empty() || self.candidates.len() != self.branches.len() {
            return Err(SfcsZkError::InvalidProof(format!(
                "selective proof {} candidate/branch count mismatch",
                self.label
            )));
        }
        let commitments = self
            .commitments
            .iter()
            .map(|commitment| point_from_hex(commitment))
            .collect::<Result<Vec<_>, _>>()?;
        let mut nonce_branches = Vec::with_capacity(self.branches.len());
        let mut challenge_sum = Fr::from(0_u64);
        for (candidate, branch) in self.candidates.iter().zip(self.branches.iter()) {
            if candidate.len() != commitments.len() {
                return Err(SfcsZkError::InvalidProof(format!(
                    "selective proof {} candidate arity mismatch",
                    self.label
                )));
            }
            let constrained = candidate.iter().filter(|value| value.is_some()).count();
            if constrained == 0
                || branch.nonce_commitments.len() != constrained
                || branch.responses.len() != constrained
            {
                return Err(SfcsZkError::InvalidProof(format!(
                    "selective proof {} branch arity mismatch",
                    self.label
                )));
            }
            let nonces = branch
                .nonce_commitments
                .iter()
                .map(|nonce| point_from_hex(nonce))
                .collect::<Result<Vec<_>, _>>()?;
            nonce_branches.push(nonces);
            challenge_sum += scalar_from_hex(&branch.challenge)?;
        }
        let challenge = derive_selective_challenge(
            &self.label,
            &commitments,
            &self.candidates,
            &nonce_branches,
        )?;
        if challenge_sum != challenge {
            return Err(SfcsZkError::InvalidProof(format!(
                "selective proof {} challenge split mismatch",
                self.label
            )));
        }
        for ((candidate, branch), nonces) in self
            .candidates
            .iter()
            .zip(self.branches.iter())
            .zip(nonce_branches.iter())
        {
            let branch_challenge = scalar_from_hex(&branch.challenge)?;
            let mut constrained_index = 0;
            for (component_index, candidate_value) in candidate.iter().enumerate() {
                let Some(candidate_value) = candidate_value else {
                    continue;
                };
                let response = scalar_from_hex(&branch.responses[constrained_index])?;
                let nonce = nonces[constrained_index];
                let relation = commitments[component_index]
                    - value_base().mul_bigint(Fr::from(*candidate_value).into_bigint());
                let left = blinding_base().mul_bigint(response.into_bigint());
                let right = nonce + relation.mul_bigint(branch_challenge.into_bigint());
                if left != right {
                    return Err(SfcsZkError::InvalidProof(format!(
                        "selective proof {} branch component {} failed",
                        self.label, component_index
                    )));
                }
                constrained_index += 1;
            }
        }
        Ok(())
    }
}

impl SfcsZkPrivateVmBitwiseProof {
    fn verify(
        &self,
        range_map: &BTreeMap<&str, &SfcsZkPrivateVmRangeProof>,
    ) -> Result<(), SfcsZkError> {
        if self.bit_proofs.len() != 32 {
            return Err(SfcsZkError::InvalidProof(format!(
                "bitwise proof at step {} must contain 32 bit proofs",
                self.step_index
            )));
        }
        if matches!(self.operation.as_str(), "and" | "or" | "xor") {
            if self.rhs_range_label.is_none() || self.public_constant.is_some() {
                return Err(SfcsZkError::InvalidProof(format!(
                    "binary bitwise proof {} must carry rhs range label only",
                    self.operation
                )));
            }
        } else if matches!(self.operation.as_str(), "andi" | "ori" | "xori") {
            if self.rhs_range_label.is_some() || self.public_constant.is_none() {
                return Err(SfcsZkError::InvalidProof(format!(
                    "immediate bitwise proof {} must carry public constant only",
                    self.operation
                )));
            }
        } else {
            return Err(SfcsZkError::InvalidProof(format!(
                "unsupported bitwise proof operation {}",
                self.operation
            )));
        }
        let lhs = range_map
            .get(self.lhs_range_label.as_str())
            .ok_or_else(|| {
                SfcsZkError::InvalidProof(format!(
                    "bitwise proof missing lhs range label {}",
                    self.lhs_range_label
                ))
            })?;
        let output = range_map
            .get(self.output_range_label.as_str())
            .ok_or_else(|| {
                SfcsZkError::InvalidProof(format!(
                    "bitwise proof missing output range label {}",
                    self.output_range_label
                ))
            })?;
        let rhs = self
            .rhs_range_label
            .as_ref()
            .map(|label| {
                range_map.get(label.as_str()).ok_or_else(|| {
                    SfcsZkError::InvalidProof(format!(
                        "bitwise proof missing rhs range label {label}"
                    ))
                })
            })
            .transpose()?;
        for (index, proof) in self.bit_proofs.iter().enumerate() {
            let expected = if let Some(rhs) = rhs {
                vec![
                    lhs.bit_commitments[index].clone(),
                    rhs.bit_commitments[index].clone(),
                    output.bit_commitments[index].clone(),
                ]
            } else {
                vec![
                    lhs.bit_commitments[index].clone(),
                    output.bit_commitments[index].clone(),
                ]
            };
            if proof.commitments != expected {
                return Err(SfcsZkError::InvalidProof(format!(
                    "bitwise proof {} bit {} is not tied to range proof bits",
                    self.operation, index
                )));
            }
            proof.verify()?;
        }
        Ok(())
    }
}

impl SfcsZkPrivateVmComparisonProof {
    fn verify(
        &self,
        range_map: &BTreeMap<&str, &SfcsZkPrivateVmRangeProof>,
    ) -> Result<(), SfcsZkError> {
        let expected_signed = matches!(self.relation.as_str(), "slt" | "slti" | "blt" | "bge");
        if expected_signed != self.signed {
            return Err(SfcsZkError::InvalidProof(format!(
                "comparison proof {} signedness mismatch",
                self.relation
            )));
        }
        let lhs = range_map
            .get(self.lhs_range_label.as_str())
            .ok_or_else(|| {
                SfcsZkError::InvalidProof(format!(
                    "comparison proof missing lhs range label {}",
                    self.lhs_range_label
                ))
            })?;
        let output = range_map
            .get(self.output_range_label.as_str())
            .ok_or_else(|| {
                SfcsZkError::InvalidProof(format!(
                    "comparison proof missing output range label {}",
                    self.output_range_label
                ))
            })?;
        let diff = range_map
            .get(self.diff_range_label.as_str())
            .ok_or_else(|| {
                SfcsZkError::InvalidProof(format!(
                    "comparison proof missing diff range label {}",
                    self.diff_range_label
                ))
            })?;
        let mut expected = Vec::new();
        expected.push(output.bit_commitments[0].clone());
        expected.extend(output.bit_commitments.iter().skip(1).cloned());
        expected.push(lhs.bit_commitments[31].clone());
        let rhs = if let Some(rhs_label) = &self.rhs_range_label {
            Some(*range_map.get(rhs_label.as_str()).ok_or_else(|| {
                SfcsZkError::InvalidProof(format!(
                    "comparison proof missing rhs range label {rhs_label}"
                ))
            })?)
        } else {
            None
        };
        if let Some(rhs) = rhs {
            expected.push(rhs.bit_commitments[31].clone());
        }
        expected.push(diff.value_commitment.clone());
        let lhs_commitment = point_from_hex(&lhs.value_commitment)?;
        let rhs_commitment = if let Some(rhs) = rhs {
            point_from_hex(&rhs.value_commitment)?
        } else {
            value_base().mul_bigint(
                Fr::from(self.public_constant.ok_or_else(|| {
                    SfcsZkError::InvalidProof(format!(
                        "comparison proof {} missing rhs source",
                        self.relation
                    ))
                })?)
                .into_bigint(),
            )
        };
        let diff_commitment = point_from_hex(&diff.value_commitment)?;
        let residual_lt = rhs_commitment
            - lhs_commitment
            - value_base().mul_bigint(Fr::from(1_u64).into_bigint())
            - diff_commitment;
        let residual_ge = lhs_commitment - rhs_commitment - diff_commitment;
        expected.push(point_to_hex(&residual_lt)?);
        expected.push(point_to_hex(&residual_ge)?);
        if self.relation_proof.commitments != expected {
            return Err(SfcsZkError::InvalidProof(format!(
                "comparison proof {} is not tied to range proof commitments",
                self.relation
            )));
        }
        self.relation_proof.verify()
    }
}

impl SfcsZkPrivateVmMemoryByteProof {
    fn verify(&self) -> Result<(), SfcsZkError> {
        if !matches!(self.kind.as_str(), "read" | "write") {
            return Err(SfcsZkError::InvalidProof(format!(
                "unsupported memory byte proof kind {}",
                self.kind
            )));
        }
        if !matches!(self.width, 1 | 2 | 4) {
            return Err(SfcsZkError::InvalidProof(format!(
                "unsupported memory byte proof width {}",
                self.width
            )));
        }
        for (index, equality) in self.byte_consistency.iter().enumerate() {
            equality.verify(&format!(
                "memory-byte:{}:{}:{}",
                self.step_index, self.kind, index
            ))?;
        }
        self.value_semantics.verify()
    }
}

impl SfcsZkPrivateVmMemoryConsistencyProof {
    fn verify(&self) -> Result<(), SfcsZkError> {
        if self.write_step_index >= self.read_step_index {
            return Err(SfcsZkError::InvalidProof(
                "memory consistency proof must reference a prior write".to_string(),
            ));
        }
        let label = format!("memory:{}:{}", self.write_step_index, self.read_step_index);
        self.address_equality.verify(&format!("{label}:address"))?;
        self.value_equality.verify(&format!("{label}:value"))?;
        Ok(())
    }
}

impl SfcsZkPrivateVmMemoryValueProof {
    fn verify(&self) -> Result<(), SfcsZkError> {
        if !matches!(self.kind.as_str(), "read" | "write") {
            return Err(SfcsZkError::InvalidProof(format!(
                "unsupported memory value proof kind {}",
                self.kind
            )));
        }
        let label = format!("memory-value:{}:{}", self.step_index, self.kind);
        self.value_equality.verify(&label)?;
        Ok(())
    }
}

impl SfcsZkPrivateVmBranchProof {
    fn verify(&self) -> Result<(), SfcsZkError> {
        let label = format!(
            "branch:{}:{}:{}",
            self.step_index, self.branch, self.branch_taken
        );
        match (self.branch.as_str(), self.branch_taken) {
            ("beq", true) | ("bne", false) => {
                let Some(equality) = &self.equality else {
                    return Err(SfcsZkError::InvalidProof(format!(
                        "branch proof {label} missing equality proof"
                    )));
                };
                if self.condition.is_some() {
                    return Err(SfcsZkError::InvalidProof(format!(
                        "branch proof {label} must not carry a condition proof"
                    )));
                }
                equality.verify(&label)?;
            }
            ("beq", false)
            | ("bne", true)
            | ("blt", _)
            | ("bge", _)
            | ("bltu", _)
            | ("bgeu", _) => {
                let Some(condition) = &self.condition else {
                    return Err(SfcsZkError::InvalidProof(format!(
                        "branch proof {label} missing condition proof"
                    )));
                };
                if self.equality.is_some() {
                    return Err(SfcsZkError::InvalidProof(format!(
                        "branch proof {label} must not carry an equality proof"
                    )));
                }
                condition.verify()?;
            }
            _ => {
                return Err(SfcsZkError::InvalidProof(format!(
                    "unsupported branch proof {} taken={}",
                    self.branch, self.branch_taken
                )))
            }
        }
        Ok(())
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

#[derive(Debug, Clone)]
struct SfcsZkPrivateVmLinearRelationPreimage {
    step_index: u64,
    relation: String,
    lhs_commitment: EdwardsProjective,
    rhs_commitment: Option<EdwardsProjective>,
    public_constant: Option<u32>,
    output_commitment: EdwardsProjective,
    relation_commitment: EdwardsProjective,
    relation_blinding: Fr,
    nonce_commitment: EdwardsProjective,
    nonce_blinding: Fr,
    range_inputs: Vec<SfcsZkPrivateVmRangeInput>,
}

#[derive(Debug, Clone)]
struct SfcsZkPrivateVmRangeInput {
    label: String,
    value: u32,
    blinding: Fr,
    commitment: EdwardsProjective,
}

#[derive(Debug, Clone)]
struct SfcsZkPrivateVmMemoryConsistencyPreimage {
    proof: SfcsZkPrivateVmMemoryConsistencyProof,
    range_inputs: Vec<SfcsZkPrivateVmRangeInput>,
}

#[derive(Debug, Clone)]
struct SfcsZkPrivateVmMemoryValuePreimage {
    proof: SfcsZkPrivateVmMemoryValueProof,
    range_inputs: Vec<SfcsZkPrivateVmRangeInput>,
}

#[derive(Debug, Clone)]
struct SfcsZkPrivateVmMemoryBytePreimage {
    proof: SfcsZkPrivateVmMemoryByteProof,
    range_inputs: Vec<SfcsZkPrivateVmRangeInput>,
}

#[derive(Debug, Clone)]
struct SfcsZkPrivateVmBitwisePreimage {
    proof: SfcsZkPrivateVmBitwiseProof,
    range_inputs: Vec<SfcsZkPrivateVmRangeInput>,
}

#[derive(Debug, Clone)]
struct SfcsZkPrivateVmComparisonPreimage {
    proof: SfcsZkPrivateVmComparisonProof,
    range_inputs: Vec<SfcsZkPrivateVmRangeInput>,
}

#[derive(Debug, Clone)]
struct SfcsZkPrivateVmBranchPreimage {
    proof: SfcsZkPrivateVmBranchProof,
    range_inputs: Vec<SfcsZkPrivateVmRangeInput>,
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
        "linear_relation_checks",
        "zk_range_proofs",
        "zk_memory_consistency_proofs",
        "zk_memory_value_proofs",
        "zk_memory_byte_proofs",
        "zk_bitwise_proofs",
        "zk_comparison_proofs",
        "zk_branch_proofs",
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
            "linear_relation_checks" => {
                serde_json::json!(embedding.proof.statement.linear_relation_checks)
            }
            "zk_range_proofs" => serde_json::json!(embedding.proof.statement.zk_range_proofs),
            "zk_memory_consistency_proofs" => {
                serde_json::json!(embedding.proof.statement.zk_memory_consistency_proofs)
            }
            "zk_memory_value_proofs" => {
                serde_json::json!(embedding.proof.statement.zk_memory_value_proofs)
            }
            "zk_memory_byte_proofs" => {
                serde_json::json!(embedding.proof.statement.zk_memory_byte_proofs)
            }
            "zk_bitwise_proofs" => {
                serde_json::json!(embedding.proof.statement.zk_bitwise_proofs)
            }
            "zk_comparison_proofs" => {
                serde_json::json!(embedding.proof.statement.zk_comparison_proofs)
            }
            "zk_branch_proofs" => serde_json::json!(embedding.proof.statement.zk_branch_proofs),
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

fn private_vm_linear_relation_preimages(
    trace: &SfcsVmExecutionTrace,
    seed: &[u8; 32],
) -> Result<Vec<SfcsZkPrivateVmLinearRelationPreimage>, SfcsZkError> {
    let mut relations = Vec::new();
    for step in &trace.steps {
        if step.rd == Some(0) {
            continue;
        }
        match step.mnemonic.as_str() {
            "add" => {
                let (Some(lhs), Some(rhs), Some(output)) = (
                    step.rs1_value_before,
                    step.rs2_value_before,
                    step.rd_value_after,
                ) else {
                    continue;
                };
                if lhs.checked_add(rhs) != Some(output) {
                    continue;
                }
                relations.push(private_vm_binary_linear_relation_preimage(
                    step.step_index,
                    "add",
                    lhs,
                    rhs,
                    output,
                    seed,
                )?);
            }
            "sub" => {
                let (Some(lhs), Some(rhs), Some(output)) = (
                    step.rs1_value_before,
                    step.rs2_value_before,
                    step.rd_value_after,
                ) else {
                    continue;
                };
                if lhs.checked_sub(rhs) != Some(output) {
                    continue;
                }
                relations.push(private_vm_binary_linear_relation_preimage(
                    step.step_index,
                    "sub",
                    lhs,
                    rhs,
                    output,
                    seed,
                )?);
            }
            "addi" => {
                let (Some(lhs), Some(output), Some(immediate)) =
                    (step.rs1_value_before, step.rd_value_after, step.immediate)
                else {
                    continue;
                };
                if immediate >= 0 {
                    let constant = immediate as u32;
                    if lhs.checked_add(constant) != Some(output) {
                        continue;
                    }
                    relations.push(private_vm_immediate_linear_relation_preimage(
                        step.step_index,
                        "addi",
                        lhs,
                        constant,
                        output,
                        seed,
                    )?);
                } else {
                    let constant = immediate.unsigned_abs();
                    if lhs.checked_sub(constant) != Some(output) {
                        continue;
                    }
                    relations.push(private_vm_immediate_linear_relation_preimage(
                        step.step_index,
                        "subi",
                        lhs,
                        constant,
                        output,
                        seed,
                    )?);
                }
            }
            "slli" => {
                let (Some(lhs), Some(output), Some(immediate)) =
                    (step.rs1_value_before, step.rd_value_after, step.immediate)
                else {
                    continue;
                };
                let Ok(shift) = u32::try_from(immediate) else {
                    continue;
                };
                if shift >= 32 {
                    continue;
                }
                let coefficient = 1_u32 << shift;
                if lhs.checked_mul(coefficient) != Some(output) {
                    continue;
                }
                relations.push(private_vm_immediate_linear_relation_preimage(
                    step.step_index,
                    "scale",
                    lhs,
                    coefficient,
                    output,
                    seed,
                )?);
            }
            _ => {}
        }
        if matches!(
            step.mnemonic.as_str(),
            "lb" | "lh" | "lw" | "lbu" | "lhu" | "sb" | "sh" | "sw"
        ) {
            let (Some(access), Some(base), Some(immediate)) =
                (&step.memory_access, step.rs1_value_before, step.immediate)
            else {
                continue;
            };
            if immediate >= 0 {
                let constant = immediate as u32;
                if base.checked_add(constant) != Some(access.address) {
                    continue;
                }
                relations.push(private_vm_immediate_linear_relation_preimage(
                    step.step_index,
                    "addi",
                    base,
                    constant,
                    access.address,
                    seed,
                )?);
            } else {
                let constant = immediate.unsigned_abs();
                if base.checked_sub(constant) != Some(access.address) {
                    continue;
                }
                relations.push(private_vm_immediate_linear_relation_preimage(
                    step.step_index,
                    "subi",
                    base,
                    constant,
                    access.address,
                    seed,
                )?);
            }
        }
    }
    Ok(relations)
}

fn private_vm_range_proof(
    input: &SfcsZkPrivateVmRangeInput,
    seed: &[u8; 32],
) -> Result<SfcsZkPrivateVmRangeProof, SfcsZkError> {
    let mut bit_commitment_points = Vec::with_capacity(32);
    let mut bit_commitments = Vec::with_capacity(32);
    let mut bit_blindings = Vec::with_capacity(32);
    let mut bit_proofs = Vec::with_capacity(32);

    for index in 0..32 {
        let bit = (input.value >> index) & 1;
        let bit_label = format!("range:{}:bit:{index}", input.label);
        let bit_blinding = private_vm_blinding_scalar(&bit_label, seed);
        let bit_commitment = commit_secret(Fr::from(bit), bit_blinding);
        let bit_proof =
            private_vm_bit_proof(&input.label, index, bit, bit_blinding, bit_commitment, seed)?;
        bit_commitments.push(point_to_hex(&bit_commitment)?);
        bit_commitment_points.push(bit_commitment);
        bit_blindings.push(bit_blinding);
        bit_proofs.push(bit_proof);
    }

    let mut recomposition_commitment = input.commitment;
    let mut recomposition_blinding = input.blinding;
    for (index, (bit_commitment, bit_blinding)) in bit_commitment_points
        .iter()
        .zip(bit_blindings.iter())
        .enumerate()
    {
        let coefficient = Fr::from(1_u64 << index);
        recomposition_commitment -= bit_commitment.mul_bigint(coefficient.into_bigint());
        recomposition_blinding -= *bit_blinding * coefficient;
    }
    let nonce_blinding = private_vm_nonce_scalar(&input.label, "range-recomposition", seed);
    let nonce_commitment = blinding_base().mul_bigint(nonce_blinding.into_bigint());
    let challenge = derive_range_recomposition_challenge(
        &input.label,
        &input.commitment,
        &bit_commitment_points,
        &recomposition_commitment,
        &nonce_commitment,
    )?;
    Ok(SfcsZkPrivateVmRangeProof {
        label: input.label.clone(),
        value_commitment: point_to_hex(&input.commitment)?,
        bit_commitments,
        bit_proofs,
        recomposition_commitment: point_to_hex(&recomposition_commitment)?,
        recomposition_nonce_commitment: point_to_hex(&nonce_commitment)?,
        recomposition_response_blinding: scalar_to_hex(
            &(nonce_blinding + challenge * recomposition_blinding),
        )?,
    })
}

fn private_vm_memory_consistency_preimages(
    trace: &SfcsVmExecutionTrace,
    seed: &[u8; 32],
) -> Result<Vec<SfcsZkPrivateVmMemoryConsistencyPreimage>, SfcsZkError> {
    let mut writes = BTreeMap::<(u32, u8), (u64, u32)>::new();
    let mut proofs = Vec::new();
    for step in &trace.steps {
        let Some(access) = &step.memory_access else {
            continue;
        };
        let key = (access.address, access.width);
        match access.kind.as_str() {
            "write" => {
                writes.insert(key, (step.step_index, access.value));
            }
            "read" => {
                if let Some((write_step_index, write_value)) = writes.get(&key).copied() {
                    if write_value == access.value {
                        proofs.push(private_vm_memory_consistency_preimage(
                            write_step_index,
                            step.step_index,
                            access.address,
                            write_value,
                            access.value,
                            seed,
                        )?);
                    }
                }
            }
            _ => {}
        }
    }
    Ok(proofs)
}

fn private_vm_memory_consistency_preimage(
    write_step_index: u64,
    read_step_index: u64,
    address: u32,
    write_value: u32,
    read_value: u32,
    seed: &[u8; 32],
) -> Result<SfcsZkPrivateVmMemoryConsistencyPreimage, SfcsZkError> {
    let label = format!("memory:{write_step_index}:{read_step_index}");
    let write_address = private_vm_range_input(&format!("{label}:write-address"), address, seed);
    let read_address = private_vm_range_input(&format!("{label}:read-address"), address, seed);
    let write_value = private_vm_range_input(&format!("{label}:write-value"), write_value, seed);
    let read_value = private_vm_range_input(&format!("{label}:read-value"), read_value, seed);
    let address_equality = private_vm_equality_proof(
        &format!("{label}:address"),
        &write_address,
        &read_address,
        seed,
    )?;
    let value_equality =
        private_vm_equality_proof(&format!("{label}:value"), &write_value, &read_value, seed)?;
    Ok(SfcsZkPrivateVmMemoryConsistencyPreimage {
        proof: SfcsZkPrivateVmMemoryConsistencyProof {
            read_step_index,
            write_step_index,
            address_equality,
            value_equality,
        },
        range_inputs: vec![write_address, read_address, write_value, read_value],
    })
}

fn private_vm_memory_value_preimages(
    trace: &SfcsVmExecutionTrace,
    seed: &[u8; 32],
) -> Result<Vec<SfcsZkPrivateVmMemoryValuePreimage>, SfcsZkError> {
    let mut proofs = Vec::new();
    for step in &trace.steps {
        let Some(access) = &step.memory_access else {
            continue;
        };
        match access.kind.as_str() {
            "write" => {
                let Some(register_value) = step.rs2_value_before else {
                    continue;
                };
                if register_value != access.value {
                    continue;
                }
                proofs.push(private_vm_memory_value_preimage(
                    step.step_index,
                    "write",
                    register_value,
                    access.value,
                    seed,
                )?);
            }
            "read" => {
                let Some(register_value) = step.rd_value_after else {
                    continue;
                };
                if register_value != access.value {
                    continue;
                }
                proofs.push(private_vm_memory_value_preimage(
                    step.step_index,
                    "read",
                    register_value,
                    access.value,
                    seed,
                )?);
            }
            _ => {}
        }
    }
    Ok(proofs)
}

fn private_vm_memory_value_preimage(
    step_index: u64,
    kind: &str,
    register_value: u32,
    memory_value: u32,
    seed: &[u8; 32],
) -> Result<SfcsZkPrivateVmMemoryValuePreimage, SfcsZkError> {
    let label = format!("memory-value:{step_index}:{kind}");
    let register = private_vm_range_input(&format!("{label}:register"), register_value, seed);
    let memory = private_vm_range_input(&format!("{label}:memory"), memory_value, seed);
    let value_equality = private_vm_equality_proof(&label, &register, &memory, seed)?;
    Ok(SfcsZkPrivateVmMemoryValuePreimage {
        proof: SfcsZkPrivateVmMemoryValueProof {
            step_index,
            kind: kind.to_string(),
            value_equality,
        },
        range_inputs: vec![register, memory],
    })
}

fn private_vm_memory_byte_preimages(
    trace: &SfcsVmExecutionTrace,
    seed: &[u8; 32],
) -> Result<Vec<SfcsZkPrivateVmMemoryBytePreimage>, SfcsZkError> {
    let mut writes = BTreeMap::<u32, (u64, u8)>::new();
    let mut proofs = Vec::new();
    for step in &trace.steps {
        let Some(access) = &step.memory_access else {
            continue;
        };
        if access.bytes.len() != access.width as usize {
            return Err(SfcsZkError::InvalidWitness(format!(
                "memory access at step {} has inconsistent byte width",
                step.step_index
            )));
        }
        let mut consistency = Vec::new();
        let mut range_inputs = Vec::new();
        let mut byte_inputs = Vec::new();
        for (offset, byte) in access.bytes.iter().copied().enumerate() {
            let address = access.address.wrapping_add(offset as u32);
            let byte_input = private_vm_range_input(
                &format!("memory-byte:{}:byte:{offset}", step.step_index),
                byte as u32,
                seed,
            );
            if access.kind == "read" {
                if let Some((write_step, write_byte)) = writes.get(&address).copied() {
                    let write_input = private_vm_range_input(
                        &format!(
                            "memory-byte:{write_step}:byte:{offset}:read:{}",
                            step.step_index
                        ),
                        write_byte as u32,
                        seed,
                    );
                    consistency.push(private_vm_equality_proof(
                        &format!("memory-byte:{}:{}:{}", step.step_index, access.kind, offset),
                        &write_input,
                        &byte_input,
                        seed,
                    )?);
                    range_inputs.push(write_input);
                }
            }
            byte_inputs.push(byte_input.clone());
            range_inputs.push(byte_input);
        }
        let register_value = match access.kind.as_str() {
            "read" => step.rd_value_after,
            "write" => step.rs2_value_before,
            _ => None,
        }
        .ok_or_else(|| {
            SfcsZkError::InvalidWitness(format!(
                "memory access at step {} lacks register value",
                step.step_index
            ))
        })?;
        let register = private_vm_range_input(
            &format!("memory-byte:{}:register", step.step_index),
            register_value,
            seed,
        );
        let (value_semantics, extra_ranges) = private_vm_memory_value_semantics(
            PrivateVmMemoryValueInputs {
                step_index: step.step_index,
                mnemonic: &step.mnemonic,
                kind: access.kind.as_str(),
                width: access.width,
                register_value,
                register: &register,
                bytes: &byte_inputs,
            },
            seed,
        )?;
        range_inputs.push(register);
        range_inputs.extend(extra_ranges);
        proofs.push(SfcsZkPrivateVmMemoryBytePreimage {
            proof: SfcsZkPrivateVmMemoryByteProof {
                step_index: step.step_index,
                kind: access.kind.clone(),
                mnemonic: step.mnemonic.clone(),
                width: access.width,
                byte_consistency: consistency,
                value_semantics,
            },
            range_inputs,
        });
        if access.kind == "write" {
            for (offset, byte) in access.bytes.iter().copied().enumerate() {
                writes.insert(
                    access.address.wrapping_add(offset as u32),
                    (step.step_index, byte),
                );
            }
        }
    }
    Ok(proofs)
}

fn private_vm_bitwise_preimages(
    trace: &SfcsVmExecutionTrace,
    seed: &[u8; 32],
) -> Result<Vec<SfcsZkPrivateVmBitwisePreimage>, SfcsZkError> {
    let mut proofs = Vec::new();
    for step in &trace.steps {
        if step.rd == Some(0) {
            continue;
        }
        let op = step.mnemonic.as_str();
        if !matches!(op, "and" | "or" | "xor" | "andi" | "ori" | "xori") {
            continue;
        }
        let (Some(lhs), Some(output)) = (step.rs1_value_before, step.rd_value_after) else {
            continue;
        };
        let rhs = if matches!(op, "and" | "or" | "xor") {
            step.rs2_value_before
        } else {
            step.immediate.map(|value| value as u32)
        }
        .ok_or_else(|| {
            SfcsZkError::InvalidWitness(format!(
                "bitwise step {} lacks rhs source",
                step.step_index
            ))
        })?;
        let expected = match op {
            "and" | "andi" => lhs & rhs,
            "or" | "ori" => lhs | rhs,
            "xor" | "xori" => lhs ^ rhs,
            _ => unreachable!(),
        };
        if expected != output {
            return Err(SfcsZkError::InvalidWitness(format!(
                "bitwise step {} output does not match trace",
                step.step_index
            )));
        }
        let prefix = format!("bitwise:{}:{op}", step.step_index);
        let lhs_input = private_vm_range_input(&format!("{prefix}:lhs"), lhs, seed);
        let output_input = private_vm_range_input(&format!("{prefix}:output"), output, seed);
        let rhs_input = matches!(op, "and" | "or" | "xor")
            .then(|| private_vm_range_input(&format!("{prefix}:rhs"), rhs, seed));
        let lhs_bits = private_vm_range_bits(&lhs_input, seed)?;
        let output_bits = private_vm_range_bits(&output_input, seed)?;
        let rhs_bits = rhs_input
            .as_ref()
            .map(|input| private_vm_range_bits(input, seed))
            .transpose()?;
        let mut bit_proofs = Vec::new();
        for index in 0..32 {
            if let Some(rhs_bits) = &rhs_bits {
                let values = vec![lhs_bits[index].0, rhs_bits[index].0, output_bits[index].0];
                let blindings = vec![lhs_bits[index].1, rhs_bits[index].1, output_bits[index].1];
                let commitments = vec![lhs_bits[index].2, rhs_bits[index].2, output_bits[index].2];
                bit_proofs.push(private_vm_selective_proof(
                    &format!("{prefix}:bit:{index}"),
                    &values,
                    &blindings,
                    &commitments,
                    &bitwise_candidates(op, None),
                    seed,
                )?);
            } else {
                let constant_bit = (rhs >> index) & 1;
                let values = vec![lhs_bits[index].0, output_bits[index].0];
                let blindings = vec![lhs_bits[index].1, output_bits[index].1];
                let commitments = vec![lhs_bits[index].2, output_bits[index].2];
                bit_proofs.push(private_vm_selective_proof(
                    &format!("{prefix}:bit:{index}"),
                    &values,
                    &blindings,
                    &commitments,
                    &bitwise_candidates(op, Some(constant_bit)),
                    seed,
                )?);
            }
        }
        let mut range_inputs = vec![lhs_input.clone(), output_input.clone()];
        if let Some(rhs_input) = rhs_input.clone() {
            range_inputs.push(rhs_input);
        }
        proofs.push(SfcsZkPrivateVmBitwisePreimage {
            proof: SfcsZkPrivateVmBitwiseProof {
                step_index: step.step_index,
                operation: op.to_string(),
                lhs_range_label: lhs_input.label,
                rhs_range_label: rhs_input.as_ref().map(|input| input.label.clone()),
                public_constant: matches!(op, "andi" | "ori" | "xori").then_some(rhs),
                output_range_label: output_input.label,
                bit_proofs,
            },
            range_inputs,
        });
    }
    Ok(proofs)
}

fn private_vm_comparison_preimages(
    trace: &SfcsVmExecutionTrace,
    seed: &[u8; 32],
) -> Result<Vec<SfcsZkPrivateVmComparisonPreimage>, SfcsZkError> {
    let mut proofs = Vec::new();
    for step in &trace.steps {
        if step.rd == Some(0) {
            continue;
        }
        let relation = step.mnemonic.as_str();
        if !matches!(relation, "slt" | "sltu" | "slti" | "sltiu") {
            continue;
        }
        let (Some(lhs), Some(output)) = (step.rs1_value_before, step.rd_value_after) else {
            continue;
        };
        let rhs = if matches!(relation, "slt" | "sltu") {
            step.rs2_value_before
        } else {
            step.immediate.map(|value| value as u32)
        }
        .ok_or_else(|| {
            SfcsZkError::InvalidWitness(format!(
                "comparison step {} lacks rhs source",
                step.step_index
            ))
        })?;
        let signed = matches!(relation, "slt" | "slti");
        let expected = if signed {
            u32::from((lhs as i32) < (rhs as i32))
        } else {
            u32::from(lhs < rhs)
        };
        if output != expected {
            return Err(SfcsZkError::InvalidWitness(format!(
                "comparison step {} output does not match trace",
                step.step_index
            )));
        }
        proofs.push(private_vm_comparison_preimage(
            PrivateVmComparisonInputs {
                step_index: step.step_index,
                relation,
                signed,
                lhs_value: lhs,
                rhs_value: rhs,
                output_value: output,
                rhs_is_public_constant: !matches!(relation, "slt" | "sltu"),
            },
            seed,
        )?);
    }
    Ok(proofs)
}

fn private_vm_branch_preimages(
    trace: &SfcsVmExecutionTrace,
    seed: &[u8; 32],
) -> Result<Vec<SfcsZkPrivateVmBranchPreimage>, SfcsZkError> {
    let mut proofs = Vec::new();
    for step in &trace.steps {
        if !matches!(
            step.mnemonic.as_str(),
            "beq" | "bne" | "blt" | "bge" | "bltu" | "bgeu"
        ) {
            continue;
        }
        let (Some(left_value), Some(right_value)) = (step.rs1_value_before, step.rs2_value_before)
        else {
            continue;
        };
        proofs.push(private_vm_branch_preimage(
            step.step_index,
            &step.mnemonic,
            step.branch_taken,
            left_value,
            right_value,
            seed,
        )?);
    }
    Ok(proofs)
}

fn private_vm_branch_preimage(
    step_index: u64,
    branch: &str,
    branch_taken: bool,
    left_value: u32,
    right_value: u32,
    seed: &[u8; 32],
) -> Result<SfcsZkPrivateVmBranchPreimage, SfcsZkError> {
    let label = format!("branch:{step_index}:{branch}:{branch_taken}");
    let left = private_vm_range_input(&format!("{label}:left"), left_value, seed);
    let right = private_vm_range_input(&format!("{label}:right"), right_value, seed);
    if matches!((branch, branch_taken), ("beq", true) | ("bne", false)) {
        if left_value != right_value {
            return Err(SfcsZkError::InvalidWitness(format!(
                "branch {label} expected equal operands"
            )));
        }
        let equality = private_vm_equality_proof(&label, &left, &right, seed)?;
        return Ok(SfcsZkPrivateVmBranchPreimage {
            proof: SfcsZkPrivateVmBranchProof {
                step_index,
                branch: branch.to_string(),
                branch_taken,
                equality: Some(equality),
                condition: None,
            },
            range_inputs: vec![left, right],
        });
    }
    let condition = match branch {
        "beq" | "bne" => {
            if left_value == right_value {
                return Err(SfcsZkError::InvalidWitness(format!(
                    "branch {label} expected non-equal operands"
                )));
            }
            private_vm_inequality_condition(&label, &left, &right, seed)?
        }
        "bltu" | "bgeu" => {
            let output = u32::from(if branch == "bltu" {
                branch_taken
            } else {
                !branch_taken
            });
            private_vm_comparison_condition(&label, false, left_value, right_value, output, seed)?
        }
        "blt" | "bge" => {
            let output = u32::from(if branch == "blt" {
                branch_taken
            } else {
                !branch_taken
            });
            private_vm_comparison_condition(&label, true, left_value, right_value, output, seed)?
        }
        _ => {
            return Err(SfcsZkError::InvalidWitness(format!(
                "unsupported branch {branch}"
            )))
        }
    };
    let mut range_inputs = vec![left, right];
    range_inputs.extend(condition.1);
    Ok(SfcsZkPrivateVmBranchPreimage {
        proof: SfcsZkPrivateVmBranchProof {
            step_index,
            branch: branch.to_string(),
            branch_taken,
            equality: None,
            condition: Some(condition.0),
        },
        range_inputs,
    })
}

struct PrivateVmComparisonInputs<'a> {
    step_index: u64,
    relation: &'a str,
    signed: bool,
    lhs_value: u32,
    rhs_value: u32,
    output_value: u32,
    rhs_is_public_constant: bool,
}

fn private_vm_comparison_preimage(
    inputs: PrivateVmComparisonInputs<'_>,
    seed: &[u8; 32],
) -> Result<SfcsZkPrivateVmComparisonPreimage, SfcsZkError> {
    let PrivateVmComparisonInputs {
        step_index,
        relation,
        signed,
        lhs_value,
        rhs_value,
        output_value,
        rhs_is_public_constant,
    } = inputs;
    let label = format!("comparison:{step_index}:{relation}");
    let lhs = private_vm_range_input(&format!("{label}:lhs"), lhs_value, seed);
    let rhs = private_vm_range_input(&format!("{label}:rhs"), rhs_value, seed);
    let output = private_vm_range_input(&format!("{label}:output"), output_value, seed);
    let diff_value = comparison_diff(signed, lhs_value, rhs_value, output_value)?;
    let diff = private_vm_range_input(&format!("{label}:diff"), diff_value, seed);
    let condition = private_vm_comparison_condition_from_inputs(
        &label, signed, &lhs, &rhs, &output, &diff, seed,
    )?;
    let range_inputs = vec![lhs.clone(), output.clone(), diff.clone(), rhs.clone()];
    Ok(SfcsZkPrivateVmComparisonPreimage {
        proof: SfcsZkPrivateVmComparisonProof {
            step_index,
            relation: relation.to_string(),
            signed,
            lhs_range_label: lhs.label,
            rhs_range_label: Some(rhs.label),
            public_constant: rhs_is_public_constant.then_some(rhs_value),
            output_range_label: output.label,
            diff_range_label: diff.label,
            relation_proof: condition,
        },
        range_inputs,
    })
}

fn private_vm_comparison_condition(
    label: &str,
    signed: bool,
    lhs_value: u32,
    rhs_value: u32,
    output_value: u32,
    seed: &[u8; 32],
) -> Result<
    (
        SfcsZkPrivateVmSelectiveProof,
        Vec<SfcsZkPrivateVmRangeInput>,
    ),
    SfcsZkError,
> {
    let lhs = private_vm_range_input(&format!("{label}:lhs"), lhs_value, seed);
    let rhs = private_vm_range_input(&format!("{label}:rhs"), rhs_value, seed);
    let output = private_vm_range_input(&format!("{label}:output"), output_value, seed);
    let diff = private_vm_range_input(
        &format!("{label}:diff"),
        comparison_diff(signed, lhs_value, rhs_value, output_value)?,
        seed,
    );
    let proof = private_vm_comparison_condition_from_inputs(
        label, signed, &lhs, &rhs, &output, &diff, seed,
    )?;
    Ok((proof, vec![lhs, rhs, output, diff]))
}

fn private_vm_comparison_condition_from_inputs(
    label: &str,
    signed: bool,
    lhs: &SfcsZkPrivateVmRangeInput,
    rhs: &SfcsZkPrivateVmRangeInput,
    output: &SfcsZkPrivateVmRangeInput,
    diff: &SfcsZkPrivateVmRangeInput,
    seed: &[u8; 32],
) -> Result<SfcsZkPrivateVmSelectiveProof, SfcsZkError> {
    let lhs_bits = private_vm_range_bits(lhs, seed)?;
    let rhs_bits = private_vm_range_bits(rhs, seed)?;
    let output_bits = private_vm_range_bits(output, seed)?;
    let residual_lt_commitment = rhs.commitment
        - lhs.commitment
        - value_base().mul_bigint(Fr::from(1_u64).into_bigint())
        - diff.commitment;
    let residual_lt_blinding = rhs.blinding - lhs.blinding - diff.blinding;
    let residual_ge_commitment = lhs.commitment - rhs.commitment - diff.commitment;
    let residual_ge_blinding = lhs.blinding - rhs.blinding - diff.blinding;
    let mut values = Vec::new();
    let mut blindings = Vec::new();
    let mut commitments = Vec::new();
    for bit in &output_bits {
        values.push(bit.0);
        blindings.push(bit.1);
        commitments.push(bit.2);
    }
    values.push(lhs_bits[31].0);
    blindings.push(lhs_bits[31].1);
    commitments.push(lhs_bits[31].2);
    values.push(rhs_bits[31].0);
    blindings.push(rhs_bits[31].1);
    commitments.push(rhs_bits[31].2);
    values.push(diff.value);
    blindings.push(diff.blinding);
    commitments.push(diff.commitment);
    values.push(0);
    blindings.push(residual_lt_blinding);
    commitments.push(residual_lt_commitment);
    values.push(0);
    blindings.push(residual_ge_blinding);
    commitments.push(residual_ge_commitment);
    let candidates = comparison_candidates(signed);
    private_vm_selective_proof(label, &values, &blindings, &commitments, &candidates, seed)
}

fn private_vm_inequality_condition(
    label: &str,
    lhs: &SfcsZkPrivateVmRangeInput,
    rhs: &SfcsZkPrivateVmRangeInput,
    seed: &[u8; 32],
) -> Result<
    (
        SfcsZkPrivateVmSelectiveProof,
        Vec<SfcsZkPrivateVmRangeInput>,
    ),
    SfcsZkError,
> {
    let lhs_value = lhs.value;
    let rhs_value = rhs.value;
    let diff_value = if lhs_value < rhs_value {
        rhs_value.wrapping_sub(lhs_value).wrapping_sub(1)
    } else {
        lhs_value.wrapping_sub(rhs_value).wrapping_sub(1)
    };
    let diff = private_vm_range_input(&format!("{label}:neq-diff"), diff_value, seed);
    let residual_lt_commitment = rhs.commitment
        - lhs.commitment
        - value_base().mul_bigint(Fr::from(1_u64).into_bigint())
        - diff.commitment;
    let residual_lt_blinding = rhs.blinding - lhs.blinding - diff.blinding;
    let residual_gt_commitment = lhs.commitment
        - rhs.commitment
        - value_base().mul_bigint(Fr::from(1_u64).into_bigint())
        - diff.commitment;
    let residual_gt_blinding = lhs.blinding - rhs.blinding - diff.blinding;
    let values = vec![diff.value, 0, 0];
    let blindings = vec![diff.blinding, residual_lt_blinding, residual_gt_blinding];
    let commitments = vec![
        diff.commitment,
        residual_lt_commitment,
        residual_gt_commitment,
    ];
    let candidates = if lhs_value < rhs_value {
        vec![vec![None, Some(0), None], vec![None, None, Some(0)]]
    } else {
        vec![vec![None, None, Some(0)], vec![None, Some(0), None]]
    };
    Ok((
        private_vm_selective_proof(label, &values, &blindings, &commitments, &candidates, seed)?,
        vec![diff],
    ))
}

fn comparison_diff(signed: bool, lhs: u32, rhs: u32, output: u32) -> Result<u32, SfcsZkError> {
    if output > 1 {
        return Err(SfcsZkError::InvalidWitness(
            "comparison output must be 0 or 1".to_string(),
        ));
    }
    if signed {
        let lhs_sign = (lhs >> 31) & 1;
        let rhs_sign = (rhs >> 31) & 1;
        if lhs_sign != rhs_sign {
            return Ok(0);
        }
    }
    if output == 1 {
        rhs.checked_sub(lhs)
            .and_then(|value| value.checked_sub(1))
            .ok_or_else(|| {
                SfcsZkError::InvalidWitness("invalid less-than comparison slack".to_string())
            })
    } else {
        lhs.checked_sub(rhs).ok_or_else(|| {
            SfcsZkError::InvalidWitness("invalid greater/equal comparison slack".to_string())
        })
    }
}

struct PrivateVmMemoryValueInputs<'a> {
    step_index: u64,
    mnemonic: &'a str,
    kind: &'a str,
    width: u8,
    register_value: u32,
    register: &'a SfcsZkPrivateVmRangeInput,
    bytes: &'a [SfcsZkPrivateVmRangeInput],
}

fn private_vm_memory_value_semantics(
    inputs: PrivateVmMemoryValueInputs<'_>,
    seed: &[u8; 32],
) -> Result<
    (
        SfcsZkPrivateVmSelectiveProof,
        Vec<SfcsZkPrivateVmRangeInput>,
    ),
    SfcsZkError,
> {
    let PrivateVmMemoryValueInputs {
        step_index,
        mnemonic,
        kind,
        width,
        register_value,
        register,
        bytes,
    } = inputs;
    if bytes.len() != width as usize {
        return Err(SfcsZkError::InvalidWitness(format!(
            "memory step {step_index} byte count does not match width"
        )));
    }
    let raw = bytes.iter().enumerate().fold(0_u32, |acc, (index, byte)| {
        acc | (byte.value << (index * 8))
    });
    let signed_load = matches!(mnemonic, "lb" | "lh");
    let unsigned_or_word = matches!(mnemonic, "lbu" | "lhu" | "lw");
    let store = kind == "write";
    let low_mask = match width {
        1 => 0x0000_00ff,
        2 => 0x0000_ffff,
        4 => u32::MAX,
        _ => {
            return Err(SfcsZkError::InvalidWitness(format!(
                "unsupported memory width {width}"
            )))
        }
    };
    let extension = match width {
        1 => 0xffff_ff00,
        2 => 0xffff_0000,
        4 => 0,
        _ => unreachable!(),
    };
    let raw_commitment =
        bytes
            .iter()
            .enumerate()
            .fold(EdwardsProjective::zero(), |acc, (index, byte)| {
                acc + byte
                    .commitment
                    .mul_bigint(Fr::from(1_u64 << (index * 8)).into_bigint())
            });
    let raw_blinding = bytes
        .iter()
        .enumerate()
        .fold(Fr::from(0_u64), |acc, (index, byte)| {
            acc + byte.blinding * Fr::from(1_u64 << (index * 8))
        });
    let mut values = Vec::new();
    let mut blindings = Vec::new();
    let mut commitments = Vec::new();
    for byte in bytes {
        let bits = private_vm_range_bits(byte, seed)?;
        for bit in bits.iter().skip(8) {
            values.push(bit.0);
            blindings.push(bit.1);
            commitments.push(bit.2);
        }
    }
    let zero_residual = register.commitment - raw_commitment;
    let zero_residual_blinding = register.blinding - raw_blinding;
    values.push(0);
    blindings.push(zero_residual_blinding);
    commitments.push(zero_residual);
    let sign_residual = register.commitment
        - raw_commitment
        - value_base().mul_bigint(Fr::from(extension).into_bigint());
    let sign_residual_blinding = register.blinding - raw_blinding;
    values.push(0);
    blindings.push(sign_residual_blinding);
    commitments.push(sign_residual);
    let sign_bit = if width == 4 {
        None
    } else {
        let top_byte = bytes.last().ok_or_else(|| {
            SfcsZkError::InvalidWitness(format!("memory step {step_index} has no bytes"))
        })?;
        let bits = private_vm_range_bits(top_byte, seed)?;
        let bit = bits[7];
        values.push(bit.0);
        blindings.push(bit.1);
        commitments.push(bit.2);
        Some(bit.0)
    };
    let mut base = vec![Some(0); bytes.len() * 24];
    if store {
        if register_value & low_mask != raw {
            return Err(SfcsZkError::InvalidWitness(format!(
                "store step {step_index} low bytes do not match register"
            )));
        }
        let shift = (width as u32) * 8;
        let coefficient = if shift == 32 { 0 } else { 1_u64 << shift };
        let high_value = if shift == 32 {
            0
        } else {
            register_value >> shift
        };
        let high = private_vm_range_input(
            &format!("memory-byte:{step_index}:{mnemonic}:store-high"),
            high_value,
            seed,
        );
        let trunc_residual = if shift == 32 {
            register.commitment - raw_commitment
        } else {
            register.commitment
                - raw_commitment
                - high
                    .commitment
                    .mul_bigint(Fr::from(coefficient).into_bigint())
        };
        let trunc_residual_blinding = if shift == 32 {
            register.blinding - raw_blinding
        } else {
            register.blinding - raw_blinding - high.blinding * Fr::from(coefficient)
        };
        values.push(0);
        blindings.push(trunc_residual_blinding);
        commitments.push(trunc_residual);
        base.push(None);
        base.push(None);
        if sign_bit.is_some() {
            base.push(None);
        }
        base.push(Some(0));
        return Ok((
            private_vm_selective_proof(
                &format!("memory-byte:{step_index}:{mnemonic}:store"),
                &values,
                &blindings,
                &commitments,
                &[base],
                seed,
            )?,
            if shift == 32 { Vec::new() } else { vec![high] },
        ));
    }
    if unsigned_or_word {
        if register_value != raw {
            return Err(SfcsZkError::InvalidWitness(format!(
                "load step {step_index} zero-extension does not match register"
            )));
        }
        base.push(Some(0));
        base.push(None);
        if sign_bit.is_some() {
            base.push(None);
        }
        return Ok((
            private_vm_selective_proof(
                &format!("memory-byte:{step_index}:{mnemonic}:unsigned"),
                &values,
                &blindings,
                &commitments,
                &[base],
                seed,
            )?,
            Vec::new(),
        ));
    }
    if signed_load {
        let sign_bit = sign_bit.ok_or_else(|| {
            SfcsZkError::InvalidWitness(format!("signed load step {step_index} missing sign bit"))
        })?;
        let expected = if sign_bit == 0 { raw } else { raw | extension };
        if register_value != expected {
            return Err(SfcsZkError::InvalidWitness(format!(
                "load step {step_index} sign-extension does not match register"
            )));
        }
        let mut positive = base.clone();
        positive.push(Some(0));
        positive.push(None);
        positive.push(Some(0));
        let mut negative = base;
        negative.push(None);
        negative.push(Some(0));
        negative.push(Some(1));
        return Ok((
            private_vm_selective_proof(
                &format!("memory-byte:{step_index}:{mnemonic}:signed"),
                &values,
                &blindings,
                &commitments,
                &[positive, negative],
                seed,
            )?,
            Vec::new(),
        ));
    }
    Err(SfcsZkError::InvalidWitness(format!(
        "unsupported memory semantic proof for {mnemonic}"
    )))
}

fn private_vm_range_bits(
    input: &SfcsZkPrivateVmRangeInput,
    seed: &[u8; 32],
) -> Result<Vec<(u32, Fr, EdwardsProjective)>, SfcsZkError> {
    let mut bits = Vec::with_capacity(32);
    for index in 0..32 {
        let bit = (input.value >> index) & 1;
        let bit_label = format!("range:{}:bit:{index}", input.label);
        let bit_blinding = private_vm_blinding_scalar(&bit_label, seed);
        let bit_commitment = commit_secret(Fr::from(bit), bit_blinding);
        bits.push((bit, bit_blinding, bit_commitment));
    }
    Ok(bits)
}

fn bitwise_candidates(operation: &str, immediate_bit: Option<u32>) -> Vec<Vec<Option<u32>>> {
    let mut rows = Vec::new();
    for lhs in 0..=1 {
        let rhs_values: Vec<u32> = immediate_bit.map_or_else(|| vec![0, 1], |bit| vec![bit]);
        for rhs in rhs_values {
            let output = match operation {
                "and" | "andi" => lhs & rhs,
                "or" | "ori" => lhs | rhs,
                "xor" | "xori" => lhs ^ rhs,
                _ => 0,
            };
            if immediate_bit.is_some() {
                rows.push(vec![Some(lhs), Some(output)]);
            } else {
                rows.push(vec![Some(lhs), Some(rhs), Some(output)]);
            }
        }
    }
    rows
}

fn comparison_candidates(signed: bool) -> Vec<Vec<Option<u32>>> {
    let output_bits = |value: u32| {
        let mut row = vec![Some(value)];
        row.extend((1..32).map(|_| Some(0)));
        row
    };
    let mut rows = Vec::new();
    if signed {
        let mut sign_true = output_bits(1);
        sign_true.extend([Some(1), Some(0), None, None, None]);
        rows.push(sign_true);
        let mut sign_false = output_bits(0);
        sign_false.extend([Some(0), Some(1), None, None, None]);
        rows.push(sign_false);
        let mut pos_lt = output_bits(1);
        pos_lt.extend([Some(0), Some(0), None, Some(0), None]);
        rows.push(pos_lt);
        let mut pos_ge = output_bits(0);
        pos_ge.extend([Some(0), Some(0), None, None, Some(0)]);
        rows.push(pos_ge);
        let mut neg_lt = output_bits(1);
        neg_lt.extend([Some(1), Some(1), None, Some(0), None]);
        rows.push(neg_lt);
        let mut neg_ge = output_bits(0);
        neg_ge.extend([Some(1), Some(1), None, None, Some(0)]);
        rows.push(neg_ge);
    } else {
        let mut lt = output_bits(1);
        lt.extend([None, None, None, Some(0), None]);
        rows.push(lt);
        let mut ge = output_bits(0);
        ge.extend([None, None, None, None, Some(0)]);
        rows.push(ge);
    }
    rows
}

fn private_vm_selective_proof(
    label: &str,
    values: &[u32],
    blindings: &[Fr],
    commitments: &[EdwardsProjective],
    candidates: &[Vec<Option<u32>>],
    seed: &[u8; 32],
) -> Result<SfcsZkPrivateVmSelectiveProof, SfcsZkError> {
    if values.len() != blindings.len() || values.len() != commitments.len() {
        return Err(SfcsZkError::InvalidWitness(format!(
            "selective proof {label} input arity mismatch"
        )));
    }
    let actual_index = candidates
        .iter()
        .position(|candidate| {
            candidate.len() == values.len()
                && candidate
                    .iter()
                    .zip(values.iter())
                    .all(|(candidate, value)| candidate.is_none_or(|candidate| candidate == *value))
        })
        .ok_or_else(|| {
            SfcsZkError::InvalidWitness(format!(
                "selective proof {label} has no candidate for actual values"
            ))
        })?;
    let mut branches = Vec::with_capacity(candidates.len());
    let mut nonce_points = Vec::with_capacity(candidates.len());
    let mut simulated_challenge_sum = Fr::from(0_u64);
    let mut actual_nonces = Vec::new();
    for (branch_index, candidate) in candidates.iter().enumerate() {
        let constrained = candidate
            .iter()
            .enumerate()
            .filter(|(_, value)| value.is_some())
            .map(|(component_index, _)| component_index)
            .collect::<Vec<_>>();
        if constrained.is_empty() {
            return Err(SfcsZkError::InvalidWitness(format!(
                "selective proof {label} candidate {branch_index} has no constraints"
            )));
        }
        if branch_index == actual_index {
            let mut nonces = Vec::new();
            let mut nonce_hex = Vec::new();
            for component_index in &constrained {
                let nonce = private_vm_nonce_scalar(
                    &format!("{label}:branch:{branch_index}:component:{component_index}"),
                    "actual",
                    seed,
                );
                let nonce_point = blinding_base().mul_bigint(nonce.into_bigint());
                nonce_hex.push(point_to_hex(&nonce_point)?);
                nonces.push(nonce);
            }
            actual_nonces = nonces;
            nonce_points.push(
                nonce_hex
                    .iter()
                    .map(|nonce| point_from_hex(nonce))
                    .collect::<Result<Vec<_>, _>>()?,
            );
            branches.push(SfcsZkPrivateVmSelectiveBranchProof {
                nonce_commitments: nonce_hex,
                challenge: String::new(),
                responses: Vec::new(),
            });
        } else {
            let challenge = private_vm_nonce_scalar(
                &format!("{label}:branch:{branch_index}"),
                "simulated-challenge",
                seed,
            );
            simulated_challenge_sum += challenge;
            let mut nonce_hex = Vec::new();
            let mut responses = Vec::new();
            let mut nonce_branch = Vec::new();
            for component_index in &constrained {
                let response = private_vm_nonce_scalar(
                    &format!("{label}:branch:{branch_index}:component:{component_index}"),
                    "simulated-response",
                    seed,
                );
                let candidate_value = candidate[*component_index].unwrap();
                let relation = commitments[*component_index]
                    - value_base().mul_bigint(Fr::from(candidate_value).into_bigint());
                let nonce_point = blinding_base().mul_bigint(response.into_bigint())
                    - relation.mul_bigint(challenge.into_bigint());
                nonce_hex.push(point_to_hex(&nonce_point)?);
                nonce_branch.push(nonce_point);
                responses.push(scalar_to_hex(&response)?);
            }
            nonce_points.push(nonce_branch);
            branches.push(SfcsZkPrivateVmSelectiveBranchProof {
                nonce_commitments: nonce_hex,
                challenge: scalar_to_hex(&challenge)?,
                responses,
            });
        }
    }
    let challenge = derive_selective_challenge(label, commitments, candidates, &nonce_points)?;
    let actual_challenge = challenge - simulated_challenge_sum;
    let actual_constrained = candidates[actual_index]
        .iter()
        .enumerate()
        .filter(|(_, value)| value.is_some())
        .map(|(component_index, _)| component_index)
        .collect::<Vec<_>>();
    let responses = actual_constrained
        .iter()
        .zip(actual_nonces.iter())
        .map(|(component_index, nonce)| {
            scalar_to_hex(&(*nonce + actual_challenge * blindings[*component_index]))
        })
        .collect::<Result<Vec<_>, _>>()?;
    branches[actual_index].challenge = scalar_to_hex(&actual_challenge)?;
    branches[actual_index].responses = responses;
    Ok(SfcsZkPrivateVmSelectiveProof {
        label: label.to_string(),
        commitments: commitments
            .iter()
            .map(point_to_hex)
            .collect::<Result<Vec<_>, _>>()?,
        candidates: candidates.to_vec(),
        branches,
    })
}

fn private_vm_range_input(label: &str, value: u32, seed: &[u8; 32]) -> SfcsZkPrivateVmRangeInput {
    let blinding = private_vm_blinding_scalar(label, seed);
    let commitment = pedersen_commit(value, blinding);
    SfcsZkPrivateVmRangeInput {
        label: label.to_string(),
        value,
        blinding,
        commitment,
    }
}

fn private_vm_equality_proof(
    label: &str,
    left: &SfcsZkPrivateVmRangeInput,
    right: &SfcsZkPrivateVmRangeInput,
    seed: &[u8; 32],
) -> Result<SfcsZkPrivateVmEqualityProof, SfcsZkError> {
    let difference_commitment = left.commitment - right.commitment;
    let difference_blinding = left.blinding - right.blinding;
    let nonce_blinding = private_vm_nonce_scalar(label, "equality", seed);
    let nonce_commitment = blinding_base().mul_bigint(nonce_blinding.into_bigint());
    let challenge = derive_equality_challenge(
        label,
        &left.commitment,
        &right.commitment,
        &difference_commitment,
        &nonce_commitment,
    )?;
    Ok(SfcsZkPrivateVmEqualityProof {
        left_commitment: point_to_hex(&left.commitment)?,
        right_commitment: point_to_hex(&right.commitment)?,
        difference_commitment: point_to_hex(&difference_commitment)?,
        nonce_commitment: point_to_hex(&nonce_commitment)?,
        response_blinding: scalar_to_hex(&(nonce_blinding + challenge * difference_blinding))?,
    })
}

fn private_vm_bit_proof(
    label: &str,
    index: usize,
    bit: u32,
    bit_blinding: Fr,
    bit_commitment: EdwardsProjective,
    seed: &[u8; 32],
) -> Result<SfcsZkPrivateVmBitProof, SfcsZkError> {
    let nonce_label = format!("{label}:bit:{index}");
    let zero_relation = bit_commitment;
    let one_relation = bit_commitment - value_base();
    let zero_nonce;
    let one_nonce;
    let zero_challenge;
    let one_challenge;
    let zero_response;
    let one_response;

    if bit == 0 {
        let actual_nonce = private_vm_nonce_scalar(&nonce_label, "zero-actual", seed);
        zero_nonce = blinding_base().mul_bigint(actual_nonce.into_bigint());
        one_challenge = private_vm_nonce_scalar(&nonce_label, "one-simulated-challenge", seed);
        one_response = private_vm_nonce_scalar(&nonce_label, "one-simulated-response", seed);
        one_nonce = blinding_base().mul_bigint(one_response.into_bigint())
            - one_relation.mul_bigint(one_challenge.into_bigint());
        let challenge =
            derive_range_bit_challenge(label, index, &bit_commitment, &zero_nonce, &one_nonce)?;
        zero_challenge = challenge - one_challenge;
        zero_response = actual_nonce + zero_challenge * bit_blinding;
    } else {
        let actual_nonce = private_vm_nonce_scalar(&nonce_label, "one-actual", seed);
        one_nonce = blinding_base().mul_bigint(actual_nonce.into_bigint());
        zero_challenge = private_vm_nonce_scalar(&nonce_label, "zero-simulated-challenge", seed);
        zero_response = private_vm_nonce_scalar(&nonce_label, "zero-simulated-response", seed);
        zero_nonce = blinding_base().mul_bigint(zero_response.into_bigint())
            - zero_relation.mul_bigint(zero_challenge.into_bigint());
        let challenge =
            derive_range_bit_challenge(label, index, &bit_commitment, &zero_nonce, &one_nonce)?;
        one_challenge = challenge - zero_challenge;
        one_response = actual_nonce + one_challenge * bit_blinding;
    }

    Ok(SfcsZkPrivateVmBitProof {
        zero_nonce_commitment: point_to_hex(&zero_nonce)?,
        one_nonce_commitment: point_to_hex(&one_nonce)?,
        zero_challenge: scalar_to_hex(&zero_challenge)?,
        one_challenge: scalar_to_hex(&one_challenge)?,
        zero_response: scalar_to_hex(&zero_response)?,
        one_response: scalar_to_hex(&one_response)?,
    })
}

fn private_vm_binary_linear_relation_preimage(
    step_index: u64,
    relation: &str,
    lhs: u32,
    rhs: u32,
    output: u32,
    seed: &[u8; 32],
) -> Result<SfcsZkPrivateVmLinearRelationPreimage, SfcsZkError> {
    let prefix = format!("linear:{step_index}:{relation}");
    let lhs_blinding = private_vm_blinding_scalar(&format!("{prefix}:lhs"), seed);
    let rhs_blinding = private_vm_blinding_scalar(&format!("{prefix}:rhs"), seed);
    let output_blinding = private_vm_blinding_scalar(&format!("{prefix}:output"), seed);
    let lhs_commitment = pedersen_commit(lhs, lhs_blinding);
    let rhs_commitment = pedersen_commit(rhs, rhs_blinding);
    let output_commitment = pedersen_commit(output, output_blinding);
    let (relation_commitment, relation_blinding) = match relation {
        "add" => (
            output_commitment - lhs_commitment - rhs_commitment,
            output_blinding - lhs_blinding - rhs_blinding,
        ),
        "sub" => (
            output_commitment - lhs_commitment + rhs_commitment,
            output_blinding - lhs_blinding + rhs_blinding,
        ),
        _ => {
            return Err(SfcsZkError::InvalidProof(format!(
                "unsupported binary linear relation {relation}"
            )))
        }
    };
    let nonce_blinding = private_vm_nonce_scalar(&prefix, "relation", seed);
    let nonce_commitment = blinding_base().mul_bigint(nonce_blinding.into_bigint());
    Ok(SfcsZkPrivateVmLinearRelationPreimage {
        step_index,
        relation: relation.to_string(),
        lhs_commitment,
        rhs_commitment: Some(rhs_commitment),
        public_constant: None,
        output_commitment,
        relation_commitment,
        relation_blinding,
        nonce_commitment,
        nonce_blinding,
        range_inputs: vec![
            SfcsZkPrivateVmRangeInput {
                label: format!("{prefix}:lhs"),
                value: lhs,
                blinding: lhs_blinding,
                commitment: lhs_commitment,
            },
            SfcsZkPrivateVmRangeInput {
                label: format!("{prefix}:rhs"),
                value: rhs,
                blinding: rhs_blinding,
                commitment: rhs_commitment,
            },
            SfcsZkPrivateVmRangeInput {
                label: format!("{prefix}:output"),
                value: output,
                blinding: output_blinding,
                commitment: output_commitment,
            },
        ],
    })
}

fn private_vm_immediate_linear_relation_preimage(
    step_index: u64,
    relation: &str,
    lhs: u32,
    constant: u32,
    output: u32,
    seed: &[u8; 32],
) -> Result<SfcsZkPrivateVmLinearRelationPreimage, SfcsZkError> {
    let prefix = format!("linear:{step_index}:{relation}");
    let lhs_blinding = private_vm_blinding_scalar(&format!("{prefix}:lhs"), seed);
    let output_blinding = private_vm_blinding_scalar(&format!("{prefix}:output"), seed);
    let lhs_commitment = pedersen_commit(lhs, lhs_blinding);
    let output_commitment = pedersen_commit(output, output_blinding);
    let (relation_commitment, relation_blinding) = match relation {
        "addi" => (
            output_commitment
                - lhs_commitment
                - value_base().mul_bigint(Fr::from(constant).into_bigint()),
            output_blinding - lhs_blinding,
        ),
        "subi" => (
            output_commitment - lhs_commitment
                + value_base().mul_bigint(Fr::from(constant).into_bigint()),
            output_blinding - lhs_blinding,
        ),
        "scale" => (
            output_commitment - lhs_commitment.mul_bigint(Fr::from(constant).into_bigint()),
            output_blinding - lhs_blinding * Fr::from(constant),
        ),
        _ => {
            return Err(SfcsZkError::InvalidProof(format!(
                "unsupported immediate linear relation {relation}"
            )))
        }
    };
    let nonce_blinding = private_vm_nonce_scalar(&prefix, "relation", seed);
    let nonce_commitment = blinding_base().mul_bigint(nonce_blinding.into_bigint());
    Ok(SfcsZkPrivateVmLinearRelationPreimage {
        step_index,
        relation: relation.to_string(),
        lhs_commitment,
        rhs_commitment: None,
        public_constant: Some(constant),
        output_commitment,
        relation_commitment,
        relation_blinding,
        nonce_commitment,
        nonce_blinding,
        range_inputs: vec![
            SfcsZkPrivateVmRangeInput {
                label: format!("{prefix}:lhs"),
                value: lhs,
                blinding: lhs_blinding,
                commitment: lhs_commitment,
            },
            SfcsZkPrivateVmRangeInput {
                label: format!("{prefix}:output"),
                value: output,
                blinding: output_blinding,
                commitment: output_commitment,
            },
        ],
    })
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
    linear_relation_preimages: &[SfcsZkPrivateVmLinearRelationPreimage],
) -> Result<Fr, SfcsZkError> {
    let mut hasher = Sha256::new();
    hasher.update(ZK_PRIVATE_VM_CHALLENGE_DOMAIN);
    hasher.update(serde_json::to_vec(statement)?);
    for (label, _, _, nonce_commitment) in nonce_commitments {
        hasher.update(label.as_bytes());
        hasher.update(nonce_commitment.as_bytes());
    }
    for relation in linear_relation_preimages {
        hasher.update(relation.step_index.to_le_bytes());
        hasher.update(relation.relation.as_bytes());
        hasher.update(point_to_hex(&relation.nonce_commitment)?.as_bytes());
    }
    Ok(Fr::from_le_bytes_mod_order(&hasher.finalize()))
}

fn derive_private_vm_challenge_from_proof(
    statement: &SfcsZkPrivateVmStatement,
    nonce_commitments: &[(String, Fr, Fr, String)],
    linear_relation_proofs: &[SfcsZkPrivateVmLinearRelationProof],
) -> Result<Fr, SfcsZkError> {
    let mut hasher = Sha256::new();
    hasher.update(ZK_PRIVATE_VM_CHALLENGE_DOMAIN);
    hasher.update(serde_json::to_vec(statement)?);
    for (label, _, _, nonce_commitment) in nonce_commitments {
        hasher.update(label.as_bytes());
        hasher.update(nonce_commitment.as_bytes());
    }
    for relation in linear_relation_proofs {
        hasher.update(relation.step_index.to_le_bytes());
        hasher.update(relation.relation.as_bytes());
        hasher.update(relation.nonce_commitment.as_bytes());
    }
    Ok(Fr::from_le_bytes_mod_order(&hasher.finalize()))
}

fn derive_range_bit_challenge(
    label: &str,
    index: usize,
    bit_commitment: &EdwardsProjective,
    zero_nonce: &EdwardsProjective,
    one_nonce: &EdwardsProjective,
) -> Result<Fr, SfcsZkError> {
    let mut hasher = Sha256::new();
    hasher.update(ZK_PRIVATE_VM_CHALLENGE_DOMAIN);
    hasher.update(b"range-bit\0");
    hasher.update(label.as_bytes());
    hasher.update((index as u64).to_le_bytes());
    hasher.update(point_to_bytes(bit_commitment)?);
    hasher.update(point_to_bytes(zero_nonce)?);
    hasher.update(point_to_bytes(one_nonce)?);
    Ok(Fr::from_le_bytes_mod_order(&hasher.finalize()))
}

fn derive_range_recomposition_challenge(
    label: &str,
    value_commitment: &EdwardsProjective,
    bit_commitments: &[EdwardsProjective],
    recomposition_commitment: &EdwardsProjective,
    nonce_commitment: &EdwardsProjective,
) -> Result<Fr, SfcsZkError> {
    let mut hasher = Sha256::new();
    hasher.update(ZK_PRIVATE_VM_CHALLENGE_DOMAIN);
    hasher.update(b"range-recomposition\0");
    hasher.update(label.as_bytes());
    hasher.update(point_to_bytes(value_commitment)?);
    for bit_commitment in bit_commitments {
        hasher.update(point_to_bytes(bit_commitment)?);
    }
    hasher.update(point_to_bytes(recomposition_commitment)?);
    hasher.update(point_to_bytes(nonce_commitment)?);
    Ok(Fr::from_le_bytes_mod_order(&hasher.finalize()))
}

fn derive_equality_challenge(
    label: &str,
    left_commitment: &EdwardsProjective,
    right_commitment: &EdwardsProjective,
    difference_commitment: &EdwardsProjective,
    nonce_commitment: &EdwardsProjective,
) -> Result<Fr, SfcsZkError> {
    let mut hasher = Sha256::new();
    hasher.update(ZK_PRIVATE_VM_CHALLENGE_DOMAIN);
    hasher.update(b"equality\0");
    hasher.update(label.as_bytes());
    hasher.update(point_to_bytes(left_commitment)?);
    hasher.update(point_to_bytes(right_commitment)?);
    hasher.update(point_to_bytes(difference_commitment)?);
    hasher.update(point_to_bytes(nonce_commitment)?);
    Ok(Fr::from_le_bytes_mod_order(&hasher.finalize()))
}

fn derive_selective_challenge(
    label: &str,
    commitments: &[EdwardsProjective],
    candidates: &[Vec<Option<u32>>],
    nonce_branches: &[Vec<EdwardsProjective>],
) -> Result<Fr, SfcsZkError> {
    let mut hasher = Sha256::new();
    hasher.update(ZK_PRIVATE_VM_CHALLENGE_DOMAIN);
    hasher.update(b"selective-or\0");
    hasher.update(label.as_bytes());
    for commitment in commitments {
        hasher.update(point_to_bytes(commitment)?);
    }
    for candidate in candidates {
        for value in candidate {
            match value {
                Some(value) => {
                    hasher.update([1]);
                    hasher.update(value.to_le_bytes());
                }
                None => hasher.update([0]),
            }
        }
    }
    for branch in nonce_branches {
        for nonce in branch {
            hasher.update(point_to_bytes(nonce)?);
        }
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
