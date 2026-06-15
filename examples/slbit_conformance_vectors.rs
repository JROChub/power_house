use power_house::{provenance::Rootprint, ObservatorySidecar};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use slbit::{BitInteractiveTranscript, LuminousClaim, SimpleLuminousSumcheck, VizHints};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

fn write_json(path: &Path, value: &impl Serialize) -> Vec<u8> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create output directory");
    }
    let mut bytes = serde_json::to_vec_pretty(value).expect("serialize vector");
    bytes.push(b'\n');
    fs::write(path, &bytes).expect("write vector");
    bytes
}

fn sha256(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn packet(
    claim_id: &str,
    color: [u8; 3],
    icon: &str,
    layer: &str,
    rounds: &[(&str, &str, &[u8])],
) -> Value {
    let claim = LuminousClaim::new(claim_id, 4096).with_viz_hints(VizHints {
        color: Some(color),
        icon: Some(icon.to_string()),
        layer_name: Some(layer.to_string()),
    });
    let mut transcript = BitInteractiveTranscript::new(claim_id.as_bytes());
    for (index, (component, note, payload)) in rounds.iter().enumerate() {
        transcript.record_round_with_note(index as u64, payload, *component, *note);
    }
    let luminous = SimpleLuminousSumcheck { claim, transcript };
    let packet = luminous.to_viz_packet().expect("valid slbit packet");
    packet.verify().expect("verified slbit packet");
    serde_json::from_str(&packet.to_json()).expect("packet JSON")
}

fn main() {
    let graph_bytes =
        fs::read("conformance/pha-v1/rootprint-valid.json").expect("Rootprint vector");
    let graph: Rootprint = serde_json::from_slice(&graph_bytes).expect("valid Rootprint");
    graph.verify().expect("verified Rootprint");
    let replay_before = graph.replay().expect("Rootprint replay");

    let mut nodes = BTreeMap::new();
    for branch in graph.branches.values() {
        let value = match branch.label.as_str() {
            "main" => packet(
                "vision-frame-7842",
                [0, 200, 255],
                "camera",
                "sensor-ingest",
                &[
                    (
                        "sensor-processing",
                        "Raw sensor frame converted into normalized features",
                        &[0xde, 0xad, 0xbe, 0xef],
                    ),
                    (
                        "calibration",
                        "Lens and exposure calibration passed",
                        &[0x10],
                    ),
                ],
            ),
            "candidate" => packet(
                "stop-sign-candidate",
                [185, 255, 61],
                "brain-circuit",
                "attention-head-7",
                &[
                    (
                        "feature-routing",
                        "Edges and color regions entered attention head 7",
                        &[0x21],
                    ),
                    (
                        "attention-head-7",
                        "Stop-sign feature strongly activated",
                        &[0x42],
                    ),
                ],
            ),
            "audit" => packet(
                "independent-evidence-audit",
                [255, 193, 77],
                "database",
                "evidence-audit",
                &[
                    (
                        "provenance-index",
                        "Source frame and model revision matched the audit record",
                        &[0x31],
                    ),
                    (
                        "policy-check",
                        "Independent evidence satisfied the acceptance policy",
                        &[0x32],
                    ),
                ],
            ),
            "accepted" => packet(
                "classification-accepted",
                [255, 113, 103],
                "bot",
                "classification",
                &[
                    (
                        "reconciliation",
                        "Candidate and audit branches reconciled",
                        &[0x51],
                    ),
                    (
                        "classification",
                        "Stop-sign classification completed and accepted",
                        &[0x52],
                    ),
                ],
            ),
            other => panic!("unexpected branch label: {other}"),
        };
        nodes.insert(branch.id.clone(), value);
    }

    let sidecar = ObservatorySidecar::new(&graph, nodes).expect("valid sidecar");
    sidecar.verify(&graph).expect("verified sidecar");
    assert_eq!(graph.replay().expect("replay"), replay_before);

    let directory = Path::new("conformance/slbit-v1");
    let sidecar_bytes = write_json(&directory.join("observatory-valid.json"), &sidecar);
    write_json(
        Path::new("publicpower/artifacts/luminous-valid.json"),
        &sidecar,
    );

    let manifest = json!({
        "schema": "power-house-slbit-conformance-v1",
        "slbit_version": "0.1.0",
        "rootprint_state_fingerprint": sidecar.rootprint_state_fingerprint,
        "semantic_packets_affect_core_identity": false,
        "files": {
            "observatory-valid.json": sha256(&sidecar_bytes),
            "rootprint-valid.json": sha256(&graph_bytes),
        }
    });
    write_json(&directory.join("manifest.json"), &manifest);
}
