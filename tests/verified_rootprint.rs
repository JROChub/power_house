use power_house::provenance::{PhaArtifact, Rootprint};
use power_house::ObservatorySidecar;
use serde_json::json;
use std::collections::BTreeMap;

fn artifact(value: u64) -> PhaArtifact {
    PhaArtifact::new(
        json!({"test": "verified-view"}),
        "test/v1",
        json!({"value":value}),
        json!({"accepted":true}),
    )
    .unwrap()
}

#[test]
fn verified_view_preserves_replay_and_sidecar_results() {
    let mut graph = Rootprint::new("root", artifact(0)).unwrap();
    let root = graph.root_branch.clone();
    let a = graph.fork(&root, "a", artifact(1)).unwrap();
    let b = graph.fork(&root, "b", artifact(2)).unwrap();
    graph.merge(&a, &b, "merge", artifact(3)).unwrap();
    let expected = graph.replay().unwrap();
    let sidecar =
        ObservatorySidecar::new(&graph, BTreeMap::from([(a, json!({"meaning":"example"}))]))
            .unwrap();
    let view = graph.verified().unwrap();
    assert!(std::ptr::eq(view.graph(), &graph));
    assert_eq!(view.state(), &expected);
    for _ in 0..3 {
        sidecar.verify_verified(&view).unwrap();
    }
    sidecar.verify(&graph).unwrap();
    assert_eq!(view.state(), &expected);
}

#[test]
fn verified_views_cannot_authorize_other_graphs_or_modified_sidecars() {
    let graph = Rootprint::new("root", artifact(0)).unwrap();
    let other = Rootprint::new("root", artifact(1)).unwrap();
    let mut sidecar = ObservatorySidecar::new(&graph, BTreeMap::new()).unwrap();
    assert!(sidecar.verify_verified(&other.verified().unwrap()).is_err());
    sidecar
        .nodes
        .insert(graph.root_branch.clone(), json!({"tampered":true}));
    assert!(sidecar.verify_verified(&graph.verified().unwrap()).is_err());
}

#[test]
fn mutation_requires_reverification_and_invalid_artifacts_still_fail() {
    let mut graph = Rootprint::new("root", artifact(0)).unwrap();
    let root = graph.root_branch.clone();
    let before = graph.verified().unwrap().state().state_fingerprint.clone();
    let branch = graph.fork(&root, "child", artifact(1)).unwrap();
    assert_ne!(graph.verified().unwrap().state().state_fingerprint, before);
    graph
        .branches
        .get_mut(&branch)
        .unwrap()
        .artifact
        .phx_fingerprint = format!("sha256:{}", "0".repeat(64));
    assert!(graph.verify().is_err());
    assert!(graph.verified().is_err());
    assert!(graph.replay().is_err());
}

#[test]
fn overflow_never_inserts_an_invalid_fork_or_merge() {
    let mut graph = Rootprint::new("root", artifact(0)).unwrap();
    let root = graph.root_branch.clone();
    let a = graph.fork(&root, "a", artifact(1)).unwrap();
    let b = graph.fork(&root, "b", artifact(2)).unwrap();
    graph.branches.get_mut(&a).unwrap().sequence = u64::MAX;
    graph.verify().unwrap();
    let before = graph.clone();
    assert!(graph.fork(&a, "overflow", artifact(3)).is_err());
    assert_eq!(graph, before);
    assert!(graph.merge(&a, &b, "overflow", artifact(3)).is_err());
    assert_eq!(graph, before);
}

#[test]
fn forged_matching_fingerprints_never_establish_branch_equivalence() {
    let mut graph = Rootprint::new("root", artifact(0)).unwrap();
    let root = graph.root_branch.clone();
    let branch = graph.fork(&root, "child", artifact(1)).unwrap();
    let forged = graph.branches[&root].artifact.phx_fingerprint.clone();
    graph
        .branches
        .get_mut(&branch)
        .unwrap()
        .artifact
        .phx_fingerprint = forged;
    assert!(graph.equivalent(&root, &branch).is_err());
}
