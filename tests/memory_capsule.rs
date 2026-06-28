use power_house::{
    memory::semantic_packet_digest,
    provenance::{PhaArtifact, Rootprint},
    ChallengeSuite, MemoryCapsule, MemoryCapsuleBuilder, MemoryError, MemoryVerificationPolicy,
    ObservatorySidecar,
};
use serde_json::json;
use std::collections::BTreeMap;

fn fixture() -> MemoryCapsule {
    let artifact = PhaArtifact::new(
        json!({"source": "memory-test", "domain": "earth-001"}),
        "power-house/memory-test/v1",
        json!({"claim": "portable proof memory", "value": 13}),
        json!({"accepted": true}),
    )
    .unwrap();
    let graph = Rootprint::new("main", artifact.clone()).unwrap();
    let replay = graph.replay().unwrap();
    let packet = json!({
        "schema": "slbit/viz-packet/v3",
        "packet_id": "slp_memory_test",
        "packet_digest": "",
        "claim": {
            "claim_id": "claim_memory_test",
            "label": "portable proof memory",
            "domain": "test",
            "status": "explained",
            "bound_core": {
                "capsule_id": "phm_memory-test",
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
    let packet_digest = semantic_packet_digest(&packet).unwrap();
    let mut packet = packet;
    packet["packet_digest"] = json!(packet_digest);
    let sidecar = ObservatorySidecar::new(
        &graph,
        BTreeMap::from([(graph.root_branch.clone(), packet.clone())]),
    )
    .unwrap();

    MemoryCapsuleBuilder::new("memory-test")
        .producer("mfenx", env!("CARGO_PKG_VERSION"))
        .with_pha(artifact)
        .with_rootprint(graph.clone())
        .with_replay_required()
        .with_semantic_packet(
            "slbit/viz-packet/v3",
            "slp_memory_test",
            graph.root_branch.clone(),
            replay.state_fingerprint.clone(),
            "claim_view",
            packet,
        )
        .unwrap()
        .with_sidecar(sidecar)
        .with_challenge_suite(ChallengeSuite::standard())
        .build()
        .unwrap()
}

#[test]
fn memory_capsule_verifies_replays_and_challenges() {
    let capsule = fixture();
    let report = capsule
        .verify(MemoryVerificationPolicy::strict())
        .expect("valid capsule");
    assert!(report.core_valid);
    assert!(report.rootprint_valid);
    assert!(report.replay_valid);
    assert_eq!(report.sidecar_valid, Some(true));
    assert_eq!(report.semantic_valid, Some(true));
    assert!(
        !report
            .soundness_report
            .as_ref()
            .unwrap()
            .expanded_table_allocated
    );

    let replay = capsule.replay().unwrap();
    assert!(replay.replay_valid);
    assert_eq!(replay.branch_count, 1);

    let challenge = capsule
        .challenge_all(MemoryVerificationPolicy::strict())
        .expect("challenge suite");
    assert_eq!(challenge.total, 10);
    assert_eq!(challenge.mismatches, 0, "{challenge:#?}");
}

#[test]
fn semantic_mutation_rejects_without_changing_core_truth() {
    let mut capsule = fixture();
    let original_core_digest = capsule.core.core_digest.clone();
    let packet = capsule
        .semantics
        .as_mut()
        .unwrap()
        .packets
        .first_mut()
        .unwrap();
    packet.packet.as_mut().unwrap()["claim"]["label"] = json!("tampered meaning");
    capsule.header.capsule_digest = Some(capsule.calculate_capsule_digest().unwrap());

    let error = capsule
        .verify(MemoryVerificationPolicy::strict())
        .unwrap_err();
    let MemoryError::Rejected(trace) = error else {
        panic!("expected rejection trace");
    };
    assert_eq!(trace.layer, "semantic");
    assert_eq!(trace.code, "PACKET_DIGEST_MISMATCH");
    assert!(trace.core_valid_before_failure);
    assert!(!trace.semantic_can_affect_core);
    assert_eq!(capsule.core.core_digest, original_core_digest);
}

#[test]
fn strict_parser_rejects_duplicate_keys_and_float_numbers() {
    let capsule = fixture();
    let mut encoded = serde_json::to_string(&capsule).unwrap();
    encoded = encoded.replacen("\"schema\"", "\"schema\",\"schema\"", 1);
    let duplicate =
        MemoryCapsule::from_slice(encoded.as_bytes(), &MemoryVerificationPolicy::strict())
            .unwrap_err();
    assert!(matches!(duplicate, MemoryError::Canonical(_)));

    let float_json = r#"{"schema":"power-house/memory-capsule/v1","n":1.5}"#;
    let float =
        MemoryCapsule::from_slice(float_json.as_bytes(), &MemoryVerificationPolicy::strict())
            .unwrap_err();
    assert!(matches!(float, MemoryError::Canonical(_)));
}
