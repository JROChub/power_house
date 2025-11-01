#![cfg(feature = "net")]

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey, SECRET_KEY_LENGTH};
use libp2p::identity;
use rand_core::OsRng;
use sha2::{Digest, Sha512};
use std::{
    error::Error,
    fmt, fs,
    path::{Path, PathBuf},
};

/// Describes how an ed25519 key should be obtained.
#[derive(Debug, Clone)]
pub enum Ed25519KeySource {
    /// Deterministic key derived from an `ed25519://` seed string.
    Seed(String),
    /// Load the secret key material from the provided file path.
    File(PathBuf),
    /// Use a freshly generated random key (fallback when nothing is supplied).
    Random,
}

impl Ed25519KeySource {
    /// Parses a `--key` CLI argument into a concrete key source.
    pub fn from_spec(spec: Option<&str>) -> Self {
        match spec {
            Some(value) if value.starts_with("ed25519://") => {
                Self::Seed(value.trim_start_matches("ed25519://").to_string())
            }
            Some(value) if !value.is_empty() => Self::File(PathBuf::from(value)),
            _ => Self::Random,
        }
    }
}

/// Cached ed25519 key material used by the networking layer.
#[derive(Debug, Clone)]
pub struct KeyMaterial {
    /// Signing key used to produce ed25519 signatures.
    pub signing: SigningKey,
    /// Verifying key associated with `signing`.
    pub verifying: VerifyingKey,
    /// Libp2p keypair derived from the same secret bytes.
    pub libp2p: identity::Keypair,
}

/// Errors reported while loading or decoding key material.
#[derive(Debug, Clone)]
pub enum KeyError {
    /// Underlying filesystem I/O failure.
    Io(String),
    /// Base64, hex, or ed25519 parsing failure.
    Decode(String),
    /// Buffer did not match the expected secret-key length.
    InvalidLength(usize),
}

impl fmt::Display for KeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "key I/O error: {err}"),
            Self::Decode(err) => write!(f, "key decode error: {err}"),
            Self::InvalidLength(len) => write!(f, "unexpected key length: {len}"),
        }
    }
}

impl Error for KeyError {}

/// Loads or derives key material according to the source specification.
pub fn load_or_derive_keypair(source: &Ed25519KeySource) -> Result<KeyMaterial, KeyError> {
    let secret_bytes = match source {
        Ed25519KeySource::Seed(seed) => derive_key_from_seed(seed),
        Ed25519KeySource::File(path) => load_key_from_file(path),
        Ed25519KeySource::Random => generate_random_key(),
    }?;
    key_material_from_secret(secret_bytes)
}

/// Encodes a public key as base64.
pub fn encode_public_key_base64(verifying: &VerifyingKey) -> String {
    BASE64.encode(verifying.to_bytes())
}

/// Encodes a signature as base64.
pub fn encode_signature_base64(sig: &Signature) -> String {
    BASE64.encode(sig.to_bytes())
}

/// Decodes a base64 signature.
pub fn decode_signature_base64(input: &str) -> Result<Signature, KeyError> {
    let bytes = BASE64
        .decode(input)
        .map_err(|err| KeyError::Decode(err.to_string()))?;
    Signature::from_slice(&bytes).map_err(|err| KeyError::Decode(err.to_string()))
}

/// Decodes a base64 public key.
pub fn decode_public_key_base64(input: &str) -> Result<VerifyingKey, KeyError> {
    let bytes = BASE64
        .decode(input)
        .map_err(|err| KeyError::Decode(err.to_string()))?;
    VerifyingKey::try_from(bytes.as_slice()).map_err(|err| KeyError::Decode(err.to_string()))
}

fn derive_key_from_seed(seed: &str) -> Result<[u8; SECRET_KEY_LENGTH], KeyError> {
    let mut hasher = Sha512::new();
    hasher.update(seed.as_bytes());
    let digest = hasher.finalize();
    let mut secret = [0u8; SECRET_KEY_LENGTH];
    secret.copy_from_slice(&digest[..SECRET_KEY_LENGTH]);
    Ok(secret)
}

fn derive_key_from_passphrase(passphrase: &str) -> [u8; SECRET_KEY_LENGTH] {
    let mut hasher = Sha512::new();
    hasher.update(passphrase.as_bytes());
    let digest = hasher.finalize();
    let mut key = [0u8; SECRET_KEY_LENGTH];
    key.copy_from_slice(&digest[..SECRET_KEY_LENGTH]);
    key
}

fn load_key_from_file(path: &Path) -> Result<[u8; SECRET_KEY_LENGTH], KeyError> {
    let contents = fs::read(path).map_err(|err| KeyError::Io(err.to_string()))?;
    if contents.len() == SECRET_KEY_LENGTH {
        let mut secret = [0u8; SECRET_KEY_LENGTH];
        secret.copy_from_slice(&contents);
        return Ok(secret);
    }
    if let Ok(text) = std::str::from_utf8(&contents) {
        let trimmed = text.trim();
        if trimmed.len() == SECRET_KEY_LENGTH * 2 && trimmed.chars().all(|c| c.is_ascii_hexdigit())
        {
            let mut secret = [0u8; SECRET_KEY_LENGTH];
            for (idx, chunk) in trimmed.as_bytes().chunks(2).enumerate() {
                let hex = std::str::from_utf8(chunk).unwrap();
                secret[idx] =
                    u8::from_str_radix(hex, 16).map_err(|err| KeyError::Decode(err.to_string()))?;
            }
            return Ok(secret);
        }
        if let Ok(decoded) = BASE64.decode(trimmed) {
            return bytes_to_secret(decoded);
        }
    }
    bytes_to_secret(contents)
}

fn generate_random_key() -> Result<[u8; SECRET_KEY_LENGTH], KeyError> {
    let mut rng = OsRng;
    let signing = SigningKey::generate(&mut rng);
    Ok(signing.to_bytes())
}

/// Loads a passphrase-protected identity file.
pub fn load_encrypted_identity(path: &Path, passphrase: &str) -> Result<KeyMaterial, KeyError> {
    let contents = fs::read_to_string(path).map_err(|err| KeyError::Io(err.to_string()))?;
    let cipher = BASE64
        .decode(contents.trim())
        .map_err(|err| KeyError::Decode(err.to_string()))?;
    if cipher.len() != SECRET_KEY_LENGTH {
        return Err(KeyError::InvalidLength(cipher.len()));
    }
    let mask = derive_key_from_passphrase(passphrase);
    let mut secret = [0u8; SECRET_KEY_LENGTH];
    for (idx, byte) in cipher.iter().enumerate() {
        secret[idx] = byte ^ mask[idx];
    }
    key_material_from_secret(secret)
}

fn keypair_from_secret(secret: &[u8; SECRET_KEY_LENGTH]) -> Result<identity::Keypair, KeyError> {
    let secret = identity::ed25519::SecretKey::try_from_bytes(*secret)
        .map_err(|err| KeyError::Decode(err.to_string()))?;
    let ed = identity::ed25519::Keypair::from(secret);
    Ok(identity::Keypair::from(ed))
}

fn key_material_from_secret(
    secret_bytes: [u8; SECRET_KEY_LENGTH],
) -> Result<KeyMaterial, KeyError> {
    let signing = SigningKey::from_bytes(&secret_bytes);
    let verifying = signing.verifying_key();
    let libp2p = keypair_from_secret(&secret_bytes)?;
    Ok(KeyMaterial {
        signing,
        verifying,
        libp2p,
    })
}

fn bytes_to_secret(bytes: impl AsRef<[u8]>) -> Result<[u8; SECRET_KEY_LENGTH], KeyError> {
    let bytes = bytes.as_ref();
    if bytes.len() != SECRET_KEY_LENGTH {
        return Err(KeyError::InvalidLength(bytes.len()));
    }
    let mut secret = [0u8; SECRET_KEY_LENGTH];
    secret.copy_from_slice(bytes);
    Ok(secret)
}

/// Signs a payload with the provided signing key.
pub fn sign_payload(signing: &SigningKey, payload: &[u8]) -> Signature {
    signing.sign(payload)
}

/// Verifies a signature against the payload using the given verifying key.
pub fn verify_signature(
    verifying: &VerifyingKey,
    payload: &[u8],
    signature: &Signature,
) -> Result<(), KeyError> {
    verifying
        .verify(payload, signature)
        .map_err(|err| KeyError::Decode(err.to_string()))
}

/// Helper that derives a verifying key from a base64 string and checks the signature.
pub fn verify_signature_base64(
    public_key_b64: &str,
    payload: &[u8],
    signature_b64: &str,
) -> Result<(), KeyError> {
    let verifying = decode_public_key_base64(public_key_b64)?;
    let signature = decode_signature_base64(signature_b64)?;
    verify_signature(&verifying, payload, &signature)
}
