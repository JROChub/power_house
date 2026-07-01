#![cfg(feature = "sfcs")]

use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

fn temp_dir() -> PathBuf {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("power-house-sfcs-cli-{suffix}"));
    fs::create_dir_all(&path).unwrap();
    path
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

fn read_json(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

#[test]
fn cli_parses_executes_and_verifies_sfcs_source() {
    let dir = temp_dir();
    let source = dir.join("dense.sfcs");
    let graph = dir.join("dense.graph.json");
    let report = dir.join("dense.report.json");
    let artifact = dir.join("dense.execution.pha");

    fs::write(
        &source,
        r#"
        input addr
        input value
        let masked = (value & 15) | 2
        let shifted = masked << 1
        let divided = shifted / 2
        let rem = shifted % 4
        let ok = divided >= rem
        let written = store(addr, divided)
        let loaded = load(addr)
        let out = if ok then loaded ^ rem else value
        output out
        "#,
    )
    .unwrap();

    let source_stdout = run(&[
        "sfcs",
        "source",
        source.to_str().unwrap(),
        "--output",
        graph.to_str().unwrap(),
    ]);
    assert!(source_stdout.contains("SFCS SOURCE"));
    assert!(source_stdout.contains("graph_digest: sha256:"));
    assert!(graph.exists());

    let eval_stdout = run(&[
        "sfcs",
        "eval",
        source.to_str().unwrap(),
        "--input",
        "addr=7",
        "--input",
        "value=29",
        "--report",
        report.to_str().unwrap(),
        "--artifact-output",
        artifact.to_str().unwrap(),
        "--label",
        "dense-cli",
    ]);
    assert!(eval_stdout.contains("SFCS EVAL"));
    assert!(eval_stdout.contains("output out=13"));

    let report_json = read_json(&report);
    assert_eq!(report_json["outputs"]["out"], 13);
    assert!(report_json["trace_steps"].as_u64().unwrap() > 0);
    assert!(report_json["dense_regions"].as_u64().unwrap() > 0);
    assert!(artifact.exists());

    let verify_stdout = run(&["sfcs", "verify-pha", artifact.to_str().unwrap()]);
    assert!(verify_stdout.contains("SFCS EXECUTION PHA VALID"));
    assert!(verify_stdout.contains("trace_digest: sha256:"));
}

#[test]
fn cli_runs_and_verifies_sfcs_vm_program() {
    let dir = temp_dir();
    let program = dir.join("rv32i.program.json");
    let inputs = dir.join("rv32i.inputs.json");
    let report = dir.join("rv32i.report.json");
    let artifact = dir.join("rv32i.execution.pha");

    fs::write(
        &program,
        serde_json::to_vec_pretty(&serde_json::json!({
            "schema": "power-house/sfcs-vm-program/v1-draft",
            "architecture": "rv32i",
            "entry_pc": 0,
            "max_steps": 16,
            "instructions": [
                0x00500093_u32,
                0x00700113_u32,
                0x002081b3_u32,
                0x00302023_u32,
                0x00002203_u32,
                0x00000073_u32
            ]
        }))
        .unwrap(),
    )
    .unwrap();
    fs::write(
        &inputs,
        serde_json::to_vec_pretty(&serde_json::json!({
            "public_registers": [4],
            "public_memory": [{"start": 0, "len": 4}]
        }))
        .unwrap(),
    )
    .unwrap();

    let run_stdout = run(&[
        "sfcs",
        "vm-run",
        program.to_str().unwrap(),
        "--inputs",
        inputs.to_str().unwrap(),
        "--report",
        report.to_str().unwrap(),
        "--artifact-output",
        artifact.to_str().unwrap(),
        "--label",
        "rv32i-cli",
    ]);
    assert!(run_stdout.contains("SFCS VM RUN"));
    assert!(run_stdout.contains("trace_digest: sha256:"));
    assert!(run_stdout.contains("execution_fractal_digest: sha256:"));
    assert!(artifact.exists());

    let report_json = read_json(&report);
    assert_eq!(report_json["steps"], 6);
    assert!(report_json["execution_fractal_digest"]
        .as_str()
        .unwrap()
        .starts_with("sha256:"));
    assert_eq!(report_json["public_outputs"]["registers"]["x4"], 12);
    assert_eq!(report_json["public_outputs"]["memory"]["0"], 12);

    let verify_stdout = run(&["sfcs", "verify-vm-pha", artifact.to_str().unwrap()]);
    assert!(verify_stdout.contains("SFCS VM EXECUTION PHA VALID"));
    assert!(verify_stdout.contains("execution_fractal_digest: sha256:"));
    assert!(verify_stdout.contains("final_state_digest: sha256:"));
}
