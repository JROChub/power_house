use power_house::{identity::Identity, provenance::PhaArtifact};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

fn write_json(path: &Path, value: &impl Serialize) -> Vec<u8> {
    let mut bytes = serde_json::to_vec_pretty(value).expect("serialize vector");
    bytes.push(b'\n');
    fs::write(path, &bytes).expect("write vector");
    bytes
}

fn sha256(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn artifact(stage: &str) -> PhaArtifact {
    PhaArtifact::new(
        json!({"producer": "power-house-identity-conformance", "stage": stage}),
        "power-house/identity-conformance/v1",
        json!({"claim": 36, "stage": stage}),
        json!({"accepted": true}),
    )
    .expect("valid artifact")
}

fn main() {
    let directory = Path::new("conformance/identity-v1");
    fs::create_dir_all(directory).expect("create conformance directory");

    let (root, mut graph) = Identity::create("main", artifact("main")).expect("identity root");
    let shared = artifact("candidate");
    let left = root
        .fork(&mut graph, "left", shared.clone())
        .expect("left identity");
    let right = root
        .fork(&mut graph, "right", shared)
        .expect("right identity");
    assert!(left.equivalent(&right, &graph).expect("equivalence"));
    let merged = Identity::merge(&left, &right, &mut graph, "accepted", artifact("accepted"))
        .expect("merged identity");
    let replay = merged.replay(&graph).expect("identity replay");

    let identity_bytes = write_json(&directory.join("identity-valid.json"), &merged);
    let rootprint_bytes = write_json(&directory.join("rootprint-valid.json"), &graph);
    let replay_bytes = write_json(&directory.join("replay-valid.json"), &replay);

    let mut files = BTreeMap::new();
    files.insert("identity-valid.json", sha256(&identity_bytes));
    files.insert("replay-valid.json", sha256(&replay_bytes));
    files.insert("rootprint-valid.json", sha256(&rootprint_bytes));
    let manifest: Value = json!({
        "schema": "power-house-identity-conformance-v1",
        "identity_root": merged.rootprint_id(),
        "state_fingerprint": replay.graph.state_fingerprint,
        "files": files,
        "requirements": {
            "fingerprint_deterministic": true,
            "graph_dag": true,
            "network_required": false,
            "replay_deterministic": true
        }
    });
    write_json(&directory.join("manifest.json"), &manifest);
}
