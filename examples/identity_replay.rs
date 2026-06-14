use power_house::{identity::Identity, provenance::PhaArtifact};
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let artifact = PhaArtifact::new(
        json!({"producer": "identity-replay"}),
        "power-house/example/v1",
        json!({"claim": 7}),
        json!({"accepted": true}),
    )?;
    let (identity, graph) = Identity::create("main", artifact)?;
    let state = identity.replay(&graph)?;
    println!("{}", state.graph.state_fingerprint);
    Ok(())
}
