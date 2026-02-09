use blake2::digest::{consts::U32, Digest};
type Blake2b256 = blake2::Blake2b<U32>;
use power_house::{
    julian::{reconcile_anchors, Proof, ProofKind, ProofLedger, Statement},
    Field, GeneralSumProof, StreamingPolynomial, TranscriptDigest,
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

fn aggregate_hashes(hashes: &[TranscriptDigest]) -> TranscriptDigest {
    let mut hasher = Blake2b256::new();
    hasher.update(b"JROC_ANCHOR");
    for digest in hashes {
        hasher.update(digest);
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&hasher.finalize());
    out
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

    assert!(
        ledger_a.entries().len() >= 2,
        "ledger is missing the dense proof entry"
    );
    let base_hashes = ledger_a.entries()[1].hashes.clone();
    let aggregated_digest = aggregate_hashes(&base_hashes);
    let mut head = [0u8; 8];
    head.copy_from_slice(&aggregated_digest[..8]);
    let aggregated_hash = u64::from_be_bytes(head) % field.modulus();
    println!(
        "Aggregated transcript digest (BLAKE2b-256, records={}): {} (field element {})",
        base_hashes.len(),
        power_house::transcript_digest_to_hex(&aggregated_digest),
        aggregated_hash
    );
    let fold_hex = power_house::transcript_digest_to_hex(&aggregated_digest);
    fs::write(dir_a.join("fold_digest.txt"), format!("{fold_hex}\n"))
        .expect("write fold digest for ledger A");
    fs::write(dir_b.join("fold_digest.txt"), format!("{fold_hex}\n"))
        .expect("write fold digest for ledger B");

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
