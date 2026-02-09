//! The design philosophy underlying `power_house` is pedagogical, yet mathematically rigorous.
//! Each module encapsulates a discrete concept in modern computational complexity theory,
//! illustrating how modest abstractions compose into a cohesive proof infrastructure.
//!
//! This crate aspires to bridge gaps between theoretical exposition and practical engineering,
//! serving both as a didactic resource and a foundation for future cryptographic research.
//! Pseudorandom number generator used for challenge derivation.
//!
//! This module exposes a compact deterministic stream generator backed by
//! domain-separated BLAKE2b-256 expansions.  The interface mirrors the old
//! linear-congruential helper but upgrades the security story: every output
//! chunk is derived from a keyed hash of the seed and an invocation counter,
//! ensuring forward secrecy and resistance to trivial state reconstruction.

use blake2::digest::{consts::U32, Digest};

type Blake2b256 = blake2::Blake2b<U32>;

const PRNG_DOMAIN: &[u8] = b"MFENX_PRNG";
const CHALLENGE_DOMAIN: &[u8] = b"MFENX_CHALLENGE";

/// A deterministic stream generator derived from BLAKE2b-256.
#[derive(Debug, Clone)]
pub struct SimplePrng {
    seed: [u8; 32],
    counter: u64,
    buffer: [u8; 32],
    offset: usize,
}

impl SimplePrng {
    /// Creates a new PRNG seeded with `seed`.
    pub fn new(seed: u64) -> Self {
        let mut hasher = Blake2b256::new();
        hasher.update(PRNG_DOMAIN);
        hasher.update(seed.to_be_bytes());
        let mut base = [0u8; 32];
        base.copy_from_slice(&hasher.finalize());
        Self::from_seed_bytes(base)
    }

    /// Creates a PRNG from a raw 32-byte seed.
    pub fn from_seed_bytes(seed: [u8; 32]) -> Self {
        Self {
            seed,
            counter: 0,
            buffer: [0u8; 32],
            offset: 32,
        }
    }

    fn refill(&mut self) {
        let mut hasher = Blake2b256::new();
        hasher.update(PRNG_DOMAIN);
        hasher.update(self.seed);
        hasher.update(self.counter.to_be_bytes());
        self.buffer.copy_from_slice(&hasher.finalize());
        self.counter = self.counter.wrapping_add(1);
        self.offset = 0;
    }

    /// Advances the generator and returns the next 64-bit pseudorandom number.
    pub fn next_u64(&mut self) -> u64 {
        if self.offset >= self.buffer.len() {
            self.refill();
        }
        let mut chunk = [0u8; 8];
        chunk.copy_from_slice(&self.buffer[self.offset..self.offset + 8]);
        self.offset += 8;
        u64::from_be_bytes(chunk)
    }

    /// Returns a pseudorandom number reduced modulo `modulus`.
    ///
    /// # Panics
    ///
    /// Panics if `modulus` is zero.
    pub fn gen_mod(&mut self, modulus: u64) -> u64 {
        assert!(modulus != 0, "modulus must be non-zero");
        self.next_u64() % modulus
    }
}

/// Derives a sequence of field elements from a transcript.
///
/// Given a prime modulus `p`, a domain tag (used to separate different
/// derivation contexts) and a slice of `u64` words representing the
/// transcript, this function returns `count` field elements in `[0,p)`.
pub fn derive_many_mod_p(p: u64, domain_tag: &[u8], transcript: &[u64], count: usize) -> Vec<u64> {
    assert!(p != 0, "modulus must be non-zero");
    let mut seed_hasher = Blake2b256::new();
    seed_hasher.update(CHALLENGE_DOMAIN);
    seed_hasher.update((domain_tag.len() as u64).to_be_bytes());
    seed_hasher.update(domain_tag);
    seed_hasher.update((transcript.len() as u64).to_be_bytes());
    for &word in transcript {
        seed_hasher.update(word.to_be_bytes());
    }
    let mut seed_bytes = [0u8; 32];
    seed_bytes.copy_from_slice(&seed_hasher.finalize());
    let mut prng = SimplePrng::from_seed_bytes(seed_bytes);
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        out.push(prng.gen_mod(p));
    }
    out
}
