#![cfg(feature = "sfcs")]

use power_house::{
    provenance::Rootprint, verify_sfcs_vm_execution_embedding, MemoryCapsuleBuilder,
    MemoryVerificationPolicy, SfcsVmError, SfcsVmInputs, SfcsVmMemoryRange, SfcsVmProgram,
};
use proptest::prelude::*;
use serde_json::json;
use std::collections::BTreeMap;

fn r_type(funct7: u32, rs2: u8, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    (funct7 << 25)
        | ((rs2 as u32) << 20)
        | ((rs1 as u32) << 15)
        | (funct3 << 12)
        | ((rd as u32) << 7)
        | opcode
}

fn i_type(imm: i32, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    (((imm as u32) & 0x0fff) << 20)
        | ((rs1 as u32) << 15)
        | (funct3 << 12)
        | ((rd as u32) << 7)
        | opcode
}

fn s_type(imm: i32, rs2: u8, rs1: u8, funct3: u32, opcode: u32) -> u32 {
    let imm = (imm as u32) & 0x0fff;
    ((imm >> 5) << 25)
        | ((rs2 as u32) << 20)
        | ((rs1 as u32) << 15)
        | (funct3 << 12)
        | ((imm & 0x1f) << 7)
        | opcode
}

fn b_type(imm: i32, rs2: u8, rs1: u8, funct3: u32, opcode: u32) -> u32 {
    let imm = (imm as u32) & 0x1fff;
    (((imm >> 12) & 0x1) << 31)
        | (((imm >> 5) & 0x3f) << 25)
        | ((rs2 as u32) << 20)
        | ((rs1 as u32) << 15)
        | (funct3 << 12)
        | (((imm >> 1) & 0x0f) << 8)
        | (((imm >> 11) & 0x1) << 7)
        | opcode
}

fn add(rd: u8, rs1: u8, rs2: u8) -> u32 {
    r_type(0x00, rs2, rs1, 0x0, rd, 0x33)
}

fn addi(rd: u8, rs1: u8, imm: i32) -> u32 {
    i_type(imm, rs1, 0x0, rd, 0x13)
}

fn lw(rd: u8, rs1: u8, imm: i32) -> u32 {
    i_type(imm, rs1, 0x2, rd, 0x03)
}

fn sw(rs2: u8, rs1: u8, imm: i32) -> u32 {
    s_type(imm, rs2, rs1, 0x2, 0x23)
}

fn beq(rs1: u8, rs2: u8, imm: i32) -> u32 {
    b_type(imm, rs2, rs1, 0x0, 0x63)
}

fn ecall() -> u32 {
    0x0000_0073
}

fn sample_vm_program() -> SfcsVmProgram {
    SfcsVmProgram::rv32i(vec![
        addi(1, 0, 5), // x1 = 5
        addi(2, 0, 7), // x2 = 7
        add(3, 1, 2),  // x3 = 12
        sw(3, 0, 0),   // memory[0..4] = 12
        lw(4, 0, 0),   // x4 = 12
        beq(4, 3, 8),  // skip x5 = 1
        addi(5, 0, 1), // skipped
        addi(5, 0, 9), // x5 = 9
        ecall(),
    ])
    .with_max_steps(64)
}

fn sample_inputs() -> SfcsVmInputs {
    SfcsVmInputs {
        registers: BTreeMap::new(),
        memory: BTreeMap::new(),
        public_registers: vec![4, 5],
        public_memory: vec![SfcsVmMemoryRange { start: 0, len: 4 }],
    }
}

#[test]
fn rv32i_vm_executes_control_memory_and_embeds_into_power_house_identity() {
    let program = sample_vm_program();
    let inputs = sample_inputs();
    let first = program.execute(&inputs).unwrap();
    let second = program.execute(&inputs).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.final_registers[4], 12);
    assert_eq!(first.final_registers[5], 9);
    assert_eq!(first.public_outputs.registers["x4"], 12);
    assert_eq!(first.public_outputs.registers["x5"], 9);
    assert_eq!(first.public_outputs.memory["0"], 12);
    assert_eq!(first.public_outputs.memory["1"], 0);
    assert!(first.trace_digest.starts_with("sha256:"));
    assert!(first.steps.iter().any(|step| step.branch_taken));
    assert!(first.steps.iter().any(|step| {
        step.memory_access
            .as_ref()
            .map(|access| access.kind.as_str() == "write")
            .unwrap_or(false)
    }));
    let execution_fractal = first.to_fractal_graph().unwrap();
    let execution_fractal_digest = execution_fractal.fractal_digest().unwrap();
    assert!(execution_fractal_digest.starts_with("sha256:"));
    assert!(execution_fractal.nodes.contains_key("vm_instr_000000"));
    assert!(execution_fractal.nodes.contains_key("vm_reg_000000"));
    assert!(execution_fractal.nodes.contains_key("vm_mem_000003"));

    let artifact = program
        .to_execution_pha_artifact("rv32i-sample", &inputs)
        .unwrap();
    artifact.verify().unwrap();
    let report = verify_sfcs_vm_execution_embedding(&artifact).unwrap();
    assert_eq!(report.trace_digest, first.trace_digest);
    assert_eq!(report.execution_fractal_digest, execution_fractal_digest);
    assert_eq!(report.final_state_digest, first.final_state_digest);
    assert_eq!(report.steps, first.steps.len());

    let rootprint = Rootprint::new("sfcs-vm-sample", artifact.clone()).unwrap();
    rootprint.verify().unwrap();
    let capsule = MemoryCapsuleBuilder::new("sfcs-vm-sample")
        .with_pha(artifact)
        .with_rootprint(rootprint)
        .with_replay_required()
        .build()
        .unwrap();
    let capsule_report = capsule.verify(MemoryVerificationPolicy::strict()).unwrap();
    assert!(capsule_report.core_valid);
    assert!(capsule_report.rootprint_valid);
    assert!(capsule_report.replay_valid);
}

#[test]
fn rv32i_vm_embedding_rejects_trace_and_public_output_mutations() {
    let program = sample_vm_program();
    let inputs = sample_inputs();
    let artifact = program
        .to_execution_pha_artifact("rv32i-sample", &inputs)
        .unwrap();

    let mut stale_trace = artifact.clone();
    stale_trace.embedded_proof.proof["trace"]["steps"][2]["instruction"] = json!(addi(3, 0, 99));
    stale_trace.refresh_phx_fingerprint().unwrap();
    stale_trace.verify().unwrap();
    assert!(matches!(
        verify_sfcs_vm_execution_embedding(&stale_trace),
        Err(SfcsVmError::InvalidEmbedding(_))
    ));

    let mut stale_public = artifact;
    stale_public.embedded_proof.public_inputs["public_outputs"]["registers"]["x5"] = json!(10);
    stale_public.refresh_phx_fingerprint().unwrap();
    stale_public.verify().unwrap();
    assert!(matches!(
        verify_sfcs_vm_execution_embedding(&stale_public),
        Err(SfcsVmError::InvalidEmbedding(_))
    ));

    let mut stale_fractal = program
        .to_execution_pha_artifact("rv32i-sample", &inputs)
        .unwrap();
    stale_fractal.embedded_proof.proof["execution_fractal"]["nodes"]["vm_instr_000000"]
        ["metadata"]["mnemonic"] = json!("tampered");
    stale_fractal.refresh_phx_fingerprint().unwrap();
    stale_fractal.verify().unwrap();
    assert!(matches!(
        verify_sfcs_vm_execution_embedding(&stale_fractal),
        Err(SfcsVmError::InvalidEmbedding(_))
    ));
}

#[test]
fn rv32i_vm_rejects_invalid_execution() {
    let misaligned = SfcsVmProgram::rv32i(vec![lw(1, 0, 2), ecall()]).with_max_steps(8);
    assert!(matches!(
        misaligned.execute(&SfcsVmInputs::empty()),
        Err(SfcsVmError::Execution(_))
    ));

    let non_halting = SfcsVmProgram::rv32i(vec![beq(0, 0, 0)]).with_max_steps(4);
    assert!(matches!(
        non_halting.execute(&SfcsVmInputs::empty()),
        Err(SfcsVmError::Execution(_))
    ));

    let bad_registers = SfcsVmInputs {
        registers: BTreeMap::from([(32, 1)]),
        memory: BTreeMap::new(),
        public_registers: vec![],
        public_memory: vec![],
    };
    assert!(matches!(
        SfcsVmProgram::rv32i(vec![ecall()]).execute(&bad_registers),
        Err(SfcsVmError::InvalidInput(_))
    ));
}

proptest! {
    #[test]
    fn rv32i_vm_trace_is_reproducible_and_input_sensitive(a in any::<u32>(), b in any::<u32>()) {
        let program = SfcsVmProgram::rv32i(vec![
            add(3, 10, 11),
            sw(3, 0, 0),
            lw(4, 0, 0),
            ecall(),
        ]).with_max_steps(16);
        let inputs = SfcsVmInputs {
            registers: BTreeMap::from([(10, a), (11, b)]),
            memory: BTreeMap::new(),
            public_registers: vec![4],
            public_memory: vec![SfcsVmMemoryRange { start: 0, len: 4 }],
        };
        let first = program.execute(&inputs).unwrap();
        let second = program.execute(&inputs).unwrap();
        prop_assert_eq!(&first, &second);
        prop_assert_eq!(first.final_registers[4], a.wrapping_add(b));
        prop_assert_eq!(first.public_outputs.registers["x4"], a.wrapping_add(b));

        let changed_inputs = SfcsVmInputs {
            registers: BTreeMap::from([(10, a.wrapping_add(1)), (11, b)]),
            memory: BTreeMap::new(),
            public_registers: vec![4],
            public_memory: vec![SfcsVmMemoryRange { start: 0, len: 4 }],
        };
        let changed = program.execute(&changed_inputs).unwrap();
        if a.wrapping_add(1).wrapping_add(b) != a.wrapping_add(b) {
            prop_assert_ne!(first.trace_digest, changed.trace_digest);
        }
    }
}
