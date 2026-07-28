#[cfg(not(feature = "sfcs"))]
fn main() {
    eprintln!("run with --features sfcs");
}

#[cfg(feature = "sfcs")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use power_house::origin::{Origin, OriginPolicy, OriginSpec};
    use power_house::verify_sfcs_execution_embedding;
    use serde_json::json;

    let initial = OriginSpec::new(
        "grid-origin",
        r#"
            input solar
            input wind
            input demand
            let supply = solar + wind
            let reserve = supply - demand
            let stable = reserve >= 0
            output reserve stable
        "#,
    )
    .with_input("solar", 144)
    .with_input("wind", 89)
    .with_input("demand", 200);

    let estimate = Origin::estimate(&initial)?;
    println!(
        "PREPARED nodes={} trace_steps={} cost={} outputs={:?}",
        estimate.report.node_count,
        estimate.report.trace_steps,
        estimate.cost.total_units,
        estimate.outputs
    );

    let mut origin = Origin::manifest(initial, OriginPolicy::new(128, 128, 128, 512))?;
    origin.verify()?;
    println!("ORIGIN VERIFIED");
    println!("rootprint_id={}", origin.identity().rootprint_id());
    println!(
        "phx_fingerprint={}",
        origin.identity().pha().phx_fingerprint
    );
    println!("receipt_digest={}", origin.receipt().receipt_digest);
    println!(
        "capacity={}/{}",
        origin.capacity().remaining_units(),
        origin.capacity().issued_units()
    );

    let child = OriginSpec::new(
        "grid-origin-boosted",
        r#"
            input solar
            input wind
            input storage
            input demand
            let supply = solar + wind + storage
            let reserve = supply - demand
            let stable = reserve >= 0
            output reserve stable
        "#,
    )
    .with_input("solar", 144)
    .with_input("wind", 89)
    .with_input("storage", 55)
    .with_input("demand", 200);
    let receipt = origin.derive(child)?;
    origin.verify()?;
    println!("DERIVATION VERIFIED generation={}", receipt.generation);
    println!("outputs={:?}", receipt.outputs);
    println!("capacity_remaining={}", receipt.remaining_creative_units);

    let mut tampered = origin.identity().pha().clone();
    tampered.embedded_proof.public_inputs["outputs"]["reserve"] = json!(i64::MAX);
    assert!(verify_sfcs_execution_embedding(&tampered).is_err());
    println!("MUTATION REJECTED");

    let checkpoint = origin.receipt().clone();
    let capacity_before = origin.capacity().remaining_units();
    let invalid = OriginSpec::new(
        "rejected-child",
        r#"
            input value
            let zero = 0
            let result = value / zero
            output result
        "#,
    )
    .with_input("value", 7);
    assert!(origin.derive(invalid).is_err());
    assert_eq!(origin.receipt(), &checkpoint);
    assert_eq!(origin.capacity().remaining_units(), capacity_before);
    origin.verify()?;
    println!("FAILED CREATION ROLLED BACK");
    println!("LIVE PROOF COMPLETE");
    Ok(())
}
