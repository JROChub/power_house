use power_house::{
    memory::semantic_packet_digest,
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
    let path = std::env::temp_dir().join(format!("power-house-memory-{suffix}"));
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
fn cli_creates_verifies_replays_challenges_and_exports_memory_capsule() {
    let dir = temp_dir();
    let pha_path = dir.join("main.pha");
    let rootprint_path = dir.join("proof.rootprint.json");
    let sidecar_path = dir.join("proof.observatory.json");
    let capsule_path = dir.join("earth-001.phm");
    let verify_report = dir.join("verify.json");
    let replay_report = dir.join("replay.json");
    let challenge_report = dir.join("challenge.json");
    let export_dir = dir.join("export");

    let artifact = PhaArtifact::new(
        json!({"source": "memory-cli"}),
        "power-house/memory-cli/v1",
        json!({"claim": 21}),
        json!({"accepted": true}),
    )
    .unwrap();
    let graph = Rootprint::new("main", artifact.clone()).unwrap();
    let replay = graph.replay().unwrap();
    let mut packet = json!({
        "schema": "slbit/viz-packet/v3",
        "packet_id": "slp_memory_cli",
        "packet_digest": "",
        "claim": {
            "claim_id": "claim_memory_cli",
            "label": "CLI memory capsule",
            "domain": "test",
            "status": "explained",
            "bound_core": {
                "capsule_id": "phm_earth-001",
                "branch_id": graph.root_branch,
                "replay_fingerprint": replay.state_fingerprint
            }
        },
        "transcript": {"rounds": []},
        "semantic_dag": {"nodes": [], "edges": []},
        "views": {"timeline": [], "claim_cards": [], "graphs": [], "diffs": []},
        "explanation_constraints": {
            "allowed_sources": ["packet_nodes"],
            "forbid_unbound_claims": true,
            "mark_generated_text_non_authoritative": true
        }
    });
    packet["packet_digest"] = json!(semantic_packet_digest(&packet).unwrap());
    let sidecar = ObservatorySidecar::new(
        &graph,
        BTreeMap::from([(graph.root_branch.clone(), packet)]),
    )
    .unwrap();

    write_json(&pha_path, &artifact);
    write_json(&rootprint_path, &graph);
    write_json(&sidecar_path, &sidecar);

    let create = run(&[
        "memory",
        "create",
        "--capsule-id",
        "earth-001",
        "--pha",
        pha_path.to_str().unwrap(),
        "--rootprint",
        rootprint_path.to_str().unwrap(),
        "--sidecar",
        sidecar_path.to_str().unwrap(),
        "--output",
        capsule_path.to_str().unwrap(),
    ]);
    assert!(create.contains("capsule_digest: sha256:"));

    let verify = run(&[
        "memory",
        "verify",
        capsule_path.to_str().unwrap(),
        "--report",
        verify_report.to_str().unwrap(),
    ]);
    assert!(verify.contains("CORE        VALID"));
    assert!(verify.contains("SEMANTIC    VALID"));

    let replay_output = run(&[
        "memory",
        "replay",
        capsule_path.to_str().unwrap(),
        "--report",
        replay_report.to_str().unwrap(),
    ]);
    assert!(replay_output.contains("replay: VALID"));

    let challenge = run(&[
        "memory",
        "challenge",
        capsule_path.to_str().unwrap(),
        "--all",
        "--report",
        challenge_report.to_str().unwrap(),
    ]);
    assert!(challenge.contains("CHALLENGE   10/10 EXPECTED REJECTIONS"));

    run(&[
        "memory",
        "export",
        capsule_path.to_str().unwrap(),
        "--format",
        "directory",
        "--output",
        export_dir.to_str().unwrap(),
    ]);
    assert!(export_dir.join("capsule.json").exists());
    assert!(export_dir.join("core.pha").exists());
    assert!(export_dir.join("rootprint.json").exists());
    assert!(export_dir.join("observatory-sidecar.json").exists());

    fs::remove_dir_all(dir).unwrap();
}
