use power_house::{
    transcript_digest_to_hex, CommittedSparsePolynomial, CommittedSparseProof, Field,
    SeededSparseProof, SeededSparseSpec, SparseMonomial,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let output = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("conformance/v1"));
    fs::create_dir_all(&output).expect("conformance directory must be creatable");

    let field = Field::new(1_000_000_007);
    let seeded = SeededSparseProof::prove(
        SeededSparseSpec::new(32, 20, 6, b"power-house-conformance-v1".to_vec()),
        &field,
    );
    let polynomial = CommittedSparsePolynomial::new(
        16,
        vec![
            SparseMonomial::new(17, vec![0, 3, 9]).unwrap(),
            SparseMonomial::new(29, vec![1, 4]).unwrap(),
            SparseMonomial::new(41, vec![2, 5, 8, 13]).unwrap(),
            SparseMonomial::new(53, vec![6, 7, 10, 11, 15]).unwrap(),
        ],
    )
    .unwrap();
    let committed = CommittedSparseProof::prove(&polynomial, &field).unwrap();

    let seeded_bytes = seeded.to_bytes();
    let polynomial_bytes = polynomial.to_bytes();
    let committed_bytes = committed.to_bytes();
    write(&output.join("seeded-valid.phsp"), &seeded_bytes);
    write(&output.join("committed-valid.phsm"), &polynomial_bytes);
    write(&output.join("committed-valid.phcp"), &committed_bytes);

    let manifest = json!({
        "schema": "power-house-sparse-conformance-v1",
        "field_modulus": field.modulus(),
        "mutation_rule": "xor each byte with 0x01; every single-byte mutation must reject",
        "seeded": {
            "file": "seeded-valid.phsp",
            "sha256": sha256(&seeded_bytes),
            "variables": seeded.spec.num_vars(),
            "terms": seeded.spec.num_terms(),
            "maximum_degree": seeded.spec.max_degree(),
            "claimed_sum": seeded.claimed_sum,
            "final_evaluation": seeded.final_evaluation,
            "polynomial_digest": transcript_digest_to_hex(&seeded.polynomial_digest),
            "transcript_digest": transcript_digest_to_hex(&seeded.transcript_digest),
        },
        "committed": {
            "polynomial_file": "committed-valid.phsm",
            "polynomial_sha256": sha256(&polynomial_bytes),
            "proof_file": "committed-valid.phcp",
            "proof_sha256": sha256(&committed_bytes),
            "variables": polynomial.num_vars(),
            "terms": polynomial.num_terms(),
            "maximum_degree": polynomial.max_degree(),
            "claimed_sum": committed.claimed_sum,
            "final_evaluation": committed.final_evaluation,
            "polynomial_commitment": transcript_digest_to_hex(&committed.polynomial_commitment),
            "transcript_digest": transcript_digest_to_hex(&committed.transcript_digest),
        }
    });
    let mut manifest_bytes = serde_json::to_vec_pretty(&manifest).unwrap();
    manifest_bytes.push(b'\n');
    write(&output.join("manifest.json"), &manifest_bytes);
}

fn sha256(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn write(path: &Path, bytes: &[u8]) {
    fs::write(path, bytes)
        .unwrap_or_else(|error| panic!("failed to write {}: {error}", path.display()));
    println!("wrote {} ({} bytes)", path.display(), bytes.len());
}
