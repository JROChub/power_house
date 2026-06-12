use power_house::{
    prove_with_rootprint,
    provenance::{ExternalProofAttachment, PhaArtifact, Rootprint},
};
use serde_json::json;
use std::fs;
use std::path::Path;

fn artifact(claim: u64) -> PhaArtifact {
    PhaArtifact::new(
        json!({"producer": "integration-test", "run": 7}),
        "power-house/integration/v1",
        json!({"claim": claim}),
        json!({"verified": true}),
    )
    .unwrap()
}

fn with_attachment(mut artifact: PhaArtifact, payload: &str) -> PhaArtifact {
    artifact.embedded_proof.external_proof_attachments = Some(vec![ExternalProofAttachment::new(
        "external-1",
        "example/external/v1",
        json!({"proof": payload}),
    )
    .unwrap()]);
    artifact
}

#[test]
fn branching_is_identical_with_and_without_external_attachments() {
    let plain = artifact(1);
    let attached = with_attachment(plain.clone(), "original");

    let plain_graph = prove_with_rootprint!(label: "main", artifact: plain).unwrap();
    let attached_graph = prove_with_rootprint!(label: "main", artifact: attached).unwrap();

    assert_eq!(plain_graph.root_branch, attached_graph.root_branch);
    assert!(plain_graph.verify().is_ok());
    assert!(attached_graph.verify().is_ok());
}

#[test]
fn fork_merge_and_equivalence_never_require_external_attachments() {
    let root = artifact(1);
    let mut graph = Rootprint::new("main", root.clone()).unwrap();

    let left = prove_with_rootprint!(
        rootprint: &mut graph,
        fork: "main",
        label: "left",
        artifact: with_attachment(root.clone(), "left"),
    )
    .unwrap();
    let right = prove_with_rootprint!(
        rootprint: &mut graph,
        fork: "main",
        label: "right",
        artifact: root,
    )
    .unwrap();

    assert!(graph.equivalent(&left, &right).unwrap());

    let merged = prove_with_rootprint!(
        rootprint: &mut graph,
        merge: [&left, &right],
        label: "accepted",
        artifact: artifact(2),
    )
    .unwrap();
    assert_eq!(graph.navigate("accepted").unwrap().id, merged);
    assert!(graph.verify().is_ok());
}

#[test]
fn epa_mutation_preserves_graph_validity_but_fails_explicit_integrity() {
    let mut graph = Rootprint::new("main", with_attachment(artifact(1), "original")).unwrap();
    let root = graph.branches.get_mut(&graph.root_branch).unwrap();
    root.artifact
        .embedded_proof
        .external_proof_attachments
        .as_mut()
        .unwrap()[0]
        .payload = json!({"proof": "mutated"});

    assert!(graph.verify().is_ok());
    assert!(graph.verify_external_proof_attachments().is_err());
}

#[test]
fn conformance_vectors_reject_core_mutations_and_isolate_epa_mutations() {
    let vector_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("conformance/pha-v1");
    let core: PhaArtifact =
        serde_json::from_slice(&fs::read(vector_dir.join("core-valid.pha")).unwrap()).unwrap();
    let attached: PhaArtifact =
        serde_json::from_slice(&fs::read(vector_dir.join("core-with-epa.pha")).unwrap()).unwrap();
    let graph: Rootprint =
        serde_json::from_slice(&fs::read(vector_dir.join("rootprint-valid.json")).unwrap())
            .unwrap();

    core.verify().unwrap();
    attached.verify_external_proof_attachments().unwrap();
    graph.verify().unwrap();
    assert_eq!(core.phx_fingerprint, attached.phx_fingerprint);

    type ArtifactMutation = Box<dyn Fn(&mut PhaArtifact)>;
    let mutations: Vec<ArtifactMutation> = vec![
        Box::new(|artifact| artifact.schema.push_str("-mutated")),
        Box::new(|artifact| artifact.provenance = json!({"mutated": true})),
        Box::new(|artifact| artifact.embedded_proof.protocol.push_str("-mutated")),
        Box::new(|artifact| artifact.embedded_proof.public_inputs = json!({"claim": 0})),
        Box::new(|artifact| artifact.embedded_proof.proof = json!({"accepted": false})),
        Box::new(|artifact| artifact.phx_fingerprint.replace_range(7..8, "0")),
    ];
    for mutate in mutations {
        let mut artifact = core.clone();
        mutate(&mut artifact);
        assert!(artifact.verify().is_err());
    }

    let mut epa_mutation = attached;
    epa_mutation
        .embedded_proof
        .external_proof_attachments
        .as_mut()
        .unwrap()[0]
        .payload = json!({"proof": "mutated"});
    epa_mutation.verify().unwrap();
    assert!(epa_mutation.verify_external_proof_attachments().is_err());
}
