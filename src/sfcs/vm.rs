//! Deterministic SFCS virtual-machine execution.
//!
//! This module is the VM foundation for the SFCS zkVM roadmap. It implements a
//! deterministic RV32I execution core and binds every executed instruction to a
//! digestible trace that can be carried as ordinary `.pha` core data. It does
//! not claim zero-knowledge privacy; that must be provided by a later proof
//! layer that verifies these transition semantics without revealing private
//! state.

use super::{digest_json, validate_sha256, SfcsGraph, SfcsNode, SfcsOp};
use crate::provenance::{PhaArtifact, PhaError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

/// Draft schema identifier for an SFCS VM program.
pub const SFCS_VM_PROGRAM_SCHEMA_V1_DRAFT: &str = "power-house/sfcs-vm-program/v1-draft";

/// Draft `.pha` embedded proof protocol for SFCS VM execution.
pub const SFCS_VM_EXECUTION_PROTOCOL_V1_DRAFT: &str = "power-house/sfcs-vm-execution/v1-draft";

const VM_PROGRAM_DOMAIN: &[u8] = b"power-house:sfcs-vm:v1-draft:program\0";
const VM_INPUT_DOMAIN: &[u8] = b"power-house:sfcs-vm:v1-draft:inputs\0";
const VM_REGISTER_DOMAIN: &[u8] = b"power-house:sfcs-vm:v1-draft:registers\0";
const VM_MEMORY_DOMAIN: &[u8] = b"power-house:sfcs-vm:v1-draft:memory\0";
const VM_STATE_DOMAIN: &[u8] = b"power-house:sfcs-vm:v1-draft:state\0";
const VM_STEP_DOMAIN: &[u8] = b"power-house:sfcs-vm:v1-draft:step\0";
const VM_TRACE_DOMAIN: &[u8] = b"power-house:sfcs-vm:v1-draft:trace\0";
const MAX_VM_STEPS: u64 = 1_000_000;
const MAX_VM_MEMORY_BYTES: usize = 16 * 1024 * 1024;

/// Deterministic RV32I program image.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcsVmProgram {
    /// Schema identifier.
    pub schema: String,
    /// Architecture label. Currently `rv32i`.
    pub architecture: String,
    /// Program entry PC. Must be 4-byte aligned.
    pub entry_pc: u32,
    /// Maximum number of instruction steps before rejection.
    pub max_steps: u64,
    /// Little-endian decoded RV32I instruction words.
    pub instructions: Vec<u32>,
}

impl SfcsVmProgram {
    /// Creates a deterministic RV32I program.
    pub fn rv32i(instructions: Vec<u32>) -> Self {
        Self {
            schema: SFCS_VM_PROGRAM_SCHEMA_V1_DRAFT.to_string(),
            architecture: "rv32i".to_string(),
            entry_pc: 0,
            max_steps: 100_000,
            instructions,
        }
    }

    /// Sets the entry PC.
    pub fn with_entry_pc(mut self, entry_pc: u32) -> Self {
        self.entry_pc = entry_pc;
        self
    }

    /// Sets the maximum execution step count.
    pub fn with_max_steps(mut self, max_steps: u64) -> Self {
        self.max_steps = max_steps;
        self
    }

    /// Verifies static VM program invariants.
    pub fn verify(&self) -> Result<(), SfcsVmError> {
        if self.schema != SFCS_VM_PROGRAM_SCHEMA_V1_DRAFT {
            return Err(SfcsVmError::UnsupportedSchema(self.schema.clone()));
        }
        if self.architecture != "rv32i" {
            return Err(SfcsVmError::InvalidProgram(format!(
                "unsupported architecture {}",
                self.architecture
            )));
        }
        if !self.entry_pc.is_multiple_of(4) {
            return Err(SfcsVmError::InvalidProgram(
                "entry_pc must be 4-byte aligned".to_string(),
            ));
        }
        if self.instructions.is_empty() {
            return Err(SfcsVmError::InvalidProgram(
                "program has no instructions".to_string(),
            ));
        }
        if self.max_steps == 0 || self.max_steps > MAX_VM_STEPS {
            return Err(SfcsVmError::InvalidProgram(format!(
                "max_steps must be 1..={MAX_VM_STEPS}"
            )));
        }
        let max_program_bytes =
            self.instructions.len().checked_mul(4).ok_or_else(|| {
                SfcsVmError::InvalidProgram("program image is too large".to_string())
            })?;
        if max_program_bytes > MAX_VM_MEMORY_BYTES {
            return Err(SfcsVmError::InvalidProgram(format!(
                "program image exceeds {MAX_VM_MEMORY_BYTES} bytes"
            )));
        }
        Ok(())
    }

    /// Returns the domain-separated digest of this program image.
    pub fn program_digest(&self) -> Result<String, SfcsVmError> {
        self.verify()?;
        Ok(digest_json(VM_PROGRAM_DOMAIN, self)?)
    }

    /// Executes the program and returns a digest-bound trace.
    pub fn execute(&self, inputs: &SfcsVmInputs) -> Result<SfcsVmExecutionTrace, SfcsVmError> {
        self.verify()?;
        inputs.verify()?;
        let program_digest = self.program_digest()?;
        let input_digest = digest_json(VM_INPUT_DOMAIN, inputs)?;
        let mut state = SfcsVmState::from_inputs(self.entry_pc, inputs);
        let initial_state_digest = state_digest(&state)?;
        let mut steps = Vec::new();
        let mut halted = false;

        for step_index in 0..self.max_steps {
            let pc = state.pc;
            let instruction = self.fetch(pc)?;
            let registers_before_digest = register_digest(&state.registers)?;
            let memory_before_digest = memory_digest(&state.memory)?;
            let state_before_digest = state_digest(&state)?;
            let decoded = decode(instruction)?;
            let result = execute_instruction(&mut state, instruction, &decoded)?;
            state.registers[0] = 0;
            let registers_after_digest = register_digest(&state.registers)?;
            let memory_after_digest = memory_digest(&state.memory)?;
            let state_after_digest = state_digest(&state)?;
            let mut trace_step = SfcsVmTraceStep {
                step_index,
                pc,
                instruction,
                mnemonic: decoded.mnemonic.to_string(),
                rd: decoded.rd,
                rs1: decoded.rs1,
                rs2: decoded.rs2,
                immediate: decoded.immediate,
                next_pc: state.pc,
                branch_taken: result.branch_taken,
                memory_access: result.memory_access,
                registers_before_digest,
                registers_after_digest,
                memory_before_digest,
                memory_after_digest,
                state_before_digest,
                state_after_digest,
                halted: result.halted,
                step_digest: String::new(),
            };
            trace_step.step_digest = digest_json(VM_STEP_DOMAIN, &trace_step.preimage())?;
            halted = result.halted;
            steps.push(trace_step);
            if halted {
                break;
            }
        }

        if !halted {
            return Err(SfcsVmError::Execution(format!(
                "program did not halt within {} steps",
                self.max_steps
            )));
        }

        let final_state_digest = state_digest(&state)?;
        let mut trace = SfcsVmExecutionTrace {
            schema: "power-house/sfcs-vm-trace/v1-draft".to_string(),
            architecture: self.architecture.clone(),
            program_digest,
            input_digest,
            initial_state_digest,
            final_state_digest,
            trace_digest: String::new(),
            steps,
            final_pc: state.pc,
            final_registers: state.registers,
            final_memory_digest: memory_digest(&state.memory)?,
            public_outputs: SfcsVmPublicOutputs {
                registers: selected_registers(&state.registers, &inputs.public_registers),
                memory: selected_memory(&state.memory, &inputs.public_memory),
            },
        };
        trace.trace_digest = digest_json(VM_TRACE_DOMAIN, &trace.preimage())?;
        Ok(trace)
    }

    /// Commits program, inputs, and deterministic VM trace into `.pha` core data.
    pub fn to_execution_pha_artifact(
        &self,
        label: impl Into<String>,
        inputs: &SfcsVmInputs,
    ) -> Result<PhaArtifact, SfcsVmError> {
        let trace = self.execute(inputs)?;
        let execution_fractal = trace.to_fractal_graph()?;
        let execution_fractal_digest = execution_fractal
            .fractal_digest()
            .map_err(SfcsVmError::Sfcs)?;
        PhaArtifact::new(
            serde_json::json!({
                "producer": "power_house_sfcs_vm",
                "label": label.into(),
                "schema": self.schema,
                "architecture": self.architecture,
                "program_digest": trace.program_digest,
                "trace_digest": trace.trace_digest,
                "execution_fractal_digest": execution_fractal_digest,
                "final_state_digest": trace.final_state_digest,
            }),
            SFCS_VM_EXECUTION_PROTOCOL_V1_DRAFT,
            serde_json::json!({
                "program_digest": trace.program_digest,
                "input_digest": trace.input_digest,
                "trace_digest": trace.trace_digest,
                "execution_fractal_digest": execution_fractal_digest,
                "initial_state_digest": trace.initial_state_digest,
                "final_state_digest": trace.final_state_digest,
                "final_memory_digest": trace.final_memory_digest,
                "final_pc": trace.final_pc,
                "steps": trace.steps.len(),
                "public_outputs": trace.public_outputs,
            }),
            serde_json::json!({
                "program": self,
                "inputs": inputs,
                "trace": trace,
                "execution_fractal": execution_fractal,
            }),
        )
        .map_err(SfcsVmError::Pha)
    }

    fn fetch(&self, pc: u32) -> Result<u32, SfcsVmError> {
        if pc < self.entry_pc || !(pc - self.entry_pc).is_multiple_of(4) {
            return Err(SfcsVmError::Execution(format!(
                "pc 0x{pc:08x} is outside the aligned program image"
            )));
        }
        let index = ((pc - self.entry_pc) / 4) as usize;
        self.instructions.get(index).copied().ok_or_else(|| {
            SfcsVmError::Execution(format!("pc 0x{pc:08x} is outside the program image"))
        })
    }
}

/// Public VM execution inputs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcsVmInputs {
    /// Initial register values. Register 0 is always forced to zero.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub registers: BTreeMap<u8, u32>,
    /// Initial byte-addressed little-endian memory.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub memory: BTreeMap<u32, u8>,
    /// Registers to expose as public outputs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub public_registers: Vec<u8>,
    /// Memory ranges to expose as public outputs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub public_memory: Vec<SfcsVmMemoryRange>,
}

impl SfcsVmInputs {
    /// Empty VM input state.
    pub fn empty() -> Self {
        Self {
            registers: BTreeMap::new(),
            memory: BTreeMap::new(),
            public_registers: Vec::new(),
            public_memory: Vec::new(),
        }
    }

    /// Verifies static input bounds.
    pub fn verify(&self) -> Result<(), SfcsVmError> {
        for register in self
            .registers
            .keys()
            .chain(self.public_registers.iter())
            .copied()
        {
            if register > 31 {
                return Err(SfcsVmError::InvalidInput(format!(
                    "register x{register} is outside RV32I range"
                )));
            }
        }
        if self.memory.len() > MAX_VM_MEMORY_BYTES {
            return Err(SfcsVmError::InvalidInput(format!(
                "initial memory exceeds {MAX_VM_MEMORY_BYTES} bytes"
            )));
        }
        for range in &self.public_memory {
            range.verify()?;
        }
        Ok(())
    }
}

/// Public memory range to expose from a VM execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcsVmMemoryRange {
    /// Start address.
    pub start: u32,
    /// Byte length.
    pub len: u32,
}

impl SfcsVmMemoryRange {
    fn verify(&self) -> Result<(), SfcsVmError> {
        if self.len == 0 {
            return Err(SfcsVmError::InvalidInput(
                "public memory range length must be non-zero".to_string(),
            ));
        }
        self.start.checked_add(self.len - 1).ok_or_else(|| {
            SfcsVmError::InvalidInput("public memory range overflows u32 address space".to_string())
        })?;
        Ok(())
    }
}

/// Digest-bound VM execution trace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcsVmExecutionTrace {
    /// Trace schema identifier.
    pub schema: String,
    /// Architecture label.
    pub architecture: String,
    /// Program digest.
    pub program_digest: String,
    /// Input digest.
    pub input_digest: String,
    /// Initial VM state digest.
    pub initial_state_digest: String,
    /// Final VM state digest.
    pub final_state_digest: String,
    /// Full trace digest.
    pub trace_digest: String,
    /// Instruction steps.
    pub steps: Vec<SfcsVmTraceStep>,
    /// Final program counter.
    pub final_pc: u32,
    /// Final register file.
    pub final_registers: [u32; 32],
    /// Final full-memory digest.
    pub final_memory_digest: String,
    /// Explicit public outputs.
    pub public_outputs: SfcsVmPublicOutputs,
}

impl SfcsVmExecutionTrace {
    /// Projects the VM execution trace into first-class SFCS fractal nodes.
    ///
    /// Each instruction becomes a committed dense execution node. Register
    /// writes and memory accesses become separate dependent nodes so reviewers
    /// can inspect exactly where state changed without changing Rootprint v1.
    pub fn to_fractal_graph(&self) -> Result<SfcsGraph, SfcsVmError> {
        let mut graph = SfcsGraph::new(Vec::new());
        graph
            .insert_node(
                SfcsNode::new("vm_state_initial", SfcsOp::Input, Vec::new())
                    .with_metadata("source", "sfcs-vm")
                    .with_metadata("state_digest", self.initial_state_digest.clone()),
            )
            .map_err(SfcsVmError::Sfcs)?;
        let mut previous = "vm_state_initial".to_string();
        for step in &self.steps {
            let step_id = format!("vm_instr_{:06}", step.step_index);
            graph
                .insert_node(
                    SfcsNode::new(&step_id, SfcsOp::DenseStep, vec![previous.clone()])
                        .with_metadata("source", "sfcs-vm")
                        .with_metadata("kind", "instruction")
                        .with_metadata("pc", format!("{:08x}", step.pc))
                        .with_metadata("instruction", format!("{:08x}", step.instruction))
                        .with_metadata("mnemonic", step.mnemonic.clone())
                        .with_metadata("step_digest", step.step_digest.clone())
                        .with_metadata("state_before_digest", step.state_before_digest.clone())
                        .with_metadata("state_after_digest", step.state_after_digest.clone()),
                )
                .map_err(SfcsVmError::Sfcs)?;
            previous = step_id.clone();
            if let Some(register) = step.rd {
                let register_id = format!("vm_reg_{:06}", step.step_index);
                graph
                    .insert_node(
                        SfcsNode::new(&register_id, SfcsOp::DenseStep, vec![previous.clone()])
                            .with_metadata("source", "sfcs-vm")
                            .with_metadata("kind", "register_write")
                            .with_metadata("register", format!("x{register}"))
                            .with_metadata(
                                "registers_before_digest",
                                step.registers_before_digest.clone(),
                            )
                            .with_metadata(
                                "registers_after_digest",
                                step.registers_after_digest.clone(),
                            ),
                    )
                    .map_err(SfcsVmError::Sfcs)?;
                previous = register_id;
            }
            if let Some(access) = &step.memory_access {
                let address_id = format!("vm_mem_addr_{:06}", step.step_index);
                graph
                    .insert_node(
                        SfcsNode::constant(&address_id, access.address as i64)
                            .with_metadata("source", "sfcs-vm")
                            .with_metadata("kind", "memory_address"),
                    )
                    .map_err(SfcsVmError::Sfcs)?;
                let memory_id = format!("vm_mem_{:06}", step.step_index);
                let (op, inputs) = if access.kind == "read" {
                    (SfcsOp::MemoryRead, vec![address_id, previous.clone()])
                } else {
                    let value_id = format!("vm_mem_value_{:06}", step.step_index);
                    graph
                        .insert_node(
                            SfcsNode::constant(&value_id, access.value as i64)
                                .with_metadata("source", "sfcs-vm")
                                .with_metadata("kind", "memory_value"),
                        )
                        .map_err(SfcsVmError::Sfcs)?;
                    (
                        SfcsOp::MemoryWrite,
                        vec![address_id, value_id, previous.clone()],
                    )
                };
                graph
                    .insert_node(
                        SfcsNode::new(&memory_id, op, inputs)
                            .with_metadata("source", "sfcs-vm")
                            .with_metadata("kind", access.kind.clone())
                            .with_metadata("address", access.address.to_string())
                            .with_metadata("width", access.width.to_string())
                            .with_metadata("value", access.value.to_string())
                            .with_metadata(
                                "memory_before_digest",
                                step.memory_before_digest.clone(),
                            )
                            .with_metadata("memory_after_digest", step.memory_after_digest.clone()),
                    )
                    .map_err(SfcsVmError::Sfcs)?;
                previous = memory_id;
            }
        }
        graph.outputs = vec![previous];
        graph.verify().map_err(SfcsVmError::Sfcs)?;
        Ok(graph)
    }

    fn preimage(&self) -> serde_json::Value {
        serde_json::json!({
            "schema": self.schema,
            "architecture": self.architecture,
            "program_digest": self.program_digest,
            "input_digest": self.input_digest,
            "initial_state_digest": self.initial_state_digest,
            "final_state_digest": self.final_state_digest,
            "steps": self.steps,
            "final_pc": self.final_pc,
            "final_registers": self.final_registers,
            "final_memory_digest": self.final_memory_digest,
            "public_outputs": self.public_outputs,
        })
    }
}

/// One VM instruction transition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcsVmTraceStep {
    /// Zero-based step index.
    pub step_index: u64,
    /// Program counter before executing the instruction.
    pub pc: u32,
    /// Raw RV32I instruction word.
    pub instruction: u32,
    /// Decoded mnemonic.
    pub mnemonic: String,
    /// Destination register, when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rd: Option<u8>,
    /// Source register 1, when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rs1: Option<u8>,
    /// Source register 2, when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rs2: Option<u8>,
    /// Decoded immediate, when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub immediate: Option<i32>,
    /// Program counter after executing the instruction.
    pub next_pc: u32,
    /// Whether this instruction took a control-flow branch.
    pub branch_taken: bool,
    /// Optional memory access.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_access: Option<SfcsVmMemoryAccess>,
    /// Register digest before the step.
    pub registers_before_digest: String,
    /// Register digest after the step.
    pub registers_after_digest: String,
    /// Memory digest before the step.
    pub memory_before_digest: String,
    /// Memory digest after the step.
    pub memory_after_digest: String,
    /// Full VM state digest before the step.
    pub state_before_digest: String,
    /// Full VM state digest after the step.
    pub state_after_digest: String,
    /// Whether the VM halted at this step.
    pub halted: bool,
    /// Step digest.
    pub step_digest: String,
}

impl SfcsVmTraceStep {
    fn preimage(&self) -> serde_json::Value {
        serde_json::json!({
            "step_index": self.step_index,
            "pc": self.pc,
            "instruction": self.instruction,
            "mnemonic": self.mnemonic,
            "rd": self.rd,
            "rs1": self.rs1,
            "rs2": self.rs2,
            "immediate": self.immediate,
            "next_pc": self.next_pc,
            "branch_taken": self.branch_taken,
            "memory_access": self.memory_access,
            "registers_before_digest": self.registers_before_digest,
            "registers_after_digest": self.registers_after_digest,
            "memory_before_digest": self.memory_before_digest,
            "memory_after_digest": self.memory_after_digest,
            "state_before_digest": self.state_before_digest,
            "state_after_digest": self.state_after_digest,
            "halted": self.halted,
        })
    }
}

/// One deterministic memory access.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcsVmMemoryAccess {
    /// `read` or `write`.
    pub kind: String,
    /// Byte address.
    pub address: u32,
    /// Access width in bytes.
    pub width: u8,
    /// Value loaded or stored.
    pub value: u32,
}

/// Public VM outputs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcsVmPublicOutputs {
    /// Selected public registers.
    pub registers: BTreeMap<String, u32>,
    /// Selected public memory bytes keyed by decimal address.
    pub memory: BTreeMap<String, u8>,
}

/// Verified SFCS VM `.pha` embedding summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcsVmExecutionEmbeddingReport {
    /// Program digest.
    pub program_digest: String,
    /// Trace digest.
    pub trace_digest: String,
    /// Digest of the execution fractal projected from VM trace steps.
    pub execution_fractal_digest: String,
    /// Final state digest.
    pub final_state_digest: String,
    /// Core `.pha` fingerprint.
    pub artifact_phx_fingerprint: String,
    /// Total executed instruction steps.
    pub steps: usize,
    /// Final program counter.
    pub final_pc: u32,
    /// Public output binding.
    pub public_outputs: SfcsVmPublicOutputs,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct SfcsVmExecutionProof {
    program: SfcsVmProgram,
    inputs: SfcsVmInputs,
    trace: SfcsVmExecutionTrace,
    execution_fractal: SfcsGraph,
}

/// Verifies a `.pha` artifact carrying an SFCS VM execution trace.
pub fn verify_vm_execution_embedding(
    artifact: &PhaArtifact,
) -> Result<SfcsVmExecutionEmbeddingReport, SfcsVmError> {
    artifact.verify().map_err(SfcsVmError::Pha)?;
    if artifact.embedded_proof.protocol != SFCS_VM_EXECUTION_PROTOCOL_V1_DRAFT {
        return Err(SfcsVmError::InvalidEmbedding(
            "embedded proof protocol is not SFCS VM execution".to_string(),
        ));
    }
    let proof: SfcsVmExecutionProof =
        serde_json::from_value(artifact.embedded_proof.proof.clone())?;
    let expected = proof.program.execute(&proof.inputs)?;
    let expected_fractal = expected.to_fractal_graph()?;
    let expected_fractal_digest = expected_fractal
        .fractal_digest()
        .map_err(SfcsVmError::Sfcs)?;
    if proof.trace != expected {
        return Err(SfcsVmError::InvalidEmbedding(
            "VM execution trace does not replay from program and inputs".to_string(),
        ));
    }
    if proof.execution_fractal != expected_fractal {
        return Err(SfcsVmError::InvalidEmbedding(
            "VM execution fractal does not replay from trace".to_string(),
        ));
    }
    for digest in [
        &expected.program_digest,
        &expected.input_digest,
        &expected.initial_state_digest,
        &expected.final_state_digest,
        &expected.final_memory_digest,
        &expected.trace_digest,
        &expected_fractal_digest,
    ] {
        validate_sha256(digest).map_err(|error| SfcsVmError::InvalidDigest(error.to_string()))?;
    }
    let provenance = &artifact.provenance;
    for (field, expected_value) in [
        ("program_digest", &expected.program_digest),
        ("trace_digest", &expected.trace_digest),
        ("execution_fractal_digest", &expected_fractal_digest),
        ("final_state_digest", &expected.final_state_digest),
    ] {
        let found = provenance
            .get(field)
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| SfcsVmError::InvalidEmbedding(format!("missing provenance {field}")))?;
        if found != expected_value {
            return Err(SfcsVmError::InvalidEmbedding(format!(
                "provenance {field} does not match replay"
            )));
        }
    }
    let public_inputs = &artifact.embedded_proof.public_inputs;
    let expected_public = serde_json::to_value(&expected.public_outputs)?;
    for (field, expected_value) in [
        ("program_digest", serde_json::json!(expected.program_digest)),
        ("input_digest", serde_json::json!(expected.input_digest)),
        ("trace_digest", serde_json::json!(expected.trace_digest)),
        (
            "execution_fractal_digest",
            serde_json::json!(expected_fractal_digest),
        ),
        (
            "initial_state_digest",
            serde_json::json!(expected.initial_state_digest),
        ),
        (
            "final_state_digest",
            serde_json::json!(expected.final_state_digest),
        ),
        (
            "final_memory_digest",
            serde_json::json!(expected.final_memory_digest),
        ),
        ("final_pc", serde_json::json!(expected.final_pc)),
        ("steps", serde_json::json!(expected.steps.len())),
        ("public_outputs", expected_public),
    ] {
        let found = public_inputs
            .get(field)
            .ok_or_else(|| SfcsVmError::InvalidEmbedding(format!("missing public {field}")))?;
        if found != &expected_value {
            return Err(SfcsVmError::InvalidEmbedding(format!(
                "public {field} does not match replay"
            )));
        }
    }
    Ok(SfcsVmExecutionEmbeddingReport {
        program_digest: expected.program_digest,
        trace_digest: expected.trace_digest,
        execution_fractal_digest: expected_fractal_digest,
        final_state_digest: expected.final_state_digest,
        artifact_phx_fingerprint: artifact.phx_fingerprint.clone(),
        steps: expected.steps.len(),
        final_pc: expected.final_pc,
        public_outputs: expected.public_outputs,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SfcsVmState {
    pc: u32,
    registers: [u32; 32],
    memory: BTreeMap<u32, u8>,
}

impl SfcsVmState {
    fn from_inputs(entry_pc: u32, inputs: &SfcsVmInputs) -> Self {
        let mut registers = [0_u32; 32];
        for (register, value) in &inputs.registers {
            if *register != 0 {
                registers[*register as usize] = *value;
            }
        }
        Self {
            pc: entry_pc,
            registers,
            memory: inputs.memory.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DecodedInstruction {
    mnemonic: &'static str,
    rd: Option<u8>,
    rs1: Option<u8>,
    rs2: Option<u8>,
    immediate: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InstructionResult {
    branch_taken: bool,
    halted: bool,
    memory_access: Option<SfcsVmMemoryAccess>,
}

fn execute_instruction(
    state: &mut SfcsVmState,
    instruction: u32,
    _decoded: &DecodedInstruction,
) -> Result<InstructionResult, SfcsVmError> {
    let pc = state.pc;
    let opcode = instruction & 0x7f;
    let mut next_pc = pc.wrapping_add(4);
    let mut branch_taken = false;
    let mut halted = false;
    let mut memory_access = None;

    match opcode {
        0x37 => write_register(state, rd(instruction), imm_u(instruction) as u32),
        0x17 => write_register(
            state,
            rd(instruction),
            pc.wrapping_add(imm_u(instruction) as u32),
        ),
        0x6f => {
            write_register(state, rd(instruction), pc.wrapping_add(4));
            next_pc = pc.wrapping_add(imm_j(instruction) as u32);
            branch_taken = true;
        }
        0x67 => {
            let target = state.registers[rs1(instruction) as usize]
                .wrapping_add(imm_i(instruction) as u32)
                & !1;
            write_register(state, rd(instruction), pc.wrapping_add(4));
            next_pc = target;
            branch_taken = true;
        }
        0x63 => {
            let left = state.registers[rs1(instruction) as usize];
            let right = state.registers[rs2(instruction) as usize];
            let take = match funct3(instruction) {
                0x0 => left == right,
                0x1 => left != right,
                0x4 => (left as i32) < (right as i32),
                0x5 => (left as i32) >= (right as i32),
                0x6 => left < right,
                0x7 => left >= right,
                _ => {
                    return Err(invalid_instruction(
                        instruction,
                        "unsupported branch funct3",
                    ))
                }
            };
            if take {
                next_pc = pc.wrapping_add(imm_b(instruction) as u32);
                branch_taken = true;
            }
        }
        0x03 => {
            let address =
                state.registers[rs1(instruction) as usize].wrapping_add(imm_i(instruction) as u32);
            let loaded = match funct3(instruction) {
                0x0 => sign_extend(load_u8(&state.memory, address)? as u32, 8) as u32,
                0x1 => {
                    ensure_aligned(address, 2)?;
                    sign_extend(load_u16(&state.memory, address)? as u32, 16) as u32
                }
                0x2 => {
                    ensure_aligned(address, 4)?;
                    load_u32(&state.memory, address)?
                }
                0x4 => load_u8(&state.memory, address)? as u32,
                0x5 => {
                    ensure_aligned(address, 2)?;
                    load_u16(&state.memory, address)? as u32
                }
                _ => return Err(invalid_instruction(instruction, "unsupported load funct3")),
            };
            write_register(state, rd(instruction), loaded);
            memory_access = Some(SfcsVmMemoryAccess {
                kind: "read".to_string(),
                address,
                width: load_width(funct3(instruction))?,
                value: loaded,
            });
        }
        0x23 => {
            let address =
                state.registers[rs1(instruction) as usize].wrapping_add(imm_s(instruction) as u32);
            let value = state.registers[rs2(instruction) as usize];
            let width = match funct3(instruction) {
                0x0 => {
                    store_u8(&mut state.memory, address, value as u8);
                    1
                }
                0x1 => {
                    ensure_aligned(address, 2)?;
                    store_u16(&mut state.memory, address, value as u16);
                    2
                }
                0x2 => {
                    ensure_aligned(address, 4)?;
                    store_u32(&mut state.memory, address, value);
                    4
                }
                _ => return Err(invalid_instruction(instruction, "unsupported store funct3")),
            };
            memory_access = Some(SfcsVmMemoryAccess {
                kind: "write".to_string(),
                address,
                width,
                value,
            });
        }
        0x13 => {
            let left = state.registers[rs1(instruction) as usize];
            let imm = imm_i(instruction);
            let value = match funct3(instruction) {
                0x0 => left.wrapping_add(imm as u32),
                0x2 => u32::from((left as i32) < imm),
                0x3 => u32::from(left < imm as u32),
                0x4 => left ^ imm as u32,
                0x6 => left | imm as u32,
                0x7 => left & imm as u32,
                0x1 => {
                    if funct7(instruction) != 0 {
                        return Err(invalid_instruction(instruction, "invalid slli funct7"));
                    }
                    left.wrapping_shl(shamt(instruction))
                }
                0x5 => match funct7(instruction) {
                    0x00 => left.wrapping_shr(shamt(instruction)),
                    0x20 => ((left as i32) >> shamt(instruction)) as u32,
                    _ => return Err(invalid_instruction(instruction, "invalid right shift")),
                },
                _ => {
                    return Err(invalid_instruction(
                        instruction,
                        "unsupported op-imm funct3",
                    ))
                }
            };
            write_register(state, rd(instruction), value);
        }
        0x33 => {
            let left = state.registers[rs1(instruction) as usize];
            let right = state.registers[rs2(instruction) as usize];
            let value = match (funct7(instruction), funct3(instruction)) {
                (0x00, 0x0) => left.wrapping_add(right),
                (0x20, 0x0) => left.wrapping_sub(right),
                (0x00, 0x1) => left.wrapping_shl(right & 0x1f),
                (0x00, 0x2) => u32::from((left as i32) < (right as i32)),
                (0x00, 0x3) => u32::from(left < right),
                (0x00, 0x4) => left ^ right,
                (0x00, 0x5) => left.wrapping_shr(right & 0x1f),
                (0x20, 0x5) => ((left as i32) >> (right & 0x1f)) as u32,
                (0x00, 0x6) => left | right,
                (0x00, 0x7) => left & right,
                _ => return Err(invalid_instruction(instruction, "unsupported op funct")),
            };
            write_register(state, rd(instruction), value);
        }
        0x0f => {
            if funct3(instruction) != 0x0 {
                return Err(invalid_instruction(
                    instruction,
                    "unsupported fence variant",
                ));
            }
        }
        0x73 => match instruction {
            0x0000_0073 | 0x0010_0073 => halted = true,
            _ => {
                return Err(invalid_instruction(
                    instruction,
                    "unsupported system instruction",
                ))
            }
        },
        _ => return Err(invalid_instruction(instruction, "unsupported opcode")),
    }

    if !next_pc.is_multiple_of(4) {
        return Err(SfcsVmError::Execution(format!(
            "instruction at 0x{pc:08x} produced unaligned pc 0x{next_pc:08x}"
        )));
    }
    state.pc = next_pc;
    Ok(InstructionResult {
        branch_taken,
        halted,
        memory_access,
    })
}

fn decode(instruction: u32) -> Result<DecodedInstruction, SfcsVmError> {
    let opcode = instruction & 0x7f;
    let decoded = match opcode {
        0x37 => DecodedInstruction {
            mnemonic: "lui",
            rd: Some(rd(instruction)),
            rs1: None,
            rs2: None,
            immediate: Some(imm_u(instruction)),
        },
        0x17 => DecodedInstruction {
            mnemonic: "auipc",
            rd: Some(rd(instruction)),
            rs1: None,
            rs2: None,
            immediate: Some(imm_u(instruction)),
        },
        0x6f => DecodedInstruction {
            mnemonic: "jal",
            rd: Some(rd(instruction)),
            rs1: None,
            rs2: None,
            immediate: Some(imm_j(instruction)),
        },
        0x67 => DecodedInstruction {
            mnemonic: "jalr",
            rd: Some(rd(instruction)),
            rs1: Some(rs1(instruction)),
            rs2: None,
            immediate: Some(imm_i(instruction)),
        },
        0x63 => DecodedInstruction {
            mnemonic: match funct3(instruction) {
                0x0 => "beq",
                0x1 => "bne",
                0x4 => "blt",
                0x5 => "bge",
                0x6 => "bltu",
                0x7 => "bgeu",
                _ => {
                    return Err(invalid_instruction(
                        instruction,
                        "unsupported branch funct3",
                    ))
                }
            },
            rd: None,
            rs1: Some(rs1(instruction)),
            rs2: Some(rs2(instruction)),
            immediate: Some(imm_b(instruction)),
        },
        0x03 => DecodedInstruction {
            mnemonic: match funct3(instruction) {
                0x0 => "lb",
                0x1 => "lh",
                0x2 => "lw",
                0x4 => "lbu",
                0x5 => "lhu",
                _ => return Err(invalid_instruction(instruction, "unsupported load funct3")),
            },
            rd: Some(rd(instruction)),
            rs1: Some(rs1(instruction)),
            rs2: None,
            immediate: Some(imm_i(instruction)),
        },
        0x23 => DecodedInstruction {
            mnemonic: match funct3(instruction) {
                0x0 => "sb",
                0x1 => "sh",
                0x2 => "sw",
                _ => return Err(invalid_instruction(instruction, "unsupported store funct3")),
            },
            rd: None,
            rs1: Some(rs1(instruction)),
            rs2: Some(rs2(instruction)),
            immediate: Some(imm_s(instruction)),
        },
        0x13 => DecodedInstruction {
            mnemonic: match funct3(instruction) {
                0x0 => "addi",
                0x2 => "slti",
                0x3 => "sltiu",
                0x4 => "xori",
                0x6 => "ori",
                0x7 => "andi",
                0x1 => "slli",
                0x5 if funct7(instruction) == 0x20 => "srai",
                0x5 => "srli",
                _ => {
                    return Err(invalid_instruction(
                        instruction,
                        "unsupported op-imm funct3",
                    ))
                }
            },
            rd: Some(rd(instruction)),
            rs1: Some(rs1(instruction)),
            rs2: None,
            immediate: Some(imm_i(instruction)),
        },
        0x33 => DecodedInstruction {
            mnemonic: match (funct7(instruction), funct3(instruction)) {
                (0x00, 0x0) => "add",
                (0x20, 0x0) => "sub",
                (0x00, 0x1) => "sll",
                (0x00, 0x2) => "slt",
                (0x00, 0x3) => "sltu",
                (0x00, 0x4) => "xor",
                (0x00, 0x5) => "srl",
                (0x20, 0x5) => "sra",
                (0x00, 0x6) => "or",
                (0x00, 0x7) => "and",
                _ => return Err(invalid_instruction(instruction, "unsupported op funct")),
            },
            rd: Some(rd(instruction)),
            rs1: Some(rs1(instruction)),
            rs2: Some(rs2(instruction)),
            immediate: None,
        },
        0x0f => DecodedInstruction {
            mnemonic: "fence",
            rd: None,
            rs1: None,
            rs2: None,
            immediate: None,
        },
        0x73 => DecodedInstruction {
            mnemonic: match instruction {
                0x0000_0073 => "ecall",
                0x0010_0073 => "ebreak",
                _ => {
                    return Err(invalid_instruction(
                        instruction,
                        "unsupported system instruction",
                    ))
                }
            },
            rd: None,
            rs1: None,
            rs2: None,
            immediate: None,
        },
        _ => return Err(invalid_instruction(instruction, "unsupported opcode")),
    };
    Ok(decoded)
}

fn write_register(state: &mut SfcsVmState, register: u8, value: u32) {
    if register != 0 {
        state.registers[register as usize] = value;
    }
}

fn load_u8(memory: &BTreeMap<u32, u8>, address: u32) -> Result<u8, SfcsVmError> {
    Ok(*memory.get(&address).unwrap_or(&0))
}

fn load_u16(memory: &BTreeMap<u32, u8>, address: u32) -> Result<u16, SfcsVmError> {
    let b0 = load_u8(memory, address)? as u16;
    let b1 = load_u8(memory, address.wrapping_add(1))? as u16;
    Ok(b0 | (b1 << 8))
}

fn load_u32(memory: &BTreeMap<u32, u8>, address: u32) -> Result<u32, SfcsVmError> {
    let b0 = load_u8(memory, address)? as u32;
    let b1 = load_u8(memory, address.wrapping_add(1))? as u32;
    let b2 = load_u8(memory, address.wrapping_add(2))? as u32;
    let b3 = load_u8(memory, address.wrapping_add(3))? as u32;
    Ok(b0 | (b1 << 8) | (b2 << 16) | (b3 << 24))
}

fn store_u8(memory: &mut BTreeMap<u32, u8>, address: u32, value: u8) {
    memory.insert(address, value);
}

fn store_u16(memory: &mut BTreeMap<u32, u8>, address: u32, value: u16) {
    store_u8(memory, address, value as u8);
    store_u8(memory, address.wrapping_add(1), (value >> 8) as u8);
}

fn store_u32(memory: &mut BTreeMap<u32, u8>, address: u32, value: u32) {
    store_u8(memory, address, value as u8);
    store_u8(memory, address.wrapping_add(1), (value >> 8) as u8);
    store_u8(memory, address.wrapping_add(2), (value >> 16) as u8);
    store_u8(memory, address.wrapping_add(3), (value >> 24) as u8);
}

fn ensure_aligned(address: u32, width: u32) -> Result<(), SfcsVmError> {
    if !address.is_multiple_of(width) {
        return Err(SfcsVmError::Execution(format!(
            "misaligned {width}-byte memory access at 0x{address:08x}"
        )));
    }
    Ok(())
}

fn load_width(funct3: u32) -> Result<u8, SfcsVmError> {
    match funct3 {
        0x0 | 0x4 => Ok(1),
        0x1 | 0x5 => Ok(2),
        0x2 => Ok(4),
        _ => Err(SfcsVmError::Execution(
            "invalid load width encoding".to_string(),
        )),
    }
}

fn state_digest(state: &SfcsVmState) -> Result<String, SfcsVmError> {
    Ok(digest_json(
        VM_STATE_DOMAIN,
        &serde_json::json!({
            "pc": state.pc,
            "registers": state.registers,
            "memory_digest": memory_digest(&state.memory)?,
        }),
    )?)
}

fn register_digest(registers: &[u32; 32]) -> Result<String, SfcsVmError> {
    Ok(digest_json(VM_REGISTER_DOMAIN, registers)?)
}

fn memory_digest(memory: &BTreeMap<u32, u8>) -> Result<String, SfcsVmError> {
    Ok(digest_json(VM_MEMORY_DOMAIN, memory)?)
}

fn selected_registers(registers: &[u32; 32], selected: &[u8]) -> BTreeMap<String, u32> {
    selected
        .iter()
        .copied()
        .map(|register| (format!("x{register}"), registers[register as usize]))
        .collect()
}

fn selected_memory(
    memory: &BTreeMap<u32, u8>,
    ranges: &[SfcsVmMemoryRange],
) -> BTreeMap<String, u8> {
    let mut selected = BTreeMap::new();
    for range in ranges {
        for offset in 0..range.len {
            let address = range.start.wrapping_add(offset);
            selected.insert(address.to_string(), *memory.get(&address).unwrap_or(&0));
        }
    }
    selected
}

fn rd(instruction: u32) -> u8 {
    ((instruction >> 7) & 0x1f) as u8
}

fn funct3(instruction: u32) -> u32 {
    (instruction >> 12) & 0x7
}

fn rs1(instruction: u32) -> u8 {
    ((instruction >> 15) & 0x1f) as u8
}

fn rs2(instruction: u32) -> u8 {
    ((instruction >> 20) & 0x1f) as u8
}

fn funct7(instruction: u32) -> u32 {
    (instruction >> 25) & 0x7f
}

fn shamt(instruction: u32) -> u32 {
    (instruction >> 20) & 0x1f
}

fn sign_extend(value: u32, bits: u32) -> i32 {
    let shift = 32 - bits;
    ((value << shift) as i32) >> shift
}

fn imm_i(instruction: u32) -> i32 {
    sign_extend(instruction >> 20, 12)
}

fn imm_s(instruction: u32) -> i32 {
    let value = ((instruction >> 7) & 0x1f) | (((instruction >> 25) & 0x7f) << 5);
    sign_extend(value, 12)
}

fn imm_b(instruction: u32) -> i32 {
    let value = (((instruction >> 31) & 0x1) << 12)
        | (((instruction >> 7) & 0x1) << 11)
        | (((instruction >> 25) & 0x3f) << 5)
        | (((instruction >> 8) & 0x0f) << 1);
    sign_extend(value, 13)
}

fn imm_u(instruction: u32) -> i32 {
    (instruction & 0xffff_f000) as i32
}

fn imm_j(instruction: u32) -> i32 {
    let value = (((instruction >> 31) & 0x1) << 20)
        | (((instruction >> 12) & 0xff) << 12)
        | (((instruction >> 20) & 0x1) << 11)
        | (((instruction >> 21) & 0x03ff) << 1);
    sign_extend(value, 21)
}

fn invalid_instruction(instruction: u32, reason: &str) -> SfcsVmError {
    SfcsVmError::Execution(format!("{reason}: instruction 0x{instruction:08x}"))
}

/// Errors returned by SFCS VM execution and embedding verification.
#[derive(Debug)]
pub enum SfcsVmError {
    /// Unsupported VM schema.
    UnsupportedSchema(String),
    /// Program image is invalid.
    InvalidProgram(String),
    /// Input state is invalid.
    InvalidInput(String),
    /// Deterministic VM execution failed.
    Execution(String),
    /// Digest is malformed.
    InvalidDigest(String),
    /// VM `.pha` embedding is inconsistent.
    InvalidEmbedding(String),
    /// Underlying SFCS fractal projection failed.
    Sfcs(super::SfcsError),
    /// JSON serialization failed.
    Json(serde_json::Error),
    /// `.pha` construction or verification failed.
    Pha(PhaError),
}

impl fmt::Display for SfcsVmError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedSchema(schema) => {
                write!(formatter, "unsupported SFCS VM schema: {schema}")
            }
            Self::InvalidProgram(message) => {
                write!(formatter, "invalid SFCS VM program: {message}")
            }
            Self::InvalidInput(message) => write!(formatter, "invalid SFCS VM input: {message}"),
            Self::Execution(message) => write!(formatter, "SFCS VM execution error: {message}"),
            Self::InvalidDigest(message) => write!(formatter, "invalid SFCS VM digest: {message}"),
            Self::InvalidEmbedding(message) => {
                write!(formatter, "invalid SFCS VM embedding: {message}")
            }
            Self::Sfcs(error) => write!(formatter, "SFCS VM fractal projection failed: {error}"),
            Self::Json(error) => write!(formatter, "SFCS VM JSON error: {error}"),
            Self::Pha(error) => write!(formatter, "SFCS VM PHA error: {error}"),
        }
    }
}

impl Error for SfcsVmError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Sfcs(error) => Some(error),
            Self::Json(error) => Some(error),
            Self::Pha(error) => Some(error),
            _ => None,
        }
    }
}

impl From<serde_json::Error> for SfcsVmError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

impl From<super::SfcsError> for SfcsVmError {
    fn from(error: super::SfcsError) -> Self {
        Self::Sfcs(error)
    }
}
