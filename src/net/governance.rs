#![cfg(feature = "net")]

use crate::net::sign::encode_public_key_base64;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use blake2::digest::{consts::U32, Digest};
use ed25519_dalek::{Signature, VerifyingKey, PUBLIC_KEY_LENGTH, SIGNATURE_LENGTH};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};
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

    /// Records a slash event for the supplied identity. Default: no-op.
    fn record_slash(&self, _key: &VerifyingKey) -> Result<(), PolicyUpdateError> {
        Ok(())
    }

    /// Returns the bonded stake weight for a key, if tracked.
    fn stake_for(&self, _key: &VerifyingKey) -> Option<u64> {
        None
    }
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

/// Governance proposal that freezes internal staking and maps stake to a public token.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationProposal {
    /// Ledger height selected for deterministic snapshotting.
    pub snapshot_height: u64,
    /// Token identifier used for migration claims (for example `native://julian`).
    pub token_contract: String,
    /// Stake-to-token conversion ratio (defaults to 1 when omitted).
    #[serde(default = "default_conversion_ratio")]
    pub conversion_ratio: u64,
    /// Treasury mint amount applied at migration cutover.
    pub treasury_mint: u64,
}

/// Canonical migration anchor payload embedded into standard net anchors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationAnchor {
    /// Schema identifier for migration anchor payloads.
    pub schema: String,
    /// Canonical migration proposal.
    pub proposal: MigrationProposal,
    /// BLAKE2b-256 hash of the proposal canonical payload.
    pub proposal_hash: String,
    /// Deterministic anchor statement written into ledger entries.
    pub statement: String,
}

fn default_conversion_ratio() -> u64 {
    1
}

impl MigrationProposal {
    /// Return canonical JSON bytes for deterministic hashing/anchoring.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, PolicyUpdateError> {
        #[derive(Serialize)]
        struct Canonical<'a> {
            snapshot_height: u64,
            token_contract: &'a str,
            conversion_ratio: u64,
            treasury_mint: u64,
        }
        let payload = Canonical {
            snapshot_height: self.snapshot_height,
            token_contract: &self.token_contract,
            conversion_ratio: if self.conversion_ratio == 0 {
                1
            } else {
                self.conversion_ratio
            },
            treasury_mint: self.treasury_mint,
        };
        serde_json::to_vec(&payload).map_err(|err| PolicyUpdateError::Decode(err.to_string()))
    }

    /// Return the BLAKE2b-256 hash hex of the canonical proposal payload.
    pub fn proposal_hash_hex(&self) -> Result<String, PolicyUpdateError> {
        type Blake2b256 = blake2::Blake2b<U32>;
        let canonical = self.canonical_bytes()?;
        let mut hasher = Blake2b256::new();
        hasher.update(b"migration-proposal-v1");
        hasher.update(canonical);
        Ok(hex::encode(hasher.finalize()))
    }

    /// Convert a proposal to a deterministic migration anchor payload.
    pub fn to_anchor_payload(&self) -> Result<MigrationAnchor, PolicyUpdateError> {
        let proposal_hash = self.proposal_hash_hex()?;
        Ok(MigrationAnchor {
            schema: "mfenx.powerhouse.migration-anchor.v1".to_string(),
            proposal: self.clone(),
            statement: format!("migration.proposal.{proposal_hash}"),
            proposal_hash,
        })
    }
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
            signers: self.signers.iter().map(encode_public_key_base64).collect(),
            members: self.members.iter().map(encode_public_key_base64).collect(),
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
// Stake-backed policy
// ---------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize, Clone)]
struct StakeEntry {
    public_key: String,
    bond: u64,
    slashed: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct StakeState {
    threshold: usize,
    bond_threshold: u64,
    signers: Vec<String>,
    entries: Vec<StakeEntry>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct StakeUpdateMetadata {
    #[serde(default)]
    deposits: Vec<StakeDeposit>,
    #[serde(default)]
    slashes: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct StakeDeposit {
    public_key: String,
    bond: u64,
}

#[derive(Clone)]
struct StakeAccount {
    key: VerifyingKey,
    bond: u64,
    slashed: bool,
}

/// Bond-backed membership policy derived from a staking registry.
pub struct StakePolicy {
    state_path: PathBuf,
    threshold: usize,
    bond_threshold: u64,
    slash_pct: u8,
    signers: HashSet<VerifyingKey>,
    state: Mutex<HashMap<Vec<u8>, StakeAccount>>,
}

impl StakePolicy {
    /// Restores staking state from a JSON registry.
    pub fn load(
        path: &Path,
        min_stake: Option<u64>,
        slash_pct: Option<u8>,
    ) -> Result<Self, PolicyUpdateError> {
        let contents =
            fs::read_to_string(path).map_err(|err| PolicyUpdateError::Io(err.to_string()))?;
        let state: StakeState = serde_json::from_str(&contents)
            .map_err(|err| PolicyUpdateError::Decode(err.to_string()))?;
        let threshold = state.threshold;
        let bond_threshold = min_stake.unwrap_or(state.bond_threshold);
        let slash_pct = slash_pct.unwrap_or(100).min(100);
        let signers = state
            .signers
            .iter()
            .map(|b64| decode_public_key(b64))
            .collect::<Result<HashSet<_>, _>>()?;

        let mut registry = HashMap::new();
        for entry in state.entries {
            let vk = decode_public_key(&entry.public_key)?;
            registry.insert(
                vk.to_bytes().to_vec(),
                StakeAccount {
                    key: vk,
                    bond: entry.bond,
                    slashed: entry.slashed,
                },
            );
        }

        Ok(Self {
            state_path: path.to_path_buf(),
            threshold,
            bond_threshold,
            slash_pct,
            signers,
            state: Mutex::new(registry),
        })
    }

    fn apply_slash(&self, account: &mut StakeAccount) {
        if account.bond == 0 {
            account.slashed = true;
            return;
        }
        let pct = self.slash_pct as u128;
        if pct == 0 {
            return;
        }
        let slash_amount = ((account.bond as u128) * pct / 100) as u64;
        account.bond = account.bond.saturating_sub(slash_amount.max(1));
        if account.bond == 0 {
            account.slashed = true;
        }
    }

    fn persist(&self, locked: &HashMap<Vec<u8>, StakeAccount>) -> Result<(), PolicyUpdateError> {
        let entries = locked
            .values()
            .map(|account| StakeEntry {
                public_key: encode_public_key_base64(&account.key),
                bond: account.bond,
                slashed: account.slashed,
            })
            .collect::<Vec<_>>();
        let state = StakeState {
            threshold: self.threshold,
            bond_threshold: self.bond_threshold,
            signers: self.signers.iter().map(encode_public_key_base64).collect(),
            entries,
        };
        let pretty = serde_json::to_string_pretty(&state)
            .map_err(|err| PolicyUpdateError::Decode(err.to_string()))?;
        fs::write(&self.state_path, pretty).map_err(|err| PolicyUpdateError::Io(err.to_string()))
    }

    fn parse_metadata(
        &self,
        update: &GovernanceUpdate,
    ) -> Result<StakeUpdateMetadata, PolicyUpdateError> {
        update
            .metadata
            .as_ref()
            .ok_or_else(|| PolicyUpdateError::Decode("stake update requires metadata".into()))
            .and_then(|meta| {
                serde_json::from_value(meta.clone())
                    .map_err(|err| PolicyUpdateError::Decode(err.to_string()))
            })
    }

    fn verify_update_with_metadata(
        &self,
        update: &GovernanceUpdate,
    ) -> Result<StakeUpdateMetadata, PolicyUpdateError> {
        if update.signatures.is_empty() {
            return Err(PolicyUpdateError::Threshold {
                required: self.threshold,
                actual: 0,
            });
        }
        let metadata = self.parse_metadata(update)?;
        let canonical = canonical_stake_payload(&metadata)?;

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
        for deposit in &metadata.deposits {
            if deposit.bond < self.bond_threshold {
                return Err(PolicyUpdateError::Decode(format!(
                    "deposit for {} below bond threshold",
                    deposit.public_key
                )));
            }
        }
        Ok(metadata)
    }
}

impl MembershipPolicy for StakePolicy {
    fn current_members(&self) -> Vec<VerifyingKey> {
        let guard = self.state.lock().expect("stake state poisoned");
        guard
            .values()
            .filter(|account| account.bond >= self.bond_threshold && !account.slashed)
            .map(|account| account.key)
            .collect()
    }

    fn verify_update(&self, update: &GovernanceUpdate) -> Result<(), PolicyUpdateError> {
        self.verify_update_with_metadata(update).map(|_| ())
    }

    fn apply_update(&mut self, update: &GovernanceUpdate) -> Result<(), PolicyUpdateError> {
        let metadata = self.verify_update_with_metadata(update)?;
        let mut guard = self.state.lock().expect("stake state poisoned");

        for deposit in metadata.deposits {
            let vk = decode_public_key(&deposit.public_key)?;
            guard.insert(
                vk.to_bytes().to_vec(),
                StakeAccount {
                    key: vk,
                    bond: deposit.bond,
                    slashed: false,
                },
            );
        }
        for slash in metadata.slashes {
            let vk = decode_public_key(&slash)?;
            guard.entry(vk.to_bytes().to_vec()).and_modify(|account| {
                self.apply_slash(account);
            });
        }
        self.persist(&guard)
    }

    fn name(&self) -> &'static str {
        "stake"
    }

    fn record_slash(&self, key: &VerifyingKey) -> Result<(), PolicyUpdateError> {
        let mut guard = self.state.lock().expect("stake state poisoned");
        guard
            .entry(key.to_bytes().to_vec())
            .and_modify(|account| {
                self.apply_slash(account);
            })
            .or_insert(StakeAccount {
                key: *key,
                bond: 0,
                slashed: true,
            });
        self.persist(&guard)
    }

    fn stake_for(&self, key: &VerifyingKey) -> Option<u64> {
        let guard = self.state.lock().expect("stake state poisoned");
        guard
            .get(&key.to_bytes().to_vec())
            .filter(|acct| !acct.slashed && acct.bond >= self.bond_threshold)
            .map(|acct| acct.bond)
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

fn canonical_stake_payload(meta: &StakeUpdateMetadata) -> Result<Vec<u8>, PolicyUpdateError> {
    serde_json::to_vec(meta).map_err(|err| PolicyUpdateError::Decode(err.to_string()))
}
