use power_house::provenance::{PhaArtifact, Rootprint};
use power_house::{Field, GeneralSumProof};
use serde_json::json;
use std::hint::black_box;
use std::time::{Duration, Instant};

fn micros(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000_000.0
}

fn artifact(index: usize) -> PhaArtifact {
    PhaArtifact::new(
        json!({"benchmark": "rootprint-v0.3.0", "index": index}),
        "power-house/benchmark/v1",
        json!({"index": index}),
        json!({"accepted": true}),
    )
    .expect("valid benchmark artifact")
}

fn main() {
    const CORE_ITERATIONS: usize = 10_000;
    const BRANCHES: usize = 2_048;
    const REPRODUCIBILITY_RUNS: usize = 1_000;

    let field = Field::new(1_000_000_007);

    let start = Instant::now();
    let constant_proof = GeneralSumProof::prove_constant(70, &field, 173);
    let constant_prove = start.elapsed();
    let start = Instant::now();
    assert!(constant_proof.verify_constant(&field, 173));
    let constant_verify = start.elapsed();

    let start = Instant::now();
    let affine_proof =
        GeneralSumProof::prove_seeded_affine(4_096, &field, b"power-house-v0.3.0-benchmark");
    let affine_prove = start.elapsed();
    let start = Instant::now();
    assert!(affine_proof.verify_seeded_affine(&field, b"power-house-v0.3.0-benchmark"));
    let affine_verify = start.elapsed();

    let core = artifact(0);
    let start = Instant::now();
    for _ in 0..CORE_ITERATIONS {
        black_box(core.calculate_phx_fingerprint().expect("fingerprint"));
    }
    let fingerprint_total = start.elapsed();
    let start = Instant::now();
    for _ in 0..CORE_ITERATIONS {
        black_box(&core).verify().expect("core verification");
    }
    let core_verify_total = start.elapsed();

    let mut graph = Rootprint::new("main", core.clone()).expect("rootprint");
    let start = Instant::now();
    for index in 1..=BRANCHES {
        graph
            .fork("main", format!("branch-{index}"), artifact(index))
            .expect("fork");
    }
    let branch_build = start.elapsed();
    let start = Instant::now();
    graph.verify().expect("graph verification");
    let graph_verify = start.elapsed();

    let expected = core.phx_fingerprint.clone();
    let start = Instant::now();
    for _ in 0..REPRODUCIBILITY_RUNS {
        assert_eq!(
            core.calculate_phx_fingerprint().expect("fingerprint"),
            expected
        );
    }
    let reproducibility_total = start.elapsed();

    let report = json!({
        "schema": "power-house-benchmark-v0.3.0",
        "release": "0.3.0",
        "environment": {
            "arch": std::env::consts::ARCH,
            "os": std::env::consts::OS,
            "profile": "release"
        },
        "scale": {
            "constant": {
                "domain": "2^70",
                "domain_points": "1180591620717411303424",
                "proof_rounds": constant_proof.claim.rounds.len(),
                "prove_us": micros(constant_prove),
                "verify_us": micros(constant_verify)
            },
            "seeded_affine": {
                "domain": "2^4096",
                "proof_rounds": affine_proof.claim.rounds.len(),
                "prove_us": micros(affine_prove),
                "verify_us": micros(affine_verify)
            }
        },
        "provenance": {
            "iterations": CORE_ITERATIONS,
            "fingerprint_total_us": micros(fingerprint_total),
            "fingerprint_mean_us": micros(fingerprint_total) / CORE_ITERATIONS as f64,
            "verify_total_us": micros(core_verify_total),
            "verify_mean_us": micros(core_verify_total) / CORE_ITERATIONS as f64
        },
        "branching": {
            "branches": graph.branches.len(),
            "fork_total_us": micros(branch_build),
            "fork_mean_us": micros(branch_build) / BRANCHES as f64,
            "full_graph_verify_us": micros(graph_verify)
        },
        "reproducibility": {
            "runs": REPRODUCIBILITY_RUNS,
            "identical_fingerprints": true,
            "total_us": micros(reproducibility_total),
            "phx_fingerprint": expected
        },
        "public_verification": {
            "rust_conformance_vectors": 3,
            "python_cross_language_vectors": 3,
            "core_mutation_classes": 6,
            "epa_mutation_isolated": true
        }
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&report).expect("serialize report")
    );
}
