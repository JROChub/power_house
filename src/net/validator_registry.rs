#![cfg(feature = "net")]

use crate::net::sign::{
    decode_public_key_base64, encode_public_key_base64, encode_signature_base64, sign_payload,
    verify_signature_base64, KeyMaterial,
};
use libp2p::{identity, multiaddr::Protocol, Multiaddr, PeerId};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Schema identifier for a signed validator registration.
pub const VALIDATOR_REGISTRATION_SCHEMA: &str = "power-house-validator-registration-v1";
/// Schema identifier for an aggregate validator registry.
pub const VALIDATOR_REGISTRY_SCHEMA: &str = "power-house-validator-registry-v1";
/// Schema identifier for a signed public observer registration.
pub const OBSERVER_REGISTRATION_SCHEMA: &str = "power-house-observer-registration-v1";
/// Schema identifier for an aggregate public observer registry.
pub const OBSERVER_REGISTRY_SCHEMA: &str = "power-house-observer-registry-v1";

const SIGNING_DOMAIN: &[u8] = b"MFENX-POWERHOUSE:validator-registration:v1\0";
const OBSERVER_SIGNING_DOMAIN: &[u8] = b"MFENX-POWERHOUSE:observer-registration:v1\0";
const MAX_REGISTRATION_LIFETIME: u64 = 400 * 24 * 60 * 60;
const CLOCK_SKEW_SECONDS: u64 = 300;

/// A validator-controlled, signed declaration of its public monitoring endpoints.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidatorRegistration {
    /// Registration schema.
    pub schema: String,
    /// EVM/native chain identifier.
    pub chain_id: u64,
    /// Stable human-readable node identifier.
    pub node_id: String,
    /// Operator name or organization.
    pub operator: String,
    /// Deployment region identifier.
    pub region: String,
    /// Libp2p peer ID derived from `public_key_b64`.
    pub peer_id: String,
    /// Base64 Ed25519 public key used by the validator.
    pub public_key_b64: String,
    /// Public or private libp2p address ending in the claimed peer ID.
    pub p2p_address: String,
    /// Prometheus endpoint exposing validator and identity metrics.
    pub metrics_url: String,
    /// Optional node-exporter endpoint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_metrics_url: Option<String>,
    /// Registration creation time in Unix seconds.
    pub issued_at_unix: u64,
    /// Registration expiration time in Unix seconds.
    pub valid_until_unix: u64,
    /// Base64 Ed25519 signature over the canonical registration payload.
    pub signature_b64: String,
}

#[derive(Serialize)]
struct CanonicalRegistration<'a> {
    schema: &'a str,
    chain_id: u64,
    node_id: &'a str,
    operator: &'a str,
    region: &'a str,
    peer_id: &'a str,
    public_key_b64: &'a str,
    p2p_address: &'a str,
    metrics_url: &'a str,
    system_metrics_url: &'a Option<String>,
    issued_at_unix: u64,
    valid_until_unix: u64,
}

#[derive(Serialize)]
struct CanonicalObserverRegistration<'a> {
    schema: &'a str,
    chain_id: u64,
    node_id: &'a str,
    operator: &'a str,
    region: &'a str,
    peer_id: &'a str,
    public_key_b64: &'a str,
    p2p_address: &'a str,
    metrics_url: &'a str,
    system_metrics_url: &'a Option<String>,
    issued_at_unix: u64,
    valid_until_unix: u64,
}

impl ValidatorRegistration {
    /// Constructs and signs a validator registration using the node identity.
    #[allow(clippy::too_many_arguments)]
    pub fn sign(
        chain_id: u64,
        node_id: String,
        operator: String,
        region: String,
        p2p_address: String,
        metrics_url: String,
        system_metrics_url: Option<String>,
        issued_at_unix: u64,
        valid_until_unix: u64,
        key_material: &KeyMaterial,
    ) -> Result<Self, ValidatorRegistryError> {
        let public_key_b64 = encode_public_key_base64(&key_material.verifying);
        let peer_id = key_material.libp2p.public().to_peer_id().to_string();
        let mut registration = Self {
            schema: VALIDATOR_REGISTRATION_SCHEMA.to_string(),
            chain_id,
            node_id,
            operator,
            region,
            peer_id,
            public_key_b64,
            p2p_address,
            metrics_url,
            system_metrics_url,
            issued_at_unix,
            valid_until_unix,
            signature_b64: String::new(),
        };
        registration.validate_fields(chain_id, issued_at_unix)?;
        registration.signature_b64 = encode_signature_base64(&sign_payload(
            &key_material.signing,
            &registration.payload()?,
        ));
        Ok(registration)
    }

    /// Verifies schema, timing, endpoint constraints, identity binding, and signature.
    pub fn verify(
        &self,
        expected_chain_id: u64,
        now_unix: u64,
    ) -> Result<(), ValidatorRegistryError> {
        self.validate_fields(expected_chain_id, now_unix)?;
        let expected_peer = peer_id_from_public_key(&self.public_key_b64)?;
        if self.peer_id != expected_peer.to_string() {
            return Err(ValidatorRegistryError::Identity(
                "peer_id is not derived from public_key_b64".to_string(),
            ));
        }
        let address: Multiaddr = self.p2p_address.parse().map_err(|err| {
            ValidatorRegistryError::Endpoint(format!("invalid p2p address: {err}"))
        })?;
        let address_peer = address.iter().last().and_then(|protocol| match protocol {
            Protocol::P2p(peer) => Some(peer),
            _ => None,
        });
        if address_peer.as_ref() != Some(&expected_peer) {
            return Err(ValidatorRegistryError::Identity(
                "p2p_address does not end in the registered peer_id".to_string(),
            ));
        }
        let p2p_host = address
            .iter()
            .find_map(|protocol| match protocol {
                Protocol::Ip4(value) => Some(value.to_string()),
                Protocol::Ip6(value) => Some(value.to_string()),
                Protocol::Dns(value) | Protocol::Dns4(value) | Protocol::Dns6(value) => {
                    Some(value.to_string())
                }
                _ => None,
            })
            .ok_or_else(|| {
                ValidatorRegistryError::Endpoint(
                    "p2p_address must contain an IP or DNS host".to_string(),
                )
            })?;
        require_matching_host("metrics_url", &self.metrics_url, &p2p_host)?;
        if let Some(url) = &self.system_metrics_url {
            require_matching_host("system_metrics_url", url, &p2p_host)?;
        }
        verify_signature_base64(&self.public_key_b64, &self.payload()?, &self.signature_b64)
            .map_err(|err| ValidatorRegistryError::Signature(err.to_string()))
    }

    fn payload(&self) -> Result<Vec<u8>, ValidatorRegistryError> {
        let canonical = CanonicalRegistration {
            schema: &self.schema,
            chain_id: self.chain_id,
            node_id: &self.node_id,
            operator: &self.operator,
            region: &self.region,
            peer_id: &self.peer_id,
            public_key_b64: &self.public_key_b64,
            p2p_address: &self.p2p_address,
            metrics_url: &self.metrics_url,
            system_metrics_url: &self.system_metrics_url,
            issued_at_unix: self.issued_at_unix,
            valid_until_unix: self.valid_until_unix,
        };
        let mut payload = SIGNING_DOMAIN.to_vec();
        payload.extend(
            serde_json::to_vec(&canonical)
                .map_err(|err| ValidatorRegistryError::Encoding(err.to_string()))?,
        );
        Ok(payload)
    }

    fn validate_fields(
        &self,
        expected_chain_id: u64,
        now_unix: u64,
    ) -> Result<(), ValidatorRegistryError> {
        if self.schema != VALIDATOR_REGISTRATION_SCHEMA {
            return Err(ValidatorRegistryError::Schema(self.schema.clone()));
        }
        if self.chain_id != expected_chain_id {
            return Err(ValidatorRegistryError::ChainId {
                expected: expected_chain_id,
                actual: self.chain_id,
            });
        }
        validate_identifier("node_id", &self.node_id)?;
        validate_identifier("region", &self.region)?;
        if self.operator.trim().is_empty() || self.operator.len() > 128 {
            return Err(ValidatorRegistryError::Field(
                "operator must contain 1 to 128 characters".to_string(),
            ));
        }
        if self.issued_at_unix > now_unix.saturating_add(CLOCK_SKEW_SECONDS) {
            return Err(ValidatorRegistryError::Timing(
                "registration issue time is in the future".to_string(),
            ));
        }
        if self.valid_until_unix <= now_unix {
            return Err(ValidatorRegistryError::Timing(
                "registration has expired".to_string(),
            ));
        }
        let lifetime = self
            .valid_until_unix
            .checked_sub(self.issued_at_unix)
            .ok_or_else(|| {
                ValidatorRegistryError::Timing(
                    "registration expiration precedes issue time".to_string(),
                )
            })?;
        if lifetime == 0 || lifetime > MAX_REGISTRATION_LIFETIME {
            return Err(ValidatorRegistryError::Timing(format!(
                "registration lifetime must be between 1 and {MAX_REGISTRATION_LIFETIME} seconds"
            )));
        }
        validate_metrics_url("metrics_url", &self.metrics_url)?;
        if let Some(url) = &self.system_metrics_url {
            validate_metrics_url("system_metrics_url", url)?;
        }
        decode_public_key_base64(&self.public_key_b64)
            .map_err(|err| ValidatorRegistryError::Identity(err.to_string()))?;
        Ok(())
    }
}

/// A set of individually signed validator registrations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValidatorRegistry {
    /// Registry schema.
    pub schema: String,
    /// Chain identifier shared by every registration.
    pub chain_id: u64,
    /// Signed validator records.
    pub registrations: Vec<ValidatorRegistration>,
}

impl ValidatorRegistry {
    /// Verifies all records and requires every identity to be admitted by policy.
    pub fn verify(
        &self,
        admitted_public_keys: &HashSet<String>,
        now_unix: u64,
    ) -> Result<(), ValidatorRegistryError> {
        if self.schema != VALIDATOR_REGISTRY_SCHEMA {
            return Err(ValidatorRegistryError::Schema(self.schema.clone()));
        }
        if self.chain_id == 0 {
            return Err(ValidatorRegistryError::Field(
                "chain_id must be non-zero".to_string(),
            ));
        }
        if self.registrations.is_empty() {
            return Err(ValidatorRegistryError::Field(
                "registry must contain at least one registration".to_string(),
            ));
        }

        let mut node_ids = HashSet::new();
        let mut peer_ids = HashSet::new();
        let mut public_keys = HashSet::new();
        let mut metrics_urls = HashSet::new();
        for registration in &self.registrations {
            registration.verify(self.chain_id, now_unix)?;
            if !admitted_public_keys.contains(&registration.public_key_b64) {
                return Err(ValidatorRegistryError::Admission(
                    registration.node_id.clone(),
                ));
            }
            require_unique(&mut node_ids, &registration.node_id, "node_id")?;
            require_unique(&mut peer_ids, &registration.peer_id, "peer_id")?;
            require_unique(
                &mut public_keys,
                &registration.public_key_b64,
                "public_key_b64",
            )?;
            require_unique(&mut metrics_urls, &registration.metrics_url, "metrics_url")?;
        }
        Ok(())
    }
}

/// A public observer-controlled, signed declaration of its monitoring endpoints.
///
/// Observers are permissionless monitoring participants. They can be discovered,
/// identity-checked, and displayed publicly, but they are never counted as
/// consensus validators by this registry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObserverRegistration {
    /// Registration schema.
    pub schema: String,
    /// EVM/native chain identifier.
    pub chain_id: u64,
    /// Stable human-readable node identifier.
    pub node_id: String,
    /// Operator name or organization.
    pub operator: String,
    /// Deployment region identifier.
    pub region: String,
    /// Libp2p peer ID derived from `public_key_b64`.
    pub peer_id: String,
    /// Base64 Ed25519 public key used by the observer.
    pub public_key_b64: String,
    /// Public libp2p address ending in the claimed peer ID.
    pub p2p_address: String,
    /// Prometheus endpoint exposing observer and identity metrics.
    pub metrics_url: String,
    /// Optional node-exporter endpoint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_metrics_url: Option<String>,
    /// Registration creation time in Unix seconds.
    pub issued_at_unix: u64,
    /// Registration expiration time in Unix seconds.
    pub valid_until_unix: u64,
    /// Base64 Ed25519 signature over the canonical registration payload.
    pub signature_b64: String,
}

impl ObserverRegistration {
    /// Constructs and signs an observer registration using the node identity.
    #[allow(clippy::too_many_arguments)]
    pub fn sign(
        chain_id: u64,
        node_id: String,
        operator: String,
        region: String,
        p2p_address: String,
        metrics_url: String,
        system_metrics_url: Option<String>,
        issued_at_unix: u64,
        valid_until_unix: u64,
        key_material: &KeyMaterial,
    ) -> Result<Self, ValidatorRegistryError> {
        let public_key_b64 = encode_public_key_base64(&key_material.verifying);
        let peer_id = key_material.libp2p.public().to_peer_id().to_string();
        let mut registration = Self {
            schema: OBSERVER_REGISTRATION_SCHEMA.to_string(),
            chain_id,
            node_id,
            operator,
            region,
            peer_id,
            public_key_b64,
            p2p_address,
            metrics_url,
            system_metrics_url,
            issued_at_unix,
            valid_until_unix,
            signature_b64: String::new(),
        };
        registration.validate_fields(chain_id, issued_at_unix)?;
        registration.signature_b64 = encode_signature_base64(&sign_payload(
            &key_material.signing,
            &registration.payload()?,
        ));
        Ok(registration)
    }

    /// Verifies schema, timing, endpoint constraints, identity binding, and signature.
    pub fn verify(
        &self,
        expected_chain_id: u64,
        now_unix: u64,
    ) -> Result<(), ValidatorRegistryError> {
        self.validate_fields(expected_chain_id, now_unix)?;
        let expected_peer = peer_id_from_public_key(&self.public_key_b64)?;
        if self.peer_id != expected_peer.to_string() {
            return Err(ValidatorRegistryError::Identity(
                "peer_id is not derived from public_key_b64".to_string(),
            ));
        }
        let address: Multiaddr = self.p2p_address.parse().map_err(|err| {
            ValidatorRegistryError::Endpoint(format!("invalid p2p address: {err}"))
        })?;
        let address_peer = address.iter().last().and_then(|protocol| match protocol {
            Protocol::P2p(peer) => Some(peer),
            _ => None,
        });
        if address_peer.as_ref() != Some(&expected_peer) {
            return Err(ValidatorRegistryError::Identity(
                "p2p_address does not end in the registered peer_id".to_string(),
            ));
        }
        let p2p_host = address
            .iter()
            .find_map(|protocol| match protocol {
                Protocol::Ip4(value) => Some(value.to_string()),
                Protocol::Ip6(value) => Some(value.to_string()),
                Protocol::Dns(value) | Protocol::Dns4(value) | Protocol::Dns6(value) => {
                    Some(value.to_string())
                }
                _ => None,
            })
            .ok_or_else(|| {
                ValidatorRegistryError::Endpoint(
                    "p2p_address must contain an IP or DNS host".to_string(),
                )
            })?;
        require_matching_host("metrics_url", &self.metrics_url, &p2p_host)?;
        if let Some(url) = &self.system_metrics_url {
            require_matching_host("system_metrics_url", url, &p2p_host)?;
        }
        verify_signature_base64(&self.public_key_b64, &self.payload()?, &self.signature_b64)
            .map_err(|err| ValidatorRegistryError::Signature(err.to_string()))
    }

    fn payload(&self) -> Result<Vec<u8>, ValidatorRegistryError> {
        let canonical = CanonicalObserverRegistration {
            schema: &self.schema,
            chain_id: self.chain_id,
            node_id: &self.node_id,
            operator: &self.operator,
            region: &self.region,
            peer_id: &self.peer_id,
            public_key_b64: &self.public_key_b64,
            p2p_address: &self.p2p_address,
            metrics_url: &self.metrics_url,
            system_metrics_url: &self.system_metrics_url,
            issued_at_unix: self.issued_at_unix,
            valid_until_unix: self.valid_until_unix,
        };
        let mut payload = OBSERVER_SIGNING_DOMAIN.to_vec();
        payload.extend(
            serde_json::to_vec(&canonical)
                .map_err(|err| ValidatorRegistryError::Encoding(err.to_string()))?,
        );
        Ok(payload)
    }

    fn validate_fields(
        &self,
        expected_chain_id: u64,
        now_unix: u64,
    ) -> Result<(), ValidatorRegistryError> {
        if self.schema != OBSERVER_REGISTRATION_SCHEMA {
            return Err(ValidatorRegistryError::Schema(self.schema.clone()));
        }
        if self.chain_id != expected_chain_id {
            return Err(ValidatorRegistryError::ChainId {
                expected: expected_chain_id,
                actual: self.chain_id,
            });
        }
        validate_identifier("node_id", &self.node_id)?;
        validate_identifier("region", &self.region)?;
        if self.operator.trim().is_empty() || self.operator.len() > 128 {
            return Err(ValidatorRegistryError::Field(
                "operator must contain 1 to 128 characters".to_string(),
            ));
        }
        if self.issued_at_unix > now_unix.saturating_add(CLOCK_SKEW_SECONDS) {
            return Err(ValidatorRegistryError::Timing(
                "registration issue time is in the future".to_string(),
            ));
        }
        if self.valid_until_unix <= now_unix {
            return Err(ValidatorRegistryError::Timing(
                "registration has expired".to_string(),
            ));
        }
        let lifetime = self
            .valid_until_unix
            .checked_sub(self.issued_at_unix)
            .ok_or_else(|| {
                ValidatorRegistryError::Timing(
                    "registration expiration precedes issue time".to_string(),
                )
            })?;
        if lifetime == 0 || lifetime > MAX_REGISTRATION_LIFETIME {
            return Err(ValidatorRegistryError::Timing(format!(
                "registration lifetime must be between 1 and {MAX_REGISTRATION_LIFETIME} seconds"
            )));
        }
        validate_metrics_url("metrics_url", &self.metrics_url)?;
        if let Some(url) = &self.system_metrics_url {
            validate_metrics_url("system_metrics_url", url)?;
        }
        decode_public_key_base64(&self.public_key_b64)
            .map_err(|err| ValidatorRegistryError::Identity(err.to_string()))?;
        Ok(())
    }
}

/// A set of signed public observer registrations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObserverRegistry {
    /// Registry schema.
    pub schema: String,
    /// Chain identifier shared by every registration.
    pub chain_id: u64,
    /// Signed observer records.
    pub registrations: Vec<ObserverRegistration>,
}

impl ObserverRegistry {
    /// Verifies all observer records without admitting them to validator quorum.
    pub fn verify(&self, now_unix: u64) -> Result<(), ValidatorRegistryError> {
        if self.schema != OBSERVER_REGISTRY_SCHEMA {
            return Err(ValidatorRegistryError::Schema(self.schema.clone()));
        }
        if self.chain_id == 0 {
            return Err(ValidatorRegistryError::Field(
                "chain_id must be non-zero".to_string(),
            ));
        }
        if self.registrations.is_empty() {
            return Err(ValidatorRegistryError::Field(
                "registry must contain at least one registration".to_string(),
            ));
        }

        let mut node_ids = HashSet::new();
        let mut peer_ids = HashSet::new();
        let mut public_keys = HashSet::new();
        let mut metrics_urls = HashSet::new();
        for registration in &self.registrations {
            registration.verify(self.chain_id, now_unix)?;
            require_unique(&mut node_ids, &registration.node_id, "node_id")?;
            require_unique(&mut peer_ids, &registration.peer_id, "peer_id")?;
            require_unique(
                &mut public_keys,
                &registration.public_key_b64,
                "public_key_b64",
            )?;
            require_unique(&mut metrics_urls, &registration.metrics_url, "metrics_url")?;
        }
        Ok(())
    }
}

fn peer_id_from_public_key(public_key_b64: &str) -> Result<PeerId, ValidatorRegistryError> {
    let verifying = decode_public_key_base64(public_key_b64)
        .map_err(|err| ValidatorRegistryError::Identity(err.to_string()))?;
    let public = identity::ed25519::PublicKey::try_from_bytes(&verifying.to_bytes())
        .map_err(|err| ValidatorRegistryError::Identity(err.to_string()))?;
    Ok(identity::PublicKey::from(public).to_peer_id())
}

fn validate_identifier(field: &str, value: &str) -> Result<(), ValidatorRegistryError> {
    let valid = !value.is_empty()
        && value.len() <= 64
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"-._".contains(&byte)
        })
        && value.as_bytes()[0].is_ascii_alphanumeric()
        && value.as_bytes()[value.len() - 1].is_ascii_alphanumeric();
    if valid {
        Ok(())
    } else {
        Err(ValidatorRegistryError::Field(format!(
            "{field} must be 1 to 64 lowercase alphanumeric, dash, dot, or underscore characters"
        )))
    }
}

fn validate_metrics_url(field: &str, value: &str) -> Result<(), ValidatorRegistryError> {
    let url = Url::parse(value).map_err(|err| ValidatorRegistryError::Endpoint(err.to_string()))?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.host_str().is_none()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != "/metrics"
        || url.port_or_known_default().is_none()
    {
        return Err(ValidatorRegistryError::Endpoint(format!(
            "{field} must be an http(s) URL ending exactly in /metrics without credentials, query, or fragment"
        )));
    }
    Ok(())
}

fn require_matching_host(
    field: &str,
    value: &str,
    p2p_host: &str,
) -> Result<(), ValidatorRegistryError> {
    let url = Url::parse(value).map_err(|err| ValidatorRegistryError::Endpoint(err.to_string()))?;
    if !url
        .host_str()
        .is_some_and(|host| host.eq_ignore_ascii_case(p2p_host))
    {
        return Err(ValidatorRegistryError::Endpoint(format!(
            "{field} host must match the p2p_address host"
        )));
    }
    Ok(())
}

fn require_unique(
    seen: &mut HashSet<String>,
    value: &str,
    field: &str,
) -> Result<(), ValidatorRegistryError> {
    if seen.insert(value.to_string()) {
        Ok(())
    } else {
        Err(ValidatorRegistryError::Duplicate(field.to_string()))
    }
}

/// Validator registry validation failure.
#[derive(Debug, thiserror::Error)]
pub enum ValidatorRegistryError {
    /// Unsupported schema.
    #[error("unsupported validator registry schema: {0}")]
    Schema(String),
    /// Chain mismatch.
    #[error("validator registration chain mismatch: expected {expected}, got {actual}")]
    ChainId {
        /// Expected chain ID.
        expected: u64,
        /// Registration chain ID.
        actual: u64,
    },
    /// Invalid field.
    #[error("invalid validator registration field: {0}")]
    Field(String),
    /// Invalid timing.
    #[error("invalid validator registration timing: {0}")]
    Timing(String),
    /// Invalid endpoint.
    #[error("invalid validator endpoint: {0}")]
    Endpoint(String),
    /// Identity binding failed.
    #[error("validator identity mismatch: {0}")]
    Identity(String),
    /// Signature failed.
    #[error("validator registration signature failed: {0}")]
    Signature(String),
    /// Canonical encoding failed.
    #[error("validator registration encoding failed: {0}")]
    Encoding(String),
    /// Identity is not admitted by the consensus policy.
    #[error("validator {0} is not admitted by the validator policy")]
    Admission(String),
    /// A supposedly unique field was repeated.
    #[error("duplicate validator registration {0}")]
    Duplicate(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::{load_or_derive_keypair, Ed25519KeySource};

    fn registration(seed: &str, node_id: &str) -> ValidatorRegistration {
        let keys = load_or_derive_keypair(&Ed25519KeySource::Seed(seed.to_string())).expect("keys");
        let peer_id = keys.libp2p.public().to_peer_id();
        ValidatorRegistration::sign(
            177155,
            node_id.to_string(),
            "MFENX LLC".to_string(),
            "sfo3".to_string(),
            format!("/ip4/127.0.0.1/tcp/7001/p2p/{peer_id}"),
            "http://127.0.0.1:9100/metrics".to_string(),
            Some("http://127.0.0.1:9101/metrics".to_string()),
            1_000,
            2_000,
            &keys,
        )
        .expect("registration")
    }

    fn observer(seed: &str, node_id: &str) -> ObserverRegistration {
        let keys = load_or_derive_keypair(&Ed25519KeySource::Seed(seed.to_string())).expect("keys");
        let peer_id = keys.libp2p.public().to_peer_id();
        ObserverRegistration::sign(
            177155,
            node_id.to_string(),
            "Independent Observer".to_string(),
            "lax1".to_string(),
            format!("/ip4/127.0.0.1/tcp/7001/p2p/{peer_id}"),
            "http://127.0.0.1:9100/metrics".to_string(),
            None,
            1_000,
            2_000,
            &keys,
        )
        .expect("observer registration")
    }

    #[test]
    fn signed_registration_binds_public_key_peer_id_and_address() {
        registration("validator-a", "validator-a")
            .verify(177155, 1_500)
            .expect("valid registration");
    }

    #[test]
    fn identity_and_signed_fields_reject_mutation() {
        let mut item = registration("validator-a", "validator-a");
        item.region = "nyc3".to_string();
        assert!(matches!(
            item.verify(177155, 1_500),
            Err(ValidatorRegistryError::Signature(_))
        ));

        let mut item = registration("validator-a", "validator-a");
        item.peer_id = registration("validator-b", "validator-b").peer_id;
        assert!(matches!(
            item.verify(177155, 1_500),
            Err(ValidatorRegistryError::Identity(_))
        ));
    }

    #[test]
    fn expiration_and_endpoint_constraints_are_enforced() {
        let item = registration("validator-a", "validator-a");
        assert!(matches!(
            item.verify(177155, 2_000),
            Err(ValidatorRegistryError::Timing(_))
        ));

        let mut item = registration("validator-a", "validator-a");
        item.metrics_url = "file:///etc/passwd".to_string();
        assert!(matches!(
            item.verify(177155, 1_500),
            Err(ValidatorRegistryError::Endpoint(_))
        ));
    }

    #[test]
    fn registry_requires_admission_and_unique_identities() {
        let item = registration("validator-a", "validator-a");
        let registry = ValidatorRegistry {
            schema: VALIDATOR_REGISTRY_SCHEMA.to_string(),
            chain_id: 177155,
            registrations: vec![item.clone()],
        };
        let admitted = HashSet::from([item.public_key_b64.clone()]);
        registry.verify(&admitted, 1_500).expect("admitted");
        assert!(matches!(
            registry.verify(&HashSet::new(), 1_500),
            Err(ValidatorRegistryError::Admission(_))
        ));

        let duplicate = ValidatorRegistry {
            registrations: vec![item.clone(), item],
            ..registry
        };
        assert!(matches!(
            duplicate.verify(&admitted, 1_500),
            Err(ValidatorRegistryError::Duplicate(_))
        ));
    }

    #[test]
    fn signed_observer_registration_verifies_without_validator_admission() {
        observer("observer-a", "observer-a")
            .verify(177155, 1_500)
            .expect("valid observer registration");
    }

    #[test]
    fn observer_identity_and_signed_fields_reject_mutation() {
        let mut item = observer("observer-a", "observer-a");
        item.region = "nyc3".to_string();
        assert!(matches!(
            item.verify(177155, 1_500),
            Err(ValidatorRegistryError::Signature(_))
        ));

        let mut item = observer("observer-a", "observer-a");
        item.peer_id = observer("observer-b", "observer-b").peer_id;
        assert!(matches!(
            item.verify(177155, 1_500),
            Err(ValidatorRegistryError::Identity(_))
        ));
    }

    #[test]
    fn observer_registry_requires_unique_signed_identities() {
        let item = observer("observer-a", "observer-a");
        let registry = ObserverRegistry {
            schema: OBSERVER_REGISTRY_SCHEMA.to_string(),
            chain_id: 177155,
            registrations: vec![item.clone()],
        };
        registry.verify(1_500).expect("observer registry");

        let duplicate = ObserverRegistry {
            registrations: vec![item.clone(), item],
            ..registry
        };
        assert!(matches!(
            duplicate.verify(1_500),
            Err(ValidatorRegistryError::Duplicate(_))
        ));
    }
}
