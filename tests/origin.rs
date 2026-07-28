#![cfg(feature = "sfcs")]

use power_house::origin::{
    Origin, OriginError, OriginPolicy, OriginSpec, ORIGIN_RECEIPT_SCHEMA_V1,
};
use power_house::verify_sfcs_execution_embedding;
use serde_json::json;

fn arithmetic_spec(label: &str) -> OriginSpec {
    OriginSpec::new(
        label,
        r#"
            input left
            input right
            let sum = left + right
            let doubled = sum * 2
            output sum doubled
        "#,
    )
    .with_input("left", 21)
    .with_input("right", 34)
}

#[test]
fn manifestation_is_verified_and_deterministic() {
    let policy = OriginPolicy::new(64, 64, 64, 1_000);
    let first = Origin::manifest(arithmetic_spec("origin"), policy.clone()).unwrap();
    let second = Origin::manifest(arithmetic_spec("origin"), policy).unwrap();

    first.verify().unwrap();
    second.verify().unwrap();
    assert_eq!(first.receipt(), second.receipt());
    assert_eq!(first.receipt().schema, ORIGIN_RECEIPT_SCHEMA_V1);
    assert_eq!(first.outputs()["sum"], 55);
    assert_eq!(first.outputs()["doubled"], 110);
    assert_eq!(
        first.capacity().issued_units(),
        first.capacity().spent_units() + first.capacity().remaining_units()
    );
}

#[test]
fn derivation_is_atomic_and_consumes_capacity() {
    let mut origin = Origin::manifest(
        arithmetic_spec("origin"),
        OriginPolicy::new(64, 64, 64, 1_000),
    )
    .unwrap();
    let parent = origin.identity().rootprint_id().clone();
    let remaining_before = origin.capacity().remaining_units();

    let receipt = origin.derive(arithmetic_spec("child")).unwrap();

    origin.verify().unwrap();
    assert_eq!(receipt.parent_rootprint_id, Some(parent));
    assert_eq!(receipt.generation, 1);
    assert!(origin.capacity().remaining_units() < remaining_before);
    assert_eq!(origin.rootprint().branches.len(), 2);
}

#[test]
fn failed_derivation_leaves_identity_lineage_and_capacity_unchanged() {
    let mut origin = Origin::manifest(
        arithmetic_spec("origin"),
        OriginPolicy::new(64, 64, 64, 1_000),
    )
    .unwrap();
    let receipt_before = origin.receipt().clone();
    let rootprint_before = origin.rootprint().clone();
    let remaining_before = origin.capacity().remaining_units();

    let invalid = OriginSpec::new(
        "invalid-child",
        r#"
            input value
            let zero = 0
            let result = value / zero
            output result
        "#,
    )
    .with_input("value", 7);
    assert!(origin.derive(invalid).is_err());

    assert_eq!(origin.receipt(), &receipt_before);
    assert_eq!(origin.rootprint(), &rootprint_before);
    assert_eq!(origin.capacity().remaining_units(), remaining_before);
    origin.verify().unwrap();
}

#[test]
fn exact_capacity_budget_prevents_any_further_creation() {
    let spec = arithmetic_spec("origin");
    let estimate = Origin::estimate(&spec).unwrap();
    let mut origin = Origin::manifest(
        spec,
        OriginPolicy::new(64, 64, 64, estimate.cost.total_units),
    )
    .unwrap();
    assert_eq!(origin.capacity().remaining_units(), 0);
    let receipt_before = origin.receipt().clone();

    let error = origin.derive(arithmetic_spec("child")).unwrap_err();
    assert!(matches!(error, OriginError::CapacityExhausted { .. }));
    assert_eq!(origin.receipt(), &receipt_before);
    origin.verify().unwrap();
}

#[test]
fn core_mutation_is_rejected_by_the_underlying_verifier() {
    let origin = Origin::manifest(
        arithmetic_spec("origin"),
        OriginPolicy::new(64, 64, 64, 1_000),
    )
    .unwrap();
    let mut tampered = origin.identity().pha().clone();
    tampered.embedded_proof.public_inputs["outputs"]["sum"] = json!(999_999);
    assert!(verify_sfcs_execution_embedding(&tampered).is_err());
}
