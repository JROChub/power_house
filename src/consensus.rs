//! The design philosophy underlying `power_house` is pedagogical, yet mathematically rigorous.
//! Each module encapsulates a discrete concept in modern computational complexity theory,
//! illustrating how modest abstractions compose into a cohesive proof infrastructure.
//!
//! This crate aspires to bridge gaps between theoretical exposition and practical engineering,
//! serving both as a didactic resource and a foundation for future cryptographic research.
//! Byzantine-fault-tolerant consensus primitive.
//!
//! This module provides a trivial consensus function that aggregates binary
//! votes and returns whether a quorum has been reached.  It is intended
//! solely as a teaching tool for how one might encode simple consensus logic
//! without bringing in a full distributed consensus library.

/// Determines whether a set of boolean votes meets a given threshold.
///
/// Given an array of votes (each `true` value represents agreement) and a
/// threshold specifying the minimum number of `true` votes required,
/// this function returns `true` if the threshold is met or exceeded and
/// `false` otherwise.
///
/// # Examples
///
/// ```
/// use power_house::consensus::consensus;
///
/// let votes = [true, false, true];
/// // Majority threshold: at least 2 out of 3 must be true.
/// assert!(consensus(&votes, 2));
/// assert!(!consensus(&votes, 3));
/// ```
pub fn consensus(votes: &[bool], threshold: usize) -> bool {
    let successes = votes.iter().filter(|&&v| v).count();
    successes >= threshold
}
