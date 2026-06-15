use power_house::{
    provenance::{PhaArtifact, Rootprint},
    ObservatorySidecar,
};
use serde_json::json;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir() -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("power-house-observatory-{suffix}"));
    fs::create_dir_all(&path).unwrap();
    path
}

fn write_json(path: &Path, value: &impl serde::Serialize) {
    fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
}

#[test]
fn cli_verifies_rootprint_before_optional_semantic_sidecar() {
    let dir = temp_dir();
    let graph_path = dir.join("graph.json");
    let sidecar_path = dir.join("observatory.json");
    let artifact = PhaArtifact::new(
        json!({"source": "observatory-cli"}),
        "power-house/observatory-cli/v1",
        json!({"claim": 17}),
        json!({"accepted": true}),
    )
    .unwrap();
    let graph = Rootprint::new("main", artifact).unwrap();
    let sidecar = ObservatorySidecar::new(
        &graph,
        BTreeMap::from([(
            graph.root_branch.clone(),
            json!({
                "schema": "slbit/viz-packet/v1",
                "claim_id": "observatory-cli-claim"
            }),
        )]),
    )
    .unwrap();
    write_json(&graph_path, &graph);
    write_json(&sidecar_path, &sidecar);

    let output = Command::new(env!("CARGO_BIN_EXE_julian"))
        .args([
            "observatory",
            "verify",
            graph_path.to_str().unwrap(),
            sidecar_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .contains("PASS: Rootprint core and Observatory sidecar verified"));

    fs::remove_dir_all(dir).unwrap();
}
