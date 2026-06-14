//! Power House Archive (`.pha`) v1 artifacts.
//!
//! A `.pha` artifact binds a Power House proof and its provenance to a
//! deterministic `phx_fingerprint`. Optional external proof attachments are
//! deliberately outside that core identity. They can be transported alongside
//! the artifact without changing whether the Power House artifact is valid.

use super::rootprint::RootprintId;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::error::Error;
use std::fmt;

/// Schema identifier for Power House Archive v1 artifacts.
pub const PHA_SCHEMA_V1: &str = "power-house/pha/v1";

const PHX_FINGERPRINT_DOMAIN: &[u8] = b"power-house:pha:v1:phx-fingerprint\0";
const SHA256_PREFIX: &str = "sha256:";

/// A secondary proof transported with a `.pha` artifact.
///
/// Attachments are not part of the Power House core fingerprint or core
/// verification path. `payload_sha256` binds the attachment payload when the
/// caller explicitly invokes attachment verification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExternalProofAttachment {
    /// Stable attachment identifier within the artifact.
    pub id: String,
    /// External proof system or format name.
    pub proof_system: String,
    /// Opaque external proof payload.
    pub payload: Value,
    /// SHA-256 digest of canonical JSON serialization of `payload`.
    pub payload_sha256: String,
    /// Optional verifier or resolver hint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verifier_hint: Option<String>,
    /// Optional non-core attachment metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<Value>,
}

impl ExternalProofAttachment {
    /// Creates an attachment and calculates its payload digest.
    pub fn new(
        id: impl Into<String>,
        proof_system: impl Into<String>,
        payload: Value,
    ) -> Result<Self, PhaError> {
        let payload_sha256 = digest_json(&payload)?;
        Ok(Self {
            id: id.into(),
            proof_system: proof_system.into(),
            payload,
            payload_sha256,
            verifier_hint: None,
            metadata: None,
        })
    }

    /// Verifies the attachment's structural fields and payload digest.
    ///
    /// This checks transport integrity only. Cryptographic semantics for an
    /// external proof system belong in an explicit caller-supplied verifier.
    pub fn verify_integrity(&self) -> Result<(), PhaError> {
        if self.id.trim().is_empty() {
            return Err(PhaError::InvalidAttachment(
                "attachment id must not be empty".to_string(),
            ));
        }
        if self.proof_system.trim().is_empty() {
            return Err(PhaError::InvalidAttachment(
                "attachment proof_system must not be empty".to_string(),
            ));
        }
        validate_sha256(&self.payload_sha256)?;
        let expected = digest_json(&self.payload)?;
        if expected != self.payload_sha256 {
            return Err(PhaError::AttachmentDigestMismatch {
                attachment_id: self.id.clone(),
                expected,
                found: self.payload_sha256.clone(),
            });
        }
        Ok(())
    }
}

/// The Power House proof embedded in a `.pha` artifact.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EmbeddedProof {
    /// Power House proof protocol identifier.
    pub protocol: String,
    /// Public inputs or statement bound to the proof.
    pub public_inputs: Value,
    /// Opaque protocol-specific Power House proof payload.
    pub proof: Value,
    /// Optional secondary proofs that never affect Power House core validity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_proof_attachments: Option<Vec<ExternalProofAttachment>>,
}

/// A portable Power House Archive artifact.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PhaArtifact {
    /// `.pha` schema identifier.
    pub schema: String,
    /// Core provenance data committed by the Power House fingerprint.
    pub provenance: Value,
    /// Embedded Power House proof and optional external attachments.
    pub embedded_proof: EmbeddedProof,
    /// Optional Rootprint node binding for identity-aware workflows.
    ///
    /// This additive v1 field is excluded from `phx_fingerprint` to preserve
    /// legacy fingerprints and avoid a circular hash with Rootprint branch
    /// identifiers. Graph-context identity verification resolves the pointer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity_root: Option<RootprintId>,
    /// Domain-separated SHA-256 identity of core fields.
    pub phx_fingerprint: String,
}

impl PhaArtifact {
    /// Creates a `.pha` v1 artifact and calculates its core fingerprint.
    pub fn new(
        provenance: Value,
        protocol: impl Into<String>,
        public_inputs: Value,
        proof: Value,
    ) -> Result<Self, PhaError> {
        let mut artifact = Self {
            schema: PHA_SCHEMA_V1.to_string(),
            provenance,
            embedded_proof: EmbeddedProof {
                protocol: protocol.into(),
                public_inputs,
                proof,
                external_proof_attachments: None,
            },
            identity_root: None,
            phx_fingerprint: String::new(),
        };
        artifact.phx_fingerprint = artifact.calculate_phx_fingerprint()?;
        Ok(artifact)
    }

    /// Calculates the fingerprint over Power House core fields only.
    ///
    /// `external_proof_attachments`, `identity_root`, and the stored
    /// `phx_fingerprint` are intentionally excluded. Identity-aware callers
    /// validate the Rootprint pointer separately.
    pub fn calculate_phx_fingerprint(&self) -> Result<String, PhaError> {
        for (name, value) in [
            ("provenance", &self.provenance),
            (
                "embedded_proof.public_inputs",
                &self.embedded_proof.public_inputs,
            ),
            ("embedded_proof.proof", &self.embedded_proof.proof),
        ] {
            if !uses_canonical_json_numbers(value) {
                return Err(PhaError::InvalidCore(format!(
                    "{name} contains a non-integer JSON number"
                )));
            }
        }
        let core = serde_json::json!({
            "embedded_proof": {
                "proof": &self.embedded_proof.proof,
                "protocol": &self.embedded_proof.protocol,
                "public_inputs": &self.embedded_proof.public_inputs,
            },
            "provenance": &self.provenance,
            "schema": &self.schema,
        });
        let encoded = serde_json::to_vec(&core).map_err(PhaError::Serialization)?;
        let mut hasher = Sha256::new();
        hasher.update(PHX_FINGERPRINT_DOMAIN);
        hasher.update(encoded);
        Ok(format!("{SHA256_PREFIX}{}", hex::encode(hasher.finalize())))
    }

    /// Recalculates and stores the Power House core fingerprint.
    pub fn refresh_phx_fingerprint(&mut self) -> Result<(), PhaError> {
        self.phx_fingerprint = self.calculate_phx_fingerprint()?;
        Ok(())
    }

    /// Verifies `.pha` schema and core fingerprint validity.
    ///
    /// This method never reads or validates `external_proof_attachments`.
    pub fn verify(&self) -> Result<(), PhaError> {
        if self.schema != PHA_SCHEMA_V1 {
            return Err(PhaError::UnsupportedSchema(self.schema.clone()));
        }
        if self.embedded_proof.protocol.trim().is_empty() {
            return Err(PhaError::InvalidCore(
                "embedded proof protocol must not be empty".to_string(),
            ));
        }
        if let Some(identity_root) = &self.identity_root {
            RootprintId::new(identity_root.as_str()).map_err(|error| {
                PhaError::InvalidCore(format!("identity_root is invalid: {error}"))
            })?;
        }
        validate_sha256(&self.phx_fingerprint)?;
        let expected = self.calculate_phx_fingerprint()?;
        if expected != self.phx_fingerprint {
            return Err(PhaError::CoreFingerprintMismatch {
                expected,
                found: self.phx_fingerprint.clone(),
            });
        }
        Ok(())
    }

    /// Returns an identity-aware copy bound to a Rootprint node.
    ///
    /// The established v1 core fingerprint is unchanged.
    pub fn with_identity_root(mut self, identity_root: RootprintId) -> Self {
        self.identity_root = Some(identity_root);
        self
    }

    /// Explicitly verifies attachment integrity after core verification.
    ///
    /// Absence of attachments is valid. This method does not interpret the
    /// cryptographic semantics of external proof systems.
    pub fn verify_external_proof_attachments(&self) -> Result<(), PhaError> {
        self.verify()?;
        if let Some(attachments) = &self.embedded_proof.external_proof_attachments {
            for attachment in attachments {
                attachment.verify_integrity()?;
            }
        }
        Ok(())
    }

    /// Explicitly verifies attachment integrity and caller-defined semantics.
    pub fn verify_external_proof_attachments_with<F>(&self, mut verifier: F) -> Result<(), PhaError>
    where
        F: FnMut(&ExternalProofAttachment) -> Result<(), String>,
    {
        self.verify_external_proof_attachments()?;
        if let Some(attachments) = &self.embedded_proof.external_proof_attachments {
            for attachment in attachments {
                verifier(attachment).map_err(|message| PhaError::ExternalVerifierRejected {
                    attachment_id: attachment.id.clone(),
                    message,
                })?;
            }
        }
        Ok(())
    }
}

/// Errors returned by `.pha` construction and verification.
#[derive(Debug)]
pub enum PhaError {
    /// JSON serialization failed.
    Serialization(serde_json::Error),
    /// The artifact schema is unsupported.
    UnsupportedSchema(String),
    /// A required core field is invalid.
    InvalidCore(String),
    /// A SHA-256 value is malformed.
    InvalidDigest(String),
    /// The stored core fingerprint does not match core content.
    CoreFingerprintMismatch {
        /// Recalculated core fingerprint.
        expected: String,
        /// Fingerprint stored in the artifact.
        found: String,
    },
    /// An attachment field is structurally invalid.
    InvalidAttachment(String),
    /// Attachment payload integrity verification failed.
    AttachmentDigestMismatch {
        /// Attachment identifier.
        attachment_id: String,
        /// Recalculated attachment digest.
        expected: String,
        /// Digest stored in the attachment.
        found: String,
    },
    /// A caller-supplied external proof verifier rejected an attachment.
    ExternalVerifierRejected {
        /// Attachment identifier.
        attachment_id: String,
        /// Verifier-provided reason.
        message: String,
    },
}

impl fmt::Display for PhaError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Serialization(error) => write!(formatter, "PHA serialization failed: {error}"),
            Self::UnsupportedSchema(schema) => {
                write!(formatter, "unsupported PHA schema: {schema}")
            }
            Self::InvalidCore(message) => write!(formatter, "invalid PHA core: {message}"),
            Self::InvalidDigest(digest) => write!(formatter, "invalid SHA-256 digest: {digest}"),
            Self::CoreFingerprintMismatch { expected, found } => write!(
                formatter,
                "PHA core fingerprint mismatch: expected {expected}, found {found}"
            ),
            Self::InvalidAttachment(message) => {
                write!(formatter, "invalid external proof attachment: {message}")
            }
            Self::AttachmentDigestMismatch {
                attachment_id,
                expected,
                found,
            } => write!(
                formatter,
                "external proof attachment {attachment_id} digest mismatch: expected {expected}, found {found}"
            ),
            Self::ExternalVerifierRejected {
                attachment_id,
                message,
            } => write!(
                formatter,
                "external verifier rejected attachment {attachment_id}: {message}"
            ),
        }
    }
}

impl Error for PhaError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Serialization(error) => Some(error),
            _ => None,
        }
    }
}

fn digest_json(value: &Value) -> Result<String, PhaError> {
    if !uses_canonical_json_numbers(value) {
        return Err(PhaError::InvalidAttachment(
            "attachment payload contains a non-integer JSON number".to_string(),
        ));
    }
    let encoded = serde_json::to_vec(value).map_err(PhaError::Serialization)?;
    Ok(format!(
        "{SHA256_PREFIX}{}",
        hex::encode(Sha256::digest(encoded))
    ))
}

fn uses_canonical_json_numbers(value: &Value) -> bool {
    match value {
        Value::Number(number) => number.is_i64() || number.is_u64(),
        Value::Array(values) => values.iter().all(uses_canonical_json_numbers),
        Value::Object(values) => values.values().all(uses_canonical_json_numbers),
        _ => true,
    }
}

fn validate_sha256(digest: &str) -> Result<(), PhaError> {
    let Some(hex_digest) = digest.strip_prefix(SHA256_PREFIX) else {
        return Err(PhaError::InvalidDigest(digest.to_string()));
    };
    if hex_digest.len() != 64
        || !hex_digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(PhaError::InvalidDigest(digest.to_string()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn artifact() -> PhaArtifact {
        PhaArtifact::new(
            json!({
                "producer": "power-house",
                "source_digest": "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            }),
            "power-house/sumcheck/v1",
            json!({"claim": "42", "field": 101}),
            json!({"rounds": [[12, 30], [7, 5]]}),
        )
        .unwrap()
    }

    fn attachment() -> ExternalProofAttachment {
        ExternalProofAttachment::new(
            "external-proof-1",
            "example/external-proof/v1",
            json!({"proof": "opaque", "public_inputs": [1, 2, 3]}),
        )
        .unwrap()
    }

    #[test]
    fn absent_attachments_are_not_serialized() {
        let encoded = serde_json::to_value(artifact()).unwrap();
        assert!(encoded["embedded_proof"]
            .get("external_proof_attachments")
            .is_none());
    }

    #[test]
    fn core_fingerprint_is_identical_with_or_without_attachments() {
        let base = artifact();
        let mut attached = base.clone();
        attached.embedded_proof.external_proof_attachments = Some(vec![attachment()]);

        assert_eq!(
            base.calculate_phx_fingerprint().unwrap(),
            attached.calculate_phx_fingerprint().unwrap()
        );
        assert_eq!(base.phx_fingerprint, attached.phx_fingerprint);
        assert!(base.verify().is_ok());
        assert!(attached.verify().is_ok());
    }

    #[test]
    fn attachment_mutation_does_not_change_core_validity() {
        let mut attached = artifact();
        attached.embedded_proof.external_proof_attachments = Some(vec![attachment()]);
        let fingerprint = attached.phx_fingerprint.clone();

        attached
            .embedded_proof
            .external_proof_attachments
            .as_mut()
            .unwrap()[0]
            .payload = json!({"proof": "mutated"});

        assert_eq!(attached.calculate_phx_fingerprint().unwrap(), fingerprint);
        assert!(attached.verify().is_ok());
        assert!(matches!(
            attached.verify_external_proof_attachments(),
            Err(PhaError::AttachmentDigestMismatch { .. })
        ));
    }

    #[test]
    fn identity_root_is_additive_and_does_not_change_v1_fingerprint() {
        let base = artifact();
        let fingerprint = base.phx_fingerprint.clone();
        let bound = base
            .with_identity_root(RootprintId::new(format!("sha256:{}", "1".repeat(64))).unwrap());

        assert_eq!(bound.calculate_phx_fingerprint().unwrap(), fingerprint);
        assert_eq!(bound.phx_fingerprint, fingerprint);
        assert!(bound.verify().is_ok());
    }

    #[test]
    fn core_mutation_invalidates_core_verification() {
        let mut artifact = artifact();
        artifact.embedded_proof.proof = json!({"rounds": []});
        assert!(matches!(
            artifact.verify(),
            Err(PhaError::CoreFingerprintMismatch { .. })
        ));
    }

    #[test]
    fn explicit_external_verifier_is_separate_from_core_verification() {
        let mut artifact = artifact();
        artifact.embedded_proof.external_proof_attachments = Some(vec![attachment()]);

        assert!(artifact.verify().is_ok());
        let error = artifact
            .verify_external_proof_attachments_with(|_| {
                Err("external policy rejected proof".to_string())
            })
            .unwrap_err();
        assert!(matches!(error, PhaError::ExternalVerifierRejected { .. }));
    }

    #[test]
    fn non_integer_numbers_are_rejected_from_canonical_content() {
        let error = PhaArtifact::new(
            json!({"measurement": 1.5}),
            "power-house/test/v1",
            json!({}),
            json!({}),
        )
        .unwrap_err();
        assert!(matches!(error, PhaError::InvalidCore(_)));

        let error = ExternalProofAttachment::new("epa", "external/test/v1", json!({"value": 1.5}))
            .unwrap_err();
        assert!(matches!(error, PhaError::InvalidAttachment(_)));
    }
}
