use power_house::{identity::Identity, provenance::PhaArtifact};
use serde_json::json;

fn artifact(stage: &str) -> Result<PhaArtifact, Box<dyn std::error::Error>> {
    Ok(PhaArtifact::new(
        json!({"producer": "identity-equivalent"}),
        "power-house/example/v1",
        json!({"stage": stage}),
        json!({"accepted": true}),
    )?)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (root, mut graph) = Identity::create("main", artifact("main")?)?;
    let shared = artifact("shared")?;
    let left = root.fork(&mut graph, "left", shared.clone())?;
    let right = root.fork(&mut graph, "right", shared)?;
    println!("{}", left.equivalent(&right, &graph)?);
    Ok(())
}
