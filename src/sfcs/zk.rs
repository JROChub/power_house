//! Zero-knowledge proof profiles for SFCS VM executions.
//!
//! This module starts with a narrow, auditable profile: a private RV32I
//! no-overflow add relation. It proves that two committed private register
//! values add to a public output for a two-instruction `add; ecall` program.
//! It does not yet prove arbitrary VM execution.

use super::{digest_json, vm::SfcsVmProgram};
use crate::provenance::{PhaArtifact, PhaError};
use ark_ec::{AffineRepr, CurveGroup, Group};
use ark_ed_on_bn254::{EdwardsAffine, EdwardsProjective, Fr};
use ark_ff::PrimeField;
use ark_serialize::{CanonicalDeserialize, CanonicalSerialize, SerializationError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::error::Error;
use std::fmt;

/// Draft `.pha` protocol for the first SFCS ZK VM profile.
pub const SFCS_ZK_PRIVATE_ADD_PROTOCOL_V1_DRAFT: &str = "power-house/sfcs-zk-private-add/v1-draft";

const ZK_POINT_DOMAIN: &[u8] = b"power-house:sfcs-zk:v1-draft:pedersen-bases\0";
const ZK_PROOF_DOMAIN: &[u8] = b"power-house:sfcs-zk:v1-draft:private-add-proof\0";
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct SfcsZkPrivateAddEmbedding {
    program: SfcsVmProgram,
    proof: SfcsZkPrivateAddProof,
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

impl From<super::SfcsError> for SfcsZkError {
    fn from(error: super::SfcsError) -> Self {
        Self::Sfcs(error)
    }
}
