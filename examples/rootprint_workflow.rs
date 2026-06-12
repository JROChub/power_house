use power_house::{prove_with_rootprint, provenance::PhaArtifact};
use serde_json::json;

fn artifact(stage: &str, accepted: bool) -> PhaArtifact {
    PhaArtifact::new(
        json!({"producer": "rootprint-workflow", "stage": stage}),
        "power-house/example/v1",
        json!({"stage": stage}),
        json!({"accepted": accepted}),
    )
    .expect("valid artifact")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut graph = prove_with_rootprint!(
        label: "main",
        artifact: artifact("baseline", true),
    )?;
    let candidate = prove_with_rootprint!(
        rootprint: &mut graph,
        fork: "main",
        label: "candidate",
        artifact: artifact("candidate", true),
    )?;
    let audit = prove_with_rootprint!(
        rootprint: &mut graph,
        fork: "main",
        label: "audit",
        artifact: artifact("audit", true),
    )?;
    prove_with_rootprint!(
        rootprint: &mut graph,
        merge: [&candidate, &audit],
        label: "accepted",
        artifact: artifact("accepted", true),
    )?;
    graph.verify()?;
    println!("{}", serde_json::to_string_pretty(&graph)?);
    Ok(())
}
