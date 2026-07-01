#![cfg(feature = "sfcs-zk")]

use power_house::{
    compile_private_add_source, memory::semantic_packet_digest, SfcsCompilerError,
    SfcsZkPrivateAddProof, SfcsZkPrivateAddWitness,
};

fn source() -> &'static str {
    "pub fn add(lhs: u32, rhs: u32) -> u32 { lhs + rhs }"
}

#[test]
fn compiler_emits_deterministic_private_add_profile() {
    let first = compile_private_add_source(source()).unwrap();
    let second = compile_private_add_source(source()).unwrap();

    assert_eq!(first, second);
    assert_eq!(first.schema, "power-house/sfcs-rust-private-add/v1-draft");
    assert_eq!(first.language, "rust-subset");
    assert_eq!(first.function_name, "add");
    assert_eq!(first.lhs_name, "lhs");
    assert_eq!(first.rhs_name, "rhs");
    assert_eq!(first.return_type, "u32");
    assert_eq!(first.lhs_register, 10);
    assert_eq!(first.rhs_register, 11);
    assert_eq!(first.output_register, 3);
    assert!(first.source_digest.starts_with("sha256:"));
    assert!(first.program_digest().unwrap().starts_with("sha256:"));

    let packet_digest = first.semantic_packet["packet_digest"].as_str().unwrap();
    assert_eq!(
        packet_digest,
        semantic_packet_digest(&first.semantic_packet).unwrap()
    );
    assert_eq!(
        first.semantic_packet["explanation_constraints"]["mark_generated_text_non_authoritative"],
        true
    );
}

#[test]
fn compiler_output_proves_and_hides_private_inputs() {
    let compiled = compile_private_add_source(source()).unwrap();
    let proof = SfcsZkPrivateAddProof::prove(
        &compiled.program,
        compiled.lhs_register,
        compiled.rhs_register,
        compiled.output_register,
        SfcsZkPrivateAddWitness {
            lhs_value: 321,
            rhs_value: 654,
            lhs_blinding_seed: [3_u8; 32],
            rhs_blinding_seed: [4_u8; 32],
        },
    )
    .unwrap();

    proof.verify(&compiled.program).unwrap();
    assert_eq!(proof.statement.output_value, 975);
    let encoded = serde_json::to_string(&proof).unwrap();
    assert!(!encoded.contains("lhs_value"));
    assert!(!encoded.contains("rhs_value"));
    assert!(!encoded.contains("321"));
    assert!(!encoded.contains("654"));
}

#[test]
fn compiler_rejects_unsupported_rust_subset() {
    for bad_source in [
        "fn add(lhs: u32, rhs: u32) -> u32 { lhs - rhs }",
        "fn add(lhs: u32, rhs: u32) -> u64 { lhs + rhs }",
        "fn add(lhs: u32, lhs: u32) -> u32 { lhs + lhs }",
        "fn add(lhs: u64, rhs: u32) -> u32 { lhs + rhs }",
        "fn add(lhs: u32, rhs: u32, extra: u32) -> u32 { lhs + rhs }",
    ] {
        assert!(matches!(
            compile_private_add_source(bad_source),
            Err(SfcsCompilerError::InvalidSource(_))
        ));
    }
}
