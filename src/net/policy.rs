#![cfg(feature = "net")]

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde::Deserialize;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

/// Governs which peer identities are permitted to participate in quorum voting.
#[derive(Debug, Clone)]
pub struct IdentityPolicy {
    allow_all: bool,
    allowed: HashSet<Vec<u8>>,
}

impl IdentityPolicy {
    /// Returns a policy that accepts every key.
    pub fn allow_all() -> Self {
        Self {
            allow_all: true,
            allowed: HashSet::new(),
        }
    }

    /// Loads a policy from a JSON allowlist file.
    ///
    /// The expected format is:
    ///
    /// ```json
    /// { "allowed": ["<base64-ed25519-public-key>", "..."] }
    /// ```
    pub fn from_allowlist_path(path: &Path) -> Result<Self, PolicyError> {
        let contents = fs::read_to_string(path).map_err(|err| PolicyError::Io(err.to_string()))?;
        let parsed: AllowListFile =
            serde_json::from_str(&contents).map_err(|err| PolicyError::Parse(err.to_string()))?;
        let mut allowed = HashSet::new();
        for entry in parsed.allowed {
            let decoded = BASE64
                .decode(entry)
                .map_err(|err| PolicyError::Parse(err.to_string()))?;
            allowed.insert(decoded);
        }
        Ok(Self {
            allow_all: false,
            allowed,
        })
    }

    /// Returns true if the key is permitted by this policy.
    pub fn permits(&self, key: &[u8]) -> bool {
        self.allow_all || self.allowed.contains(key)
    }
}

#[derive(Debug, Clone, Deserialize)]
struct AllowListFile {
    allowed: Vec<String>,
}

/// Errors surfaced while decoding an identity policy.
#[derive(Debug, Clone)]
pub enum PolicyError {
    /// File-system failure while loading the allowlist.
    Io(String),
    /// The allowlist could not be parsed or decode operations failed.
    Parse(String),
}

impl std::fmt::Display for PolicyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "policy I/O error: {err}"),
            Self::Parse(err) => write!(f, "policy parse error: {err}"),
        }
    }
}

impl std::error::Error for PolicyError {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn allow_all_accepts_everything() {
        let policy = IdentityPolicy::allow_all();
        assert!(policy.permits(b"foo"));
    }

    #[test]
    fn allowlist_file_accepts_only_listed_keys() {
        let mut path = std::env::temp_dir();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        path.push(format!("allowlist_{nanos}.json"));
        let key = BASE64.encode(b"abc");
        fs::write(&path, format!("{{\"allowed\":[\"{key}\"]}}")).unwrap();
        let policy = IdentityPolicy::from_allowlist_path(&path).unwrap();
        fs::remove_file(&path).unwrap();
        assert!(policy.permits(b"abc"));
        assert!(!policy.permits(b"def"));
    }
}
