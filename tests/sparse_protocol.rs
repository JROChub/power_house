use power_house::{
    CommittedSparsePolynomial, CommittedSparseProof, Field, SeededSparseProof, SparseMonomial,
};
use proptest::prelude::*;
use sha2::{Digest, Sha256};
use std::fs;

fn dense_sum(polynomial: &CommittedSparsePolynomial, field: &Field) -> u64 {
    let mut total = 0;
    for assignment in 0..(1usize << polynomial.num_vars()) {
        for term in polynomial.terms() {
            if term
                .variables()
                .iter()
                .all(|&variable| assignment & (1usize << variable) != 0)
            {
                total = field.add(total, term.coefficient());
            }
        }
    }
    total
}

proptest! {
    #[test]
    fn committed_sparse_matches_dense_enumeration(
        num_vars in 1usize..9,
        raw_terms in prop::collection::vec(
            (1u64..1_000_000, prop::collection::vec(0usize..8, 1..7)),
            1..16,
        ),
    ) {
        let field = Field::new(1_000_000_007);
        let terms = raw_terms
            .into_iter()
            .filter_map(|(coefficient, variables)| {
                let mut variables: Vec<_> = variables
                    .into_iter()
                    .map(|variable| variable % num_vars)
                    .collect();
                variables.sort_unstable();
                variables.dedup();
                SparseMonomial::new(coefficient, variables).ok()
            })
            .collect::<Vec<_>>();
        prop_assume!(!terms.is_empty());

        let polynomial = CommittedSparsePolynomial::new(num_vars, terms).unwrap();
        let proof = CommittedSparseProof::prove(&polynomial, &field).unwrap();
        prop_assert_eq!(proof.claimed_sum, dense_sum(&polynomial, &field));
        prop_assert!(proof.verify(&polynomial, &field).is_ok());

        let decoded_polynomial =
            CommittedSparsePolynomial::from_bytes(&polynomial.to_bytes()).unwrap();
        let decoded_proof = CommittedSparseProof::from_bytes(&proof.to_bytes()).unwrap();
        prop_assert!(decoded_proof.verify(&decoded_polynomial, &field).is_ok());
    }

    #[test]
    fn sparse_decoders_never_panic_on_arbitrary_bytes(bytes in prop::collection::vec(any::<u8>(), 0..2048)) {
        let committed_polynomial = std::panic::catch_unwind(|| {
            CommittedSparsePolynomial::from_bytes(&bytes)
        });
        let committed_proof = std::panic::catch_unwind(|| {
            CommittedSparseProof::from_bytes(&bytes)
        });
        let seeded_proof = std::panic::catch_unwind(|| SeededSparseProof::from_bytes(&bytes));

        prop_assert!(committed_polynomial.is_ok());
        prop_assert!(committed_proof.is_ok());
        prop_assert!(seeded_proof.is_ok());
    }
}

#[test]
fn every_single_byte_committed_mutation_is_rejected() {
    let field = Field::new(1_000_000_007);
    let polynomial_bytes = fs::read("conformance/v1/committed-valid.phsm").unwrap();
    let proof_bytes = fs::read("conformance/v1/committed-valid.phcp").unwrap();
    let polynomial = CommittedSparsePolynomial::from_bytes(&polynomial_bytes).unwrap();
    let proof = CommittedSparseProof::from_bytes(&proof_bytes).unwrap();

    for index in 0..proof_bytes.len() {
        let mut mutated = proof_bytes.clone();
        mutated[index] ^= 1;
        let accepted = CommittedSparseProof::from_bytes(&mutated)
            .and_then(|candidate| candidate.verify(&polynomial, &field))
            .is_ok();
        assert!(!accepted, "proof mutation at byte {index} was accepted");
    }

    for index in 0..polynomial_bytes.len() {
        let mut mutated = polynomial_bytes.clone();
        mutated[index] ^= 1;
        let accepted = CommittedSparsePolynomial::from_bytes(&mutated)
            .and_then(|candidate| proof.verify(&candidate, &field))
            .is_ok();
        assert!(
            !accepted,
            "polynomial mutation at byte {index} was accepted"
        );
    }
}

#[test]
fn every_single_byte_seeded_mutation_is_rejected() {
    let field = Field::new(1_000_000_007);
    let bytes = fs::read("conformance/v1/seeded-valid.phsp").unwrap();

    for index in 0..bytes.len() {
        let mut mutated = bytes.clone();
        mutated[index] ^= 1;
        let accepted = SeededSparseProof::from_bytes(&mutated)
            .and_then(|candidate| candidate.verify(&field))
            .is_ok();
        assert!(!accepted, "seeded mutation at byte {index} was accepted");
    }
}

#[test]
fn oversized_polynomial_degree_is_rejected_before_allocation() {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"PHSMv1\0\0");
    bytes.extend_from_slice(&8u64.to_be_bytes());
    bytes.extend_from_slice(&1u64.to_be_bytes());
    bytes.extend_from_slice(&1u64.to_be_bytes());
    bytes.extend_from_slice(&u64::MAX.to_be_bytes());

    assert!(CommittedSparsePolynomial::from_bytes(&bytes).is_err());
}

#[test]
fn committed_conformance_vectors_match_manifest() {
    let field = Field::new(1_000_000_007);
    let manifest: serde_json::Value = serde_json::from_slice(
        &fs::read("conformance/v1/manifest.json").expect("conformance manifest"),
    )
    .unwrap();
    let seeded_bytes = fs::read("conformance/v1/seeded-valid.phsp").unwrap();
    let polynomial_bytes = fs::read("conformance/v1/committed-valid.phsm").unwrap();
    let proof_bytes = fs::read("conformance/v1/committed-valid.phcp").unwrap();

    assert_eq!(
        hex::encode(Sha256::digest(&seeded_bytes)),
        manifest["seeded"]["sha256"].as_str().unwrap()
    );
    assert_eq!(
        hex::encode(Sha256::digest(&polynomial_bytes)),
        manifest["committed"]["polynomial_sha256"].as_str().unwrap()
    );
    assert_eq!(
        hex::encode(Sha256::digest(&proof_bytes)),
        manifest["committed"]["proof_sha256"].as_str().unwrap()
    );

    let seeded = SeededSparseProof::from_bytes(&seeded_bytes).unwrap();
    assert!(seeded.verify(&field).is_ok());
    let polynomial = CommittedSparsePolynomial::from_bytes(&polynomial_bytes).unwrap();
    let proof = CommittedSparseProof::from_bytes(&proof_bytes).unwrap();
    assert!(proof.verify(&polynomial, &field).is_ok());
}
