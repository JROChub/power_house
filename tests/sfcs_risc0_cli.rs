#![cfg(feature = "sfcs-risc0")]

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
    let path = std::env::temp_dir().join(format!("power-house-sfcs-risc0-cli-{suffix}"));
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
fn cli_proves_packages_and_reverifies_private_guest_execution() {
    let dir = temp_dir();
    let input = dir.join("private.bin");
    let artifact = dir.join("private.pha");
    let rootprint = dir.join("private.rootprint.json");
    let capsule = dir.join("private.phm");
    let packet = dir.join("private.slbit.json");
    let sidecar = dir.join("private.observatory.json");
    let report = dir.join("private.report.json");
    fs::write(
        &input,
        [0x1357_9bdf_u32.to_le_bytes(), 0x2468_ace0_u32.to_le_bytes()].concat(),
    )
    .unwrap();
    let program = Path::new("conformance/sfcs-risc0/private-sum-v1.bin");

    let stdout = run(&[
        "sfcs",
        "risc0-prove",
        program.to_str().unwrap(),
        "--input",
        input.to_str().unwrap(),
        "--artifact-output",
        artifact.to_str().unwrap(),
        "--rootprint-output",
        rootprint.to_str().unwrap(),
        "--capsule-output",
        capsule.to_str().unwrap(),
        "--semantic-output",
        packet.to_str().unwrap(),
        "--sidecar-output",
        sidecar.to_str().unwrap(),
        "--report",
        report.to_str().unwrap(),
        "--label",
        "private-sum-cli",
    ]);
    assert!(stdout.contains("SFCS RISC0 PRIVATE VM VERIFIED"));
    assert!(stdout.contains("private_input_embedded: false"));

    let report = read_json(&report);
    assert_eq!(report["private_input_embedded"], false);
    assert_eq!(report["development_receipts_accepted"], false);
    assert!(report["phx_fingerprint"]
        .as_str()
        .unwrap()
        .starts_with("sha256:"));
    assert_eq!(
        read_json(&packet)["claim"]["bound_core"]["profile"],
        "power-house/sfcs-risc0-private-vm/v1"
    );

    let pha_stdout = run(&["sfcs", "verify-risc0-pha", artifact.to_str().unwrap()]);
    assert!(pha_stdout.contains("SFCS RISC0 PRIVATE VM PHA VALID"));
    let capsule_stdout = run(&["sfcs", "verify-risc0-capsule", capsule.to_str().unwrap()]);
    assert!(capsule_stdout.contains("SFCS RISC0 PRIVATE VM CAPSULE VALID"));
}
