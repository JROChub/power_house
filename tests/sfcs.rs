#![cfg(feature = "sfcs")]

use power_house::{
    provenance::Rootprint, verify_sfcs_pha_embedding, MemoryCapsuleBuilder,
    MemoryVerificationPolicy, SfcsError, SfcsGraph, SfcsNode, SfcsOp,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;

fn arithmetic_graph() -> SfcsGraph {
    let mut graph = SfcsGraph::new(vec!["z".to_string()]);
    graph
        .insert_node(SfcsNode::new("a", SfcsOp::Input, vec![]))
        .unwrap();
    graph
        .insert_node(SfcsNode::new("b", SfcsOp::Input, vec![]))
        .unwrap();
    graph.insert_node(SfcsNode::constant("c", 7)).unwrap();
    graph
        .insert_node(SfcsNode::new(
            "sum",
            SfcsOp::Add,
            vec!["a".to_string(), "b".to_string()],
        ))
        .unwrap();
    graph
        .insert_node(SfcsNode::new(
            "z",
            SfcsOp::Mul,
            vec!["sum".to_string(), "c".to_string()],
        ))
        .unwrap();
    graph
}

#[test]
fn sfcs_graph_commits_into_pha_and_rootprint_without_new_core_rules() {
    let graph = arithmetic_graph();
    graph.verify().unwrap();

    let digest = graph.fractal_digest().unwrap();
    let artifact = graph.to_pha_artifact("arithmetic").unwrap();
    artifact.verify().unwrap();
    let embedding = verify_sfcs_pha_embedding(&artifact).unwrap();
    assert_eq!(embedding.graph_digest, digest);
    assert_eq!(embedding.node_count, 5);
    assert_eq!(embedding.fast_path_nodes, 5);
    assert_eq!(embedding.dense_nodes, 0);
    assert_eq!(
        artifact.embedded_proof.protocol,
        "power-house/sfcs/v1-draft"
    );
    assert_eq!(artifact.provenance["fractal_digest"], json!(digest));

    let rootprint = Rootprint::new("sfcs-arithmetic", artifact.clone()).unwrap();
    rootprint.verify().unwrap();
    let replay = rootprint.replay().unwrap();
    assert_eq!(
        replay.branches[0].artifact_phx_fingerprint,
        artifact.phx_fingerprint
    );

    let capsule = MemoryCapsuleBuilder::new("sfcs-arithmetic")
        .with_pha(artifact)
        .with_rootprint(rootprint)
        .with_replay_required()
        .build()
        .unwrap();
    let report = capsule.verify(MemoryVerificationPolicy::strict()).unwrap();
    assert!(report.core_valid);
    assert!(report.rootprint_valid);
    assert!(report.replay_valid);
}

#[test]
fn structure_discovery_is_deterministic_and_separates_dense_nodes() {
    let mut graph = arithmetic_graph();
    graph
        .insert_node(SfcsNode::new(
            "zz",
            SfcsOp::DenseStep,
            vec!["z".to_string()],
        ))
        .unwrap();
    graph.outputs = vec!["zz".to_string()];

    let first = graph.discover_structure().unwrap();
    let second = graph.discover_structure().unwrap();
    assert_eq!(first, second);
    assert!(first.fast_path_nodes.contains(&"sum".to_string()));
    assert_eq!(first.dense_nodes, vec!["zz".to_string()]);
}

#[test]
fn draft_evaluator_is_deterministic_for_arithmetic_subset() {
    let graph = arithmetic_graph();
    let output = graph
        .evaluate(&BTreeMap::from([
            ("a".to_string(), 5),
            ("b".to_string(), 6),
        ]))
        .unwrap();
    assert_eq!(output["z"], 77);
}

#[test]
fn fractal_mutation_changes_pha_identity() {
    let original = arithmetic_graph();
    let mut mutated = arithmetic_graph();
    mutated
        .nodes
        .get_mut("c")
        .unwrap()
        .params
        .insert("value".to_string(), 8);

    let original_artifact = original.to_pha_artifact("arithmetic").unwrap();
    let mutated_artifact = mutated.to_pha_artifact("arithmetic").unwrap();
    assert_ne!(
        original.fractal_digest().unwrap(),
        mutated.fractal_digest().unwrap()
    );
    assert_ne!(
        original_artifact.phx_fingerprint,
        mutated_artifact.phx_fingerprint
    );
}

#[test]
fn cycle_rejects() {
    let mut graph = SfcsGraph::new(vec!["a".to_string()]);
    graph
        .insert_node(SfcsNode::new("a", SfcsOp::DenseStep, vec!["b".to_string()]))
        .unwrap();
    graph
        .insert_node(SfcsNode::new("b", SfcsOp::DenseStep, vec!["a".to_string()]))
        .unwrap();
    assert!(graph.verify().is_err());
}

#[test]
fn strict_parser_rejects_duplicate_keys_and_float_numbers() {
    let graph = arithmetic_graph();
    let encoded = String::from_utf8(graph.canonical_bytes().unwrap()).unwrap();
    assert!(SfcsGraph::from_slice(encoded.as_bytes()).is_ok());

    let duplicate = encoded.replacen("\"schema\"", "\"schema\",\"schema\"", 1);
    assert!(matches!(
        SfcsGraph::from_slice(duplicate.as_bytes()),
        Err(SfcsError::Canonical(_))
    ));

    let floating = encoded.replace("\"value\":7", "\"value\":7.5");
    assert!(matches!(
        SfcsGraph::from_slice(floating.as_bytes()),
        Err(SfcsError::Canonical(_))
    ));
}

#[test]
fn duplicate_outputs_and_duplicate_inputs_reject() {
    let mut duplicate_outputs = arithmetic_graph();
    duplicate_outputs.outputs = vec!["z".to_string(), "z".to_string()];
    assert!(duplicate_outputs.verify().is_err());

    let mut duplicate_inputs = SfcsGraph::new(vec!["sum".to_string()]);
    duplicate_inputs
        .insert_node(SfcsNode::new("a", SfcsOp::Input, vec![]))
        .unwrap();
    duplicate_inputs
        .insert_node(SfcsNode::new(
            "sum",
            SfcsOp::Add,
            vec!["a".to_string(), "a".to_string()],
        ))
        .unwrap();
    assert!(duplicate_inputs.verify().is_err());
}

#[test]
fn core_valid_embedding_mutations_are_rejected_by_sfcs_verifier() {
    let graph = arithmetic_graph();
    let artifact = graph.to_pha_artifact("arithmetic").unwrap();

    let mut stale_digest = artifact.clone();
    let mut proof_graph: SfcsGraph =
        serde_json::from_value(stale_digest.embedded_proof.proof.clone()).unwrap();
    proof_graph
        .nodes
        .get_mut("c")
        .unwrap()
        .params
        .insert("value".to_string(), 8);
    stale_digest.embedded_proof.proof = serde_json::to_value(proof_graph).unwrap();
    stale_digest.refresh_phx_fingerprint().unwrap();
    stale_digest.verify().unwrap();
    assert!(matches!(
        verify_sfcs_pha_embedding(&stale_digest),
        Err(SfcsError::InvalidEmbedding(_))
    ));

    let mut stale_counters = artifact;
    let mut public_inputs = stale_counters.embedded_proof.public_inputs.clone();
    public_inputs["fast_path_nodes"] = json!(4);
    stale_counters.embedded_proof.public_inputs = public_inputs;
    stale_counters.refresh_phx_fingerprint().unwrap();
    stale_counters.verify().unwrap();
    assert!(matches!(
        verify_sfcs_pha_embedding(&stale_counters),
        Err(SfcsError::InvalidEmbedding(_))
    ));
}

#[test]
fn strict_parser_roundtrip_matches_canonical_graph() {
    let graph = arithmetic_graph();
    let reparsed = SfcsGraph::from_slice(&graph.canonical_bytes().unwrap()).unwrap();
    assert_eq!(graph, reparsed);
    assert_eq!(
        graph.fractal_digest().unwrap(),
        reparsed.fractal_digest().unwrap()
    );

    let mut value: Value = serde_json::from_slice(&graph.canonical_bytes().unwrap()).unwrap();
    value["nodes"]["z"]["inputs"] = json!(["c", "sum"]);
    let reordered: SfcsGraph = serde_json::from_value(value).unwrap();
    assert_ne!(
        graph.fractal_digest().unwrap(),
        reordered.fractal_digest().unwrap()
    );
    assert_eq!(
        graph
            .evaluate(&BTreeMap::from([
                ("a".to_string(), 5),
                ("b".to_string(), 6),
            ]))
            .unwrap(),
        reordered
            .evaluate(&BTreeMap::from([
                ("a".to_string(), 5),
                ("b".to_string(), 6),
            ]))
            .unwrap()
    );
}
