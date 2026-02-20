#![cfg(feature = "net")]

use crate::commands::stake_snapshot::{StakeSnapshotArtifact, StakeSnapshotEntry};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use blake2::digest::{consts::U32, Digest as BlakeDigest};
use serde::{Deserialize, Serialize};
use sha3::Keccak256;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Options for building a deterministic migration claim manifest.
#[derive(Debug, Clone)]
pub struct BuildClaimsOptions {
    /// Claim amount source: `stake`, `balance`, or `total`.
    pub amount_source: String,
    /// Include slashed accounts in output claims.
    pub include_slashed: bool,
    /// Conversion ratio to compute `mint_amount` previews.
    pub conversion_ratio: u64,
    /// Domain separator used for deterministic claim-id derivation.
    pub claim_id_salt: String,
    /// Optional token identifier included in the artifact metadata.
    pub token_contract: Option<String>,
    /// Optional override for the snapshot height embedded in each claim leaf.
    pub snapshot_height_override: Option<u64>,
    /// Claim mode: `native` (default) or `erc20`.
    pub claim_mode: String,
}

#[derive(Debug, Clone, Copy)]
enum AmountSource {
    Stake,
    Balance,
    Total,
}

impl AmountSource {
    fn parse(raw: &str) -> Result<Self, String> {
        match raw {
            "stake" => Ok(Self::Stake),
            "balance" => Ok(Self::Balance),
            "total" => Ok(Self::Total),
            other => Err(format!(
                "invalid --amount-source '{other}' (expected stake|balance|total)"
            )),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Stake => "stake",
            Self::Balance => "balance",
            Self::Total => "total",
        }
    }

    fn amount_for(self, entry: &StakeSnapshotEntry) -> u64 {
        match self {
            Self::Stake => entry.stake,
            Self::Balance => entry.balance,
            Self::Total => entry.stake.saturating_add(entry.balance),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum ClaimMode {
    Native,
    Erc20,
}

impl ClaimMode {
    fn parse(raw: &str) -> Result<Self, String> {
        match raw {
            "native" => Ok(Self::Native),
            "erc20" => Ok(Self::Erc20),
            other => Err(format!("invalid --mode '{other}' (expected native|erc20)")),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Erc20 => "erc20",
        }
    }

    fn claim_id_format(self) -> &'static str {
        match self {
            Self::Native => {
                "blake2b256(claim_id_salt || uint64be(snapshot_height) || pubkey_bytes)"
            }
            Self::Erc20 => "keccak256(claim_id_salt || uint256(snapshot_height) || pubkey_bytes)",
        }
    }

    fn leaf_format(self) -> &'static str {
        match self {
            Self::Native => {
                "blake2b256(uint64be(snapshot_height) || bytes32(claim_id) || uint16be(pubkey_len) || pubkey_bytes || uint64be(amount))"
            }
            Self::Erc20 => {
                "keccak256(abi.encodePacked(uint256(snapshot_height), bytes32(claim_id), address(account), uint256(amount)))"
            }
        }
    }

    fn pair_hash_format(self) -> &'static str {
        match self {
            Self::Native => "blake2b256(sorted_pair(left,right))",
            Self::Erc20 => "keccak256(sorted_pair(left,right))",
        }
    }

    fn hash_pair_fn(self) -> HashPairFn {
        match self {
            Self::Native => hash_pair_blake,
            Self::Erc20 => hash_pair_keccak,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MigrationClaimsArtifact {
    schema: String,
    generated_at_ms: u64,
    source_snapshot: String,
    snapshot_height: u64,
    claim_mode: String,
    amount_source: String,
    include_slashed: bool,
    conversion_ratio: u64,
    token_contract: Option<String>,
    claim_id_format: String,
    leaf_format: String,
    pair_hash_format: String,
    merkle_root: String,
    claim_count: usize,
    excluded: ExcludedCounts,
    claims: Vec<MigrationClaimEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExcludedCounts {
    slashed: usize,
    zero_amount: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MigrationClaimEntry {
    index: usize,
    pubkey_b64: String,
    account: String,
    claim_id: String,
    balance: u64,
    stake: u64,
    slashed: bool,
    raw_amount: u64,
    mint_amount: String,
    leaf: String,
    proof: Vec<String>,
}

#[derive(Debug, Clone)]
struct ClaimWorkItem {
    entry: StakeSnapshotEntry,
    account: String,
    claim_id_hex: String,
    raw_amount: u64,
    leaf: [u8; 32],
}

type HashPairFn = fn([u8; 32], [u8; 32]) -> [u8; 32];

type Blake2b256 = blake2::Blake2b<U32>;

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn u256_from_u64(value: u64) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[24..].copy_from_slice(&value.to_be_bytes());
    out
}

fn keccak256(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Keccak256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}

fn blake2b256(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Blake2b256::new();
    hasher.update(bytes);
    hasher.finalize().into()
}

fn derive_account_from_pubkey(pubkey_bytes: &[u8]) -> String {
    let mut payload = Vec::with_capacity(32 + pubkey_bytes.len());
    payload.extend_from_slice(b"mfenx-migration-address-v1");
    payload.extend_from_slice(pubkey_bytes);
    let digest = blake2b256(&payload);
    format!("0x{}", hex::encode(&digest[12..]))
}

fn derive_claim_id_erc20(pubkey_bytes: &[u8], snapshot_height: u64, salt: &str) -> [u8; 32] {
    let mut data = Vec::with_capacity(salt.len() + 32 + pubkey_bytes.len());
    data.extend_from_slice(salt.as_bytes());
    data.extend_from_slice(&u256_from_u64(snapshot_height));
    data.extend_from_slice(pubkey_bytes);
    keccak256(&data)
}

fn derive_claim_id_native(pubkey_bytes: &[u8], snapshot_height: u64, salt: &str) -> [u8; 32] {
    let mut data = Vec::with_capacity(48 + salt.len() + pubkey_bytes.len());
    data.extend_from_slice(b"mfenx-migration-native-claim-id-v1");
    data.extend_from_slice(salt.as_bytes());
    data.extend_from_slice(&snapshot_height.to_be_bytes());
    data.extend_from_slice(pubkey_bytes);
    blake2b256(&data)
}

fn encode_leaf_erc20(
    snapshot_height: u64,
    claim_id: [u8; 32],
    account: &str,
    amount: u64,
) -> Result<[u8; 32], String> {
    let account_hex = account
        .strip_prefix("0x")
        .ok_or_else(|| "erc20 account missing 0x prefix".to_string())?;
    if account_hex.len() != 40 {
        return Err(format!(
            "erc20 account has invalid length: expected 40 hex chars, got {}",
            account_hex.len()
        ));
    }
    let account_bytes_vec = hex::decode(account_hex)
        .map_err(|err| format!("erc20 account decode failed for {account}: {err}"))?;
    if account_bytes_vec.len() != 20 {
        return Err(format!(
            "erc20 account has invalid byte length: expected 20, got {}",
            account_bytes_vec.len()
        ));
    }
    let mut data = Vec::with_capacity(32 + 32 + 20 + 32);
    data.extend_from_slice(&u256_from_u64(snapshot_height));
    data.extend_from_slice(&claim_id);
    data.extend_from_slice(&account_bytes_vec);
    data.extend_from_slice(&u256_from_u64(amount));
    Ok(keccak256(&data))
}

fn encode_leaf_native(
    snapshot_height: u64,
    claim_id: [u8; 32],
    pubkey_bytes: &[u8],
    amount: u64,
) -> [u8; 32] {
    let mut data = Vec::with_capacity(64 + pubkey_bytes.len());
    data.extend_from_slice(b"mfenx-migration-native-leaf-v1");
    data.extend_from_slice(&snapshot_height.to_be_bytes());
    data.extend_from_slice(&claim_id);
    let pk_len = (pubkey_bytes.len() as u16).to_be_bytes();
    data.extend_from_slice(&pk_len);
    data.extend_from_slice(pubkey_bytes);
    data.extend_from_slice(&amount.to_be_bytes());
    blake2b256(&data)
}

fn hash_pair_keccak(left: [u8; 32], right: [u8; 32]) -> [u8; 32] {
    let mut data = Vec::with_capacity(64);
    if left <= right {
        data.extend_from_slice(&left);
        data.extend_from_slice(&right);
    } else {
        data.extend_from_slice(&right);
        data.extend_from_slice(&left);
    }
    keccak256(&data)
}

fn hash_pair_blake(left: [u8; 32], right: [u8; 32]) -> [u8; 32] {
    let mut data = Vec::with_capacity(64);
    if left <= right {
        data.extend_from_slice(&left);
        data.extend_from_slice(&right);
    } else {
        data.extend_from_slice(&right);
        data.extend_from_slice(&left);
    }
    blake2b256(&data)
}

fn build_layers(leaves: &[[u8; 32]], hash_pair: HashPairFn) -> Vec<Vec<[u8; 32]>> {
    let mut layers = Vec::new();
    layers.push(leaves.to_vec());
    while layers.last().map(|l| l.len()).unwrap_or(0) > 1 {
        let layer = layers.last().cloned().unwrap_or_default();
        let mut next = Vec::with_capacity(layer.len().div_ceil(2));
        let mut idx = 0usize;
        while idx < layer.len() {
            let left = layer[idx];
            let right = if idx + 1 < layer.len() {
                layer[idx + 1]
            } else {
                layer[idx]
            };
            next.push(hash_pair(left, right));
            idx += 2;
        }
        layers.push(next);
    }
    layers
}

fn proof_for_index(layers: &[Vec<[u8; 32]>], index: usize) -> Vec<[u8; 32]> {
    let mut proof = Vec::new();
    let mut idx = index;
    for layer in layers.iter().take(layers.len().saturating_sub(1)) {
        let sib = idx ^ 1;
        let sibling = if sib < layer.len() {
            layer[sib]
        } else {
            layer[idx]
        };
        proof.push(sibling);
        idx /= 2;
    }
    proof
}

fn verify_proof(
    leaf: [u8; 32],
    proof: &[[u8; 32]],
    expected_root: [u8; 32],
    hash_pair: HashPairFn,
) -> bool {
    let mut computed = leaf;
    for item in proof {
        computed = hash_pair(computed, *item);
    }
    computed == expected_root
}

/// Builds deterministic migration claims + Merkle proofs from a snapshot artifact.
///
/// Returns the computed root (hex string) and writes a JSON artifact to `output`.
pub fn run_build_claims(
    snapshot_path: &str,
    output: &str,
    opts: &BuildClaimsOptions,
) -> Result<String, String> {
    let source = Path::new(snapshot_path);
    let bytes = std::fs::read(source)
        .map_err(|e| format!("failed to read snapshot {}: {e}", source.display()))?;
    let snapshot: StakeSnapshotArtifact =
        serde_json::from_slice(&bytes).map_err(|e| format!("invalid snapshot JSON: {e}"))?;

    let amount_source = AmountSource::parse(&opts.amount_source)?;
    let claim_mode = ClaimMode::parse(&opts.claim_mode)?;
    let hash_pair = claim_mode.hash_pair_fn();
    let snapshot_height = opts
        .snapshot_height_override
        .unwrap_or(snapshot.snapshot_height);

    let mut entries = snapshot.entries.clone();
    entries.sort_by(|a, b| a.pubkey_b64.cmp(&b.pubkey_b64));

    let mut excluded = ExcludedCounts {
        slashed: 0,
        zero_amount: 0,
    };
    let mut work = Vec::new();

    for entry in entries {
        if entry.slashed && !opts.include_slashed {
            excluded.slashed += 1;
            continue;
        }
        let raw_amount = amount_source.amount_for(&entry);
        if raw_amount == 0 {
            excluded.zero_amount += 1;
            continue;
        }

        let pubkey_bytes = BASE64
            .decode(entry.pubkey_b64.as_bytes())
            .map_err(|e| format!("invalid pubkey_b64 for {}: {e}", entry.pubkey_b64))?;

        let (account, claim_id, leaf) = match claim_mode {
            ClaimMode::Native => {
                let claim_id =
                    derive_claim_id_native(&pubkey_bytes, snapshot_height, &opts.claim_id_salt);
                let leaf = encode_leaf_native(snapshot_height, claim_id, &pubkey_bytes, raw_amount);
                (entry.pubkey_b64.clone(), claim_id, leaf)
            }
            ClaimMode::Erc20 => {
                let account = derive_account_from_pubkey(&pubkey_bytes);
                let claim_id =
                    derive_claim_id_erc20(&pubkey_bytes, snapshot_height, &opts.claim_id_salt);
                let leaf = encode_leaf_erc20(snapshot_height, claim_id, &account, raw_amount)?;
                (account, claim_id, leaf)
            }
        };

        work.push(ClaimWorkItem {
            entry,
            account,
            claim_id_hex: format!("0x{}", hex::encode(claim_id)),
            raw_amount,
            leaf,
        });
    }

    if work.is_empty() {
        return Err("no eligible claims found in snapshot".to_string());
    }

    let leaves = work.iter().map(|w| w.leaf).collect::<Vec<_>>();
    let layers = build_layers(&leaves, hash_pair);
    let root = layers
        .last()
        .and_then(|l| l.first())
        .copied()
        .ok_or_else(|| "failed to compute merkle root".to_string())?;

    let claims = work
        .iter()
        .enumerate()
        .map(|(idx, item)| {
            let proof = proof_for_index(&layers, idx);
            if !verify_proof(item.leaf, &proof, root, hash_pair) {
                return Err(format!("proof generation failed at index {idx}"));
            }
            let proof_hex = proof
                .iter()
                .map(|p| format!("0x{}", hex::encode(p)))
                .collect::<Vec<_>>();
            let mint_amount =
                (item.raw_amount as u128).saturating_mul(opts.conversion_ratio as u128);
            Ok(MigrationClaimEntry {
                index: idx,
                pubkey_b64: item.entry.pubkey_b64.clone(),
                account: item.account.clone(),
                claim_id: item.claim_id_hex.clone(),
                balance: item.entry.balance,
                stake: item.entry.stake,
                slashed: item.entry.slashed,
                raw_amount: item.raw_amount,
                mint_amount: mint_amount.to_string(),
                leaf: format!("0x{}", hex::encode(item.leaf)),
                proof: proof_hex,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;

    let artifact = MigrationClaimsArtifact {
        schema: "mfenx.powerhouse.migration-claims.v2".to_string(),
        generated_at_ms: now_millis(),
        source_snapshot: snapshot_path.to_string(),
        snapshot_height,
        claim_mode: claim_mode.as_str().to_string(),
        amount_source: amount_source.as_str().to_string(),
        include_slashed: opts.include_slashed,
        conversion_ratio: opts.conversion_ratio,
        token_contract: opts.token_contract.clone(),
        claim_id_format: claim_mode.claim_id_format().to_string(),
        leaf_format: claim_mode.leaf_format().to_string(),
        pair_hash_format: claim_mode.pair_hash_format().to_string(),
        merkle_root: format!("0x{}", hex::encode(root)),
        claim_count: claims.len(),
        excluded,
        claims,
    };

    let output_path = Path::new(output);
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create {}: {e}", parent.display()))?;
    }
    let encoded = serde_json::to_vec_pretty(&artifact)
        .map_err(|e| format!("failed to encode claims artifact: {e}"))?;
    std::fs::write(output_path, encoded)
        .map_err(|e| format!("failed to write {}: {e}", output_path.display()))?;

    Ok(artifact.merkle_root)
}

#[cfg(test)]
mod tests {
    use super::{run_build_claims, BuildClaimsOptions};
    use crate::commands::stake_snapshot::run_snapshot;
    use serde_json::json;
    use std::fs;

    fn temp_path(name: &str) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        p.push(format!("{name}_{ts}"));
        p
    }

    #[test]
    fn native_claims_are_deterministic() {
        let reg_a = temp_path("claims_registry_a.json");
        let reg_b = temp_path("claims_registry_b.json");
        let snap_a = temp_path("claims_snapshot_a.json");
        let snap_b = temp_path("claims_snapshot_b.json");
        let out_a = temp_path("claims_a.json");
        let out_b = temp_path("claims_b.json");

        let payload_a = json!({
            "accounts": {
                "zKey": {"balance": 5, "stake": 7, "slashed": false},
                "aKey": {"balance": 9, "stake": 3, "slashed": false}
            }
        });
        let payload_b = json!({
            "accounts": {
                "aKey": {"balance": 9, "stake": 3, "slashed": false},
                "zKey": {"balance": 5, "stake": 7, "slashed": false}
            }
        });

        fs::write(&reg_a, serde_json::to_vec(&payload_a).unwrap()).unwrap();
        fs::write(&reg_b, serde_json::to_vec(&payload_b).unwrap()).unwrap();

        run_snapshot(reg_a.to_str().unwrap(), 42, snap_a.to_str().unwrap()).unwrap();
        run_snapshot(reg_b.to_str().unwrap(), 42, snap_b.to_str().unwrap()).unwrap();

        let opts = BuildClaimsOptions {
            amount_source: "total".to_string(),
            include_slashed: false,
            conversion_ratio: 1,
            claim_id_salt: "mfenx-native-test-v1".to_string(),
            token_contract: None,
            snapshot_height_override: None,
            claim_mode: "native".to_string(),
        };

        let root_a =
            run_build_claims(snap_a.to_str().unwrap(), out_a.to_str().unwrap(), &opts).unwrap();
        let root_b =
            run_build_claims(snap_b.to_str().unwrap(), out_b.to_str().unwrap(), &opts).unwrap();

        assert_eq!(root_a, root_b);

        let artifact_a: serde_json::Value =
            serde_json::from_slice(&fs::read(&out_a).unwrap()).unwrap();
        let artifact_b: serde_json::Value =
            serde_json::from_slice(&fs::read(&out_b).unwrap()).unwrap();

        assert_eq!(artifact_a["claim_mode"], "native");
        assert_eq!(artifact_a["merkle_root"], artifact_b["merkle_root"]);
        assert_eq!(artifact_a["claims"], artifact_b["claims"]);

        let _ = fs::remove_file(reg_a);
        let _ = fs::remove_file(reg_b);
        let _ = fs::remove_file(snap_a);
        let _ = fs::remove_file(snap_b);
        let _ = fs::remove_file(out_a);
        let _ = fs::remove_file(out_b);
    }

    #[test]
    fn erc20_mode_emits_hex_accounts() {
        let reg = temp_path("claims_registry_erc20.json");
        let snap = temp_path("claims_snapshot_erc20.json");
        let out = temp_path("claims_erc20.json");

        let payload = json!({
            "accounts": {
                "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=": {
                    "balance": 100,
                    "stake": 5,
                    "slashed": false
                }
            }
        });

        fs::write(&reg, serde_json::to_vec(&payload).unwrap()).unwrap();
        run_snapshot(reg.to_str().unwrap(), 7, snap.to_str().unwrap()).unwrap();

        let opts = BuildClaimsOptions {
            amount_source: "total".to_string(),
            include_slashed: false,
            conversion_ratio: 1,
            claim_id_salt: "mfenx-erc20-test-v1".to_string(),
            token_contract: Some("0x0000000000000000000000000000000000000001".to_string()),
            snapshot_height_override: None,
            claim_mode: "erc20".to_string(),
        };

        let root = run_build_claims(snap.to_str().unwrap(), out.to_str().unwrap(), &opts).unwrap();
        assert!(root.starts_with("0x"));

        let artifact: serde_json::Value = serde_json::from_slice(&fs::read(&out).unwrap()).unwrap();
        assert_eq!(artifact["claim_mode"], "erc20");
        let account = artifact["claims"][0]["account"].as_str().unwrap_or("");
        assert!(account.starts_with("0x"));
        assert_eq!(account.len(), 42);

        let _ = fs::remove_file(reg);
        let _ = fs::remove_file(snap);
        let _ = fs::remove_file(out);
    }
}
