//! Streaming polynomial utilities for on-demand sum-check evaluation.
use std::fmt;
use std::sync::Arc;

#[derive(Clone)]
/// Streaming representation of a multilinear polynomial over a Boolean hypercube.
pub struct StreamingPolynomial {
    num_vars: usize,
    modulus: u64,
    evaluator: Arc<dyn Fn(usize) -> u64 + Send + Sync>,
}

impl fmt::Debug for StreamingPolynomial {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StreamingPolynomial")
            .field("num_vars", &self.num_vars)
            .field("modulus", &self.modulus)
            .finish()
    }
}

impl StreamingPolynomial {
    /// Creates a streaming polynomial from an evaluator closure.
    pub fn new<F>(num_vars: usize, modulus: u64, evaluator: F) -> Self
    where
        F: Fn(usize) -> u64 + Send + Sync + 'static,
    {
        Self {
            num_vars,
            modulus,
            evaluator: Arc::new(evaluator),
        }
    }

    /// Returns the number of variables.
    pub fn num_vars(&self) -> usize {
        self.num_vars
    }

    /// Returns the field modulus.
    pub fn modulus(&self) -> u64 {
        self.modulus
    }

    /// Evaluates the polynomial at the Boolean assignment encoded by `idx`.
    pub fn evaluate(&self, idx: usize) -> u64 {
        (self.evaluator)(idx)
    }

    /// Returns a clone of the underlying evaluator.
    pub fn evaluator(&self) -> Arc<dyn Fn(usize) -> u64 + Send + Sync> {
        Arc::clone(&self.evaluator)
    }
}
