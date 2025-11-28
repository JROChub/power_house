#![cfg(feature = "net")]

use crate::net::sign::verify_signature_base64;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Individual availability attestation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attestation {
    /// Hex-encoded share root.
    pub share_root: String,
    /// Optional Pedersen share root.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pedersen_root: Option<String>,
    /// Base64 public key.
    pub public_key: String,
    /// Base64 signature over the share root bytes.
    pub signature: String,
    /// Timestamp (ms) when produced.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ts: Option<u64>,
}

/// Aggregated quorum certificate for a share root.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttestationQuorum {
    /// Hex-encoded share root.
    pub share_root: String,
    /// Optional Pedersen share root.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pedersen_root: Option<String>,
    /// Attestations that met the threshold.
    pub attestations: Vec<Attestation>,
    /// Whether quorum was reached.
    pub quorum_reached: bool,
    /// Total stake weight observed (if provided by caller).
    pub total_stake: u64,
    /// Signer pks that contributed (for evidence or rewards).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub signers: Vec<String>,
    /// Timestamp of QC (ms).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ts: Option<u64>,
}

/// Verify attestations and check if quorum is met.
pub fn aggregate_attestations(
    attestations: &[Attestation],
    quorum: usize,
    stake_lookup: impl Fn(&str) -> Option<u64>,
) -> Result<AttestationQuorum, String> {
    if attestations.is_empty() {
        return Ok(AttestationQuorum {
            share_root: String::new(),
            pedersen_root: None,
            attestations: Vec::new(),
            quorum_reached: false,
            total_stake: 0,
            signers: Vec::new(),
            ts: None,
        });
    }
    let root = &attestations[0].share_root;
    let pedersen_root = attestations
        .iter()
        .find_map(|a| a.pedersen_root.clone())
        .unwrap_or_default();
    let mut valid = Vec::new();
    let mut seen = HashSet::new();
    let mut stake_sum = 0u64;
    for att in attestations {
        if att.share_root != *root {
            continue;
        }
        if !pedersen_root.is_empty() {
            if let Some(att_p) = &att.pedersen_root {
                if att_p != &pedersen_root {
                    continue;
                }
            } else {
                continue;
            }
        }
        if !seen.insert(att.public_key.clone()) {
            continue;
        }
        if verify_signature_base64(&att.public_key, root.as_bytes(), &att.signature).is_ok() {
            if let Some(weight) = stake_lookup(&att.public_key) {
                stake_sum = stake_sum.saturating_add(weight);
            }
            valid.push(att.clone());
        }
    }
    let qc = AttestationQuorum {
        share_root: root.clone(),
        pedersen_root: if pedersen_root.is_empty() {
            None
        } else {
            Some(pedersen_root)
        },
        quorum_reached: stake_sum >= quorum as u64 || valid.len() >= quorum,
        attestations: valid.clone(),
        total_stake: stake_sum,
        signers: seen.into_iter().collect(),
        ts: valid.iter().filter_map(|a| a.ts).max(),
    };
    Ok(qc)
}
