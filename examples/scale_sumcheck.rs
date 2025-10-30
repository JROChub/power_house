use power_house::{Field, GeneralSumProof, ProofStats};
use std::fs::File;
use std::io::Write;

fn make_evaluator(num_vars: usize, modulus: u64) -> impl Fn(usize) -> u64 + Send + Sync + 'static {
    move |idx: usize| {
        let mut acc = (idx as u64) % modulus;
        for bit in 0..num_vars {
            let bit_value = ((idx >> bit) & 1) as u64;
            if bit_value == 0 {
                continue;
            }
            let coef = ((bit as u64 + 3).pow(2)) % modulus;
            acc = (acc + coef) % modulus;
        }
        for bit in 0..num_vars.saturating_sub(1) {
            let a = ((idx >> bit) & 1) as u64;
            let b = ((idx >> (bit + 1)) & 1) as u64;
            if a == 0 || b == 0 {
                continue;
            }
            let coef = (17 + (bit as u64 * 5)) % modulus;
            acc = (acc + coef) % modulus;
        }
        if num_vars >= 3 {
            let a = ((idx >> 0) & 1) as u64;
            let b = ((idx >> 1) & 1) as u64;
            let c = ((idx >> 2) & 1) as u64;
            if a == 1 && b == 1 && c == 1 {
                acc = (acc + 29) % modulus;
            }
        }
        acc % modulus
    }
}

fn ms(duration: &std::time::Duration) -> f64 {
    duration.as_secs_f64() * 1_000.0
}

fn summarize(stats: &ProofStats) -> (f64, f64, f64) {
    if stats.round_durations.is_empty() {
        return (0.0, 0.0, 0.0);
    }
    let total = ms(&stats.total_duration);
    let max = stats.round_durations.iter().map(ms).fold(0.0f64, f64::max);
    let mean = total / (stats.round_durations.len() as f64);
    (total, mean, max)
}

fn main() {
    let field = Field::new(257);
    let args: Vec<String> = std::env::args().collect();
    let max_dim = args.get(1).and_then(|s| s.parse::<usize>().ok());
    let default_dims = [8usize, 10, 12, 14, 16, 18];
    let dimensions: Vec<usize> = match max_dim {
        Some(m) => (8..=m).step_by(2).collect(),
        None => default_dims.to_vec(),
    };
    if dimensions.is_empty() {
        eprintln!("No dimensions selected; provide a max >= 8.");
        return;
    }
    let mut rows = Vec::new();
    println!(
        "{:>5} | {:>10} | {:>10} | {:>10} | {:>12} | {:>12}",
        "vars", "2^vars", "total(ms)", "avg(ms)", "max_round(ms)", "final_eval"
    );
    println!("{}", "-".repeat(70));
    for &vars in &dimensions {
        let evaluator = make_evaluator(vars, field.modulus());
        let (proof, stats) = GeneralSumProof::prove_streaming_with_stats(vars, &field, evaluator);
        let (total_ms, avg_ms, max_round_ms) = summarize(&stats);
        let size = 1usize << vars;
        rows.push((
            vars,
            size,
            total_ms,
            avg_ms,
            max_round_ms,
            proof.final_evaluation,
        ));
        println!(
            "{:>5} | {:>10} | {:>10.3} | {:>10.3} | {:>12.3} | {:>12}",
            vars, size, total_ms, avg_ms, max_round_ms, proof.final_evaluation
        );
    }

    if let Ok(path) = std::env::var("POWER_HOUSE_SCALE_OUT") {
        let mut file = File::create(&path).expect("create csv output");
        writeln!(
            file,
            "vars,size,total_ms,avg_ms,max_round_ms,final_evaluation"
        )
        .expect("write csv header");
        for (vars, size, total_ms, avg_ms, max_round_ms, final_eval) in rows {
            writeln!(
                file,
                "{vars},{size},{total_ms:.6},{avg_ms:.6},{max_round_ms:.6},{final_eval}"
            )
            .expect("write csv row");
        }
        println!("CSV exported to {path}");
    }
}
