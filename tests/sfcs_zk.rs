#![cfg(feature = "sfcs-zk")]

use power_house::{
    encode_rv32_add, provenance::Rootprint, verify_sfcs_zk_private_add_embedding,
    MemoryCapsuleBuilder, MemoryVerificationPolicy, SfcsVmProgram, SfcsZkError,
    SfcsZkPrivateAddProof, SfcsZkPrivateAddWitness,
};
use serde_json::json;

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
