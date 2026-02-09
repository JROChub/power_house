#![cfg(feature = "net")]

use crate::{
    compute_fold_digest, data::digest_from_hex, data::digest_to_hex,
    julian::JULIAN_GENESIS_STATEMENT, AnchorMetadata, EntryAnchor, LedgerAnchor,
};
use serde::{Deserialize, Serialize};
use std::{env, error::Error, fmt};

/// Canonical schema identifiers that are embedded inside anchors and envelopes.
pub const SCHEMA_ANCHOR: &str = "mfenx.powerhouse.anchor.v1";
/// Schema identifier used for signed network envelopes.
pub const SCHEMA_ENVELOPE: &str = "mfenx.powerhouse.envelope.v1";
/// Current envelope schema major version.
pub const ENVELOPE_SCHEMA_VERSION: u32 = 1;
/// Network identifier used across all JULIAN Protocol deployments for MFENX Power-House.
pub const NETWORK_ID: &str = "MFENX-POWERHOUSE";

/// Machine-readable representation of a single anchor entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AnchorEntryJson {
    /// Statement string associated with the transcript hashes.
    pub statement: String,
    /// Deterministic transcript hash list for this statement.
    pub hashes: Vec<String>,
    /// Optional Merkle root over the hashes (hex encoded).
    #[serde(default)]
    pub merkle_root: Option<String>,
}

/// Machine-readable representation of a JULIAN ledger anchor.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AnchorJson {
    /// Schema identifier (`mfenx.powerhouse.anchor.v1`).
    pub schema: String,
    /// Network identifier (`MFENX-POWERHOUSE`).
    pub network: String,
    /// Logical node identifier emitting the anchor.
    pub node_id: String,
    /// Name of the genesis statement embedded in every anchor.
    pub genesis: String,
    /// Ordered ledger entries containing transcript hashes.
    pub entries: Vec<AnchorEntryJson>,
    /// Quorum threshold expected by the originating node.
    pub quorum: usize,
    /// Millisecond timestamp representing when the anchor was produced.
    pub timestamp_ms: u64,
    /// Challenge derivation mode (matches transcript logs).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub challenge_mode: Option<String>,
    /// Fold digest hashed across transcript digests (hex).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fold_digest: Option<String>,
    /// Crate version that emitted this anchor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crate_version: Option<String>,
    /// Data-availability commitments this anchor depends on.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub da_commitments: Vec<DaCommitmentJson>,
    /// Optional evidence root (hex) for slashing records.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_root: Option<String>,
}

/// Data-availability commitment describing blob binding.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DaCommitmentJson {
    /// Namespace of the blob.
    pub namespace: String,
    /// Blob hash (hex).
    pub blob_hash: String,
    /// Share root (hex).
    pub share_root: String,
    /// Pedersen share root (hex) for ZK circuits.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pedersen_root: Option<String>,
    /// Optional attestation QC (stake-weighted) over the share root.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attestation_qc: Option<String>,
}
/// Signed envelope broadcast across the gossip layer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AnchorEnvelope {
    /// Schema identifier (`mfenx.powerhouse.envelope.v1`).
    pub schema: String,
    /// Envelope schema version (major).
    #[serde(default = "default_envelope_version")]
    pub schema_version: u32,
    /// Base64-encoded ed25519 public key for signature verification.
    pub public_key: String,
    /// Sender node identifier.
    pub node_id: String,
    /// Base64-encoded JSON payload representing [`AnchorJson`].
    pub payload: String,
    /// Base64-encoded ed25519 signature over the payload bytes.
    pub signature: String,
}

/// Errors produced while converting between ledger anchors and JSON forms.
#[derive(Debug, Clone)]
pub enum AnchorCodecError {
    /// The schema field did not match the expected identifier.
    InvalidSchema {
        /// Expected schema identifier.
        expected: &'static str,
        /// Encountered schema identifier.
        found: String,
    },
    /// The network field did not match the expected identifier.
    InvalidNetwork {
        /// Expected network identifier.
        expected: &'static str,
        /// Encountered network identifier.
        found: String,
    },
    /// The ledger anchor was missing the JULIAN genesis entry.
    MissingGenesis,
    /// A transcript hash was malformed.
    InvalidDigest {
        /// Index of the entry containing the malformed hash.
        entry: usize,
        /// Reason for the failure.
        reason: String,
    },
}

impl fmt::Display for AnchorCodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSchema { expected, found } => {
                write!(f, "invalid schema: expected {expected}, found {found}")
            }
            Self::InvalidNetwork { expected, found } => {
                write!(f, "invalid network: expected {expected}, found {found}")
            }
            Self::MissingGenesis => write!(f, "ledger anchor missing JULIAN genesis entry"),
            Self::InvalidDigest { entry, reason } => {
                write!(
                    f,
                    "ledger anchor entry {entry} has invalid digest: {reason}"
                )
            }
        }
    }
}

impl Error for AnchorCodecError {}

impl AnchorJson {
    /// Constructs a machine-readable anchor from a ledger anchor.
    pub fn from_ledger(
        node_id: impl Into<String>,
        quorum: usize,
        anchor: &LedgerAnchor,
        timestamp_ms: u64,
        da_commitments: Vec<DaCommitmentJson>,
        evidence_root: Option<String>,
    ) -> Result<Self, AnchorCodecError> {
        if anchor.entries.is_empty()
            || anchor.entries.first().map(|e| e.statement.as_str())
                != Some(JULIAN_GENESIS_STATEMENT)
        {
            return Err(AnchorCodecError::MissingGenesis);
        }
        let entries = anchor
            .entries
            .iter()
            .map(|entry| AnchorEntryJson {
                statement: entry.statement.clone(),
                hashes: entry.hashes.iter().map(digest_to_hex).collect(),
                merkle_root: Some(digest_to_hex(&entry.merkle_root)),
            })
            .collect();
        let fold_digest = anchor
            .metadata
            .fold_digest
            .unwrap_or_else(|| compute_fold_digest(anchor));
        Ok(Self {
            schema: SCHEMA_ANCHOR.to_string(),
            network: NETWORK_ID.to_string(),
            node_id: node_id.into(),
            genesis: JULIAN_GENESIS_STATEMENT.to_string(),
            entries,
            quorum,
            timestamp_ms,
            challenge_mode: anchor.metadata.challenge_mode.clone(),
            fold_digest: Some(digest_to_hex(&fold_digest)),
            crate_version: anchor.metadata.crate_version.clone(),
            da_commitments,
            evidence_root,
        })
    }

    /// Converts the JSON representation back into a ledger anchor.
    pub fn into_ledger(self) -> Result<LedgerAnchor, AnchorCodecError> {
        if self.schema != SCHEMA_ANCHOR {
            return Err(AnchorCodecError::InvalidSchema {
                expected: SCHEMA_ANCHOR,
                found: self.schema,
            });
        }
        if self.network != NETWORK_ID {
            return Err(AnchorCodecError::InvalidNetwork {
                expected: NETWORK_ID,
                found: self.network,
            });
        }
        if self.entries.first().map(|e| e.statement.as_str()) != Some(JULIAN_GENESIS_STATEMENT) {
            return Err(AnchorCodecError::MissingGenesis);
        }
        let mut entries = Vec::with_capacity(self.entries.len());
        for (idx, entry) in self.entries.into_iter().enumerate() {
            let mut hashes = Vec::with_capacity(entry.hashes.len());
            for hash_str in entry.hashes {
                let digest = digest_from_hex(&hash_str)
                    .map_err(|reason| AnchorCodecError::InvalidDigest { entry: idx, reason })?;
                hashes.push(digest);
            }
            let merkle_root = if let Some(root_hex) = entry.merkle_root {
                digest_from_hex(&root_hex)
                    .map_err(|reason| AnchorCodecError::InvalidDigest { entry: idx, reason })?
            } else {
                crate::merkle_root(&hashes)
            };
            entries.push(EntryAnchor {
                statement: entry.statement,
                hashes,
                merkle_root,
            });
        }
        let mut metadata = AnchorMetadata {
            challenge_mode: self.challenge_mode,
            crate_version: self
                .crate_version
                .or_else(|| Some(env!("CARGO_PKG_VERSION").to_string())),
            ..AnchorMetadata::default()
        };
        if let Some(fold_hex) = self.fold_digest {
            metadata.fold_digest = Some(
                digest_from_hex(&fold_hex)
                    .map_err(|reason| AnchorCodecError::InvalidDigest { entry: 0, reason })?,
            );
        }
        if metadata.fold_digest.is_none() {
            let temp = LedgerAnchor {
                entries: entries.clone(),
                metadata: AnchorMetadata::default(),
            };
            metadata.fold_digest = Some(compute_fold_digest(&temp));
        }
        Ok(LedgerAnchor { entries, metadata })
    }

    /// Serialises the anchor to JSON text.
    pub fn to_json_string(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Deserialises an anchor from JSON text.
    pub fn from_json_str(input: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(input)
    }
}

impl AnchorEnvelope {
    /// Ensures the envelope schema field matches the expected identifier.
    pub fn validate(&self) -> Result<(), AnchorCodecError> {
        if self.schema != SCHEMA_ENVELOPE {
            return Err(AnchorCodecError::InvalidSchema {
                expected: SCHEMA_ENVELOPE,
                found: self.schema.clone(),
            });
        }
        if self.schema_version > ENVELOPE_SCHEMA_VERSION {
            return Err(AnchorCodecError::InvalidSchema {
                expected: "schema_version <= current",
                found: format!("{}", self.schema_version),
            });
        }
        Ok(())
    }
}

fn default_envelope_version() -> u32 {
    ENVELOPE_SCHEMA_VERSION
}
