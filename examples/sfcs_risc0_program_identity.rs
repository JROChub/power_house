//! Prints the execution identity of a RISC Zero guest binary.

#[cfg(not(feature = "sfcs-risc0"))]
fn main() {
    eprintln!("sfcs_risc0_program_identity requires --features sfcs-risc0");
    std::process::exit(2);
}

#[cfg(feature = "sfcs-risc0")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use power_house::SfcsRisc0PrivateVmProof;
    use risc0_zkvm::compute_image_id;

    let path = std::env::args()
        .nth(1)
        .ok_or("usage: sfcs_risc0_program_identity <guest.bin>")?;
    let program = std::fs::read(path)?;
    let image_id = compute_image_id(&program)?;
    let graph_digest = SfcsRisc0PrivateVmProof::program_graph(&program)?.fractal_digest()?;
    println!("image_id=risc0:{image_id}");
    println!("graph_digest={graph_digest}");
    Ok(())
}
