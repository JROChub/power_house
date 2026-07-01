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
fn cli_compiles_public_rust_subset_to_sfcs_graph() {
    let dir = temp_dir();
    let source = dir.join("score.rs");
    let graph = dir.join("score.graph.json");
    let semantic = dir.join("score.semantic.json");
    let report = dir.join("score.report.json");
    let artifact = dir.join("score.pha");

    fs::write(
        &source,
        "pub fn score(a: u32, b: u32, c: u32) -> u32 { if a > b { (a - b) * c } else { (b - a) * c } }\n",
    )
    .unwrap();

    let compile_stdout = run(&[
        "sfcs",
        "rust-public",
        source.to_str().unwrap(),
        "--graph-output",
        graph.to_str().unwrap(),
        "--semantic-output",
        semantic.to_str().unwrap(),
        "--artifact-output",
        artifact.to_str().unwrap(),
        "--report",
        report.to_str().unwrap(),
        "--label",
        "score-rust-public",
    ]);
    assert!(compile_stdout.contains("SFCS RUST PUBLIC"));
    assert!(compile_stdout.contains("graph_digest: sha256:"));
    assert!(compile_stdout.contains("semantic_packet_digest: sha256:"));
    assert!(graph.exists());
    assert!(semantic.exists());
    assert!(artifact.exists());

    let report_json = read_json(&report);
    assert_eq!(report_json["function_name"], "score");
    assert_eq!(
        report_json["parameters"],
        serde_json::json!(["a", "b", "c"])
    );
    assert!(report_json["graph_digest"]
        .as_str()
        .unwrap()
        .starts_with("sha256:"));

    let verify_stdout = run(&["sfcs", "verify-pha", artifact.to_str().unwrap()]);
    assert!(verify_stdout.contains("SFCS GRAPH PHA VALID"));
}

#[test]
fn cli_compiles_llvm_ir_subset_to_sfcs_graph() {
    let dir = temp_dir();
    let source = dir.join("score.ll");
    let graph = dir.join("score-llvm.graph.json");
    let semantic = dir.join("score-llvm.semantic.json");
    let report = dir.join("score-llvm.report.json");
    let artifact = dir.join("score-llvm.pha");

    fs::write(
        &source,
        r#"
        define i32 @score(i32 %a, i32 %b) {
        entry:
          %sum = add i32 %a, %b
          %out = mul i32 %sum, 2
          ret i32 %out
        }
        "#,
    )
    .unwrap();

    let compile_stdout = run(&[
        "sfcs",
        "llvm-ir",
        source.to_str().unwrap(),
        "--graph-output",
        graph.to_str().unwrap(),
        "--semantic-output",
        semantic.to_str().unwrap(),
        "--artifact-output",
        artifact.to_str().unwrap(),
        "--report",
        report.to_str().unwrap(),
        "--label",
        "score-llvm-ir",
    ]);
    assert!(compile_stdout.contains("SFCS LLVM IR"));
    assert!(compile_stdout.contains("graph_digest: sha256:"));
    assert!(compile_stdout.contains("semantic_packet_digest: sha256:"));
    assert!(graph.exists());
    assert!(semantic.exists());
    assert!(artifact.exists());

    let report_json = read_json(&report);
    assert_eq!(report_json["function_name"], "score");
    assert_eq!(report_json["parameters"], serde_json::json!(["a", "b"]));
    assert!(report_json["graph_digest"]
        .as_str()
        .unwrap()
        .starts_with("sha256:"));

    let verify_stdout = run(&["sfcs", "verify-pha", artifact.to_str().unwrap()]);
    assert!(verify_stdout.contains("SFCS GRAPH PHA VALID"));
}

#[test]
fn cli_compiles_wasm_stack_subset_to_sfcs_graph() {
    let dir = temp_dir();
    let source = dir.join("score.wasmstack");
    let graph = dir.join("score-wasm.graph.json");
    let semantic = dir.join("score-wasm.semantic.json");
    let report = dir.join("score-wasm.report.json");
    let artifact = dir.join("score-wasm.pha");

    fs::write(
        &source,
        r#"
        param a i32
        param b i32
        local.get a
        local.get b
        i32.add
        i32.const 2
        i32.mul
        return
        "#,
    )
    .unwrap();

    let compile_stdout = run(&[
        "sfcs",
        "wasm-stack",
        source.to_str().unwrap(),
        "--graph-output",
        graph.to_str().unwrap(),
        "--semantic-output",
        semantic.to_str().unwrap(),
        "--artifact-output",
        artifact.to_str().unwrap(),
        "--report",
        report.to_str().unwrap(),
        "--label",
        "score-wasm-stack",
    ]);
    assert!(compile_stdout.contains("SFCS WASM STACK"));
    assert!(compile_stdout.contains("graph_digest: sha256:"));
    assert!(compile_stdout.contains("semantic_packet_digest: sha256:"));
    assert!(graph.exists());
    assert!(semantic.exists());
    assert!(artifact.exists());

    let report_json = read_json(&report);
    assert_eq!(report_json["parameters"], serde_json::json!(["a", "b"]));
    assert!(report_json["graph_digest"]
        .as_str()
        .unwrap()
        .starts_with("sha256:"));

    let verify_stdout = run(&["sfcs", "verify-pha", artifact.to_str().unwrap()]);
    assert!(verify_stdout.contains("SFCS GRAPH PHA VALID"));
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

#[test]
fn cli_proves_and_verifies_sfcs_vm_constraints() {
    let dir = temp_dir();
    let program = dir.join("rv32i.constraints.program.json");
    let inputs = dir.join("rv32i.constraints.inputs.json");
    let report = dir.join("rv32i.constraints.report.json");
    let artifact = dir.join("rv32i.constraints.pha");

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

    let prove_stdout = run(&[
        "sfcs",
        "vm-constraints",
        program.to_str().unwrap(),
        "--inputs",
        inputs.to_str().unwrap(),
        "--report",
        report.to_str().unwrap(),
        "--artifact-output",
        artifact.to_str().unwrap(),
        "--label",
        "rv32i-constraints-cli",
    ]);
    assert!(prove_stdout.contains("SFCS VM CONSTRAINTS"));
    assert!(prove_stdout.contains("proof_digest: sha256:"));
    assert!(prove_stdout.contains("transition_checks: 6"));
    assert!(prove_stdout.contains("memory_consistency_checks: 2"));
    assert!(artifact.exists());

    let report_json = read_json(&report);
    assert_eq!(report_json["steps"], 6);
    assert_eq!(report_json["transition_checks"], 6);
    assert_eq!(report_json["memory_consistency_checks"], 2);
    assert!(report_json["proof_digest"]
        .as_str()
        .unwrap()
        .starts_with("sha256:"));

    let verify_stdout = run(&[
        "sfcs",
        "verify-vm-constraints-pha",
        artifact.to_str().unwrap(),
    ]);
    assert!(verify_stdout.contains("SFCS VM CONSTRAINT PHA VALID"));
    assert!(verify_stdout.contains("transition_checks: 6"));
    assert!(verify_stdout.contains("memory_consistency_checks: 2"));
}

#[cfg(feature = "sfcs-zk")]
#[test]
fn cli_proves_and_verifies_sfcs_zk_private_add() {
    let dir = temp_dir();
    let program = dir.join("private-add.program.json");
    let report = dir.join("private-add.report.json");
    let artifact = dir.join("private-add.pha");

    fs::write(
        &program,
        serde_json::to_vec_pretty(&serde_json::json!({
            "schema": "power-house/sfcs-vm-program/v1-draft",
            "architecture": "rv32i",
            "entry_pc": 0,
            "max_steps": 8,
            "instructions": [
                0x00b501b3_u32,
                0x00000073_u32
            ]
        }))
        .unwrap(),
    )
    .unwrap();

    let prove_stdout = run(&[
        "sfcs",
        "zk-private-add",
        program.to_str().unwrap(),
        "--lhs-register",
        "10",
        "--rhs-register",
        "11",
        "--output-register",
        "3",
        "--lhs-value",
        "5",
        "--rhs-value",
        "7",
        "--lhs-blinding",
        "0707070707070707070707070707070707070707070707070707070707070707",
        "--rhs-blinding",
        "0909090909090909090909090909090909090909090909090909090909090909",
        "--report",
        report.to_str().unwrap(),
        "--artifact-output",
        artifact.to_str().unwrap(),
    ]);
    assert!(prove_stdout.contains("SFCS ZK PRIVATE ADD"));
    assert!(prove_stdout.contains("proof_digest: sha256:"));
    assert!(prove_stdout.contains("output x3=12"));

    let report_json = read_json(&report);
    assert_eq!(report_json["output_value"], 12);
    assert!(report_json["lhs_commitment"]
        .as_str()
        .unwrap()
        .starts_with("edwards:"));
    assert!(artifact.exists());

    let verify_stdout = run(&["sfcs", "verify-zk-pha", artifact.to_str().unwrap()]);
    assert!(verify_stdout.contains("SFCS ZK PRIVATE ADD PHA VALID"));
    assert!(verify_stdout.contains("public_output: x3=12"));
}

#[cfg(feature = "sfcs-zk")]
#[test]
fn cli_proves_and_verifies_sfcs_zk_private_vm() {
    let dir = temp_dir();
    let program = dir.join("private-vm.program.json");
    let witness = dir.join("private-vm.witness.json");
    let report = dir.join("private-vm.report.json");
    let artifact = dir.join("private-vm.pha");

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
        &witness,
        serde_json::to_vec_pretty(&serde_json::json!({
            "inputs": {
                "registers": {
                    "10": 777777777_u32,
                    "11": 222222222_u32
                },
                "memory": {
                    "128": 99,
                    "129": 88
                },
                "public_registers": [4],
                "public_memory": [{"start": 0, "len": 4}]
            },
            "blinding_seed_hex": "4242424242424242424242424242424242424242424242424242424242424242"
        }))
        .unwrap(),
    )
    .unwrap();

    let prove_stdout = run(&[
        "sfcs",
        "zk-private-vm",
        program.to_str().unwrap(),
        "--witness",
        witness.to_str().unwrap(),
        "--report",
        report.to_str().unwrap(),
        "--artifact-output",
        artifact.to_str().unwrap(),
    ]);
    assert!(prove_stdout.contains("SFCS ZK PRIVATE VM"));
    assert!(prove_stdout.contains("proof_digest: sha256:"));
    assert!(prove_stdout.contains("transition_checks: 6"));
    assert!(prove_stdout.contains("linear_relation_checks: 5"));
    assert!(prove_stdout.contains("zk_range_proofs: 19"));
    assert!(prove_stdout.contains("zk_memory_consistency_proofs: 1"));
    assert!(prove_stdout.contains("zk_memory_value_proofs: 2"));
    assert!(prove_stdout.contains("zk_branch_proofs: 0"));
    assert!(prove_stdout.contains("private_witness_embedded: false"));

    let report_json = read_json(&report);
    assert_eq!(
        report_json["profile"],
        "power-house/sfcs-zk-private-vm/v1-draft"
    );
    assert_eq!(report_json["public_outputs"]["registers"]["x4"], 12);
    assert_eq!(report_json["public_outputs"]["memory"]["0"], 12);
    assert_eq!(report_json["linear_relation_checks"], 5);
    assert_eq!(report_json["zk_range_proofs"], 19);
    assert_eq!(report_json["zk_memory_consistency_proofs"], 1);
    assert_eq!(report_json["zk_memory_value_proofs"], 2);
    assert_eq!(report_json["zk_branch_proofs"], 0);
    assert_eq!(report_json["private_witness_embedded"], false);
    assert!(report_json["commitments"]["trace_digest"]
        .as_str()
        .unwrap()
        .starts_with("edwards:"));
    assert!(artifact.exists());

    let artifact_json = fs::read_to_string(&artifact).unwrap();
    assert!(!artifact_json.contains("777777777"));
    assert!(!artifact_json.contains("222222222"));
    assert!(!artifact_json.contains("\"inputs\""));
    assert!(!artifact_json.contains("\"trace\""));

    let verify_stdout = run(&["sfcs", "verify-zk-pha", artifact.to_str().unwrap()]);
    assert!(verify_stdout.contains("SFCS ZK PRIVATE VM PHA VALID"));
    assert!(verify_stdout.contains("transition_checks: 6"));
    assert!(verify_stdout.contains("linear_relation_checks: 5"));
    assert!(verify_stdout.contains("zk_range_proofs: 19"));
    assert!(verify_stdout.contains("zk_memory_consistency_proofs: 1"));
    assert!(verify_stdout.contains("zk_memory_value_proofs: 2"));
    assert!(verify_stdout.contains("zk_branch_proofs: 0"));
    assert!(verify_stdout.contains("private_witness_embedded: false"));
}

#[cfg(feature = "sfcs-zk")]
#[test]
fn cli_runs_rust_private_add_end_to_end() {
    let dir = temp_dir();
    let source = dir.join("private_add.rs");
    let report = dir.join("private-add.e2e.report.json");
    let artifact = dir.join("private-add.e2e.pha");
    let rootprint = dir.join("private-add.e2e.rootprint.json");
    let sidecar = dir.join("private-add.e2e.observatory.json");
    let capsule = dir.join("private-add.e2e.phm");

    fs::write(
        &source,
        "pub fn add(lhs: u32, rhs: u32) -> u32 { return lhs + rhs; }\n",
    )
    .unwrap();

    let prove_stdout = run(&[
        "sfcs",
        "rust-private-add",
        source.to_str().unwrap(),
        "--lhs-value",
        "144",
        "--rhs-value",
        "233",
        "--lhs-blinding",
        "1111111111111111111111111111111111111111111111111111111111111111",
        "--rhs-blinding",
        "2222222222222222222222222222222222222222222222222222222222222222",
        "--artifact-output",
        artifact.to_str().unwrap(),
        "--rootprint-output",
        rootprint.to_str().unwrap(),
        "--sidecar-output",
        sidecar.to_str().unwrap(),
        "--capsule-output",
        capsule.to_str().unwrap(),
        "--report",
        report.to_str().unwrap(),
        "--label",
        "rust-private-add-e2e",
    ]);
    assert!(prove_stdout.contains("SFCS RUST PRIVATE ADD"));
    assert!(prove_stdout.contains("proof_digest: sha256:"));
    assert!(prove_stdout.contains("capsule_digest: sha256:"));
    assert!(prove_stdout.contains("output x3=377"));
    assert!(prove_stdout.contains("truth_boundary: semantic packet data is non-core"));

    assert!(artifact.exists());
    assert!(rootprint.exists());
    assert!(sidecar.exists());
    assert!(capsule.exists());
    assert!(report.exists());

    let report_json = read_json(&report);
    assert_eq!(report_json["output_value"], 377);
    assert_eq!(report_json["memory_core_valid"], true);
    assert_eq!(report_json["memory_rootprint_valid"], true);
    assert_eq!(report_json["memory_replay_valid"], true);
    assert_eq!(report_json["memory_sidecar_valid"], true);
    assert_eq!(report_json["memory_semantic_valid"], true);
    assert!(report_json["source_digest"]
        .as_str()
        .unwrap()
        .starts_with("sha256:"));

    let verify_zk_stdout = run(&["sfcs", "verify-zk-pha", artifact.to_str().unwrap()]);
    assert!(verify_zk_stdout.contains("SFCS ZK PRIVATE ADD PHA VALID"));
    assert!(verify_zk_stdout.contains("public_output: x3=377"));

    let memory_stdout = run(&["memory", "verify", capsule.to_str().unwrap()]);
    assert!(memory_stdout.contains("POWER HOUSE MEMORY VERIFY"));
    assert!(memory_stdout.contains("CORE        VALID"));
    assert!(memory_stdout.contains("ROOTPRINT   VALID"));
    assert!(memory_stdout.contains("REPLAY      VALID"));
    assert!(memory_stdout.contains("SIDECAR     VALID"));
    assert!(memory_stdout.contains("SEMANTIC    VALID"));
}
