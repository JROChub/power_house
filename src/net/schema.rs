#![cfg(feature = "net")]

use crate::{
    alien::JULIAN_GENESIS_STATEMENT, data::digest_from_hex, data::digest_to_hex, EntryAnchor,
    LedgerAnchor,
};
use serde::{Deserialize, Serialize};
use std::{error::Error, fmt};

/// Canonical schema identifiers that are embedded inside anchors and envelopes.
pub const SCHEMA_ANCHOR: &str = "jrocnet.anchor.v1";
/// Schema identifier used for signed network envelopes.
pub const SCHEMA_ENVELOPE: &str = "jrocnet.envelope.v1";
/// Current envelope schema major version.
pub const ENVELOPE_SCHEMA_VERSION: u32 = 1;
/// Network identifier used across all JULIAN Protocol deployments for JROC-NET.
pub const NETWORK_ID: &str = "JROC-NET";

/// Machine-readable representation of a single anchor entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AnchorEntryJson {
    /// Statement string associated with the transcript hashes.
    pub statement: String,
    /// Deterministic transcript hash list for this statement.
    pub hashes: Vec<String>,
}

/// Machine-readable representation of a JULIAN ledger anchor.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AnchorJson {
    /// Schema identifier (`jrocnet.anchor.v1`).
    pub schema: String,
    /// Network identifier (`JROC-NET`).
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
}

/// Signed envelope broadcast across the gossip layer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AnchorEnvelope {
    /// Schema identifier (`jrocnet.envelope.v1`).
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
                hashes: entry
                    .hashes
                    .iter()
                    .map(|digest| digest_to_hex(digest))
                    .collect(),
            })
            .collect();
        Ok(Self {
            schema: SCHEMA_ANCHOR.to_string(),
            network: NETWORK_ID.to_string(),
            node_id: node_id.into(),
            genesis: JULIAN_GENESIS_STATEMENT.to_string(),
            entries,
            quorum,
            timestamp_ms,
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
            entries.push(EntryAnchor {
                statement: entry.statement,
                hashes,
            });
        }
        Ok(LedgerAnchor { entries })
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
