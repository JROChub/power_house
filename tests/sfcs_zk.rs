#![cfg(feature = "sfcs-zk")]

use power_house::{
    encode_rv32_add, provenance::Rootprint, verify_sfcs_zk_private_add_embedding,
    verify_sfcs_zk_private_vm_embedding, MemoryCapsuleBuilder, MemoryVerificationPolicy,
    SfcsVmInputs, SfcsVmMemoryRange, SfcsVmProgram, SfcsZkError, SfcsZkPrivateAddProof,
    SfcsZkPrivateAddWitness, SfcsZkPrivateVmProof, SfcsZkPrivateVmWitness,
};
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

fn encode_rv32_sub(rd: u8, rs1: u8, rs2: u8) -> u32 {
    r_type(0x20, rs2, rs1, 0x0, rd, 0x33)
}

fn addi(rd: u8, rs1: u8, imm: i32) -> u32 {
    i_type(imm, rs1, 0x0, rd, 0x13)
}

fn encode_rv32_slli(rd: u8, rs1: u8, shift: u8) -> u32 {
    i_type(shift as i32, rs1, 0x1, rd, 0x13)
}

fn lw(rd: u8, rs1: u8, imm: i32) -> u32 {
    i_type(imm, rs1, 0x2, rd, 0x03)
}

fn sw(rs2: u8, rs1: u8, imm: i32) -> u32 {
    s_type(imm, rs2, rs1, 0x2, 0x23)
}

fn sb(rs2: u8, rs1: u8, imm: i32) -> u32 {
    s_type(imm, rs2, rs1, 0x0, 0x23)
}

fn sh(rs2: u8, rs1: u8, imm: i32) -> u32 {
    s_type(imm, rs2, rs1, 0x1, 0x23)
}

fn beq(rs1: u8, rs2: u8, imm: i32) -> u32 {
    b_type(imm, rs2, rs1, 0x0, 0x63)
}

fn bne(rs1: u8, rs2: u8, imm: i32) -> u32 {
    b_type(imm, rs2, rs1, 0x1, 0x63)
}

fn bltu(rs1: u8, rs2: u8, imm: i32) -> u32 {
    b_type(imm, rs2, rs1, 0x6, 0x63)
}

fn bgeu(rs1: u8, rs2: u8, imm: i32) -> u32 {
    b_type(imm, rs2, rs1, 0x7, 0x63)
}

fn and(rd: u8, rs1: u8, rs2: u8) -> u32 {
    r_type(0x00, rs2, rs1, 0x7, rd, 0x33)
}

fn xor(rd: u8, rs1: u8, rs2: u8) -> u32 {
    r_type(0x00, rs2, rs1, 0x4, rd, 0x33)
}

fn ori(rd: u8, rs1: u8, imm: i32) -> u32 {
    i_type(imm, rs1, 0x6, rd, 0x13)
}

fn sltu(rd: u8, rs1: u8, rs2: u8) -> u32 {
    r_type(0x00, rs2, rs1, 0x3, rd, 0x33)
}

fn slti(rd: u8, rs1: u8, imm: i32) -> u32 {
    i_type(imm, rs1, 0x2, rd, 0x13)
}

fn lb(rd: u8, rs1: u8, imm: i32) -> u32 {
    i_type(imm, rs1, 0x0, rd, 0x03)
}

fn lh(rd: u8, rs1: u8, imm: i32) -> u32 {
    i_type(imm, rs1, 0x1, rd, 0x03)
}

fn lbu(rd: u8, rs1: u8, imm: i32) -> u32 {
    i_type(imm, rs1, 0x4, rd, 0x03)
}

fn lhu(rd: u8, rs1: u8, imm: i32) -> u32 {
    i_type(imm, rs1, 0x5, rd, 0x03)
}

fn ecall() -> u32 {
    0x0000_0073
}

fn private_add_program() -> SfcsVmProgram {
    SfcsVmProgram::rv32i(vec![encode_rv32_add(3, 10, 11), 0x0000_0073]).with_max_steps(8)
}

fn witness(lhs: u32, rhs: u32) -> SfcsZkPrivateAddWitness {
    SfcsZkPrivateAddWitness {
        lhs_value: lhs,
        rhs_value: rhs,
        lhs_blinding_seed: [7_u8; 32],
        rhs_blinding_seed: [9_u8; 32],
    }
}

fn private_vm_program() -> SfcsVmProgram {
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

fn private_vm_witness() -> SfcsZkPrivateVmWitness {
    SfcsZkPrivateVmWitness {
        inputs: SfcsVmInputs {
            registers: BTreeMap::from([(10, 777_777_777), (11, 222_222_222)]),
            memory: BTreeMap::from([(128, 99), (129, 88), (130, 77), (131, 66)]),
            public_registers: vec![4, 5],
            public_memory: vec![SfcsVmMemoryRange { start: 0, len: 4 }],
        },
        blinding_seed: [42_u8; 32],
    }
}

#[test]
fn private_add_proof_verifies_and_embeds_without_revealing_private_inputs() {
    let program = private_add_program();
    let proof =
        SfcsZkPrivateAddProof::prove(&program, 10, 11, 3, witness(123_456_789, 987_654)).unwrap();
    proof.verify(&program).unwrap();
    assert_eq!(proof.statement.output_value, 124_444_443);
    assert!(proof.proof_digest.starts_with("sha256:"));
    assert!(proof.statement.lhs_commitment.starts_with("edwards:"));
    assert!(proof.statement.rhs_commitment.starts_with("edwards:"));

    let encoded = serde_json::to_string(&proof).unwrap();
    assert!(!encoded.contains("lhs_value"));
    assert!(!encoded.contains("rhs_value"));
    assert!(!encoded.contains("123456789"));
    assert!(!encoded.contains("987654"));

    let artifact = proof.to_pha_artifact("private-add", &program).unwrap();
    artifact.verify().unwrap();
    assert_eq!(
        artifact.embedded_proof.protocol,
        "power-house/sfcs-zk-private-add/v1-draft"
    );
    let verified = verify_sfcs_zk_private_add_embedding(&artifact).unwrap();
    assert_eq!(verified.proof_digest, proof.proof_digest);

    let rootprint = Rootprint::new("sfcs-zk-private-add", artifact.clone()).unwrap();
    rootprint.verify().unwrap();
    let capsule = MemoryCapsuleBuilder::new("sfcs-zk-private-add")
        .with_pha(artifact)
        .with_rootprint(rootprint)
        .with_replay_required()
        .build()
        .unwrap();
    let report = capsule.verify(MemoryVerificationPolicy::strict()).unwrap();
    assert!(report.core_valid);
    assert!(report.rootprint_valid);
    assert!(report.replay_valid);
}

#[test]
fn private_vm_proof_verifies_embeds_and_hides_witness_inputs() {
    let program = private_vm_program();
    let witness = private_vm_witness();
    let proof = SfcsZkPrivateVmProof::prove(&program, witness).unwrap();
    proof.verify(&program).unwrap();

    assert_eq!(
        proof.statement.schema,
        "power-house/sfcs-zk-private-vm/v1-draft"
    );
    assert_eq!(proof.statement.public_outputs.registers["x4"], 12);
    assert_eq!(proof.statement.public_outputs.registers["x5"], 9);
    assert_eq!(proof.statement.public_outputs.memory["0"], 12);
    assert_eq!(proof.statement.steps, 8);
    assert_eq!(proof.statement.transition_checks, 8);
    assert_eq!(proof.statement.register_range_checks, 256);
    assert_eq!(proof.statement.memory_consistency_checks, 2);
    assert_eq!(proof.statement.linear_relation_checks, 6);
    assert_eq!(proof.linear_relation_proofs.len(), 6);
    assert_eq!(proof.statement.zk_range_proofs, 37);
    assert_eq!(proof.range_proofs.len(), 37);
    assert_eq!(proof.statement.zk_memory_consistency_proofs, 1);
    assert_eq!(proof.memory_consistency_proofs.len(), 1);
    assert_eq!(proof.statement.zk_memory_value_proofs, 2);
    assert_eq!(proof.memory_value_proofs.len(), 2);
    assert_eq!(proof.statement.zk_memory_byte_proofs, 2);
    assert_eq!(proof.memory_byte_proofs.len(), 2);
    assert_eq!(proof.statement.zk_bitwise_proofs, 0);
    assert_eq!(proof.statement.zk_comparison_proofs, 0);
    assert_eq!(proof.statement.zk_branch_proofs, 1);
    assert_eq!(proof.branch_proofs.len(), 1);
    assert_eq!(proof.statement.commitments.len(), 6);
    assert!(proof.proof_digest.starts_with("sha256:"));
    assert!(proof
        .statement
        .commitments
        .values()
        .all(|commitment| commitment.starts_with("edwards:")));

    let encoded_proof = serde_json::to_string(&proof).unwrap();
    assert!(!encoded_proof.contains("777777777"));
    assert!(!encoded_proof.contains("222222222"));

    let artifact = proof.to_pha_artifact("private-vm", &program).unwrap();
    artifact.verify().unwrap();
    assert_eq!(
        artifact.embedded_proof.protocol,
        "power-house/sfcs-zk-private-vm/v1-draft"
    );
    let encoded_artifact = serde_json::to_string(&artifact).unwrap();
    assert!(!encoded_artifact.contains("777777777"));
    assert!(!encoded_artifact.contains("222222222"));
    assert!(!encoded_artifact.contains("\"inputs\""));
    assert!(!encoded_artifact.contains("\"trace\""));

    let verified = verify_sfcs_zk_private_vm_embedding(&artifact).unwrap();
    assert_eq!(verified.proof_digest, proof.proof_digest);

    let rootprint = Rootprint::new("sfcs-zk-private-vm", artifact.clone()).unwrap();
    rootprint.verify().unwrap();
    let capsule = MemoryCapsuleBuilder::new("sfcs-zk-private-vm")
        .with_pha(artifact)
        .with_rootprint(rootprint)
        .with_replay_required()
        .build()
        .unwrap();
    let report = capsule.verify(MemoryVerificationPolicy::strict()).unwrap();
    assert!(report.core_valid);
    assert!(report.rootprint_valid);
    assert!(report.replay_valid);
}

#[test]
fn private_vm_embedding_rejects_mutations() {
    let program = private_vm_program();
    let proof = SfcsZkPrivateVmProof::prove(&program, private_vm_witness()).unwrap();
    let artifact = proof.to_pha_artifact("private-vm", &program).unwrap();

    let mut stale_public = artifact.clone();
    stale_public.embedded_proof.public_inputs["steps"] = json!(9);
    stale_public.refresh_phx_fingerprint().unwrap();
    stale_public.verify().unwrap();
    assert!(matches!(
        verify_sfcs_zk_private_vm_embedding(&stale_public),
        Err(SfcsZkError::InvalidEmbedding(_))
    ));

    let mut stale_commitment = artifact.clone();
    stale_commitment.embedded_proof.proof["proof"]["statement"]["commitments"]["trace_digest"] =
        json!("edwards:00");
    stale_commitment.refresh_phx_fingerprint().unwrap();
    stale_commitment.verify().unwrap();
    assert!(matches!(
        verify_sfcs_zk_private_vm_embedding(&stale_commitment),
        Err(SfcsZkError::InvalidProof(_))
    ));

    let mut stale_response = artifact;
    stale_response.embedded_proof.proof["proof"]["opening_proofs"][0]["response_value"] =
        json!("fr:00");
    stale_response.refresh_phx_fingerprint().unwrap();
    stale_response.verify().unwrap();
    assert!(matches!(
        verify_sfcs_zk_private_vm_embedding(&stale_response),
        Err(SfcsZkError::InvalidProof(_))
    ));

    let mut stale_relation = proof.to_pha_artifact("private-vm", &program).unwrap();
    stale_relation.embedded_proof.proof["proof"]["linear_relation_proofs"][0]
        ["response_blinding"] = json!("fr:00");
    stale_relation.refresh_phx_fingerprint().unwrap();
    stale_relation.verify().unwrap();
    assert!(matches!(
        verify_sfcs_zk_private_vm_embedding(&stale_relation),
        Err(SfcsZkError::InvalidProof(_))
    ));
}

#[test]
fn private_vm_linear_relations_cover_sub_subi_and_scale() {
    let program = SfcsVmProgram::rv32i(vec![
        addi(1, 0, 20), // addi
        addi(2, 0, 7),  // addi
        encode_rv32_sub(3, 1, 2),
        addi(4, 3, -5), // subi
        encode_rv32_slli(5, 4, 2),
        ecall(),
    ])
    .with_max_steps(16);
    let mut witness = private_vm_witness();
    witness.inputs.public_registers = vec![5];
    witness.inputs.public_memory = Vec::new();
    let proof = SfcsZkPrivateVmProof::prove(&program, witness).unwrap();
    proof.verify(&program).unwrap();
    assert_eq!(proof.statement.public_outputs.registers["x5"], 32);
    assert_eq!(proof.statement.linear_relation_checks, 5);
    assert_eq!(proof.statement.zk_range_proofs, 11);
    assert_eq!(proof.statement.zk_memory_consistency_proofs, 0);
    assert_eq!(proof.statement.zk_memory_value_proofs, 0);
    assert_eq!(proof.statement.zk_branch_proofs, 0);
    let relations = proof
        .linear_relation_proofs
        .iter()
        .map(|proof| proof.relation.as_str())
        .collect::<Vec<_>>();
    assert_eq!(relations, ["addi", "addi", "sub", "subi", "scale"]);

    let mut mutated = proof;
    mutated.linear_relation_proofs[2].relation_commitment = mutated
        .linear_relation_proofs
        .first()
        .unwrap()
        .relation_commitment
        .clone();
    assert!(matches!(
        mutated.verify(&program),
        Err(SfcsZkError::InvalidProof(_))
    ));

    let mut mutated_range = SfcsZkPrivateVmProof::prove(&program, private_vm_witness()).unwrap();
    mutated_range.range_proofs[0].bit_proofs[0].zero_response = "fr:00".to_string();
    assert!(matches!(
        mutated_range.verify(&program),
        Err(SfcsZkError::InvalidProof(_))
    ));

    let memory_program = private_vm_program();
    let mut mutated_memory =
        SfcsZkPrivateVmProof::prove(&memory_program, private_vm_witness()).unwrap();
    mutated_memory.memory_consistency_proofs[0]
        .value_equality
        .response_blinding = "fr:00".to_string();
    assert!(matches!(
        mutated_memory.verify(&memory_program),
        Err(SfcsZkError::InvalidProof(_))
    ));

    let mut mutated_memory_value =
        SfcsZkPrivateVmProof::prove(&memory_program, private_vm_witness()).unwrap();
    mutated_memory_value.memory_value_proofs[0]
        .value_equality
        .response_blinding = "fr:00".to_string();
    assert!(matches!(
        mutated_memory_value.verify(&memory_program),
        Err(SfcsZkError::InvalidProof(_))
    ));

    let mut mutated_branch =
        SfcsZkPrivateVmProof::prove(&memory_program, private_vm_witness()).unwrap();
    mutated_branch.branch_proofs[0]
        .equality
        .as_mut()
        .unwrap()
        .response_blinding = "fr:00".to_string();
    assert!(matches!(
        mutated_branch.verify(&memory_program),
        Err(SfcsZkError::InvalidProof(_))
    ));
}

#[test]
fn private_vm_proves_bitwise_comparison_order_branches_and_partial_memory() {
    let program = SfcsVmProgram::rv32i(vec![
        addi(1, 0, 12),  // x1 = 12
        addi(2, 0, 10),  // x2 = 10
        and(3, 1, 2),    // x3 = 8
        xor(4, 1, 2),    // x4 = 6
        ori(5, 4, 1),    // x5 = 7
        sltu(6, 4, 1),   // x6 = 1
        slti(7, 4, 10),  // x7 = 1
        bne(1, 2, 8),    // taken, proves non-equality
        addi(8, 0, 99),  // skipped
        bltu(2, 1, 8),   // taken, proves unsigned order
        addi(8, 0, 98),  // skipped
        bgeu(1, 2, 8),   // taken, proves inverted unsigned order
        addi(8, 0, 97),  // skipped
        addi(9, 0, 256), // x9 = byte-memory base
        sb(10, 9, 0),    // byte 0xff
        lbu(11, 9, 0),   // zero-extend 0xff
        lb(12, 9, 0),    // sign-extend 0xff
        sh(10, 9, 2),    // bytes 0xff, 0x80
        lhu(13, 9, 2),   // zero-extend 0x80ff
        lh(14, 9, 2),    // sign-extend 0x80ff
        ecall(),
    ])
    .with_max_steps(64);
    let witness = SfcsZkPrivateVmWitness {
        inputs: SfcsVmInputs {
            registers: BTreeMap::from([(10, 0xffff_80ff)]),
            memory: BTreeMap::new(),
            public_registers: vec![3, 4, 5, 6, 7, 11, 12, 13, 14],
            public_memory: vec![SfcsVmMemoryRange { start: 256, len: 4 }],
        },
        blinding_seed: [77_u8; 32],
    };
    let proof = SfcsZkPrivateVmProof::prove(&program, witness).unwrap();
    proof.verify(&program).unwrap();

    assert_eq!(proof.statement.public_outputs.registers["x3"], 8);
    assert_eq!(proof.statement.public_outputs.registers["x4"], 6);
    assert_eq!(proof.statement.public_outputs.registers["x5"], 7);
    assert_eq!(proof.statement.public_outputs.registers["x6"], 1);
    assert_eq!(proof.statement.public_outputs.registers["x7"], 1);
    assert_eq!(proof.statement.public_outputs.registers["x11"], 0xff);
    assert_eq!(proof.statement.public_outputs.registers["x12"], 0xffff_ffff);
    assert_eq!(proof.statement.public_outputs.registers["x13"], 0x80ff);
    assert_eq!(proof.statement.public_outputs.registers["x14"], 0xffff_80ff);
    assert_eq!(proof.statement.zk_bitwise_proofs, 3);
    assert_eq!(proof.bitwise_proofs.len(), 3);
    assert_eq!(proof.statement.zk_comparison_proofs, 2);
    assert_eq!(proof.comparison_proofs.len(), 2);
    assert_eq!(proof.statement.zk_branch_proofs, 3);
    assert_eq!(proof.branch_proofs.len(), 3);
    assert_eq!(proof.statement.zk_memory_byte_proofs, 6);
    assert_eq!(proof.memory_byte_proofs.len(), 6);
    assert_eq!(proof.statement.zk_memory_consistency_proofs, 1);
    assert_eq!(proof.statement.zk_memory_value_proofs, 6);
    assert_eq!(proof.statement.zk_range_proofs, 88);

    let mut bitwise_mutation = proof.clone();
    bitwise_mutation.bitwise_proofs[0].bit_proofs[0].branches[0].responses[0] = "fr:00".to_string();
    assert!(matches!(
        bitwise_mutation.verify(&program),
        Err(SfcsZkError::InvalidProof(_))
    ));

    let mut comparison_mutation = proof.clone();
    comparison_mutation.comparison_proofs[0]
        .relation_proof
        .branches[0]
        .responses[0] = "fr:00".to_string();
    assert!(matches!(
        comparison_mutation.verify(&program),
        Err(SfcsZkError::InvalidProof(_))
    ));

    let mut branch_mutation = proof.clone();
    branch_mutation.branch_proofs[0]
        .condition
        .as_mut()
        .unwrap()
        .branches[0]
        .responses[0] = "fr:00".to_string();
    assert!(matches!(
        branch_mutation.verify(&program),
        Err(SfcsZkError::InvalidProof(_))
    ));

    let mut memory_mutation = proof;
    memory_mutation.memory_byte_proofs[0]
        .value_semantics
        .branches[0]
        .responses[0] = "fr:00".to_string();
    assert!(matches!(
        memory_mutation.verify(&program),
        Err(SfcsZkError::InvalidProof(_))
    ));
}

#[test]
fn private_add_embedding_rejects_mutations() {
    let program = private_add_program();
    let proof = SfcsZkPrivateAddProof::prove(&program, 10, 11, 3, witness(5, 7)).unwrap();
    let artifact = proof.to_pha_artifact("private-add", &program).unwrap();

    let mut stale_public = artifact.clone();
    stale_public.embedded_proof.public_inputs["output_value"] = json!(13);
    stale_public.refresh_phx_fingerprint().unwrap();
    stale_public.verify().unwrap();
    assert!(matches!(
        verify_sfcs_zk_private_add_embedding(&stale_public),
        Err(SfcsZkError::InvalidEmbedding(_))
    ));

    let mut stale_proof = artifact.clone();
    stale_proof.embedded_proof.proof["proof"]["statement"]["output_value"] = json!(13);
    stale_proof.refresh_phx_fingerprint().unwrap();
    stale_proof.verify().unwrap();
    assert!(matches!(
        verify_sfcs_zk_private_add_embedding(&stale_proof),
        Err(SfcsZkError::InvalidProof(_))
    ));

    let mut stale_challenge = artifact;
    stale_challenge.embedded_proof.proof["proof"]["challenge"] = json!("fr:00");
    stale_challenge.refresh_phx_fingerprint().unwrap();
    stale_challenge.verify().unwrap();
    assert!(matches!(
        verify_sfcs_zk_private_add_embedding(&stale_challenge),
        Err(SfcsZkError::InvalidProof(_))
    ));
}

#[test]
fn private_add_profile_rejects_overflow_and_wrong_program() {
    let program = private_add_program();
    assert!(matches!(
        SfcsZkPrivateAddProof::prove(&program, 10, 11, 3, witness(u32::MAX, 1)),
        Err(SfcsZkError::InvalidWitness(_))
    ));

    let wrong_program =
        SfcsVmProgram::rv32i(vec![encode_rv32_add(4, 10, 11), 0x0000_0073]).with_max_steps(8);
    assert!(matches!(
        SfcsZkPrivateAddProof::prove(&wrong_program, 10, 11, 3, witness(5, 7)),
        Err(SfcsZkError::InvalidProgram(_))
    ));
}
