//! The design philosophy underlying `power_house` is pedagogical, yet mathematically rigorous.
//! Each module encapsulates a discrete concept in modern computational complexity theory,
//! illustrating how modest abstractions compose into a cohesive proof infrastructure.
//!
//! This crate aspires to bridge gaps between theoretical exposition and practical engineering,
//! serving both as a didactic resource and a foundation for future cryptographic research.
//! Pseudorandom number generator used for challenge derivation.
//!
//! This module defines a very small linear-congruential generator (LCG) that
//! serves as a deterministic source of pseudorandomness.  It can be used to
//! derive challenges for interactive proof protocols in the absence of
//! cryptographic hashes.  Although the generator is not cryptographically
//! secure, it is sufficient for demonstration purposes when running tests
//! against honest provers.

/// A simple linear-congruential pseudorandom number generator.
///
/// The generator updates its internal state on each call to
/// [`next_u64`](Self::next_u64) using the recurrence
///
/// ```text
/// state ← state × A + C (mod 2^64)
/// ```
///
/// where `A` and `C` are fixed constants.  The multiplier constant is
/// chosen from [Numerical Recipes](https://en.wikipedia.org/wiki/Linear_congruential_generator#Parameters_in_common_use)
/// and has good statistical properties for demonstration purposes.  No
/// external randomness is used; callers must provide an initial seed.
#[derive(Debug, Clone)]
pub struct SimplePrng {
    state: u64,
}

impl SimplePrng {
    /// Multiplier constant for the LCG.
    const A: u64 = 6364136223846793005;
    /// Increment constant for the LCG.
    const C: u64 = 1;

    /// Creates a new PRNG seeded with `seed`.
    pub fn new(seed: u64) -> Self {
        // Seed of zero is permitted; the first output will be C.
        Self { state: seed }
    }

    /// Advances the generator and returns the next 64-bit pseudorandom number.
    pub fn next_u64(&mut self) -> u64 {
        let state = self.state;
        // Wrapping multiplication and addition on u64.
        let new_state = state.wrapping_mul(Self::A).wrapping_add(Self::C);
        self.state = new_state;
        new_state
    }

    /// Returns a pseudorandom number reduced modulo `modulus`.
    ///
    /// # Panics
    ///
    /// Panics if `modulus` is zero.
    pub fn gen_mod(&mut self, modulus: u64) -> u64 {
        assert!(modulus != 0, "modulus must be non-zero");
        // We deliberately take the lower 64 bits as our pseudorandom output.
        self.next_u64() % modulus
    }
}

/// Derives a sequence of field elements from a transcript.
///
/// Given a prime modulus `p`, a domain tag (used to separate different
/// derivation contexts) and a slice of `u64` words representing the
/// transcript, this function returns `count` field elements in `[0,p)`.
///
/// The derivation is deterministic: it computes a seed as the sum of all
/// transcript words plus the domain tag bytes interpreted as a `u64` seed
/// and then runs a small LCG to generate the required pseudorandom values.
pub fn derive_many_mod_p(p: u64, domain_tag: &[u8], transcript: &[u64], count: usize) -> Vec<u64> {
    // Compute a simple seed by mixing the transcript words.
    let mut seed: u64 = 0;
    for &w in transcript {
        seed = seed.wrapping_add(w);
    }
    // Incorporate domain tag bytes into the seed to avoid cross-domain reuse.
    for chunk in domain_tag.chunks(8) {
        let mut chunk_arr = [0u8; 8];
        for (i, b) in chunk.iter().enumerate() {
            chunk_arr[i] = *b;
        }
        // Interpret up to 8 bytes as little-endian u64.
        let part = u64::from_le_bytes(chunk_arr);
        seed = seed.wrapping_add(part);
    }
    // Initialise the PRNG with the derived seed.
    let mut prng = SimplePrng::new(seed);
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        out.push(prng.gen_mod(p));
    }
    out
}
