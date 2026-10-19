//! Compare independent sidecar verification with one borrowed verified graph.
use power_house::provenance::{PhaArtifact, Rootprint};
use power_house::ObservatorySidecar;
use serde_json::json;
use std::{collections::BTreeMap, hint::black_box, time::Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let artifact = PhaArtifact::new(
        json!({"benchmark":true}),
        "benchmark/v1",
        json!({"payload":"x".repeat(4096)}),
        json!({"accepted":true}),
    )?;
    let mut graph = Rootprint::new("root", artifact.clone())?;
    let root = graph.root_branch.clone();
    for index in 0..512 {
        graph.fork(&root, format!("branch-{index}"), artifact.clone())?;
    }
    let sidecar = ObservatorySidecar::new(
        &graph,
        BTreeMap::from([(root, json!({"meaning":"benchmark"}))]),
    )?;
    let start = Instant::now();
    for _ in 0..16 {
        black_box(&sidecar).verify(black_box(&graph))?;
    }
    let independent = start.elapsed();
    let start = Instant::now();
    let verified = graph.verified()?;
    for _ in 0..16 {
        black_box(&sidecar).verify_verified(black_box(&verified))?;
    }
    let reused = start.elapsed();
    println!(
        "{}",
        json!({"branches":graph.branches.len(),"sidecars":16,"independent_us":independent.as_micros(),"reused_including_verification_us":reused.as_micros(),"replay_fingerprint":verified.state().state_fingerprint})
    );
    Ok(())
}
