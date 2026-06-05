use power_house::{
    transcript_digest, transcript_digest_to_hex, Field, GeneralSumProof, TranscriptDigest,
};
use std::time::Instant;

const SEXTILLION: u128 = 1_000_000_000_000_000_000_000;

fn format_u128(value: u128) -> String {
    let digits = value.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (idx, ch) in digits.chars().enumerate() {
        if idx > 0 && (digits.len() - idx).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

fn proof_digest(proof: &GeneralSumProof) -> TranscriptDigest {
    transcript_digest(&proof.challenges, &proof.round_sums, proof.final_evaluation)
}

fn main() {
    let field = Field::new(1_000_000_007);
    let num_vars = 70usize;
    let constant = 173u64;
    let domain_size = 1u128 << num_vars;

    assert!(
        domain_size > SEXTILLION,
        "2^70 must exceed one sextillion Boolean points"
    );

    let prove_start = Instant::now();
    let proof = GeneralSumProof::prove_constant(num_vars, &field, constant);
    let prove_elapsed = prove_start.elapsed();

    let verify_start = Instant::now();
    let trace = proof
        .verify_constant_with_trace(&field, constant)
        .expect("sextillion constant proof must verify");
    let verify_elapsed = verify_start.elapsed();

    assert_eq!(trace.challenges, proof.challenges);
    assert_eq!(trace.round_sums, proof.round_sums);
    assert_eq!(trace.final_evaluation, proof.final_evaluation);

    println!("Power-House Sextillion Verification Certificate");
    println!("================================================");
    println!("domain_variables: {}", num_vars);
    println!("domain_points: {}", format_u128(domain_size));
    println!("domain_exceeds_sextillion: {}", domain_size > SEXTILLION);
    println!("field_modulus: {}", field.modulus());
    println!("constant_polynomial_value: {}", constant);
    println!("claimed_sum_mod_field: {}", proof.claim.claimed_sum);
    println!("proof_rounds: {}", proof.claim.rounds.len());
    println!("verifier_replayed_rounds: {}", trace.round_sums.len());
    println!("final_evaluation: {}", trace.final_evaluation);
    println!(
        "proof_digest: {}",
        transcript_digest_to_hex(&proof_digest(&proof))
    );
    println!(
        "prove_time_ms: {:.3}",
        prove_elapsed.as_secs_f64() * 1_000.0
    );
    println!(
        "verify_time_ms: {:.3}",
        verify_elapsed.as_secs_f64() * 1_000.0
    );
}
