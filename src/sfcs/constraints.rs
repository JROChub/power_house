//! Public VM constraint proofs for SFCS execution traces.
//!
//! This module is a transparent transition-proof layer for arbitrary public
//! RV32I executions supported by [`super::vm`]. It is not a zero-knowledge
//! proof: the verifier recomputes the public VM execution and checks that every
//! transition, register range, memory access, digest, and execution-fractal
//! binding matches the committed proof object.

use super::{
    digest_json,
    vm::{SfcsVmError, SfcsVmInputs, SfcsVmProgram},
    SfcsError,
};
use crate::provenance::{PhaArtifact, PhaError};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;

/// Draft `.pha` protocol for public SFCS VM constraint proofs.
pub const SFCS_VM_CONSTRAINT_PROTOCOL_V1_DRAFT: &str = "power-house/sfcs-vm-constraints/v1-draft";

const VM_CONSTRAINT_DOMAIN: &[u8] = b"power-house:sfcs-vm:v1-draft:constraints\0";

/// Transparent constraint proof for an SFCS VM execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcsVmConstraintProof {
    /// Constraint proof schema.
    pub schema: String,
    /// Program digest.
    pub program_digest: String,
    /// Input digest.
    pub input_digest: String,
    /// Full VM trace digest.
    pub trace_digest: String,
    /// Digest of the execution fractal projected from the VM trace.
    pub execution_fractal_digest: String,
    /// Final VM state digest.
    pub final_state_digest: String,
    /// Final memory digest.
    pub final_memory_digest: String,
    /// Number of executed VM instructions.
    pub steps: usize,
    /// Number of instruction fetch/alignment/transition checks covered.
    pub transition_checks: u64,
    /// Number of committed register range checks covered.
    pub register_range_checks: u64,
    /// Number of memory range checks covered.
    pub memory_range_checks: u64,
    /// Number of memory consistency checks covered.
    pub memory_consistency_checks: u64,
    /// Number of branch/control-flow checks covered.
    pub branch_checks: u64,
    /// Public memory events used by the consistency checker.
    pub memory_events: Vec<SfcsVmMemoryConstraintEvent>,
    /// Per-step transition commitments.
    pub step_commitments: Vec<SfcsVmTransitionCommitment>,
    /// Domain-separated proof digest.
    pub proof_digest: String,
}

impl SfcsVmConstraintProof {
    /// Proves transition and memory consistency for a public VM execution.
    pub fn prove(
        program: &SfcsVmProgram,
        inputs: &SfcsVmInputs,
    ) -> Result<Self, SfcsVmConstraintError> {
        let trace = program.execute(inputs)?;
        let execution_fractal = trace.to_fractal_graph()?;
        let execution_fractal_digest = execution_fractal.fractal_digest()?;
        let mut memory_events = Vec::new();
        let mut step_commitments = Vec::new();
        let mut memory_range_checks = 0_u64;
        let mut memory_consistency_checks = 0_u64;
        let mut branch_checks = 0_u64;

        for step in &trace.steps {
            if step.branch_taken {
                branch_checks += 1;
            }
            if let Some(access) = &step.memory_access {
                memory_range_checks += u64::from(access.width);
                memory_consistency_checks += 1;
                memory_events.push(SfcsVmMemoryConstraintEvent {
                    step_index: step.step_index,
                    kind: access.kind.clone(),
                    address: access.address,
                    width: access.width,
                    value: access.value,
                    memory_before_digest: step.memory_before_digest.clone(),
                    memory_after_digest: step.memory_after_digest.clone(),
                    event_digest: digest_json(VM_CONSTRAINT_DOMAIN, access)?,
                });
            }
            step_commitments.push(SfcsVmTransitionCommitment {
                step_index: step.step_index,
                pc: step.pc,
                instruction: step.instruction,
                mnemonic: step.mnemonic.clone(),
                next_pc: step.next_pc,
                branch_taken: step.branch_taken,
                state_before_digest: step.state_before_digest.clone(),
                state_after_digest: step.state_after_digest.clone(),
                step_digest: step.step_digest.clone(),
            });
        }

        let steps = trace.steps.len();
        let mut proof = Self {
            schema: SFCS_VM_CONSTRAINT_PROTOCOL_V1_DRAFT.to_string(),
            program_digest: trace.program_digest,
            input_digest: trace.input_digest,
            trace_digest: trace.trace_digest,
            execution_fractal_digest,
            final_state_digest: trace.final_state_digest,
            final_memory_digest: trace.final_memory_digest,
            steps,
            transition_checks: steps as u64,
            register_range_checks: (steps as u64).saturating_mul(32),
            memory_range_checks,
            memory_consistency_checks,
            branch_checks,
            memory_events,
            step_commitments,
            proof_digest: String::new(),
        };
        proof.proof_digest = digest_json(VM_CONSTRAINT_DOMAIN, &proof.preimage())?;
        Ok(proof)
    }

    /// Verifies this proof by recomputing the public VM execution.
    pub fn verify(
        &self,
        program: &SfcsVmProgram,
        inputs: &SfcsVmInputs,
    ) -> Result<(), SfcsVmConstraintError> {
        if self.schema != SFCS_VM_CONSTRAINT_PROTOCOL_V1_DRAFT {
            return Err(SfcsVmConstraintError::UnsupportedSchema(
                self.schema.clone(),
            ));
        }
        let expected = Self::prove(program, inputs)?;
        if self != &expected {
            return Err(SfcsVmConstraintError::InvalidProof(
                "VM constraint proof does not match replayed execution".to_string(),
            ));
        }
        Ok(())
    }

    /// Commits the constraint proof into ordinary `.pha` core data.
    pub fn to_pha_artifact(
        &self,
        label: impl Into<String>,
        program: &SfcsVmProgram,
        inputs: &SfcsVmInputs,
    ) -> Result<PhaArtifact, SfcsVmConstraintError> {
        self.verify(program, inputs)?;
        PhaArtifact::new(
            serde_json::json!({
                "producer": "power_house_sfcs_vm_constraints",
                "label": label.into(),
                "profile": SFCS_VM_CONSTRAINT_PROTOCOL_V1_DRAFT,
                "program_digest": self.program_digest,
                "trace_digest": self.trace_digest,
                "proof_digest": self.proof_digest,
            }),
            SFCS_VM_CONSTRAINT_PROTOCOL_V1_DRAFT,
            serde_json::json!({
                "program_digest": self.program_digest,
                "input_digest": self.input_digest,
                "trace_digest": self.trace_digest,
                "execution_fractal_digest": self.execution_fractal_digest,
                "final_state_digest": self.final_state_digest,
                "final_memory_digest": self.final_memory_digest,
                "steps": self.steps,
                "transition_checks": self.transition_checks,
                "register_range_checks": self.register_range_checks,
                "memory_range_checks": self.memory_range_checks,
                "memory_consistency_checks": self.memory_consistency_checks,
                "branch_checks": self.branch_checks,
                "proof_digest": self.proof_digest,
            }),
            serde_json::json!({
                "program": program,
                "inputs": inputs,
                "proof": self,
            }),
        )
        .map_err(SfcsVmConstraintError::Pha)
    }

    fn preimage(&self) -> serde_json::Value {
        serde_json::json!({
            "schema": self.schema,
            "program_digest": self.program_digest,
            "input_digest": self.input_digest,
            "trace_digest": self.trace_digest,
            "execution_fractal_digest": self.execution_fractal_digest,
            "final_state_digest": self.final_state_digest,
            "final_memory_digest": self.final_memory_digest,
            "steps": self.steps,
            "transition_checks": self.transition_checks,
            "register_range_checks": self.register_range_checks,
            "memory_range_checks": self.memory_range_checks,
            "memory_consistency_checks": self.memory_consistency_checks,
            "branch_checks": self.branch_checks,
            "memory_events": self.memory_events,
            "step_commitments": self.step_commitments,
        })
    }
}

/// One memory event covered by a VM constraint proof.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcsVmMemoryConstraintEvent {
    /// VM step index.
    pub step_index: u64,
    /// `read` or `write`.
    pub kind: String,
    /// Byte address.
    pub address: u32,
    /// Access width in bytes.
    pub width: u8,
    /// Loaded or stored value.
    pub value: u32,
    /// Memory digest before the access.
    pub memory_before_digest: String,
    /// Memory digest after the access.
    pub memory_after_digest: String,
    /// Event digest.
    pub event_digest: String,
}

/// One committed VM transition covered by a constraint proof.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcsVmTransitionCommitment {
    /// VM step index.
    pub step_index: u64,
    /// Program counter before execution.
    pub pc: u32,
    /// Instruction word.
    pub instruction: u32,
    /// Decoded mnemonic.
    pub mnemonic: String,
    /// Program counter after execution.
    pub next_pc: u32,
    /// Whether a branch or jump was taken.
    pub branch_taken: bool,
    /// State digest before execution.
    pub state_before_digest: String,
    /// State digest after execution.
    pub state_after_digest: String,
    /// VM step digest.
    pub step_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct SfcsVmConstraintEmbedding {
    program: SfcsVmProgram,
    inputs: SfcsVmInputs,
    proof: SfcsVmConstraintProof,
}

/// Verifies a `.pha` artifact carrying a public VM constraint proof.
pub fn verify_vm_constraint_embedding(
    artifact: &PhaArtifact,
) -> Result<SfcsVmConstraintProof, SfcsVmConstraintError> {
    artifact.verify().map_err(SfcsVmConstraintError::Pha)?;
    if artifact.embedded_proof.protocol != SFCS_VM_CONSTRAINT_PROTOCOL_V1_DRAFT {
        return Err(SfcsVmConstraintError::InvalidEmbedding(
            "embedded proof protocol is not SFCS VM constraints".to_string(),
        ));
    }
    let embedding: SfcsVmConstraintEmbedding =
        serde_json::from_value(artifact.embedded_proof.proof.clone())?;
    embedding
        .proof
        .verify(&embedding.program, &embedding.inputs)?;
    for (field, expected) in [
        ("program_digest", &embedding.proof.program_digest),
        ("trace_digest", &embedding.proof.trace_digest),
        ("proof_digest", &embedding.proof.proof_digest),
    ] {
        let found = artifact
            .provenance
            .get(field)
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                SfcsVmConstraintError::InvalidEmbedding(format!("missing provenance {field}"))
            })?;
        if found != expected {
            return Err(SfcsVmConstraintError::InvalidEmbedding(format!(
                "provenance {field} does not match proof"
            )));
        }
        let public = artifact
            .embedded_proof
            .public_inputs
            .get(field)
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                SfcsVmConstraintError::InvalidEmbedding(format!("missing public {field}"))
            })?;
        if public != expected {
            return Err(SfcsVmConstraintError::InvalidEmbedding(format!(
                "public {field} does not match proof"
            )));
        }
    }
    Ok(embedding.proof)
}

/// Errors returned by public VM constraint proofs.
#[derive(Debug)]
pub enum SfcsVmConstraintError {
    /// Unsupported proof schema.
    UnsupportedSchema(String),
    /// Constraint proof verification failed.
    InvalidProof(String),
    /// `.pha` embedding is inconsistent.
    InvalidEmbedding(String),
    /// VM execution failed.
    Vm(SfcsVmError),
    /// SFCS graph or digest operation failed.
    Sfcs(SfcsError),
    /// JSON serialization failed.
    Json(serde_json::Error),
    /// `.pha` construction or verification failed.
    Pha(PhaError),
}

impl fmt::Display for SfcsVmConstraintError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedSchema(schema) => {
                write!(formatter, "unsupported SFCS VM constraint schema: {schema}")
            }
            Self::InvalidProof(message) => {
                write!(formatter, "invalid SFCS VM constraint proof: {message}")
            }
            Self::InvalidEmbedding(message) => {
                write!(formatter, "invalid SFCS VM constraint embedding: {message}")
            }
            Self::Vm(error) => write!(formatter, "SFCS VM constraint execution error: {error}"),
            Self::Sfcs(error) => write!(formatter, "SFCS VM constraint graph error: {error}"),
            Self::Json(error) => write!(formatter, "SFCS VM constraint JSON error: {error}"),
            Self::Pha(error) => write!(formatter, "SFCS VM constraint PHA error: {error}"),
        }
    }
}

impl Error for SfcsVmConstraintError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Vm(error) => Some(error),
            Self::Sfcs(error) => Some(error),
            Self::Json(error) => Some(error),
            Self::Pha(error) => Some(error),
            _ => None,
        }
    }
}

impl From<SfcsVmError> for SfcsVmConstraintError {
    fn from(error: SfcsVmError) -> Self {
        Self::Vm(error)
    }
}

impl From<SfcsError> for SfcsVmConstraintError {
    fn from(error: SfcsError) -> Self {
        Self::Sfcs(error)
    }
}

impl From<serde_json::Error> for SfcsVmConstraintError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

impl From<PhaError> for SfcsVmConstraintError {
    fn from(error: PhaError) -> Self {
        Self::Pha(error)
    }
}
