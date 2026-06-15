//! Non-core semantic sidecars for proof observatories.
//!
//! Sidecars bind visualization packets to a verified Rootprint replay state
//! without changing `.pha` fingerprints, branch IDs, replay, or proof validity.

use crate::provenance::{Rootprint, RootprintError};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

/// Schema identifier for Power House Observatory sidecars.
pub const OBSERVATORY_SIDECAR_SCHEMA_V1: &str = "power-house/observatory-sidecar/v1";

const SIDECAR_DOMAIN: &[u8] = b"power-house:observatory-sidecar:v1\0";
const SHA256_PREFIX: &str = "sha256:";

/// Semantic visualization packets bound to Rootprint nodes.
///
/// Node packets are deliberately opaque to Power House. A producer can use
/// `slbit`, another semantic format, or application-specific JSON.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObservatorySidecar {
    /// Sidecar schema identifier.
    pub schema: String,
    /// Rootprint replay fingerprint this sidecar describes.
    pub rootprint_state_fingerprint: String,
    /// Visualization packets keyed by exact Rootprint branch ID.
    pub nodes: BTreeMap<String, Value>,
    /// Domain-separated digest of sidecar fields excluding this value.
    pub sidecar_sha256: String,
}

impl ObservatorySidecar {
    /// Creates a sidecar bound to a valid Rootprint graph.
    pub fn new(
        graph: &Rootprint,
        nodes: BTreeMap<String, Value>,
    ) -> Result<Self, ObservatoryError> {
        let replay = graph.replay().map_err(ObservatoryError::Rootprint)?;
        let mut sidecar = Self {
            schema: OBSERVATORY_SIDECAR_SCHEMA_V1.to_string(),
            rootprint_state_fingerprint: replay.state_fingerprint,
            nodes,
            sidecar_sha256: String::new(),
        };
        sidecar.validate_node_references(graph)?;
        sidecar.sidecar_sha256 = sidecar.calculate_sha256()?;
        Ok(sidecar)
    }

    /// Calculates the deterministic sidecar digest.
    pub fn calculate_sha256(&self) -> Result<String, ObservatoryError> {
        let projection = serde_json::json!({
            "nodes": &self.nodes,
            "rootprint_state_fingerprint": &self.rootprint_state_fingerprint,
            "schema": &self.schema,
        });
        let encoded = serde_json::to_vec(&projection).map_err(ObservatoryError::Serialization)?;
        let mut hasher = Sha256::new();
        hasher.update(SIDECAR_DOMAIN);
        hasher.update(encoded);
        Ok(format!("{SHA256_PREFIX}{}", hex::encode(hasher.finalize())))
    }

    /// Verifies graph binding, node references, and sidecar integrity.
    ///
    /// This method does not interpret packet semantics and never changes the
    /// result of Rootprint or `.pha` verification.
    pub fn verify(&self, graph: &Rootprint) -> Result<(), ObservatoryError> {
        if self.schema != OBSERVATORY_SIDECAR_SCHEMA_V1 {
            return Err(ObservatoryError::UnsupportedSchema(self.schema.clone()));
        }
        validate_sha256(&self.rootprint_state_fingerprint)?;
        validate_sha256(&self.sidecar_sha256)?;
        let replay = graph.replay().map_err(ObservatoryError::Rootprint)?;
        if replay.state_fingerprint != self.rootprint_state_fingerprint {
            return Err(ObservatoryError::RootprintBindingMismatch {
                expected: replay.state_fingerprint,
                found: self.rootprint_state_fingerprint.clone(),
            });
        }
        self.validate_node_references(graph)?;
        let expected = self.calculate_sha256()?;
        if expected != self.sidecar_sha256 {
            return Err(ObservatoryError::SidecarDigestMismatch {
                expected,
                found: self.sidecar_sha256.clone(),
            });
        }
        Ok(())
    }

    fn validate_node_references(&self, graph: &Rootprint) -> Result<(), ObservatoryError> {
        for (branch_id, packet) in &self.nodes {
            if !graph.branches.contains_key(branch_id) {
                return Err(ObservatoryError::UnknownBranch(branch_id.clone()));
            }
            if !packet.is_object() {
                return Err(ObservatoryError::InvalidPacket(branch_id.clone()));
            }
        }
        Ok(())
    }
}

/// Errors returned by Observatory sidecar operations.
#[derive(Debug)]
pub enum ObservatoryError {
    /// Rootprint verification or replay failed.
    Rootprint(RootprintError),
    /// Sidecar schema is unsupported.
    UnsupportedSchema(String),
    /// A digest is malformed.
    InvalidDigest(String),
    /// Sidecar references a branch absent from the bound graph.
    UnknownBranch(String),
    /// A semantic packet is not a JSON object.
    InvalidPacket(String),
    /// Sidecar binds to a different Rootprint replay state.
    RootprintBindingMismatch {
        /// Recalculated Rootprint state fingerprint.
        expected: String,
        /// Fingerprint stored in the sidecar.
        found: String,
    },
    /// Sidecar content does not match its stored digest.
    SidecarDigestMismatch {
        /// Recalculated sidecar digest.
        expected: String,
        /// Digest stored in the sidecar.
        found: String,
    },
    /// Deterministic JSON serialization failed.
    Serialization(serde_json::Error),
}

impl fmt::Display for ObservatoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rootprint(error) => write!(formatter, "Rootprint verification failed: {error}"),
            Self::UnsupportedSchema(schema) => {
                write!(
                    formatter,
                    "unsupported Observatory sidecar schema: {schema}"
                )
            }
            Self::InvalidDigest(digest) => write!(formatter, "invalid SHA-256 digest: {digest}"),
            Self::UnknownBranch(branch) => {
                write!(
                    formatter,
                    "sidecar references unknown Rootprint branch: {branch}"
                )
            }
            Self::InvalidPacket(branch) => {
                write!(
                    formatter,
                    "sidecar packet for branch {branch} is not an object"
                )
            }
            Self::RootprintBindingMismatch { expected, found } => write!(
                formatter,
                "Rootprint state mismatch: expected {expected}, found {found}"
            ),
            Self::SidecarDigestMismatch { expected, found } => write!(
                formatter,
                "Observatory sidecar digest mismatch: expected {expected}, found {found}"
            ),
            Self::Serialization(error) => {
                write!(
                    formatter,
                    "Observatory sidecar serialization failed: {error}"
                )
            }
        }
    }
}

impl Error for ObservatoryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Rootprint(error) => Some(error),
            Self::Serialization(error) => Some(error),
            _ => None,
        }
    }
}

fn validate_sha256(value: &str) -> Result<(), ObservatoryError> {
    let Some(hex_digest) = value.strip_prefix(SHA256_PREFIX) else {
        return Err(ObservatoryError::InvalidDigest(value.to_string()));
    };
    if hex_digest.len() != 64
        || !hex_digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(ObservatoryError::InvalidDigest(value.to_string()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provenance::PhaArtifact;
    use serde_json::json;

    fn graph() -> Rootprint {
        let artifact = PhaArtifact::new(
            json!({"source": "observatory-test"}),
            "power-house/observatory-test/v1",
            json!({"claim": 7}),
            json!({"accepted": true}),
        )
        .unwrap();
        Rootprint::new("main", artifact).unwrap()
    }

    #[test]
    fn sidecar_is_bound_but_non_core() {
        let graph = graph();
        let replay_before = graph.replay().unwrap();
        let nodes = BTreeMap::from([(
            graph.root_branch.clone(),
            json!({"schema": "slbit/viz-packet/v1", "claim_id": "example"}),
        )]);
        let sidecar = ObservatorySidecar::new(&graph, nodes).unwrap();

        sidecar.verify(&graph).unwrap();
        assert_eq!(graph.replay().unwrap(), replay_before);
    }

    #[test]
    fn semantic_mutation_fails_sidecar_integrity_only() {
        let graph = graph();
        let nodes = BTreeMap::from([(
            graph.root_branch.clone(),
            json!({"schema": "slbit/viz-packet/v1", "claim_id": "example"}),
        )]);
        let mut sidecar = ObservatorySidecar::new(&graph, nodes).unwrap();
        sidecar.nodes.get_mut(&graph.root_branch).unwrap()["claim_id"] = json!("mutated");

        assert!(matches!(
            sidecar.verify(&graph),
            Err(ObservatoryError::SidecarDigestMismatch { .. })
        ));
        graph.verify().unwrap();
    }

    #[test]
    fn unknown_branch_is_rejected() {
        let graph = graph();
        let nodes = BTreeMap::from([(
            format!("sha256:{}", "0".repeat(64)),
            json!({"schema": "slbit/viz-packet/v1"}),
        )]);
        assert!(matches!(
            ObservatorySidecar::new(&graph, nodes),
            Err(ObservatoryError::UnknownBranch(_))
        ));
    }
}
