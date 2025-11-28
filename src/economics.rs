//! Token economics scaffolding for DA layer monetization.
//!
//! This module sketches fee and staking policies that can be enforced by the
//! networking layer to price blob submission and reward availability attestations.

/// Fee policy applied to blob submissions.
#[derive(Debug, Clone)]
pub struct FeePolicy {
    /// Fee per byte for submitted blobs.
    pub fee_per_byte: u64,
    /// Minimum flat fee.
    pub min_fee: u64,
}

/// Stake record for a validator.
#[derive(Debug, Clone)]
pub struct StakeAccount {
    /// Public key bytes.
    pub public_key: Vec<u8>,
    /// Bonded stake.
    pub stake: u64,
    /// Whether the account is slashed.
    pub slashed: bool,
}

/// Compute the required fee for a blob of `size` bytes.
pub fn compute_fee(policy: &FeePolicy, size: usize) -> u64 {
    let variable = policy.fee_per_byte.saturating_mul(size as u64);
    std::cmp::max(variable, policy.min_fee)
}
