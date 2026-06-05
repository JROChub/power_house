use power_house::{
    transcript_digest_to_hex, CommittedSparsePolynomial, CommittedSparseProof, Field, SimplePrng,
    SparseMonomial,
};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

const FIELD_MODULUS: u64 = 1_000_000_007;
const DEFAULT_NUM_VARS: usize = 1_000_000;
const DEFAULT_NUM_TERMS: usize = 8_192;
const DEFAULT_MAX_DEGREE: usize = 12;
const WORKLOAD_SEED: u64 = 0x504f_5745_5248_4f55;

fn main() {
    let command = env::args().nth(1).unwrap_or_else(|| "all".to_string());
    let polynomial_path = env::args()
        .nth(2)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/external_interaction_model.phsm"));
    let proof_path = env::args()
        .nth(3)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/external_interaction_model.phcp"));

    match command.as_str() {
        "generate" => generate(&polynomial_path),
        "prove" => prove(&polynomial_path, &proof_path),
        "verify" => verify(&polynomial_path, &proof_path),
        "all" => {
            generate(&polynomial_path);
            prove(&polynomial_path, &proof_path);
            verify(&polynomial_path, &proof_path);
        }
        _ => panic!("command must be one of: all, generate, prove, verify"),
    }
}

fn generate(path: &Path) {
    let num_vars = env_usize("POWER_HOUSE_COMMITTED_VARS", DEFAULT_NUM_VARS);
    let num_terms = env_usize("POWER_HOUSE_COMMITTED_TERMS", DEFAULT_NUM_TERMS);
    let max_degree = env_usize("POWER_HOUSE_COMMITTED_DEGREE", DEFAULT_MAX_DEGREE);
    assert!(max_degree <= num_vars, "degree cannot exceed variables");
    let mut prng = SimplePrng::new(WORKLOAD_SEED);
    let mut terms = Vec::with_capacity(num_terms);
    for _ in 0..num_terms {
        let degree = if max_degree == 1 {
            1
        } else {
            2 + prng.gen_mod((max_degree - 1) as u64) as usize
        };
        let coefficient = 1 + prng.gen_mod(FIELD_MODULUS - 1);
        let mut variables = Vec::with_capacity(degree);
        while variables.len() < degree {
            let candidate = prng.gen_mod(num_vars as u64) as usize;
            if !variables.contains(&candidate) {
                variables.push(candidate);
            }
        }
        terms.push(SparseMonomial::new(coefficient, variables).expect("valid monomial"));
    }
    let polynomial = CommittedSparsePolynomial::new(num_vars, terms).expect("valid polynomial");
    write_file(path, &polynomial.to_bytes());
    println!("generated_workload: {}", path.display());
    println!("workload_variables: {}", polynomial.num_vars());
    println!("workload_terms: {}", polynomial.num_terms());
    println!("workload_max_degree: {}", polynomial.max_degree());
    println!(
        "workload_commitment: {}",
        transcript_digest_to_hex(&polynomial.commitment())
    );
}

fn env_usize(name: &str, default: usize) -> usize {
    env::var(name)
        .ok()
        .map(|value| {
            value
                .parse::<usize>()
                .expect("size environment must be usize")
        })
        .unwrap_or(default)
}

fn prove(polynomial_path: &Path, proof_path: &Path) {
    let field = Field::new(FIELD_MODULUS);
    let polynomial = load_polynomial(polynomial_path);
    let start = Instant::now();
    let proof = CommittedSparseProof::prove(&polynomial, &field).expect("proof must succeed");
    let elapsed = start.elapsed();
    write_file(proof_path, &proof.to_bytes());
    println!("proof_path: {}", proof_path.display());
    println!("proof_rounds: {}", proof.rounds.len());
    println!(
        "proof_commitment: {}",
        transcript_digest_to_hex(&proof.polynomial_commitment)
    );
    println!(
        "proof_transcript_digest: {}",
        transcript_digest_to_hex(&proof.transcript_digest)
    );
    println!("prove_time_ms: {:.3}", elapsed.as_secs_f64() * 1000.0);
}

fn verify(polynomial_path: &Path, proof_path: &Path) {
    let field = Field::new(FIELD_MODULUS);
    let polynomial = load_polynomial(polynomial_path);
    let proof = CommittedSparseProof::from_bytes(
        &fs::read(proof_path).expect("committed proof file must be readable"),
    )
    .expect("committed proof must decode");
    let start = Instant::now();
    let report = proof
        .verify(&polynomial, &field)
        .expect("external committed workload must verify");
    let elapsed = start.elapsed();
    println!("verification_status: verified");
    println!("verified_rounds: {}", report.rounds_verified);
    println!("term_incidences: {}", report.term_incidences);
    println!("final_evaluation: {}", report.final_evaluation);
    println!(
        "verified_commitment: {}",
        transcript_digest_to_hex(&report.polynomial_digest)
    );
    println!(
        "verified_transcript_digest: {}",
        transcript_digest_to_hex(&report.transcript_digest)
    );
    println!("verify_time_ms: {:.3}", elapsed.as_secs_f64() * 1000.0);
}

fn load_polynomial(path: &Path) -> CommittedSparsePolynomial {
    CommittedSparsePolynomial::from_bytes(
        &fs::read(path).expect("external polynomial file must be readable"),
    )
    .expect("external polynomial must decode")
}

fn write_file(path: &Path, bytes: &[u8]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("output directory must be creatable");
    }
    fs::write(path, bytes).expect("output file must be writable");
}
