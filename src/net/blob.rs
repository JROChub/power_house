#![cfg(feature = "net")]

use crate::net::schema::{NETWORK_ID, SCHEMA_ENVELOPE};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;

/// Schema identifier for blob payloads.
pub const SCHEMA_BLOB: &str = "jrocnet.blob.v1";
/// Gossip topic for blob envelopes.
pub const TOPIC_BLOBS: &str = "jrocnet/blobs/v1";

/// JSON payload representing a submitted blob.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlobJson {
    /// Schema identifier (`jrocnet.blob.v1`).
    pub schema: String,
    /// Network identifier (`JROC-NET`).
    pub network: String,
    /// Logical namespace for the blob.
    pub namespace: String,
    /// Blake2b-256 digest of the raw blob (hex).
    pub hash: String,
    /// Raw blob size in bytes.
    pub size: u64,
    /// Base64-encoded blob contents.
    pub data: String,
    /// Number of data shards used for erasure coding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_shards: Option<u8>,
    /// Number of parity shards used for erasure coding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parity_shards: Option<u8>,
    /// Commitment to the encoded shares (hex).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub share_root: Option<String>,
    /// Optional attestation signature (base64) over `share_root`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attestation_sig: Option<String>,
    /// Optional attestation public key (base64).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attestation_pk: Option<String>,
    /// Publisher identity (base64 public key) responsible for availability.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publisher_pk: Option<String>,
}

impl BlobJson {
    /// Build a blob payload from raw bytes and namespace.
    pub fn from_bytes(namespace: impl Into<String>, data: &[u8]) -> Self {
        let hash = blake2b_hex(data);
        Self {
            schema: SCHEMA_BLOB.to_string(),
            network: NETWORK_ID.to_string(),
            namespace: namespace.into(),
            hash,
            size: data.len() as u64,
            data: BASE64.encode(data),
            data_shards: None,
            parity_shards: None,
            share_root: None,
            attestation_sig: None,
            attestation_pk: None,
            publisher_pk: None,
        }
    }

    /// Decode the base64 payload into raw bytes.
    pub fn decode_data(&self) -> Result<Vec<u8>, BlobCodecError> {
        BASE64
            .decode(self.data.as_bytes())
            .map_err(|err| BlobCodecError::Decode(err.to_string()))
    }

    /// Validate schema, network, and hash/size consistency.
    pub fn validate(&self) -> Result<(), BlobCodecError> {
        if self.schema != SCHEMA_BLOB {
            return Err(BlobCodecError::InvalidSchema {
                expected: SCHEMA_BLOB,
                found: self.schema.clone(),
            });
        }
        if self.network != NETWORK_ID {
            return Err(BlobCodecError::InvalidNetwork {
                expected: NETWORK_ID,
                found: self.network.clone(),
            });
        }
        let decoded = self.decode_data()?;
        if decoded.len() as u64 != self.size {
            return Err(BlobCodecError::SizeMismatch {
                expected: self.size,
                actual: decoded.len() as u64,
            });
        }
        let recomputed = blake2b_hex(&decoded);
        if recomputed != self.hash {
            return Err(BlobCodecError::HashMismatch {
                expected: self.hash.clone(),
                actual: recomputed,
            });
        }
        Ok(())
    }
}

/// Signed envelope carrying a blob payload.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlobEnvelope {
    /// Schema identifier (`jrocnet.envelope.v1`).
    pub schema: String,
    /// Envelope schema version (major).
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    /// Base64-encoded ed25519 public key for signature verification.
    pub public_key: String,
    /// Sender node identifier.
    pub node_id: String,
    /// Base64-encoded JSON payload representing [`BlobJson`].
    pub payload: String,
    /// Base64-encoded ed25519 signature over the payload bytes.
    pub signature: String,
}

impl BlobEnvelope {
    /// Ensure the envelope conforms to expectations.
    pub fn validate(&self) -> Result<(), BlobCodecError> {
        if self.schema != SCHEMA_ENVELOPE {
            return Err(BlobCodecError::InvalidSchema {
                expected: SCHEMA_ENVELOPE,
                found: self.schema.clone(),
            });
        }
        if self.schema_version == 0 {
            return Err(BlobCodecError::InvalidVersion(self.schema_version));
        }
        Ok(())
    }
}

fn default_schema_version() -> u32 {
    1
}

fn blake2b_hex(data: &[u8]) -> String {
    let mut hasher = blake2::Blake2b::<blake2::digest::consts::U32>::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

/// Errors produced while decoding or validating blobs.
#[derive(Debug, Clone)]
pub enum BlobCodecError {
    /// Unexpected schema identifier.
    InvalidSchema {
        /// Expected schema.
        expected: &'static str,
        /// Found schema.
        found: String,
    },
    /// Unexpected network identifier.
    InvalidNetwork {
        /// Expected network.
        expected: &'static str,
        /// Found network.
        found: String,
    },
    /// Payload failed to decode.
    Decode(String),
    /// Blob size mismatch.
    SizeMismatch {
        /// Size claimed by the blob metadata.
        expected: u64,
        /// Actual decoded payload size.
        actual: u64,
    },
    /// Blob hash mismatch.
    HashMismatch {
        /// Expected hex digest.
        expected: String,
        /// Recomputed hex digest.
        actual: String,
    },
    /// Envelope schema version invalid.
    InvalidVersion(u32),
}

impl fmt::Display for BlobCodecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSchema { expected, found } => {
                write!(f, "invalid schema: expected {expected}, found {found}")
            }
            Self::InvalidNetwork { expected, found } => {
                write!(f, "invalid network: expected {expected}, found {found}")
            }
            Self::Decode(reason) => write!(f, "decode error: {reason}"),
            Self::SizeMismatch { expected, actual } => {
                write!(f, "size mismatch: expected {expected}, actual {actual}")
            }
            Self::HashMismatch { expected, actual } => {
                write!(f, "hash mismatch: expected {expected}, actual {actual}")
            }
            Self::InvalidVersion(v) => write!(f, "invalid envelope version {v}"),
        }
    }
}

impl std::error::Error for BlobCodecError {}

/// Compute a SHA-256 digest used for gossip de-duplication.
pub fn sha256_digest(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let mut out = [0u8; 32];
    out.copy_from_slice(&hasher.finalize());
    out
}
