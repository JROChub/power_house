use power_house::{identity::Identity, provenance::PhaArtifact};
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let artifact = PhaArtifact::new(
        json!({"producer": "identity-create"}),
        "power-house/example/v1",
        json!({"claim": 1}),
        json!({"accepted": true}),
    )?;
    let (identity, graph) = Identity::create("main", artifact)?;
    identity.verify(&graph)?;
    println!("{}", identity.rootprint_id());
    Ok(())
}
