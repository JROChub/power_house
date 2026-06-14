use power_house::{identity::Identity, provenance::PhaArtifact};
use serde_json::json;

fn artifact(stage: &str) -> Result<PhaArtifact, Box<dyn std::error::Error>> {
    Ok(PhaArtifact::new(
        json!({"stage": stage}),
        "power-house/example/v1",
        json!({"claim": stage}),
        json!({"accepted": true}),
    )?)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (root, mut graph) = Identity::create("main", artifact("main")?)?;
    let branch = root.fork(&mut graph, "candidate", artifact("candidate")?)?;
    branch.verify(&graph)?;
    println!("{}", branch.rootprint_id());
    Ok(())
}
