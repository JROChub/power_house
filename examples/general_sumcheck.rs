use power_house::{Field, GeneralSumClaim, MultilinearPolynomial};

fn main() {
    let field = Field::new(97);
    // Define a 3-variable multilinear polynomial via its evaluations on {0,1}^3.
    // Ordering: x0 toggles fastest, followed by x1, then x2.
    let mut evals = Vec::with_capacity(8);
    for x2 in 0..=1u64 {
        for x1 in 0..=1u64 {
            for x0 in 0..=1u64 {
                let mut val = 0;
                val = field.add(val, x0);
                val = field.add(val, field.mul(4, x1));
                val = field.add(val, field.mul(7, x2));
                // Add a triple interaction term.
                let triple = field.mul(x0, field.mul(x1, x2));
                val = field.add(val, field.mul(9, triple));
                evals.push(val);
            }
        }
    }
    let poly = MultilinearPolynomial::from_evaluations(3, evals);
    let claim = GeneralSumClaim::prove(&poly, &field);
    if claim.verify(&poly, &field) {
        println!(
            "Sum-check verified: sum = {} over {} variables.",
            claim.claimed_sum, claim.num_vars
        );
    } else {
        eprintln!("Verification failed.");
        std::process::exit(1);
    }
}
