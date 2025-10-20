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
        let base_transcript = [self.p, self.claimed_sum, self.g1_a, self.g1_b, 0u64, 0u64, self.k as u64];
        let r1_values = derive_many_mod_p(self.p, b"power_house:v1:sumcheck:r1", &base_transcript, 1);
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
        let transcript = [self.p, self.claimed_sum, self.g1_a, self.g1_b, self.g2_a, self.g2_b, self.k as u64];
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
        let base_transcript = [forged.p, forged.claimed_sum, forged.g1_a, forged.g1_b, 0u64, 0u64, forged.k as u64];
        let r1 = derive_many_mod_p(forged.p, b"power_house:v1:sumcheck:r1", &base_transcript, 1)[0];
        let s1 = field.add(field.mul(forged.g1_a, r1), forged.g1_b);
        // Solve for b: a*r + b = t => r irrelevant here; ensure g2(0)+g2(1) = s1
        // g2(0) = b, g2(1) = a + b => sum = a + 2b.  We know desired sum s1.
        // 2b + a = s1 => b = (s1 - a) / 2.
        let inv2 = field.inv(2);
        forged.g2_b = field.mul(field.sub(s1, forged.g2_a), inv2);
        assert!(!forged.verify_demo());
    }
}
