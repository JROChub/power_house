use power_house::{prove_with_rootprint, provenance::PhaArtifact, ObservatorySidecar};
use serde_json::json;
use slbit::{BitInteractiveTranscript, LuminousClaim, SimpleLuminousSumcheck, VizHints};
use std::collections::BTreeMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let artifact = PhaArtifact::new(
        json!({
            "producer": "vision-pipeline",
            "frame": 7842
        }),
        "vision/v1",
        json!({
            "claim": "stop-sign-detected",
            "confidence_millionths": 987_000
        }),
        json!({
            "accepted": true
        }),
    )?;

    let graph = prove_with_rootprint!(
        label: "drone-perception-7842",
        artifact: artifact,
    )?;
    graph.verify()?;
    let rootprint_fingerprint = graph.replay()?.state_fingerprint;

    let claim = LuminousClaim::new("drone-camera-frame-7842", 4096).with_viz_hints(VizHints {
        color: Some([0, 200, 255]),
        icon: Some("camera".into()),
        layer_name: Some("perception-conv3".into()),
    });
    let mut transcript = BitInteractiveTranscript::new(b"drone-seed-7842");
    transcript.record_round_with_note(
        0,
        &[0xde, 0xad, 0xbe, 0xef],
        "sensor-processing",
        "Raw sensor frame converted into features",
    );
    transcript.record_round_with_note(
        1,
        &[0x42],
        "attention-head-7",
        "Stop-sign feature strongly activated",
    );

    let luminous = SimpleLuminousSumcheck { claim, transcript };
    let metadata = luminous.to_luminous_metadata();
    let packet = luminous.to_viz_packet()?;
    packet.verify()?;

    let packet_json = serde_json::from_str(&packet.to_json())?;
    let sidecar = ObservatorySidecar::new(
        &graph,
        BTreeMap::from([(graph.root_branch.clone(), packet_json)]),
    )?;
    sidecar.verify(&graph)?;

    assert_eq!(graph.replay()?.state_fingerprint, rootprint_fingerprint);
    println!("Rootprint replay fingerprint: {rootprint_fingerprint}");
    println!("Semantic transcript: {}", metadata.transcript_digest);
    println!("Observatory sidecar: {}", sidecar.sidecar_sha256);
    Ok(())
}
