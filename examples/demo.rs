use power_house::{Field, SumClaim};

fn main() {
    let field = Field::new(101);
    let claim = SumClaim::prove_demo(&field, 8);
    if claim.verify_demo() {
        println!("Claim verified successfully.");
    } else {
        eprintln!("Claim verification failed.");
        std::process::exit(1);
    }
}
