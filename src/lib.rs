#![deny(missing_docs)]

//! The design philosophy underlying `power_house` is pedagogical, yet mathematically rigorous.
//! Each module encapsulates a discrete concept in modern computational complexity theory,
//! illustrating how modest abstractions compose into a cohesive proof infrastructure.
//!
//! This crate aspires to bridge gaps between theoretical exposition and practical engineering,
//! serving both as a didactic resource and a foundation for future cryptographic research.
//! # power_house
//!
//! **Power-House** is a zero-dependency Rust crate that showcases a set of cryptographic
//! and verification primitives inspired by interactive proof systems, the
//! sum-check protocol and the ALIEN theorem.  The goal of this crate is to
//! demonstrate how one can build powerful proof systems and consensus logic
//! without relying on any external libraries.  All code in this crate uses
//! only the Rust standard library.
//!
//! ## Features
//!
//! * **Finite field arithmetic** via the [`Field`](field/struct.Field.html) type.
//! * **Sum-check demonstration**: the [`sumcheck`](sumcheck/index.html) module
//!   contains functions to compute the true sum of a small bivariate polynomial
//!   over the Boolean hypercube, build a one-shot claim, and verify it with
//!   negligible soundness error.
//! * **Pseudorandom number generator (PRNG)**: the [`prng`](prng/index.html)
//!   module exposes a very small linear-congruential generator that can be
//!   used to derive deterministic challenges from a transcript.  This serves
//!   as a stand-in for a verifiable random function (VRF) in contexts where
//!   cryptographic hashes are unavailable.
//! * **Byzantine-fault-tolerant consensus**: the [`consensus`](consensus/index.html)
//!   module provides a trivial consensus primitive that takes a set of binary
//!   votes and returns whether the threshold has been met.  It is intended as
//!   a pedagogical example of how one might aggregate prover responses.
//! * **ALIEN theorem blueprint**: the [`alien`](alien/index.html) module
//!   outlines, through documentation and type stubs, how one could combine
//!   interactive proofs, VRF randomness, consensus and provability logic
//!   into a globally verifiable proof ledger.  This module is meant to
//!   illustrate the ideas described in the ALIEN theorem statement included
//!   in the problem statement, but it does not implement a full ledger.
//!
//! ## Usage
//!
//! The following example demonstrates how to compute and verify a sum-check
//! claim for the demo polynomial \f$\,f(x_1,x_2) = x_1 + x_2 + 2 x_1 x_2\,\) modulo a
//! small prime \f$p\,\) using this crate:
//!
//! ```rust
//! use power_house::{Field, sumcheck::SumClaim};
//!
//! // Choose a prime field of order 101.
//! let field = Field::new(101);
//!
//! // Prover creates an honest claim with default round count k=8.
//! let claim = SumClaim::prove_demo(&field, 8);
//! // The verifier checks that the claim is valid.
//! assert!(claim.verify_demo());
//! ```
//!
//! The crate can be extended with richer protocols by building on these
//! primitives.  It is intentionally minimal and does not offer a complete
//! blockchain or proof ledger implementation.

pub mod alien;
pub mod consensus;
mod data;
mod field;
mod io;
mod multilinear;
mod prng;
mod streaming;
pub mod sumcheck;
mod transcript;

pub use alien::{
    julian_genesis_anchor, julian_genesis_hash, reconcile_anchors, reconcile_anchors_with_quorum,
    EntryAnchor, LedgerAnchor, Proof, ProofKind, ProofLedger, Statement, JULIAN_GENESIS_STATEMENT,
};
pub use consensus::consensus;
pub use data::{
    compute_digest as transcript_digest, parse_record as parse_transcript_record,
    verify_record_lines as verify_transcript_lines, write_record as write_transcript_record,
};
pub use field::Field;
pub use io::write_text_series;
pub use multilinear::MultilinearPolynomial;
pub use prng::SimplePrng;
pub use streaming::StreamingPolynomial;
pub use sumcheck::{ChainedSumProof, GeneralSumClaim, GeneralSumProof, ProofStats, SumClaim};
pub use transcript::Transcript;
