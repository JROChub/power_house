use power_house::{
    transcript_digest_to_hex, Field, SeededSparseProof, SeededSparseSpec, SparseVerificationReport,
};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

const DEFAULT_NUM_VARS: usize = 1_000_000;
const DEFAULT_NUM_TERMS: usize = 8_192;
const DEFAULT_MAX_DEGREE: usize = 12;
const FIELD_MODULUS: u64 = 1_000_000_007;
const PUBLIC_SEED: &[u8] = b"power-house-public-sparse-record-v1";
const LOG10_2: f64 = std::f64::consts::LOG10_2;

fn main() {
    let num_vars = parse_arg(1, DEFAULT_NUM_VARS);
    let num_terms = parse_arg(2, DEFAULT_NUM_TERMS);
    let max_degree = parse_arg(3, DEFAULT_MAX_DEGREE);
    let output = env::var_os("POWER_HOUSE_SPARSE_CERT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/power_house_sparse_record.phsp"));

    let field = Field::new(FIELD_MODULUS);
    let spec = SeededSparseSpec::new(num_vars, num_terms, max_degree, PUBLIC_SEED.to_vec());
    let (mantissa, decimal_exponent, decimal_digits) = scientific_power_of_two(num_vars);

    let prove_start = Instant::now();
    let proof = SeededSparseProof::prove(spec, &field);
    let prove_time = prove_start.elapsed();

    let encoded = proof.to_bytes();
    write_certificate(&output, &encoded);
    let decoded =
        SeededSparseProof::from_bytes(&fs::read(&output).expect("certificate must be readable"))
            .expect("certificate must decode");

    let verify_start = Instant::now();
    let report = decoded
        .verify(&field)
        .expect("seeded sparse certificate must verify");
    let verify_time = verify_start.elapsed();

    print_report(
        &report,
        num_vars,
        num_terms,
        max_degree,
        mantissa,
        decimal_exponent,
        decimal_digits,
        encoded.len(),
        &output,
        prove_time.as_secs_f64() * 1000.0,
        verify_time.as_secs_f64() * 1000.0,
    );
}

fn parse_arg(index: usize, default: usize) -> usize {
    env::args()
        .nth(index)
        .map(|value| value.parse::<usize>().expect("size argument must be usize"))
        .unwrap_or(default)
}

fn scientific_power_of_two(exponent: usize) -> (f64, usize, usize) {
    let log10_points = exponent as f64 * LOG10_2;
    let decimal_exponent = log10_points.floor() as usize;
    let mantissa = 10f64.powf(log10_points - decimal_exponent as f64);
    (mantissa, decimal_exponent, decimal_exponent + 1)
}

fn write_certificate(path: &Path, encoded: &[u8]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("certificate directory must be creatable");
    }
    fs::write(path, encoded).expect("certificate must be writable");
}

#[allow(clippy::too_many_arguments)]
fn print_report(
    report: &SparseVerificationReport,
    num_vars: usize,
    num_terms: usize,
    max_degree: usize,
    mantissa: f64,
    decimal_exponent: usize,
    decimal_digits: usize,
    certificate_bytes: usize,
    output: &Path,
    prove_time_ms: f64,
    verify_time_ms: f64,
) {
    println!("Power-House Public Sparse Computation Certificate");
    println!("=================================================");
    println!("domain_variables: {num_vars}");
    println!("domain_points_scientific: {mantissa:.6}e{decimal_exponent}");
    println!("domain_decimal_digits: {decimal_digits}");
    println!("sparse_terms: {num_terms}");
    println!("maximum_term_degree: {max_degree}");
    println!("term_incidences: {}", report.term_incidences);
    println!("verifier_replayed_rounds: {}", report.rounds_verified);
    println!("final_evaluation: {}", report.final_evaluation);
    println!(
        "polynomial_digest: {}",
        transcript_digest_to_hex(&report.polynomial_digest)
    );
    println!(
        "transcript_digest: {}",
        transcript_digest_to_hex(&report.transcript_digest)
    );
    println!("certificate_bytes: {certificate_bytes}");
    println!("certificate_path: {}", output.display());
    println!("prove_time_ms: {prove_time_ms:.3}");
    println!("verify_time_ms: {verify_time_ms:.3}");
}
