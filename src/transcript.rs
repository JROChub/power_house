//! Lightweight Fiat–Shamir transcript utilities.
//!
//! The [`Transcript`] type provides a minimal interface for recording field
//! elements and deriving deterministic challenges using a domain-separated
//! BLAKE2b-256 expander.  Challenges are produced by hashing the accumulated
//! transcript words together with a monotonic counter and reducing the result
//! modulo the ambient field.

use crate::{prng::derive_many_mod_p, Field};

/// Stateful helper that derives challenges from a recorded transcript.
#[derive(Debug, Clone)]
pub struct Transcript {
    domain_tag: &'static [u8],
    words: Vec<u64>,
    counter: u64,
}

impl Transcript {
    /// Creates an empty transcript associated with the given domain tag.
    pub fn new(domain_tag: &'static [u8]) -> Self {
        Self {
            domain_tag,
            words: Vec::new(),
            counter: 0,
        }
    }

    /// Appends a single `u64` word to the transcript.
    pub fn append(&mut self, value: u64) {
        self.words.push(value);
    }

    /// Appends all `u64` words in the provided slice to the transcript.
    pub fn append_slice(&mut self, values: &[u64]) {
        self.words.extend_from_slice(values);
    }

    /// Returns an immutable view of the accumulated transcript words.
    pub fn snapshot(&self) -> &[u64] {
        &self.words
    }

    /// Derives the next challenge in `[0, p)` using the Fiat–Shamir transform.
    ///
    /// Each invocation mixes the current transcript words with a strictly
    /// increasing counter, absorbs the resulting challenge into the transcript,
    /// and returns it to the caller.
    pub fn challenge(&mut self, field: &Field) -> u64 {
        self.words.push(self.counter);
        let challenge = derive_many_mod_p(field.modulus(), self.domain_tag, &self.words, 1)[0];
        self.words.pop();
        self.words.push(challenge);
        self.counter = self.counter.wrapping_add(1);
        challenge
    }
}
