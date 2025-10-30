use power_house::{
    alien::{reconcile_anchors, Proof, ProofKind, ProofLedger, Statement},
    Field, GeneralSumProof, StreamingPolynomial,
};
use std::fs;

fn build_dense_polynomial(field: &Field, num_vars: usize) -> StreamingPolynomial {
    let modulus = field.modulus();
    StreamingPolynomial::new(num_vars, modulus, move |idx| {
        let mut acc = (idx as u64) % modulus;
        for bit in 0..num_vars {
            let bit_value = ((idx >> bit) & 1) as u64;
            if bit_value == 0 {
                continue;
            }
            let coef = ((bit as u64 + 5).pow(2)) % modulus;
            acc = (acc + coef) % modulus;
        }
        if num_vars >= 3 {
            let a = ((idx >> 0) & 1) as u64;
            let b = ((idx >> 1) & 1) as u64;
            let c = ((idx >> 2) & 1) as u64;
            if a == 1 && b == 1 && c == 1 {
                acc = (acc + 31) % modulus;
            }
        }
        acc % modulus
    })
}

fn constant_polynomial(field: &Field, num_vars: usize, target_sum: u64) -> StreamingPolynomial {
    let modulus = field.modulus();
    let points = 1usize << num_vars;
    let inv_points = field.inv(points as u64 % modulus);
    let constant = field.mul(target_sum % modulus, inv_points);
    StreamingPolynomial::new(num_vars, modulus, move |_| constant)
}

fn aggregate_hashes(hashes: &[u64], modulus: u64, mode: &str) -> u64 {
    match mode {
        "sum" => hashes.iter().fold(0u64, |acc, h| (acc + h) % modulus),
        _ => hashes.iter().fold(0u64, |acc, h| acc ^ h),
    }
}

fn prepare_dir(path: &std::path::Path) {
    if path.exists() {
        let _ = fs::remove_dir_all(path);
    }
    fs::create_dir_all(path).expect("create log directory");
}

fn main() {
    let field = Field::new(257);
    let base_poly = build_dense_polynomial(&field, 10);
    let (base_proof, base_stats) =
        GeneralSumProof::prove_streaming_with_stats_poly(&base_poly, &field);

    println!(
        "Base proof: vars={}, claimed_sum={}, final_eval={}, total_time={:.3} ms",
        base_proof.claim.num_vars,
        base_proof.claim.claimed_sum,
        base_proof.final_evaluation,
        base_stats.total_duration.as_secs_f64() * 1_000.0
    );

    let dir_a = std::env::temp_dir().join("power_house_anchor_a");
    let dir_b = std::env::temp_dir().join("power_house_anchor_b");
    prepare_dir(&dir_a);
    prepare_dir(&dir_b);

    let mut ledger_a = ProofLedger::new();
    ledger_a.enable_logging(&dir_a);
    let base_statement = Statement {
        description: "Dense polynomial proof".into(),
    };
    ledger_a.submit(
        base_statement.clone(),
        Proof {
            kind: ProofKind::StreamingGeneral {
                polynomial: base_poly.clone(),
                proof: base_proof.clone(),
            },
            data: Vec::new(),
        },
    );

    let base_hashes = ledger_a.entries()[0].hashes.clone();
    let hash_mode = std::env::var("POWER_HOUSE_HASH_MODE").unwrap_or_else(|_| "xor".into());
    let aggregated_hash = aggregate_hashes(&base_hashes, field.modulus(), &hash_mode);
    println!(
        "Aggregated transcript hash (mode={}, records={}): {}",
        hash_mode,
        base_hashes.len(),
        aggregated_hash
    );

    let anchor_poly = constant_polynomial(&field, 6, aggregated_hash);
    let (anchor_proof, anchor_stats) =
        GeneralSumProof::prove_streaming_with_stats_poly(&anchor_poly, &field);
    println!(
        "Anchor proof: vars={}, claimed_sum={}, final_eval={}, total_time={:.3} ms",
        anchor_proof.claim.num_vars,
        anchor_proof.claim.claimed_sum,
        anchor_proof.final_evaluation,
        anchor_stats.total_duration.as_secs_f64() * 1_000.0
    );

    ledger_a.submit(
        Statement {
            description: "Hash anchor proof".into(),
        },
        Proof {
            kind: ProofKind::StreamingGeneral {
                polynomial: anchor_poly.clone(),
                proof: anchor_proof.clone(),
            },
            data: Vec::new(),
        },
    );

    let mut ledger_b = ProofLedger::new();
    ledger_b.enable_logging(&dir_b);
    ledger_b.submit(
        base_statement,
        Proof {
            kind: ProofKind::StreamingGeneral {
                polynomial: base_poly,
                proof: base_proof,
            },
            data: Vec::new(),
        },
    );
    ledger_b.submit(
        Statement {
            description: "Hash anchor proof".into(),
        },
        Proof {
            kind: ProofKind::StreamingGeneral {
                polynomial: anchor_poly,
                proof: anchor_proof,
            },
            data: Vec::new(),
        },
    );

    let anchor_a = ledger_a.anchor();
    let anchor_b = ledger_b.anchor();
    match reconcile_anchors(&[anchor_a.clone(), anchor_b.clone()]) {
        Ok(()) => println!("Ledger anchors reconciled successfully."),
        Err(err) => println!("Ledger anchor mismatch: {}", err),
    }

    println!("Ledger A logs: {}", dir_a.display());
    println!("Ledger B logs: {}", dir_b.display());
}
