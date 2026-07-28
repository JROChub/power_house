use power_house::rollup::{verify_zk_rollup, RollupCommitment, ZkRollupProof};

#[test]
fn legacy_deterministic_groth16_envelope_is_rejected_fail_closed() {
    let commitment = RollupCommitment {
        namespace: "security-regression".to_string(),
        share_root: "00".repeat(32),
        pedersen_root: Some("00".repeat(32)),
        settlement_slot: None,
    };
    let proof = ZkRollupProof {
        proof: vec![1, 2, 3],
        public_inputs: vec![0; 128],
        merkle_path: b"[]".to_vec(),
    };

    let error = verify_zk_rollup(&commitment, &proof).unwrap_err();
    assert!(error.contains("retired"));
    assert!(error.contains("verifier key"));
}
