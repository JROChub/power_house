#![cfg(feature = "sfcs")]

use power_house::{
    provenance::Rootprint, verify_sfcs_execution_embedding, verify_sfcs_pha_embedding,
    MemoryCapsuleBuilder, MemoryVerificationPolicy, SfcsError, SfcsGraph, SfcsNode, SfcsOp,
    SfcsRegionKind, SfcsRewriteKind,
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
fn textual_program_maps_directly_to_fractal_graph() {
    let parsed = SfcsGraph::from_program(
        r#"
        # Source maps directly to fractal nodes, not a flattened circuit.
        input a
        input b
        const c 7
        add sum a b
        mul z sum c
        output z
        "#,
    )
    .unwrap();
    let manual = arithmetic_graph();
    assert_eq!(parsed, manual);
    assert_eq!(
        parsed
            .evaluate(&BTreeMap::from([
                ("a".to_string(), 5),
                ("b".to_string(), 6),
            ]))
            .unwrap()["z"],
        77
    );
    assert!(SfcsGraph::from_program("input a\noutput z").is_err());
    assert!(SfcsGraph::from_program("const a nope\noutput a").is_err());
}

#[test]
fn textual_program_supports_control_ops_and_committed_metadata() {
    let parsed = SfcsGraph::from_program(
        r#"
        input a
        input b
        sub delta a b
        eq same a b
        not changed same
        branch out changed delta a
        label delta Difference node
        meta delta source user-supplied-logic
        output out
        "#,
    )
    .unwrap();

    let delta = &parsed.nodes["delta"];
    assert_eq!(delta.label.as_deref(), Some("Difference node"));
    assert_eq!(delta.metadata["source"], "user-supplied-logic");
    assert_eq!(
        parsed
            .evaluate(&BTreeMap::from([
                ("a".to_string(), 9),
                ("b".to_string(), 4),
            ]))
            .unwrap()["out"],
        5
    );
    assert_eq!(
        parsed
            .evaluate(&BTreeMap::from([
                ("a".to_string(), 4),
                ("b".to_string(), 4),
            ]))
            .unwrap()["out"],
        4
    );

    let mut mutated = parsed.clone();
    mutated
        .nodes
        .get_mut("delta")
        .unwrap()
        .metadata
        .insert("source".to_string(), "tampered-source".to_string());
    assert_ne!(
        parsed.fractal_digest().unwrap(),
        mutated.fractal_digest().unwrap()
    );

    assert!(matches!(
        SfcsGraph::from_program("input a\nlabel a bad\u{0007}\noutput a"),
        Err(SfcsError::InvalidGraph(_))
    ));
}

#[test]
fn execution_trace_is_deterministic_and_input_sensitive() {
    let graph = arithmetic_graph();
    let inputs = BTreeMap::from([("a".to_string(), 5), ("b".to_string(), 6)]);
    let first = graph.execution_trace(&inputs).unwrap();
    let second = graph.execution_trace(&inputs).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.outputs["z"], 77);
    assert_eq!(first.steps.len(), 5);
    assert!(first
        .steps
        .iter()
        .all(|step| step.step_digest.starts_with("sha256:")));

    let changed = graph
        .execution_trace(&BTreeMap::from([
            ("a".to_string(), 6),
            ("b".to_string(), 6),
        ]))
        .unwrap();
    assert_ne!(first.input_digest, changed.input_digest);
    assert_ne!(first.output_digest, changed.output_digest);
    assert_ne!(first.trace_digest, changed.trace_digest);
}

#[test]
fn synthesis_plan_records_fast_path_and_dense_boundaries() {
    let mut graph = arithmetic_graph();
    graph
        .insert_node(SfcsNode::new(
            "opaque",
            SfcsOp::DenseStep,
            vec!["z".to_string()],
        ))
        .unwrap();
    graph.outputs = vec!["opaque".to_string()];

    let first = graph.synthesis_plan().unwrap();
    let second = graph.synthesis_plan().unwrap();
    assert_eq!(first, second);
    assert_eq!(first.operations.len(), 2);
    assert_eq!(first.operations[0].kind, SfcsRewriteKind::FastPathExtract);
    assert_eq!(first.operations[1].kind, SfcsRewriteKind::DenseBoundary);
    assert_eq!(first.dense_nodes, vec!["opaque".to_string()]);
    assert!(first.synthesis_digest.starts_with("sha256:"));
    assert!(first.embedding_invariant_digest.starts_with("sha256:"));
}

#[test]
fn structure_regions_are_connected_replayable_subfractals() {
    let graph = SfcsGraph::from_program(
        r#"
        input a
        input b
        const c 3
        add sum a b
        dense opaque sum
        mul z opaque c
        output z
        "#,
    )
    .unwrap();

    let discovery = graph.discover_structure().unwrap();
    assert_eq!(discovery.fast_path_regions, 2);
    assert_eq!(discovery.dense_regions, 1);
    assert_eq!(discovery.regions.len(), 3);
    assert!(discovery
        .regions
        .iter()
        .all(|region| region.region_digest.starts_with("sha256:")));
    assert_eq!(discovery.regions[0].kind, SfcsRegionKind::FastPath);
    assert_eq!(discovery.regions[0].node_ids, vec!["a", "b", "sum"]);
    assert_eq!(discovery.regions[1].kind, SfcsRegionKind::DenseBoundary);
    assert_eq!(discovery.regions[1].node_ids, vec!["opaque"]);
    assert_eq!(discovery.regions[2].kind, SfcsRegionKind::FastPath);
    assert_eq!(discovery.regions[2].node_ids, vec!["c", "z"]);
    assert_eq!(discovery.regions[2].entry_nodes, vec!["z"]);
    assert_eq!(discovery.regions[2].output_nodes, vec!["z"]);

    let plan = graph.synthesis_plan().unwrap();
    assert_eq!(plan.operations.len(), 3);
    assert_eq!(plan.fast_path_regions, 2);
    assert_eq!(plan.dense_regions, 1);
    for (operation, region) in plan.operations.iter().zip(plan.regions.iter()) {
        assert_eq!(operation.region_digest, region.region_digest);
    }
}

#[test]
fn execution_pha_embedding_replays_trace_and_synthesis_plan() {
    let graph = arithmetic_graph();
    let inputs = BTreeMap::from([("a".to_string(), 5), ("b".to_string(), 6)]);
    let artifact = graph
        .to_execution_pha_artifact("arithmetic-execution", &inputs)
        .unwrap();
    artifact.verify().unwrap();
    assert_eq!(
        artifact.embedded_proof.protocol,
        "power-house/sfcs-execution/v1-draft"
    );
    let report = verify_sfcs_execution_embedding(&artifact).unwrap();
    assert_eq!(report.node_count, 5);
    assert_eq!(report.trace_steps, 5);
    assert_eq!(report.dense_nodes, 0);

    let mut tampered = artifact.clone();
    tampered.embedded_proof.public_inputs["outputs"]["z"] = json!(78);
    tampered.refresh_phx_fingerprint().unwrap();
    tampered.verify().unwrap();
    assert!(matches!(
        verify_sfcs_execution_embedding(&tampered),
        Err(SfcsError::InvalidEmbedding(_))
    ));

    let mut stale_trace = artifact;
    stale_trace.embedded_proof.proof["trace"]["outputs"]["z"] = json!(78);
    stale_trace.refresh_phx_fingerprint().unwrap();
    stale_trace.verify().unwrap();
    assert!(matches!(
        verify_sfcs_execution_embedding(&stale_trace),
        Err(SfcsError::InvalidEmbedding(_))
    ));
}

#[test]
fn execution_embedding_rejects_stale_region_public_inputs() {
    let graph = arithmetic_graph();
    let inputs = BTreeMap::from([("a".to_string(), 5), ("b".to_string(), 6)]);
    let mut artifact = graph
        .to_execution_pha_artifact("arithmetic-execution", &inputs)
        .unwrap();
    artifact.embedded_proof.public_inputs["structure_regions"] = json!(99);
    artifact.refresh_phx_fingerprint().unwrap();
    artifact.verify().unwrap();
    assert!(matches!(
        verify_sfcs_execution_embedding(&artifact),
        Err(SfcsError::InvalidEmbedding(_))
    ));
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
    public_inputs["fast_path_regions"] = json!(4);
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
