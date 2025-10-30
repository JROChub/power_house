//! Multilinear polynomial utilities.
//!
//! This module introduces a compact representation of multilinear polynomials
//! over prime fields.  The evaluations of the polynomial on the Boolean
//! hypercube are stored explicitly, enabling efficient summations, Boolean
//! evaluations, and barycentric interpolation at arbitrary field points without
//! pulling in external algebra crates.

use crate::Field;

/// Represents an *n*-variate multilinear polynomial via its values on `{0,1}ⁿ`.
///
/// Values are stored in *little-endian* order with respect to the variable
/// index: the evaluation at `(x₀, …, x_{n-1})` lives at index
/// `x₀ + 2·x₁ + ⋯ + 2^{n-1}·x_{n-1}`.  This convention matches the ordering
/// used by the sum-check implementation, keeping all folding operations cache
/// friendly.
#[derive(Debug, Clone)]
pub struct MultilinearPolynomial {
    num_vars: usize,
    evals: Vec<u64>,
}

impl MultilinearPolynomial {
    /// Creates a polynomial from its Boolean-hypercube evaluations.
    ///
    /// # Panics
    ///
    /// Panics if the `evaluations` length is not exactly `2^num_vars`.
    pub fn from_evaluations(num_vars: usize, evaluations: Vec<u64>) -> Self {
        let expected_len = 1usize.checked_shl(num_vars as u32).unwrap_or(0);
        assert_eq!(
            evaluations.len(),
            expected_len,
            "expected 2^num_vars evaluations"
        );
        Self {
            num_vars,
            evals: evaluations,
        }
    }

    /// Returns the number of variables.
    pub fn num_vars(&self) -> usize {
        self.num_vars
    }

    /// Returns the raw evaluation table.
    pub fn evaluations(&self) -> &[u64] {
        &self.evals
    }

    /// Reduces the evaluation table modulo the field and returns an owned copy.
    pub fn evaluations_mod_p(&self, field: &Field) -> Vec<u64> {
        self.evals.iter().map(|&v| v % field.modulus()).collect()
    }

    /// Returns a streaming iterator over `(assignment_bits, value)` pairs.
    pub fn iter(&self) -> impl Iterator<Item = (usize, u64)> + '_ {
        self.evals
            .iter()
            .enumerate()
            .map(|(idx, &value)| (idx, value))
    }

    /// Computes the sum of all evaluations modulo the field.
    pub fn sum_over_hypercube(&self, field: &Field) -> u64 {
        self.evals.iter().fold(0u64, |acc, &v| field.add(acc, v))
    }

    /// Evaluates the polynomial at a Boolean point.
    ///
    /// The `assignment` slice must contain exactly `num_vars` entries, each of
    /// which is interpreted as 0 or 1 modulo `p`.
    pub fn evaluate_boolean(&self, field: &Field, assignment: &[u64]) -> u64 {
        assert_eq!(
            assignment.len(),
            self.num_vars,
            "boolean assignment length mismatch"
        );
        let mut idx = 0usize;
        for (i, &bit) in assignment.iter().enumerate() {
            let bit_mod = bit % field.modulus();
            assert!(
                bit_mod == 0 || bit_mod == 1,
                "boolean assignment must contain only 0/1"
            );
            if bit_mod == 1 {
                idx |= 1 << i;
            }
        }
        self.evals[idx] % field.modulus()
    }

    /// Evaluates the polynomial at an arbitrary field point.
    ///
    /// This uses a straightforward barycentric interpolation across the stored
    /// evaluations.  The complexity is `O(n · 2^{n-1})`, suitable for the small
    /// demonstration polynomials showcased by this crate.
    pub fn evaluate(&self, field: &Field, point: &[u64]) -> u64 {
        assert_eq!(
            point.len(),
            self.num_vars,
            "evaluation point length mismatch"
        );
        let mut layer = self.evaluations_mod_p(field);
        for &coord in point {
            let r = coord % field.modulus();
            let mut next = Vec::with_capacity(layer.len() / 2);
            for chunk in layer.chunks_exact(2) {
                let v0 = chunk[0];
                let v1 = chunk[1];
                let diff = field.sub(v1, v0);
                let eval = field.add(field.mul(diff, r), v0);
                next.push(eval);
            }
            layer = next;
        }
        layer[0]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_boolean_evaluation_ordering() {
        let poly = MultilinearPolynomial::from_evaluations(2, vec![0, 1, 2, 3]);
        let field = Field::new(101);
        // Assignment (x0=0,x1=0) -> index 0
        assert_eq!(poly.evaluate_boolean(&field, &[0, 0]), 0);
        // Assignment (1,0) -> index 1
        assert_eq!(poly.evaluate_boolean(&field, &[1, 0]), 1);
        // Assignment (0,1) -> index 2
        assert_eq!(poly.evaluate_boolean(&field, &[0, 1]), 2);
        // Assignment (1,1) -> index 3
        assert_eq!(poly.evaluate_boolean(&field, &[1, 1]), 3);
    }

    #[test]
    fn test_arbitrary_evaluation() {
        let poly = MultilinearPolynomial::from_evaluations(2, vec![0, 1, 2, 3]);
        let field = Field::new(101);
        // Expected multilinear interpolation:
        // f(x0, x1) = x0 + 2x1
        let val = poly.evaluate(&field, &[5, 7]);
        // Evaluate manually: 5 + 2*7 = 19 mod 101
        assert_eq!(val, 19);
    }
}
