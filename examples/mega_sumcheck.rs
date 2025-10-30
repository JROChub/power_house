use power_house::{
    alien::{Proof, ProofKind, ProofLedger, Statement},
    ChainedSumProof, Field, GeneralSumProof, MultilinearPolynomial, ProofStats,
};
use std::fs;

fn build_high_dimensional(field: &Field, num_vars: usize) -> MultilinearPolynomial {
    let size = 1usize << num_vars;
    let mut evals = Vec::with_capacity(size);
    for idx in 0..size {
        let mut acc = 0u64;
        for bit in 0..num_vars {
            let bit_value = ((idx >> bit) & 1) as u64;
            let coef = (3 * (bit as u64 + 1)) % field.modulus();
            acc = field.add(acc, field.mul(coef, bit_value));
        }
        for bit in 0..num_vars.saturating_sub(1) {
            let a = ((idx >> bit) & 1) as u64;
            let b = ((idx >> (bit + 1)) & 1) as u64;
            let coef = (5 * (bit as u64 + 2)) % field.modulus();
            acc = field.add(acc, field.mul(coef, field.mul(a, b)));
        }
        if num_vars >= 3 {
            let a = ((idx >> 0) & 1) as u64;
            let b = ((idx >> 1) & 1) as u64;
            let c = ((idx >> 2) & 1) as u64;
            let triple = field.mul(a, field.mul(b, c));
            acc = field.add(acc, field.mul(17, triple));
        }
        evals.push(acc % field.modulus());
    }
    MultilinearPolynomial::from_evaluations(num_vars, evals)
}

fn constant_polynomial(field: &Field, target_sum: u64, num_vars: usize) -> MultilinearPolynomial {
    let points = 1usize << num_vars;
    let inv_points = field.inv(points as u64 % field.modulus());
    let constant = field.mul(target_sum % field.modulus(), inv_points);
    MultilinearPolynomial::from_evaluations(num_vars, vec![constant; points])
}

fn ms(duration: &std::time::Duration) -> f64 {
    duration.as_secs_f64() * 1_000.0
}

fn main() {
    let field = Field::new(257);
    let mega_poly = build_high_dimensional(&field, 10);
    let (mega_proof, stats): (GeneralSumProof, ProofStats) =
        GeneralSumProof::prove_with_stats(&mega_poly, &field);

    println!(
        "Mega sum-check: variables={}, sum={}, final={}, total={:.3} ms",
        mega_proof.claim.num_vars,
        mega_proof.claim.claimed_sum,
        mega_proof.final_evaluation,
        ms(&stats.total_duration)
    );
    for (round, duration) in stats.round_durations.iter().enumerate() {
        println!("  round {:02}: {:.3} ms", round, ms(duration));
    }

    let poly_chain_1 = mega_poly.clone();
    let poly_chain_2 = constant_polynomial(&field, mega_proof.final_evaluation, 6);
    let proof_chain_2 = GeneralSumProof::prove(&poly_chain_2, &field);
    let poly_chain_3 = constant_polynomial(&field, proof_chain_2.final_evaluation, 5);
    let chain_polynomials = vec![
        poly_chain_1.clone(),
        poly_chain_2.clone(),
        poly_chain_3.clone(),
    ];
    let (chain_proof, chain_stats) = ChainedSumProof::prove_with_stats(&chain_polynomials, &field);
    println!("Chained proof length: {}", chain_proof.len());
    println!(
        "Chained proof verifies: {}",
        chain_proof.verify(&chain_polynomials, &field)
    );
    for (idx, stats) in chain_stats.iter().enumerate() {
        println!(
            "  chained proof {:02}: vars={}, total={:.3} ms",
            idx,
            chain_polynomials[idx].num_vars(),
            ms(&stats.total_duration)
        );
    }

    let mut ledger = ProofLedger::new();
    let log_dir = std::env::temp_dir().join("power_house_ledger_logs");
    if log_dir.exists() {
        fs::remove_dir_all(&log_dir).unwrap();
    }
    ledger.enable_logging(&log_dir);

    let general_entry = Proof {
        kind: ProofKind::General {
            polynomial: mega_poly.clone(),
            proof: mega_proof.clone(),
        },
        data: Vec::new(),
    };
    ledger.submit(
        Statement {
            description: "10-variable mega polynomial".into(),
        },
        general_entry,
    );

    let chain_entry = Proof {
        kind: ProofKind::Chain {
            polynomials: chain_polynomials.clone(),
            proof: chain_proof.clone(),
        },
        data: Vec::new(),
    };
    ledger.submit(
        Statement {
            description: "Chained mega transcript".into(),
        },
        chain_entry,
    );

    for (idx, entry) in ledger.entries().iter().enumerate() {
        println!(
            "Ledger entry {} => accepted={}, transcripts={}, final_values={:?}",
            idx,
            entry.accepted,
            entry.transcripts.len(),
            entry.final_values
        );
        if !entry.log_paths.is_empty() {
            println!("  logs written to:");
            for path in &entry.log_paths {
                println!("    {}", path.display());
            }
        }
        if let Some(err) = &entry.log_error {
            println!("  log error: {}", err);
        }
    }
}
