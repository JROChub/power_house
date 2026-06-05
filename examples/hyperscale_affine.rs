use power_house::{
    transcript_digest, transcript_digest_to_hex, Field, GeneralSumProof, TranscriptDigest,
};
use std::env;
use std::time::Instant;

const DEFAULT_NUM_VARS: usize = 4096;
const FIELD_MODULUS: u64 = 1_000_000_007;
const SEED: &[u8] = b"power-house-hyperscale-seeded-affine-v1";
const LOG10_2: f64 = std::f64::consts::LOG10_2;
const SEXTILLION_LOG10: f64 = 21.0;

fn proof_digest(proof: &GeneralSumProof) -> TranscriptDigest {
    transcript_digest(&proof.challenges, &proof.round_sums, proof.final_evaluation)
}

fn scientific_power_of_two(exponent: usize) -> (f64, i32, usize) {
    let log10_points = exponent as f64 * LOG10_2;
    let decimal_exponent = log10_points.floor() as i32;
    let mantissa = 10f64.powf(log10_points - decimal_exponent as f64);
    let digits = decimal_exponent as usize + 1;
    (mantissa, decimal_exponent, digits)
}

fn parse_num_vars() -> usize {
    env::args()
        .nth(1)
        .map(|value| value.parse::<usize>().expect("num_vars must be usize"))
        .unwrap_or(DEFAULT_NUM_VARS)
}

fn main() {
    let num_vars = parse_num_vars();
    assert!(num_vars >= 70, "num_vars must be at least sextillion scale");

    let field = Field::new(FIELD_MODULUS);
    let (mantissa, decimal_exponent, decimal_digits) = scientific_power_of_two(num_vars);
    let sextillion_gap_log10 = num_vars as f64 * LOG10_2 - SEXTILLION_LOG10;

    let prove_start = Instant::now();
    let proof = GeneralSumProof::prove_seeded_affine(num_vars, &field, SEED);
    let prove_time = prove_start.elapsed();

    let verify_start = Instant::now();
    let trace = proof
        .verify_seeded_affine_with_trace(&field, SEED)
        .expect("seeded affine hyperscale proof must verify");
    let verify_time = verify_start.elapsed();

    println!("Power-House Hyperscale Seeded-Affine Certificate");
    println!("=================================================");
    println!("domain_variables: {num_vars}");
    println!("domain_points_scientific: {mantissa:.6}e{decimal_exponent}");
    println!("domain_decimal_digits: {decimal_digits}");
    println!("domain_log10_over_sextillion: {sextillion_gap_log10:.3}");
    println!("field_modulus: {}", field.modulus());
    println!("public_seed_hex: {}", hex_seed(SEED));
    println!("polynomial_model: seeded affine multilinear");
    println!("claimed_sum_mod_field: {}", proof.claim.claimed_sum);
    println!("proof_rounds: {}", proof.claim.rounds.len());
    println!("verifier_replayed_rounds: {}", trace.round_sums.len());
    println!("nonzero_linear_rounds: {}", nonzero_rounds(&proof));
    println!("final_evaluation: {}", trace.final_evaluation);
    println!(
        "proof_digest: {}",
        transcript_digest_to_hex(&proof_digest(&proof))
    );
    println!("prove_time_ms: {:.3}", prove_time.as_secs_f64() * 1000.0);
    println!("verify_time_ms: {:.3}", verify_time.as_secs_f64() * 1000.0);
}

fn nonzero_rounds(proof: &GeneralSumProof) -> usize {
    proof.claim.rounds.iter().filter(|&&(a, _)| a != 0).count()
}

fn hex_seed(seed: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(seed.len() * 2);
    for &byte in seed {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}
