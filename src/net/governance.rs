#![cfg(feature = "net")]

use crate::net::sign::encode_public_key_base64;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use ed25519_dalek::{Signature, VerifyingKey, PUBLIC_KEY_LENGTH, SIGNATURE_LENGTH};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Abstract governance backend that decides which peer keys are currently eligible.
pub trait MembershipPolicy: Send + Sync {
    /// Returns the currently active membership public keys.
    fn current_members(&self) -> Vec<VerifyingKey>;

    /// Validates a proposed update and, if valid, produces a new policy.
    fn verify_update(&self, update: &GovernanceUpdate) -> Result<(), PolicyUpdateError>;

    /// Applies an update (after verification) and persists internal state.
    fn apply_update(&mut self, update: &GovernanceUpdate) -> Result<(), PolicyUpdateError>;

    /// Human-friendly label for metrics/logging.
    fn name(&self) -> &'static str;
}

/// Raw governance update payload used to evolve membership.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceUpdate {
    /// Replacement membership set expressed as base64 ed25519 public keys.
    pub new_members: Vec<String>,
    /// Optional metadata describing the rotation.
    pub metadata: Option<serde_json::Value>,
    /// Authorising signatures from the governing authority.
    pub signatures: Vec<SignedApproval>,
}

/// Signature authorising a governance update.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedApproval {
    /// Signer public key (base64).
    pub signer: String,
    /// Base64 ed25519 signature over the canonical update payload.
    pub signature: String,
}

/// Errors raised when verifying or applying membership updates.
#[derive(Debug, Error)]
pub enum PolicyUpdateError {
    #[error("unsupported operation")]
    /// Operation is not supported by this policy backend.
    Unsupported,
    #[error("io error: {0}")]
    /// Underlying filesystem failure.
    Io(String),
    #[error("decode error: {0}")]
    /// Input decoding/serialization failure.
    Decode(String),
    #[error("threshold not met (required {required}, had {actual})")]
    /// Approval threshold was not satisfied.
    Threshold {
        /// Required approval threshold.
        required: usize,
        /// Number of approvals present in the update.
        actual: usize,
    },
    #[error("unauthorised signer")]
    /// Update included a signature from a non-governing key.
    Unauthorized,
    #[error("signature verification failed")]
    /// Update signatures failed to verify.
    BadSignature,
}

// ---------------------------------------------------------------------
// Static allowlist policy
// ---------------------------------------------------------------------

/// Read-only membership backend powered by a static allowlist.
pub struct StaticPolicy {
    members: Vec<VerifyingKey>,
}

impl StaticPolicy {
    /// Loads a static membership set from a JSON allowlist file.
    pub fn from_allowlist(path: &Path) -> Result<Self, PolicyUpdateError> {
        let contents =
            fs::read_to_string(path).map_err(|err| PolicyUpdateError::Io(err.to_string()))?;
        let allow: AllowListFile = serde_json::from_str(&contents)
            .map_err(|err| PolicyUpdateError::Decode(err.to_string()))?;
        let mut members = Vec::new();
        for base64 in allow.allowed {
            let vk = decode_public_key(&base64)?;
            members.push(vk);
        }
        Ok(Self { members })
    }

    /// Constructs a policy that accepts every key (bootstrap/permissionless).
    pub fn allow_all() -> Self {
        Self {
            members: Vec::new(),
        }
    }

    /// Builds a static policy from an inline list of base64-encoded public keys.
    pub fn from_base64_strings(list: &[String]) -> Result<Self, PolicyUpdateError> {
        let mut members = Vec::new();
        for base64 in list {
            let vk = decode_public_key(base64)?;
            members.push(vk);
        }
        Ok(Self { members })
    }
}

impl MembershipPolicy for StaticPolicy {
    fn current_members(&self) -> Vec<VerifyingKey> {
        if self.members.is_empty() {
            Vec::new()
        } else {
            self.members.clone()
        }
    }

    fn verify_update(&self, _update: &GovernanceUpdate) -> Result<(), PolicyUpdateError> {
        Err(PolicyUpdateError::Unsupported)
    }

    fn apply_update(&mut self, _update: &GovernanceUpdate) -> Result<(), PolicyUpdateError> {
        Err(PolicyUpdateError::Unsupported)
    }

    fn name(&self) -> &'static str {
        "static"
    }
}

#[derive(Debug, Deserialize)]
struct AllowListFile {
    allowed: Vec<String>,
}

// ---------------------------------------------------------------------
// Multisig policy
// ---------------------------------------------------------------------

/// Governance backend secured by a K-of-N signer set.
pub struct MultisigPolicy {
    state_path: PathBuf,
    threshold: usize,
    signers: HashSet<VerifyingKey>,
    members: Vec<VerifyingKey>,
}

#[derive(Debug, Serialize, Deserialize)]
struct MultisigState {
    threshold: usize,
    signers: Vec<String>,
    members: Vec<String>,
}

impl MultisigPolicy {
    /// Restores a multisig policy from the provided state file.
    pub fn load(path: &Path) -> Result<Self, PolicyUpdateError> {
        let contents =
            fs::read_to_string(path).map_err(|err| PolicyUpdateError::Io(err.to_string()))?;
        let state: MultisigState = serde_json::from_str(&contents)
            .map_err(|err| PolicyUpdateError::Decode(err.to_string()))?;
        let threshold = state.threshold;
        let signers = state
            .signers
            .iter()
            .map(|b64| decode_public_key(b64))
            .collect::<Result<HashSet<_>, _>>()?;
        let members = state
            .members
            .iter()
            .map(|b64| decode_public_key(b64))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            state_path: path.to_path_buf(),
            threshold,
            signers,
            members,
        })
    }

    fn persist(&self) -> Result<(), PolicyUpdateError> {
        let state = MultisigState {
            threshold: self.threshold,
            signers: self
                .signers
                .iter()
                .map(|vk| encode_public_key_base64(vk))
                .collect(),
            members: self
                .members
                .iter()
                .map(|vk| encode_public_key_base64(vk))
                .collect(),
        };
        let canonical = serde_json::to_string_pretty(&state).unwrap();
        fs::write(&self.state_path, canonical).map_err(|err| PolicyUpdateError::Io(err.to_string()))
    }
}

impl MembershipPolicy for MultisigPolicy {
    fn current_members(&self) -> Vec<VerifyingKey> {
        self.members.clone()
    }

    fn verify_update(&self, update: &GovernanceUpdate) -> Result<(), PolicyUpdateError> {
        if update.signatures.is_empty() {
            return Err(PolicyUpdateError::Threshold {
                required: self.threshold,
                actual: 0,
            });
        }
        let canonical = canonical_update_payload(update)?;
        let mut approvals = 0usize;
        let mut seen: HashSet<[u8; PUBLIC_KEY_LENGTH]> = HashSet::new();
        for approval in &update.signatures {
            let signer = decode_public_key(&approval.signer)?;
            if !self.signers.contains(&signer) {
                return Err(PolicyUpdateError::Unauthorized);
            }
            if !seen.insert(signer.to_bytes()) {
                continue;
            }
            let signature_bytes = BASE64
                .decode(&approval.signature)
                .map_err(|err| PolicyUpdateError::Decode(err.to_string()))?;
            if signature_bytes.len() != SIGNATURE_LENGTH {
                return Err(PolicyUpdateError::Decode("invalid signature length".into()));
            }
            let sig_array: [u8; SIGNATURE_LENGTH] = signature_bytes
                .as_slice()
                .try_into()
                .expect("signature length checked");
            let signature = Signature::from_bytes(&sig_array);
            signer
                .verify_strict(&canonical, &signature)
                .map_err(|_| PolicyUpdateError::BadSignature)?;
            approvals += 1;
        }
        if approvals < self.threshold {
            return Err(PolicyUpdateError::Threshold {
                required: self.threshold,
                actual: approvals,
            });
        }
        Ok(())
    }

    fn apply_update(&mut self, update: &GovernanceUpdate) -> Result<(), PolicyUpdateError> {
        self.verify_update(update)?;
        let mut members = Vec::new();
        for base64 in &update.new_members {
            let vk = decode_public_key(base64)?;
            members.push(vk);
        }
        self.members = members;
        self.persist()
    }

    fn name(&self) -> &'static str {
        "multisig"
    }
}

// ---------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------

fn decode_public_key(input: &str) -> Result<VerifyingKey, PolicyUpdateError> {
    let decoded = BASE64
        .decode(input)
        .map_err(|err| PolicyUpdateError::Decode(err.to_string()))?;
    if decoded.len() != PUBLIC_KEY_LENGTH {
        return Err(PolicyUpdateError::Decode(
            "unexpected public key length".into(),
        ));
    }
    VerifyingKey::from_bytes(decoded.as_slice().try_into().unwrap())
        .map_err(|err| PolicyUpdateError::Decode(err.to_string()))
}

fn canonical_update_payload(update: &GovernanceUpdate) -> Result<Vec<u8>, PolicyUpdateError> {
    #[derive(Serialize)]
    struct Canonical<'a> {
        new_members: &'a [String],
        metadata: &'a Option<serde_json::Value>,
    }
    serde_json::to_vec(&Canonical {
        new_members: &update.new_members,
        metadata: &update.metadata,
    })
    .map_err(|err| PolicyUpdateError::Decode(err.to_string()))
}
