use power_house::provenance::PhaArtifact;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir() -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("power-house-identity-{suffix}"));
    fs::create_dir_all(&path).unwrap();
    path
}

fn write_json(path: &Path, value: &impl serde::Serialize) {
    fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
}

fn run(args: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_julian"))
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "julian {:?} failed:\nstdout={}\nstderr={}",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn artifact(value: u64) -> PhaArtifact {
    PhaArtifact::new(
        json!({"source": "identity-cli"}),
        "power-house/identity-cli/v1",
        json!({"value": value}),
        json!({"accepted": true}),
    )
    .unwrap()
}

#[test]
fn cli_runs_complete_offline_identity_workflow() {
    let dir = temp_dir();
    let root_pha = dir.join("root.pha");
    let shared_pha = dir.join("shared.pha");
    let merged_pha = dir.join("merged.pha");
    let bound_pha = dir.join("root-bound.pha");
    let graph = dir.join("graph.json");
    let root_identity = dir.join("root.identity.json");
    let left_identity = dir.join("left.identity.json");
    let right_identity = dir.join("right.identity.json");
    let merged_identity = dir.join("merged.identity.json");
    let replay = dir.join("replay.json");

    write_json(&root_pha, &artifact(1));
    write_json(&shared_pha, &artifact(2));
    write_json(&merged_pha, &artifact(3));

    run(&[
        "identity",
        "create",
        root_pha.to_str().unwrap(),
        "--label",
        "main",
        "--identity-output",
        root_identity.to_str().unwrap(),
        "--rootprint-output",
        graph.to_str().unwrap(),
        "--artifact-output",
        bound_pha.to_str().unwrap(),
    ]);
    run(&[
        "identity",
        "fork",
        root_identity.to_str().unwrap(),
        graph.to_str().unwrap(),
        shared_pha.to_str().unwrap(),
        "--label",
        "left",
        "--identity-output",
        left_identity.to_str().unwrap(),
    ]);
    run(&[
        "identity",
        "fork",
        root_identity.to_str().unwrap(),
        graph.to_str().unwrap(),
        shared_pha.to_str().unwrap(),
        "--label",
        "right",
        "--identity-output",
        right_identity.to_str().unwrap(),
    ]);
    assert_eq!(
        run(&[
            "identity",
            "equivalent",
            left_identity.to_str().unwrap(),
            right_identity.to_str().unwrap(),
            graph.to_str().unwrap(),
        ])
        .trim(),
        "equivalent"
    );
    run(&[
        "identity",
        "merge",
        left_identity.to_str().unwrap(),
        right_identity.to_str().unwrap(),
        graph.to_str().unwrap(),
        merged_pha.to_str().unwrap(),
        "--label",
        "accepted",
        "--identity-output",
        merged_identity.to_str().unwrap(),
    ]);
    assert!(run(&[
        "identity",
        "verify",
        merged_identity.to_str().unwrap(),
        graph.to_str().unwrap(),
    ])
    .contains("verified offline"));
    run(&[
        "identity",
        "replay",
        merged_identity.to_str().unwrap(),
        graph.to_str().unwrap(),
        "--output",
        replay.to_str().unwrap(),
    ]);

    let replay_value: serde_json::Value =
        serde_json::from_slice(&fs::read(replay).unwrap()).unwrap();
    assert!(replay_value["graph"]["state_fingerprint"]
        .as_str()
        .unwrap()
        .starts_with("sha256:"));
    let bound_value: serde_json::Value =
        serde_json::from_slice(&fs::read(bound_pha).unwrap()).unwrap();
    assert!(bound_value["identity_root"]
        .as_str()
        .unwrap()
        .starts_with("sha256:"));

    fs::remove_dir_all(dir).unwrap();
}
