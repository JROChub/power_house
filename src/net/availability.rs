#![cfg(feature = "net")]

use crate::merkle::MerkleProofNode;
use crate::merkle::{build_proof as build_merkle_proof, merkle_root, MerkleProof};
use crate::verify_merkle_proof;
use ark_crypto_primitives::crh::{pedersen, CRHScheme};
use ark_ed_on_bn254::EdwardsProjective as PedersenCurve;
use ark_serialize::CanonicalSerialize;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use rand::{rngs::StdRng, SeedableRng};
use reed_solomon_erasure::galois_8::ReedSolomon;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Commitment to a set of erasure-coded shares.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShareCommitment {
    /// Number of data shards.
    pub data_shards: u8,
    /// Number of parity shards.
    pub parity_shards: u8,
    /// Commitment over share hashes (hex Merkle root).
    pub share_root: String,
    /// Pedersen commitment over share hashes (hex), for ZK circuits.
    #[serde(default)]
    pub pedersen_root: String,
    /// Individual share hashes.
    pub share_hashes: Vec<[u8; 32]>,
}

/// Evidence describing a missing share or mismatched commitment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AvailabilityEvidence {
    /// Namespace of the blob.
    pub namespace: String,
    /// Blob hash.
    pub blob_hash: String,
    /// Index of the missing/invalid share.
    pub idx: usize,
    /// Optional share data (base64) if present but invalid.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub share: Option<String>,
    /// Reason for the fault.
    pub reason: String,
}

#[derive(Clone)]
struct PedersenWindow;
impl pedersen::Window for PedersenWindow {
    // 4 * 130 = 520 bits, enough for 64-byte concat inputs with padding.
    const WINDOW_SIZE: usize = 4;
    const NUM_WINDOWS: usize = 130;
}

fn pedersen_params() -> pedersen::Parameters<PedersenCurve> {
    // Deterministic, network-wide parameters derived from a fixed seed.
    let mut rng = StdRng::from_seed([0u8; 32]);
    pedersen::CRH::<PedersenCurve, PedersenWindow>::setup(&mut rng).expect("pedersen setup")
}

fn pedersen_hash_bytes(params: &pedersen::Parameters<PedersenCurve>, data: &[u8]) -> [u8; 32] {
    let point = pedersen::CRH::<PedersenCurve, PedersenWindow>::evaluate(params, data)
        .expect("pedersen evaluate");
    let mut out = Vec::new();
    point
        .serialize_compressed(&mut out)
        .expect("serialize pedersen");
    let mut buf = [0u8; 32];
    buf.copy_from_slice(&out[..32]);
    buf
}

fn pedersen_leaf(data: &[u8]) -> [u8; 32] {
    let mut leaf = Vec::with_capacity(1 + data.len());
    leaf.push(0u8);
    leaf.extend_from_slice(data);
    pedersen_hash_bytes(&pedersen_params(), &leaf)
}

fn pedersen_hash_pair(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut buf = Vec::with_capacity(1 + 64);
    buf.push(1u8);
    buf.extend_from_slice(left);
    buf.extend_from_slice(right);
    pedersen_hash_bytes(&pedersen_params(), &buf)
}

/// Compute a Pedersen-based Merkle root over share hashes (for ZK circuits).
pub fn pedersen_merkle_root(leaves: &[[u8; 32]]) -> [u8; 32] {
    if leaves.is_empty() {
        return pedersen_leaf(&[]);
    }
    let mut level: Vec<[u8; 32]> = leaves.iter().map(|leaf| pedersen_leaf(leaf)).collect();
    while level.len() > 1 {
        let mut next = Vec::with_capacity((level.len() + 1) / 2);
        for chunk in level.chunks(2) {
            if chunk.len() == 1 {
                next.push(chunk[0]);
            } else {
                next.push(pedersen_hash_pair(&chunk[0], &chunk[1]));
            }
        }
        level = next;
    }
    level[0]
}

/// Compute erasure-coded shares and a commitment root.
pub fn encode_shares(
    data: &[u8],
    data_shards: u8,
    parity_shards: u8,
) -> Result<(Vec<Vec<u8>>, ShareCommitment), String> {
    let rs = ReedSolomon::new(data_shards as usize, parity_shards as usize)
        .map_err(|err| format!("reed-solomon init: {err}"))?;
    let shard_len = (data.len() + data_shards as usize - 1) / data_shards as usize;
    let mut shards: Vec<Vec<u8>> =
        vec![vec![0u8; shard_len]; (data_shards + parity_shards) as usize];
    for (i, chunk) in data.chunks(shard_len).enumerate() {
        shards[i][..chunk.len()].copy_from_slice(chunk);
    }
    rs.encode(&mut shards)
        .map_err(|err| format!("encode: {err}"))?;
    let share_hashes: Vec<[u8; 32]> = shards.iter().map(|s| sha256(s)).collect();
    let root = merkle_root(&share_hashes);
    let pedersen_root = pedersen_merkle_root(&share_hashes);
    let commitment = ShareCommitment {
        data_shards,
        parity_shards,
        share_root: hex::encode(root),
        pedersen_root: hex::encode(pedersen_root),
        share_hashes: share_hashes.clone(),
    };
    Ok((shards, commitment))
}

/// Build a Merkle proof for a given share index.
pub fn share_proof(share_hashes: &[[u8; 32]], idx: usize) -> Result<MerkleProof, String> {
    build_merkle_proof(share_hashes, idx).ok_or_else(|| "invalid index".to_string())
}

/// Build a Pedersen Merkle proof for a given share index.
pub fn pedersen_share_proof(share_hashes: &[[u8; 32]], idx: usize) -> Result<MerkleProof, String> {
    if share_hashes.is_empty() || idx >= share_hashes.len() {
        return Err("invalid index".into());
    }
    let mut layer: Vec<[u8; 32]> = share_hashes.iter().map(|h| pedersen_leaf(h)).collect();
    let mut i = idx;
    let mut path = Vec::new();
    while layer.len() > 1 {
        if i % 2 == 0 {
            if i + 1 < layer.len() {
                path.push(MerkleProofNode {
                    sibling: layer[i + 1],
                    left: false,
                });
            }
        } else {
            path.push(MerkleProofNode {
                sibling: layer[i - 1],
                left: true,
            });
        }
        let mut next = Vec::with_capacity((layer.len() + 1) / 2);
        for chunk in layer.chunks(2) {
            if chunk.len() == 1 {
                next.push(chunk[0]);
            } else {
                next.push(pedersen_hash_pair(&chunk[0], &chunk[1]));
            }
        }
        layer = next;
        i /= 2;
    }
    Ok(MerkleProof {
        root: layer[0],
        leaf: share_hashes[idx],
        index: idx,
        path,
    })
}

/// Verify a sampled share against the commitment.
pub fn verify_sample(root_hex: &str, share: &[u8], proof: &MerkleProof) -> Result<(), String> {
    let expected_root = hex::decode(root_hex).map_err(|e| e.to_string())?;
    if expected_root.len() != 32 {
        return Err("bad root length".into());
    }
    let mut hasher = Sha256::new();
    hasher.update(share);
    let leaf: [u8; 32] = hasher.finalize().into();
    if leaf != proof.leaf {
        return Err("share hash mismatch".into());
    }
    if proof.root != expected_root.as_slice() {
        return Err("root mismatch".into());
    }
    if !verify_merkle_proof(proof) {
        return Err("invalid proof".into());
    }
    Ok(())
}

fn sha256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}

/// Build DA fault evidence for a missing share.
pub fn build_missing_share_evidence(
    namespace: &str,
    blob_hash: &str,
    idx: usize,
) -> AvailabilityEvidence {
    AvailabilityEvidence {
        namespace: namespace.to_string(),
        blob_hash: blob_hash.to_string(),
        idx,
        share: None,
        reason: "blob-missing".into(),
    }
}

/// Build DA fault evidence for a share hash mismatch.
pub fn build_bad_share_evidence(
    namespace: &str,
    blob_hash: &str,
    idx: usize,
    share: &[u8],
) -> AvailabilityEvidence {
    AvailabilityEvidence {
        namespace: namespace.to_string(),
        blob_hash: blob_hash.to_string(),
        idx,
        share: Some(BASE64.encode(share)),
        reason: "share-mismatch".into(),
    }
}
