use power_house::{
    provenance::{PhaArtifact, Rootprint},
    ObservatoryError, ObservatorySidecar,
};
use serde_json::json;
use slbit::{BitInteractiveTranscript, LuminousClaim, SimpleLuminousSumcheck, VizHints};
use std::collections::BTreeMap;

fn graph() -> Rootprint {
    let artifact = PhaArtifact::new(
        json!({"source": "observatory-integration"}),
        "power-house/observatory-integration/v1",
        json!({"claim": "accepted"}),
        json!({"accepted": true}),
    )
    .unwrap();
    Rootprint::new("main", artifact).unwrap()
}

fn packet() -> serde_json::Value {
    let claim = LuminousClaim::new("integration-claim", 4096).with_viz_hints(VizHints {
        color: Some([0, 200, 255]),
        icon: Some("camera".into()),
        layer_name: Some("perception-conv3".into()),
    });
    let mut transcript = BitInteractiveTranscript::new(b"integration-seed");
    transcript.record_round_with_note(
        0,
        &[0x42],
        "attention-head-7",
        "Stop-sign feature strongly activated",
    );
    let luminous = SimpleLuminousSumcheck { claim, transcript };
    let packet = luminous.to_viz_packet().unwrap();
    packet.verify().unwrap();
    serde_json::from_str(&packet.to_json()).unwrap()
}

#[test]
fn slbit_sidecar_never_changes_power_house_identity() {
    let graph = graph();
    let replay = graph.replay().unwrap();
    let core_fingerprint = graph
        .branches
        .get(&graph.root_branch)
        .unwrap()
        .artifact
        .phx_fingerprint
        .clone();
    let sidecar = ObservatorySidecar::new(
        &graph,
        BTreeMap::from([(graph.root_branch.clone(), packet())]),
    )
    .unwrap();

    sidecar.verify(&graph).unwrap();
    assert_eq!(graph.replay().unwrap(), replay);
    assert_eq!(
        graph
            .branches
            .get(&graph.root_branch)
            .unwrap()
            .artifact
            .phx_fingerprint,
        core_fingerprint
    );
}

#[test]
fn semantic_mutation_rejects_sidecar_without_affecting_graph() {
    let graph = graph();
    let mut sidecar = ObservatorySidecar::new(
        &graph,
        BTreeMap::from([(graph.root_branch.clone(), packet())]),
    )
    .unwrap();
    sidecar.nodes.get_mut(&graph.root_branch).unwrap()["rounds"][0]["note"] =
        json!("mutated annotation");

    assert!(matches!(
        sidecar.verify(&graph),
        Err(ObservatoryError::SidecarDigestMismatch { .. })
    ));
    graph.verify().unwrap();
}
