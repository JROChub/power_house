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
    let path = std::env::temp_dir().join(format!("power-house-rootprint-{suffix}"));
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

#[test]
fn cli_runs_core_rootprint_workflow_and_separate_attachment_command() {
    let dir = temp_dir();
    let artifact_path = dir.join("main.pha");
    let attached_path = dir.join("attached.pha");
    let payload_path = dir.join("external.json");
    let rootprint_path = dir.join("graph.json");

    let artifact = PhaArtifact::new(
        json!({"source": "cli-test"}),
        "power-house/cli-test/v1",
        json!({"claim": 9}),
        json!({"proof": "ok"}),
    )
    .unwrap();
    write_json(&artifact_path, &artifact);
    write_json(&payload_path, &json!({"external": "opaque"}));

    let attach_output = run(&[
        "attach-external-proof",
        artifact_path.to_str().unwrap(),
        "--id",
        "external-1",
        "--proof-system",
        "external/test/v1",
        "--payload",
        payload_path.to_str().unwrap(),
        "--output",
        attached_path.to_str().unwrap(),
    ]);
    assert!(attach_output.contains("core_unchanged: true"));

    run(&[
        "rootprint",
        "init",
        artifact_path.to_str().unwrap(),
        "--label",
        "main",
        "--output",
        rootprint_path.to_str().unwrap(),
    ]);
    run(&[
        "rootprint",
        "fork",
        rootprint_path.to_str().unwrap(),
        "main",
        attached_path.to_str().unwrap(),
        "--label",
        "with-epa",
    ]);
    run(&[
        "rootprint",
        "fork",
        rootprint_path.to_str().unwrap(),
        "main",
        artifact_path.to_str().unwrap(),
        "--label",
        "pure",
    ]);
    assert_eq!(
        run(&[
            "rootprint",
            "equivalent",
            rootprint_path.to_str().unwrap(),
            "with-epa",
            "pure",
        ])
        .trim(),
        "equivalent"
    );
    run(&[
        "rootprint",
        "merge",
        rootprint_path.to_str().unwrap(),
        "with-epa",
        "pure",
        artifact_path.to_str().unwrap(),
        "--label",
        "accepted",
    ]);
    assert!(
        run(&["rootprint", "verify", rootprint_path.to_str().unwrap(),])
            .contains("PASS: Rootprint core verified")
    );
    assert!(run(&[
        "rootprint",
        "navigate",
        rootprint_path.to_str().unwrap(),
        "accepted",
    ])
    .contains("\"label\": \"accepted\""));

    fs::remove_dir_all(dir).unwrap();
}
