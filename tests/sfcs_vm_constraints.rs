#![cfg(feature = "sfcs")]

use power_house::{
    provenance::Rootprint, verify_sfcs_vm_constraint_embedding, MemoryCapsuleBuilder,
    MemoryVerificationPolicy, SfcsVmConstraintError, SfcsVmConstraintProof, SfcsVmInputs,
    SfcsVmMemoryRange, SfcsVmProgram,
};
use serde_json::json;
use std::collections::BTreeMap;

fn memory_program() -> SfcsVmProgram {
    SfcsVmProgram::rv32i(vec![
        0x0050_0093_u32,
        0x0070_0113_u32,
        0x0020_81b3_u32,
        0x0030_2023_u32,
        0x0000_2203_u32,
        0x0000_0073_u32,
    ])
    .with_max_steps(16)
}

fn public_inputs() -> SfcsVmInputs {
    SfcsVmInputs {
        registers: BTreeMap::new(),
        memory: BTreeMap::new(),
        public_registers: vec![4],
        public_memory: vec![SfcsVmMemoryRange { start: 0, len: 4 }],
    }
}

#[test]
fn vm_constraint_proof_covers_transitions_memory_and_ranges() {
    let program = memory_program();
    let inputs = public_inputs();
    let proof = SfcsVmConstraintProof::prove(&program, &inputs).unwrap();

    proof.verify(&program, &inputs).unwrap();
    assert_eq!(proof.steps, 6);
    assert_eq!(proof.transition_checks, 6);
    assert_eq!(proof.register_range_checks, 192);
    assert_eq!(proof.memory_consistency_checks, 2);
    assert_eq!(proof.memory_events.len(), 2);
    assert!(proof.memory_range_checks >= 8);
    assert!(proof.proof_digest.starts_with("sha256:"));
    assert!(proof.execution_fractal_digest.starts_with("sha256:"));

    let artifact = proof
        .to_pha_artifact("vm-constraints", &program, &inputs)
        .unwrap();
    artifact.verify().unwrap();
    let verified = verify_sfcs_vm_constraint_embedding(&artifact).unwrap();
    assert_eq!(verified, proof);

    let rootprint = Rootprint::new("vm-constraints", artifact.clone()).unwrap();
    let capsule = MemoryCapsuleBuilder::new("vm-constraints")
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
fn vm_constraint_embedding_rejects_transition_and_public_input_mutations() {
    let program = memory_program();
    let inputs = public_inputs();
    let proof = SfcsVmConstraintProof::prove(&program, &inputs).unwrap();
    let artifact = proof
        .to_pha_artifact("vm-constraints", &program, &inputs)
        .unwrap();

    let mut stale_step = artifact.clone();
    stale_step.embedded_proof.proof["proof"]["step_commitments"][2]["instruction"] = json!(0_u32);
    stale_step.refresh_phx_fingerprint().unwrap();
    stale_step.verify().unwrap();
    assert!(matches!(
        verify_sfcs_vm_constraint_embedding(&stale_step),
        Err(SfcsVmConstraintError::InvalidProof(_))
    ));

    let mut stale_public = artifact;
    stale_public.embedded_proof.public_inputs["proof_digest"] =
        json!("sha256:".to_owned() + &"0".repeat(64));
    stale_public.refresh_phx_fingerprint().unwrap();
    stale_public.verify().unwrap();
    assert!(matches!(
        verify_sfcs_vm_constraint_embedding(&stale_public),
        Err(SfcsVmConstraintError::InvalidEmbedding(_))
    ));
}
