//! The design philosophy underlying `power_house` is pedagogical, yet mathematically rigorous.
//! Each module encapsulates a discrete concept in modern computational complexity theory,
//! illustrating how modest abstractions compose into a cohesive proof infrastructure.
//!
//! This crate aspires to bridge gaps between theoretical exposition and practical engineering,
//! serving both as a didactic resource and a foundation for future cryptographic research.
//! Sum-check protocol demonstration.
//!
//! This module implements a small demonstration of the sum-check protocol
//! originally introduced by Lund, Fortnow, Karloff and Nisan.  The protocol
//! enables a prover to convince a verifier of the value of a multi-variate
//! polynomial evaluated over all Boolean assignments without revealing the
//! entire computation.  Here we consider the polynomial
//!
//! ```text
//! f(x₁, x₂) = x₁ + x₂ + 2 × x₁ × x₂
//! ```
//!
//! and work over a finite field of prime order.  The prover claims the
//! sum of `f` over the hypercube {0,1}² and the verifier performs a
//! two-round interaction to check the claim.  Our implementation uses
//! deterministic pseudorandomness derived from the transcript to select
//! verifier challenges, yielding a non-interactive variant suitable for
//! embedding into a proof ledger.  The soundness error decreases
//! exponentially in the parameter `k`.

use crate::{field::Field, prng::derive_many_mod_p};
use crate::{MultilinearPolynomial, StreamingPolynomial, Transcript};
use std::sync::Arc;
use std::time::{Duration, Instant};
#[cfg(not(target_arch = "wasm32"))]
use rayon::prelude::*;

/// Evaluates the demo polynomial `f(x₁, x₂) = x₁ + x₂ + 2·x₁·x₂ (mod p)`.
///
/// The parameters `x1` and `x2` may be any integers; they are reduced
/// modulo the field modulus before evaluation.
pub fn f_demo(field: &Field, x1: u64, x2: u64) -> u64 {
    let t1 = field.add(x1 % field.modulus(), x2 % field.modulus());
    // Multiply by 2 (mod p).
    let two = 2 % field.modulus();
    let t2 = field.mul(two, field.mul(x1 % field.modulus(), x2 % field.modulus()));
    field.add(t1, t2)
}

/// Computes the true sum of the demo polynomial over all Boolean inputs.
///
/// Returns \(\sum_{x₁ ∈ {0,1}} \sum_{x₂ ∈ {0,1}} f(x₁, x₂)\) modulo `p`.
pub fn true_sum_demo(field: &Field) -> u64 {
    let mut s = 0;
    for &x1 in &[0u64, 1] {
        for &x2 in &[0u64, 1] {
            s = field.add(s, f_demo(field, x1, x2));
        }
    }
    s
}

/// Represents a claim in the one-shot sum-check protocol.
///
/// A `SumClaim` records the field modulus `p`, the claimed sum of the
/// polynomial, the linear coefficients of the first and second univariate
/// polynomials (`g1` and `g2`), and the number of rounds `k`.  See
/// [`prove_demo`](SumClaim::prove_demo) and [`verify_demo`](SumClaim::verify_demo)
/// for details on how these values are derived and checked.
#[derive(Debug, Clone)]
pub struct SumClaim {
    /// Prime modulus of the field.
    pub p: u64,
    /// Claimed sum of the polynomial over the hypercube.
    pub claimed_sum: u64,
    /// Coefficient `a` of the first polynomial `g1(z) = a·z + b`.
    pub g1_a: u64,
    /// Constant term `b` of the first polynomial `g1`.
    pub g1_b: u64,
    /// Coefficient `a` of the second polynomial `g2(z) = a·z + b`.
    pub g2_a: u64,
    /// Constant term `b` of the second polynomial `g2`.
    pub g2_b: u64,
    /// Number of random checks to perform in the final step.
    pub k: usize,
}

impl SumClaim {
    /// Proves the sum of the demo polynomial without interaction.
    ///
    /// This function constructs an honest claim for the demo polynomial
    /// defined by [`f_demo`](crate::sumcheck::f_demo) over the field with
    /// modulus `field.modulus()`.  It deterministically derives a
    /// randomness seed from the transcript to choose the challenge `r1`
    /// and the final checks `r2_i`.  The soundness error is bounded
    /// by `2^(−k)`, so larger `k` values provide stronger security.
    pub fn prove_demo(field: &Field, k: usize) -> Self {
        let p = field.modulus();
        // Compute the true sum S of f over {0,1}².
        let s = true_sum_demo(field);
        // Compute g1 coefficients by sampling g1 at 0 and 1.
        let g1_0 = f_demo(field, 0, 0).wrapping_add(f_demo(field, 0, 1)) % p;
        let g1_1 = f_demo(field, 1, 0).wrapping_add(f_demo(field, 1, 1)) % p;
        // g1(z) = a·z + b
        let g1_a = field.sub(g1_1, g1_0);
        let g1_b = g1_0;
        // Derive r1 deterministically from the base transcript.
        let base_transcript = [p, s, g1_a, g1_b, 0u64, 0u64, k as u64];
        // Use a domain tag specific to the sum-check protocol.
        let r1_values = derive_many_mod_p(p, b"power_house:v1:sumcheck:r1", &base_transcript, 1);
        let r1 = r1_values[0];
        // Compute S1 = g1(r1) mod p.
        let _s1 = field.add(field.mul(g1_a, r1), g1_b);
        // Compute g2 coefficients by sampling g2 at 0 and 1.
        let g2_0 = f_demo(field, r1, 0);
        let g2_1 = f_demo(field, r1, 1);
        let g2_a = field.sub(g2_1, g2_0);
        let g2_b = g2_0;
        SumClaim {
            p,
            claimed_sum: s,
            g1_a,
            g1_b,
            g2_a,
            g2_b,
            k,
        }
    }

    /// Verifies an honest sum-check claim.
    ///
    /// The verifier reconstructs the challenges `r1` and `r2_i` by
    /// applying the same deterministic derivation used by the prover.  It
    /// then performs the checks specified by the sum-check protocol: the
    /// consistency of `g1(0)+g1(1)`, the consistency of `g2(0)+g2(1)` and
    /// `g1(r1)`, and finally `k` evaluations of `f(r1,r2_i)` against
    /// `g2(r2_i)`.  Returns `true` if all checks pass and `false` otherwise.
    pub fn verify_demo(&self) -> bool {
        let field = Field::new(self.p);
        // Check 1: g1(0) + g1(1) == claimed_sum
        let g1_0 = self.g1_b;
        let g1_1 = field.add(self.g1_a, self.g1_b);
        let lhs1 = field.add(g1_0, g1_1);
        if lhs1 != self.claimed_sum {
            return false;
        }
        // Derive r1 from base transcript.
        let base_transcript = [
            self.p,
            self.claimed_sum,
            self.g1_a,
            self.g1_b,
            0u64,
            0u64,
            self.k as u64,
        ];
        let r1_values =
            derive_many_mod_p(self.p, b"power_house:v1:sumcheck:r1", &base_transcript, 1);
        let r1 = r1_values[0];
        // S1 = g1(r1)
        let s1 = field.add(field.mul(self.g1_a, r1), self.g1_b);
        // Check 2: g2(0) + g2(1) == S1
        let g2_0 = self.g2_b;
        let g2_1 = field.add(self.g2_a, self.g2_b);
        let lhs2 = field.add(g2_0, g2_1);
        if lhs2 != s1 {
            return false;
        }
        // Final: derive r2_i challenges.
        // Transcript includes all public data.
        let transcript = [
            self.p,
            self.claimed_sum,
            self.g1_a,
            self.g1_b,
            self.g2_a,
            self.g2_b,
            self.k as u64,
        ];
        let r2s = derive_many_mod_p(self.p, b"power_house:v1:sumcheck:r2", &transcript, self.k);
        for &r2 in &r2s {
            // Compute g2(r2).
            let left = field.add(field.mul(self.g2_a, r2), self.g2_b);
            // Compute f(r1, r2).
            let right = f_demo(&field, r1, r2);
            if left != right {
                return false;
            }
        }
        true
    }
}

/// Domain tag used for the generalized sum-check Fiat–Shamir transcript.
const GENERAL_SUMCHECK_DOMAIN: &[u8] = b"power_house:v2:sumcheck";

/// Generalized non-interactive sum-check claim for multilinear polynomials.
#[derive(Debug, Clone)]
pub struct GeneralSumClaim {
    /// Prime modulus of the field.
    pub p: u64,
    /// Number of variables in the multilinear polynomial.
    pub num_vars: usize,
    /// Sum of the polynomial over the Boolean hypercube.
    pub claimed_sum: u64,
    /// Sequence of linear polynomial coefficients `(a_i, b_i)` where
    /// `g_i(z) = a_i·z + b_i`.
    pub rounds: Vec<(u64, u64)>,
}

/// Detailed verification transcript for a generalized sum-check claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneralSumTrace {
    /// Fiat–Shamir challenges sampled by the verifier.
    pub challenges: Vec<u64>,
    /// Running sum claims prior to each folding round.
    pub round_sums: Vec<u64>,
    /// Final polynomial evaluation at the derived challenge point.
    pub final_evaluation: u64,
}

/// Complete non-interactive proof with auxiliary transcript data.
#[derive(Debug, Clone)]
pub struct GeneralSumProof {
    /// Honest claim produced by the prover.
    pub claim: GeneralSumClaim,
    /// Fiat–Shamir challenges reused by the verifier.
    pub challenges: Vec<u64>,
    /// Running sums recorded before each folding step.
    pub round_sums: Vec<u64>,
    /// Final evaluation of the polynomial at the verifier's random point.
    pub final_evaluation: u64,
}

/// Timing information collected while producing a generalized sum-check proof.
#[derive(Debug, Clone)]
pub struct ProofStats {
    /// Total wall-clock time taken to produce the proof.
    pub total_duration: Duration,
    /// Duration of each folding round.
    pub round_durations: Vec<Duration>,
}

/// A single link in a chained proof, referencing its parent evaluation.
#[derive(Debug, Clone)]
pub struct ChainLink {
    /// Final evaluation of the parent proof, if any.
    pub parent_final: Option<u64>,
    /// Proof for the current polynomial.
    pub proof: GeneralSumProof,
}

/// Sequence of generalized proofs where each link commits to its predecessor.
#[derive(Debug, Clone)]
pub struct ChainedSumProof {
    links: Vec<ChainLink>,
}

impl GeneralSumClaim {
    /// Constructs a non-interactive sum-check proof for an arbitrary multilinear polynomial.
    pub fn prove(poly: &MultilinearPolynomial, field: &Field) -> Self {
        GeneralSumProof::prove(poly, field).claim
    }

    /// Constructs a sum-check proof using a streaming evaluator without materialising the full table.
    pub fn prove_streaming<F>(num_vars: usize, field: &Field, evaluator: F) -> Self
    where
        F: Fn(usize) -> u64 + Send + Sync + 'static,
    {
        GeneralSumProof::prove_streaming(num_vars, field, evaluator).claim
    }

    /// Streaming variant that reuses an existing streaming polynomial.
    pub fn prove_streaming_poly(poly: &StreamingPolynomial, field: &Field) -> Self {
        GeneralSumProof::prove_streaming_poly(poly, field).claim
    }

    /// Proves the sum alongside transcript metadata.
    pub fn prove_with_trace(poly: &MultilinearPolynomial, field: &Field) -> GeneralSumProof {
        GeneralSumProof::prove(poly, field)
    }

    /// Verifies a generalized sum-check claim against the provided polynomial.
    pub fn verify(&self, poly: &MultilinearPolynomial, field: &Field) -> bool {
        self.verify_with_trace(poly, field).is_some()
    }

    /// Streaming variant of [`Self::verify`].
    pub fn verify_streaming(&self, poly: &StreamingPolynomial, field: &Field) -> bool {
        self.verify_streaming_with_trace(poly, field).is_some()
    }

    /// Performs verification and returns the reconstructed transcript when successful.
    pub fn verify_with_trace(
        &self,
        poly: &MultilinearPolynomial,
        field: &Field,
    ) -> Option<GeneralSumTrace> {
        verify_general_sum(self, poly, field)
    }

    /// Streaming variant of [`Self::verify_with_trace`].
    pub fn verify_streaming_with_trace(
        &self,
        poly: &StreamingPolynomial,
        field: &Field,
    ) -> Option<GeneralSumTrace> {
        verify_general_sum_streaming(self, poly, field)
    }
}

impl GeneralSumProof {
    /// Produces a non-interactive proof for the given polynomial.
    pub fn prove(poly: &MultilinearPolynomial, field: &Field) -> Self {
        Self::prove_with_stats(poly, field).0
    }

    /// Produces a proof using a streaming evaluator without retaining the full hypercube.
    pub fn prove_streaming<F>(num_vars: usize, field: &Field, evaluator: F) -> Self
    where
        F: Fn(usize) -> u64 + Send + Sync + 'static,
    {
        Self::prove_streaming_with_stats(num_vars, field, evaluator).0
    }

    /// Streaming variant that reuses an existing streaming polynomial.
    pub fn prove_streaming_poly(poly: &StreamingPolynomial, field: &Field) -> Self {
        Self::prove_streaming_with_stats_poly(poly, field).0
    }

    /// Produces a proof together with per-round timing information.
    pub fn prove_with_stats(poly: &MultilinearPolynomial, field: &Field) -> (Self, ProofStats) {
        let p = field.modulus();
        let num_vars = poly.num_vars();
        let mut layer = poly.evaluations_mod_p(field);
        let claimed_sum = poly.sum_over_hypercube(field);

        let mut transcript = Transcript::new(GENERAL_SUMCHECK_DOMAIN);
        transcript.append(p);
        transcript.append(num_vars as u64);
        transcript.append(claimed_sum);

        let total_start = Instant::now();
        let mut rounds = Vec::with_capacity(num_vars);
        let mut challenges = Vec::with_capacity(num_vars);
        let mut round_sums = Vec::with_capacity(num_vars);
        let mut round_durations = Vec::with_capacity(num_vars);

        let mut running_sum = claimed_sum;

        for _ in 0..num_vars {
            round_sums.push(running_sum);
            let round_start = Instant::now();

            let mut g0_sum = 0u64;
            let mut g1_sum = 0u64;
            for chunk in layer.chunks(2) {
                let v0 = chunk[0];
                let v1 = chunk[1];
                g0_sum = field.add(g0_sum, v0);
                g1_sum = field.add(g1_sum, v1);
            }
            let a = field.sub(g1_sum, g0_sum);
            let b = g0_sum;
            rounds.push((a, b));

            transcript.append(a);
            transcript.append(b);
            let r = transcript.challenge(field);
            challenges.push(r);

            let mut next_layer = Vec::with_capacity(layer.len() / 2);
            let mut next_sum = 0u64;
            for chunk in layer.chunks(2) {
                let v0 = chunk[0];
                let v1 = chunk[1];
                let diff = field.sub(v1, v0);
                let eval = field.add(field.mul(diff, r), v0);
                next_sum = field.add(next_sum, eval);
                next_layer.push(eval);
            }
            layer = next_layer;
            running_sum = next_sum;
            round_durations.push(round_start.elapsed());
        }

        assert_eq!(
            layer.len(),
            1,
            "folding a multilinear polynomial must end with a single value"
        );
        let final_evaluation = layer[0];

        let claim = GeneralSumClaim {
            p,
            num_vars,
            claimed_sum,
            rounds,
        };

        let proof = GeneralSumProof {
            claim,
            challenges: challenges.clone(),
            round_sums: round_sums.clone(),
            final_evaluation,
        };
        let stats = ProofStats {
            total_duration: total_start.elapsed(),
            round_durations,
        };
        (proof, stats)
    }

    /// Streaming variant of [`Self::prove_with_stats`].
    pub fn prove_streaming_with_stats_poly(
        poly: &StreamingPolynomial,
        field: &Field,
    ) -> (Self, ProofStats) {
        assert_eq!(poly.modulus(), field.modulus(), "field mismatch");
        prove_streaming_with_stats_inner(poly.num_vars(), field, poly.evaluator())
    }

    /// Streaming variant of [`Self::prove_with_stats`] that accepts an evaluator closure.
    pub fn prove_streaming_with_stats<F>(
        num_vars: usize,
        field: &Field,
        evaluator: F,
    ) -> (Self, ProofStats)
    where
        F: Fn(usize) -> u64 + Send + Sync + 'static,
    {
        let eval: Arc<dyn Fn(usize) -> u64 + Send + Sync> = Arc::new(evaluator);
        prove_streaming_with_stats_inner(num_vars, field, eval)
    }

    /// Verifies the proof against the polynomial.
    pub fn verify(&self, poly: &MultilinearPolynomial, field: &Field) -> bool {
        self.verify_with_trace(poly, field).is_some()
    }

    /// Verifies the proof and returns the reconstructed transcript if successful.
    pub fn verify_with_trace(
        &self,
        poly: &MultilinearPolynomial,
        field: &Field,
    ) -> Option<GeneralSumTrace> {
        let trace = self.claim.verify_with_trace(poly, field)?;
        if trace.challenges != self.challenges
            || trace.round_sums != self.round_sums
            || trace.final_evaluation != self.final_evaluation
        {
            return None;
        }
        Some(trace)
    }

    /// Streaming variant of [`Self::verify`].
    pub fn verify_streaming(&self, poly: &StreamingPolynomial, field: &Field) -> bool {
        self.verify_streaming_with_trace(poly, field).is_some()
    }

    /// Streaming variant of [`Self::verify_with_trace`].
    pub fn verify_streaming_with_trace(
        &self,
        poly: &StreamingPolynomial,
        field: &Field,
    ) -> Option<GeneralSumTrace> {
        let trace = self.claim.verify_streaming_with_trace(poly, field)?;
        if trace.challenges != self.challenges
            || trace.round_sums != self.round_sums
            || trace.final_evaluation != self.final_evaluation
        {
            return None;
        }
        Some(trace)
    }
}

fn prove_streaming_with_stats_inner(
    num_vars: usize,
    field: &Field,
    evaluator: Arc<dyn Fn(usize) -> u64 + Send + Sync>,
) -> (GeneralSumProof, ProofStats) {
    assert!(num_vars >= 1, "num_vars must be at least 1");
    let p = field.modulus();
    let size = 1usize << num_vars;
    let field = *field;
    let use_parallel = {
        #[cfg(not(target_arch = "wasm32"))]
        {
            const PARALLEL_THRESHOLD: usize = 1 << 16;
            size >= PARALLEL_THRESHOLD && rayon::current_num_threads() > 1
        }
        #[cfg(target_arch = "wasm32")]
        {
            false
        }
    };

    let mut transcript = Transcript::new(GENERAL_SUMCHECK_DOMAIN);
    transcript.append(p);
    transcript.append(num_vars as u64);

    let mut round_sums = Vec::with_capacity(num_vars);
    let mut rounds = Vec::with_capacity(num_vars);
    let mut challenges = Vec::with_capacity(num_vars);
    let mut round_durations = Vec::with_capacity(num_vars);

    let total_start = Instant::now();

    let (claimed_sum, g0_sum, g1_sum) = if use_parallel {
        #[cfg(not(target_arch = "wasm32"))]
        {
            (0..size / 2)
                .into_par_iter()
                .map(|pair| {
                    let idx = pair * 2;
                    let v0 = evaluator(idx) % p;
                    let v1 = evaluator(idx + 1) % p;
                    (v0, v1, field.add(v0, v1))
                })
                .reduce(
                    || (0u64, 0u64, 0u64),
                    |acc, (v0, v1, sum)| {
                        (
                            field.add(acc.0, v0),
                            field.add(acc.1, v1),
                            field.add(acc.2, sum),
                        )
                    },
                )
        }
        #[cfg(target_arch = "wasm32")]
        {
            (0u64, 0u64, 0u64)
        }
    } else {
        let mut claimed_sum = 0u64;
        let mut g0_sum = 0u64;
        let mut g1_sum = 0u64;
        for idx in (0..size).step_by(2) {
            let v0 = evaluator(idx) % p;
            let v1 = evaluator(idx + 1) % p;
            g0_sum = field.add(g0_sum, v0);
            g1_sum = field.add(g1_sum, v1);
            claimed_sum = field.add(claimed_sum, field.add(v0, v1));
        }
        (claimed_sum, g0_sum, g1_sum)
    };
    transcript.append(claimed_sum);
    round_sums.push(claimed_sum);

    let round_start = Instant::now();
    let first_a = field.sub(g1_sum, g0_sum);
    let first_b = g0_sum;
    rounds.push((first_a, first_b));
    transcript.append(first_a);
    transcript.append(first_b);
    let mut r = transcript.challenge(&field);
    challenges.push(r);

    let (mut layer, mut current_sum) = if use_parallel {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let layer: Vec<u64> = (0..size / 2)
                .into_par_iter()
                .map(|pair| {
                    let idx = pair * 2;
                    let v0 = evaluator(idx) % p;
                    let v1 = evaluator(idx + 1) % p;
                    let diff = field.sub(v1, v0);
                    field.add(field.mul(diff, r), v0)
                })
                .collect();
            let current_sum = layer
                .par_iter()
                .cloned()
                .reduce(|| 0u64, |acc, v| field.add(acc, v));
            (layer, current_sum)
        }
        #[cfg(target_arch = "wasm32")]
        {
            (Vec::new(), 0u64)
        }
    } else {
        let mut layer = Vec::with_capacity(size / 2);
        let mut current_sum = 0u64;
        for idx in (0..size).step_by(2) {
            let v0 = evaluator(idx) % p;
            let v1 = evaluator(idx + 1) % p;
            let diff = field.sub(v1, v0);
            let val = field.add(field.mul(diff, r), v0);
            current_sum = field.add(current_sum, val);
            layer.push(val);
        }
        (layer, current_sum)
    };
    round_durations.push(round_start.elapsed());

    for _round in 1..num_vars {
        round_sums.push(current_sum);
        let round_start = Instant::now();
        let use_parallel_layer = {
            #[cfg(not(target_arch = "wasm32"))]
            {
                const PARALLEL_LAYER_THRESHOLD: usize = 1 << 14;
                use_parallel && layer.len() >= PARALLEL_LAYER_THRESHOLD
            }
            #[cfg(target_arch = "wasm32")]
            {
                false
            }
        };
        let (g0_sum, g1_sum) = if use_parallel_layer {
            #[cfg(not(target_arch = "wasm32"))]
            {
                layer
                    .par_chunks(2)
                    .map(|chunk| (chunk[0], chunk[1]))
                    .reduce(
                        || (0u64, 0u64),
                        |acc, (v0, v1)| (field.add(acc.0, v0), field.add(acc.1, v1)),
                    )
            }
            #[cfg(target_arch = "wasm32")]
            {
                (0u64, 0u64)
            }
        } else {
            let mut g0_sum = 0u64;
            let mut g1_sum = 0u64;
            for chunk in layer.chunks(2) {
                g0_sum = field.add(g0_sum, chunk[0]);
                g1_sum = field.add(g1_sum, chunk[1]);
            }
            (g0_sum, g1_sum)
        };
        let a = field.sub(g1_sum, g0_sum);
        let b = g0_sum;
        rounds.push((a, b));
        transcript.append(a);
        transcript.append(b);
        r = transcript.challenge(&field);
        challenges.push(r);

        let (next_layer, next_sum) = if use_parallel_layer {
            #[cfg(not(target_arch = "wasm32"))]
            {
                let next_layer: Vec<u64> = layer
                    .par_chunks(2)
                    .map(|chunk| {
                        let v0 = chunk[0];
                        let v1 = chunk[1];
                        let diff = field.sub(v1, v0);
                        field.add(field.mul(diff, r), v0)
                    })
                    .collect();
                let next_sum = next_layer
                    .par_iter()
                    .cloned()
                    .reduce(|| 0u64, |acc, v| field.add(acc, v));
                (next_layer, next_sum)
            }
            #[cfg(target_arch = "wasm32")]
            {
                (Vec::new(), 0u64)
            }
        } else {
            let mut next_layer = Vec::with_capacity(layer.len() / 2);
            let mut next_sum = 0u64;
            for chunk in layer.chunks(2) {
                let v0 = chunk[0];
                let v1 = chunk[1];
                let diff = field.sub(v1, v0);
                let val = field.add(field.mul(diff, r), v0);
                next_sum = field.add(next_sum, val);
                next_layer.push(val);
            }
            (next_layer, next_sum)
        };
        layer = next_layer;
        current_sum = next_sum;
        round_durations.push(round_start.elapsed());
    }

    let final_evaluation = layer[0];
    let proof = GeneralSumProof {
        claim: GeneralSumClaim {
            p,
            num_vars,
            claimed_sum,
            rounds,
        },
        challenges,
        round_sums,
        final_evaluation,
    };
    let stats = ProofStats {
        total_duration: total_start.elapsed(),
        round_durations,
    };
    (proof, stats)
}

impl ChainedSumProof {
    /// Builds a chained proof where each claimed sum must equal the previous final evaluation.
    ///
    /// # Panics
    ///
    /// Panics if any polynomial's claimed sum does not match its parent final evaluation.
    pub fn prove(polynomials: &[MultilinearPolynomial], field: &Field) -> Self {
        let mut links = Vec::with_capacity(polynomials.len());
        let mut previous_final: Option<u64> = None;
        for poly in polynomials {
            let parent_for_this = previous_final;
            let proof = GeneralSumProof::prove(poly, field);
            if let Some(expected_sum) = parent_for_this {
                if field.sub(proof.claim.claimed_sum, expected_sum) != 0 {
                    panic!(
                        "chained proof mismatch: expected sum {} but found {}",
                        expected_sum, proof.claim.claimed_sum
                    );
                }
            }
            previous_final = Some(proof.final_evaluation);
            links.push(ChainLink {
                parent_final: parent_for_this,
                proof,
            });
        }
        Self { links }
    }

    /// Produces a chain along with timing measurements for each proof.
    pub fn prove_with_stats(
        polynomials: &[MultilinearPolynomial],
        field: &Field,
    ) -> (Self, Vec<ProofStats>) {
        let mut stats = Vec::with_capacity(polynomials.len());
        let mut links = Vec::with_capacity(polynomials.len());
        let mut previous_final: Option<u64> = None;
        for poly in polynomials {
            let parent_for_this = previous_final;
            let (proof, proof_stats) = GeneralSumProof::prove_with_stats(poly, field);
            stats.push(proof_stats);
            if let Some(expected_sum) = parent_for_this {
                if field.sub(proof.claim.claimed_sum, expected_sum) != 0 {
                    panic!(
                        "chained proof mismatch: expected sum {} but found {}",
                        expected_sum, proof.claim.claimed_sum
                    );
                }
            }
            previous_final = Some(proof.final_evaluation);
            links.push(ChainLink {
                parent_final: parent_for_this,
                proof,
            });
        }
        (Self { links }, stats)
    }

    /// Returns the recorded links.
    pub fn links(&self) -> &[ChainLink] {
        &self.links
    }

    /// Returns a mutable view of the recorded links.
    pub fn links_mut(&mut self) -> &mut [ChainLink] {
        &mut self.links
    }

    /// Returns the number of proofs in the chain.
    pub fn len(&self) -> usize {
        self.links.len()
    }

    /// Returns true if the chain has no proofs.
    pub fn is_empty(&self) -> bool {
        self.links.is_empty()
    }

    /// Verifies the entire chain and returns the per-proof traces on success.
    pub fn verify_with_traces(
        &self,
        polynomials: &[MultilinearPolynomial],
        field: &Field,
    ) -> Option<Vec<GeneralSumTrace>> {
        if self.links.len() != polynomials.len() {
            return None;
        }
        let mut traces = Vec::with_capacity(self.links.len());
        let mut previous_final: Option<u64> = None;
        for (link, poly) in self.links.iter().zip(polynomials) {
            if link.parent_final != previous_final {
                return None;
            }
            let trace = link.proof.verify_with_trace(poly, field)?;
            if let Some(expected_sum) = previous_final {
                if field.sub(link.proof.claim.claimed_sum, expected_sum) != 0 {
                    return None;
                }
            }
            previous_final = Some(trace.final_evaluation);
            traces.push(trace);
        }
        Some(traces)
    }

    /// Verifies the chain of proofs.
    pub fn verify(&self, polynomials: &[MultilinearPolynomial], field: &Field) -> bool {
        self.verify_with_traces(polynomials, field).is_some()
    }
}

fn verify_general_sum(
    claim: &GeneralSumClaim,
    poly: &MultilinearPolynomial,
    field: &Field,
) -> Option<GeneralSumTrace> {
    if claim.p != field.modulus() {
        return None;
    }
    if claim.num_vars != poly.num_vars() {
        return None;
    }
    if claim.rounds.len() != claim.num_vars {
        return None;
    }

    let mut transcript = Transcript::new(GENERAL_SUMCHECK_DOMAIN);
    transcript.append(claim.p);
    transcript.append(claim.num_vars as u64);
    transcript.append(claim.claimed_sum);

    let mut layer = poly.evaluations_mod_p(field);
    let mut running_claim = claim.claimed_sum;
    let mut challenges = Vec::with_capacity(claim.num_vars);
    let mut round_sums = Vec::with_capacity(claim.num_vars);

    for &(a, b) in &claim.rounds {
        round_sums.push(running_claim);
        let sum_check = field.add(b, field.add(a, b));
        if sum_check != running_claim {
            return None;
        }

        transcript.append(a);
        transcript.append(b);
        let r = transcript.challenge(field);
        challenges.push(r);

        let mut next_layer = Vec::with_capacity(layer.len() / 2);
        let mut next_sum = 0u64;
        for chunk in layer.chunks(2) {
            let v0 = chunk[0];
            let v1 = chunk[1];
            let diff = field.sub(v1, v0);
            let eval = field.add(field.mul(diff, r), v0);
            next_sum = field.add(next_sum, eval);
            next_layer.push(eval);
        }
        layer = next_layer;
        running_claim = next_sum;
    }

    if layer.len() != 1 {
        return None;
    }

    let final_evaluation = poly.evaluate(field, &challenges);
    if final_evaluation != running_claim {
        return None;
    }

    Some(GeneralSumTrace {
        challenges,
        round_sums,
        final_evaluation,
    })
}

fn verify_general_sum_streaming(
    claim: &GeneralSumClaim,
    poly: &StreamingPolynomial,
    field: &Field,
) -> Option<GeneralSumTrace> {
    if claim.p != field.modulus() || claim.p != poly.modulus() {
        return None;
    }
    if claim.num_vars != poly.num_vars() || claim.rounds.len() != claim.num_vars {
        return None;
    }

    let p = claim.p;
    let num_vars = claim.num_vars;
    let size = 1usize << num_vars;
    let eval = poly.evaluator();
    let mut transcript = Transcript::new(GENERAL_SUMCHECK_DOMAIN);
    transcript.append(p);
    transcript.append(num_vars as u64);
    transcript.append(claim.claimed_sum);

    let mut round_sums = Vec::with_capacity(num_vars);
    let mut challenges = Vec::with_capacity(num_vars);

    let mut computed_sum = 0u64;
    let mut g0_sum = 0u64;
    let mut g1_sum = 0u64;
    for idx in (0..size).step_by(2) {
        let v0 = eval(idx) % p;
        let v1 = eval(idx + 1) % p;
        g0_sum = field.add(g0_sum, v0);
        g1_sum = field.add(g1_sum, v1);
        computed_sum = field.add(computed_sum, field.add(v0, v1));
    }
    if computed_sum != claim.claimed_sum {
        return None;
    }
    round_sums.push(computed_sum);

    let mut layer = Vec::with_capacity(size / 2);
    let mut running_sum = computed_sum;

    for (round_idx, &(a, b)) in claim.rounds.iter().enumerate() {
        if b % p != g0_sum || field.sub(g1_sum, g0_sum) != a {
            return None;
        }
        transcript.append(a);
        transcript.append(b);
        let r = transcript.challenge(field);
        challenges.push(r);

        let mut next_layer = Vec::with_capacity(if round_idx == 0 {
            size / 2
        } else {
            layer.len() / 2
        });
        let mut next_sum = 0u64;
        if round_idx == 0 {
            for idx in (0..size).step_by(2) {
                let v0 = eval(idx) % p;
                let v1 = eval(idx + 1) % p;
                let diff = field.sub(v1, v0);
                let val = field.add(field.mul(diff, r), v0);
                next_sum = field.add(next_sum, val);
                next_layer.push(val);
            }
        } else {
            for chunk in layer.chunks(2) {
                let v0 = chunk[0];
                let v1 = chunk[1];
                let diff = field.sub(v1, v0);
                let val = field.add(field.mul(diff, r), v0);
                next_sum = field.add(next_sum, val);
                next_layer.push(val);
            }
        }
        layer = next_layer;
        running_sum = next_sum;
        if round_idx + 1 < num_vars {
            round_sums.push(running_sum);
            g0_sum = 0u64;
            g1_sum = 0u64;
            for chunk in layer.chunks(2) {
                g0_sum = field.add(g0_sum, chunk[0]);
                g1_sum = field.add(g1_sum, chunk[1]);
            }
        }
    }

    if layer.len() != 1 {
        return None;
    }
    let final_evaluation = layer[0];
    if final_evaluation != running_sum {
        return None;
    }

    Some(GeneralSumTrace {
        challenges,
        round_sums,
        final_evaluation,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Field;

    #[test]
    fn test_demo_true_sum() {
        let field = Field::new(101);
        let sum = true_sum_demo(&field);
        // Manually compute the sum: f(0,0)=0, f(0,1)=1, f(1,0)=1, f(1,1)=4 => sum=6
        assert_eq!(sum, 6);
    }

    #[test]
    fn test_prove_and_verify() {
        let field = Field::new(101);
        let claim = SumClaim::prove_demo(&field, 8);
        assert!(claim.verify_demo());
    }

    #[test]
    fn test_cheating_prover_fails() {
        let field = Field::new(101);
        // Build an honest claim first.
        let honest = SumClaim::prove_demo(&field, 4);
        // Attempt to cheat: modify g2_a by adding 1.
        let mut forged = honest.clone();
        forged.g2_a = field.add(forged.g2_a, 1);
        // Adjust g2_b so that g2(0)+g2(1) still sums to S1.
        let base_transcript = [
            forged.p,
            forged.claimed_sum,
            forged.g1_a,
            forged.g1_b,
            0u64,
            0u64,
            forged.k as u64,
        ];
        let r1 = derive_many_mod_p(forged.p, b"power_house:v1:sumcheck:r1", &base_transcript, 1)[0];
        let s1 = field.add(field.mul(forged.g1_a, r1), forged.g1_b);
        // Solve for b: a*r + b = t => r irrelevant here; ensure g2(0)+g2(1) = s1
        // g2(0) = b, g2(1) = a + b => sum = a + 2b.  We know desired sum s1.
        // 2b + a = s1 => b = (s1 - a) / 2.
        let inv2 = field.inv(2);
        forged.g2_b = field.mul(field.sub(s1, forged.g2_a), inv2);
        assert!(!forged.verify_demo());
    }

    fn sample_poly(field: &Field) -> MultilinearPolynomial {
        let mut evals = Vec::with_capacity(8);
        for x2 in 0..=1u64 {
            for x1 in 0..=1u64 {
                for x0 in 0..=1u64 {
                    let mut val = 0;
                    val = field.add(val, x0);
                    val = field.add(val, field.mul(2, x1));
                    val = field.add(val, field.mul(3, x2));
                    let triple = field.mul(x0, field.mul(x1, x2));
                    val = field.add(val, field.mul(5, triple));
                    evals.push(val);
                }
            }
        }
        MultilinearPolynomial::from_evaluations(3, evals)
    }

    #[test]
    fn test_general_sumcheck_prove_verify() {
        let field = Field::new(101);
        let poly = sample_poly(&field);
        let claim = GeneralSumClaim::prove(&poly, &field);
        assert!(claim.verify(&poly, &field));
    }

    #[test]
    fn test_general_sumcheck_rejects_tampering() {
        let field = Field::new(101);
        let poly = sample_poly(&field);
        let mut claim = GeneralSumClaim::prove(&poly, &field);
        assert!(claim.verify(&poly, &field));
        // Tamper with the first round coefficient.
        if let Some((a, b)) = claim.rounds.get_mut(0) {
            *a = field.add(*a, 1);
            *b = field.add(*b, 1);
        }
        assert!(!claim.verify(&poly, &field));
    }

    #[test]
    fn test_general_sumproof_trace_matches() {
        let field = Field::new(101);
        let poly = sample_poly(&field);
        let proof = GeneralSumProof::prove(&poly, &field);
        let trace = proof
            .verify_with_trace(&poly, &field)
            .expect("proof should verify");
        assert_eq!(trace.challenges, proof.challenges);
        assert_eq!(trace.round_sums, proof.round_sums);
        assert_eq!(trace.final_evaluation, proof.final_evaluation);
    }

    #[test]
    fn test_general_sumproof_stats() {
        let field = Field::new(101);
        let poly = sample_poly(&field);
        let (_proof, stats) = GeneralSumProof::prove_with_stats(&poly, &field);
        assert_eq!(stats.round_durations.len(), poly.num_vars());
    }

    #[test]
    fn test_streaming_matches_standard() {
        let field = Field::new(101);
        let poly = sample_poly(&field);
        let evals = poly.evaluations().to_vec();
        let num_vars = poly.num_vars();
        let streaming_poly =
            StreamingPolynomial::new(num_vars, field.modulus(), move |idx| evals[idx]);
        let (streaming, _) =
            GeneralSumProof::prove_streaming_with_stats_poly(&streaming_poly, &field);
        let standard = GeneralSumProof::prove(&poly, &field);
        assert_eq!(streaming.claim.rounds, standard.claim.rounds);
        assert_eq!(streaming.final_evaluation, standard.final_evaluation);
        assert!(streaming.verify_streaming(&streaming_poly, &field));
    }

    fn sample_poly_highdim(field: &Field) -> MultilinearPolynomial {
        let mut evals = Vec::with_capacity(32);
        for x4 in 0..=1u64 {
            for x3 in 0..=1u64 {
                for x2 in 0..=1u64 {
                    for x1 in 0..=1u64 {
                        for x0 in 0..=1u64 {
                            let vars = [x0, x1, x2, x3, x4];
                            let mut acc = 1u64;
                            let lin_coefs = [3u64, 5, 7, 11, 13];
                            for (coef, &var) in lin_coefs.iter().zip(vars.iter()) {
                                acc = field.add(acc, field.mul(*coef, var));
                            }
                            // Couple interactions for additional structure.
                            let pair_coefs = [(0usize, 1usize, 17u64), (1, 2, 19), (3, 4, 23)];
                            for &(i, j, coef) in &pair_coefs {
                                let pair = field.mul(vars[i], vars[j]);
                                acc = field.add(acc, field.mul(coef, pair));
                            }
                            // Triple interaction term.
                            let triple = field.mul(vars[0], field.mul(vars[2], vars[4]));
                            acc = field.add(acc, field.mul(29, triple));
                            evals.push(acc);
                        }
                    }
                }
            }
        }
        MultilinearPolynomial::from_evaluations(5, evals)
    }

    #[test]
    fn test_general_sumcheck_highdimensional() {
        let field = Field::new(149);
        let poly = sample_poly_highdim(&field);
        let claim = GeneralSumClaim::prove(&poly, &field);
        assert!(claim.verify(&poly, &field));
    }

    #[test]
    fn test_chained_sum_proof_roundtrip() {
        let field = Field::new(197);
        let poly_a = sample_poly(&field);
        let first = GeneralSumProof::prove(&poly_a, &field);
        let poly_b = constant_polynomial(first.final_evaluation, 4, &field);
        let second = GeneralSumProof::prove(&poly_b, &field);
        let poly_c = constant_polynomial(second.final_evaluation, 3, &field);
        let polynomials = vec![poly_a.clone(), poly_b.clone(), poly_c.clone()];
        let (chain, stats) = ChainedSumProof::prove_with_stats(&polynomials, &field);
        assert_eq!(stats.len(), polynomials.len());
        assert!(chain.verify(&polynomials, &field));
    }

    #[test]
    fn test_chained_sum_proof_detects_tampering() {
        let field = Field::new(211);
        let poly_a = sample_poly(&field);
        let first = GeneralSumProof::prove(&poly_a, &field);
        let poly_b = constant_polynomial(first.final_evaluation, 4, &field);
        let polynomials = vec![poly_a.clone(), poly_b.clone()];
        let (mut chain, _stats) = ChainedSumProof::prove_with_stats(&polynomials, &field);
        if let Some(link) = chain.links_mut().get_mut(1) {
            if let Some(parent) = link.parent_final {
                link.parent_final = Some(field.add(parent, 1));
            }
        }
        assert!(!chain.verify(&polynomials, &field));
    }

    fn constant_polynomial(
        target_sum: u64,
        num_vars: usize,
        field: &Field,
    ) -> MultilinearPolynomial {
        let points = 1usize << num_vars;
        let inv_points = field.inv(points as u64 % field.modulus());
        let constant = field.mul(target_sum % field.modulus(), inv_points);
        MultilinearPolynomial::from_evaluations(num_vars, vec![constant; points])
    }
}
