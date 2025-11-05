//! Simple Merkle accumulator utilities for anchor transcript digests.
//!
//! The tree uses domain-separated BLAKE2b-256 hashing to combine leaves.

use blake2::digest::{consts::U32, Digest};
use blake2::Blake2b;
use serde_json::json;

use crate::data::{digest_from_hex, digest_to_hex};
use crate::TranscriptDigest;

const MERKLE_DOMAIN: &[u8] = b"JROC_MERKLE";

fn hash_pair(left: &TranscriptDigest, right: &TranscriptDigest) -> TranscriptDigest {
    let mut hasher = Blake2b::<U32>::new();
    hasher.update(MERKLE_DOMAIN);
    hasher.update(left);
    hasher.update(right);
    let mut out = [0u8; 32];
    out.copy_from_slice(&hasher.finalize());
    out
}

fn hash_leaf(leaf: &TranscriptDigest) -> TranscriptDigest {
    let mut hasher = Blake2b::<U32>::new();
    hasher.update(MERKLE_DOMAIN);
    hasher.update([0u8]); // leaf marker
    hasher.update(leaf);
    let mut out = [0u8; 32];
    out.copy_from_slice(&hasher.finalize());
    out
}

fn hash_empty() -> TranscriptDigest {
    let mut hasher = Blake2b::<U32>::new();
    hasher.update(MERKLE_DOMAIN);
    hasher.update([1u8]); // empty marker
    let mut out = [0u8; 32];
    out.copy_from_slice(&hasher.finalize());
    out
}

/// Computes the Merkle root for the provided leaf digests.
pub fn merkle_root(leaves: &[TranscriptDigest]) -> TranscriptDigest {
    if leaves.is_empty() {
        return hash_empty();
    }
    let mut level: Vec<TranscriptDigest> = leaves.iter().map(hash_leaf).collect();
    while level.len() > 1 {
        let mut next = Vec::with_capacity((level.len() + 1) / 2);
        for chunk in level.chunks(2) {
            if chunk.len() == 1 {
                next.push(chunk[0]);
            } else {
                next.push(hash_pair(&chunk[0], &chunk[1]));
            }
        }
        level = next;
    }
    level[0]
}

/// Describes a sibling hash encountered while walking a Merkle tree.
#[derive(Debug, Clone)]
pub struct MerkleProofNode {
    /// Sibling digest that must be paired with the running hash.
    pub sibling: TranscriptDigest,
    /// `true` if the sibling is on the left-hand side of the pair.
    pub left: bool,
}

/// Merkle inclusion proof for a single transcript digest.
#[derive(Debug, Clone)]
pub struct MerkleProof {
    /// Merkle root committed to in the anchor.
    pub root: TranscriptDigest,
    /// Leaf digest whose inclusion is being proven.
    pub leaf: TranscriptDigest,
    /// Index of the leaf within the original list of digests.
    pub index: usize,
    /// Merkle path consisting of sibling hashes and directions.
    pub path: Vec<MerkleProofNode>,
}

/// Constructs an inclusion proof for the leaf at `index` within `leaves`.
pub fn build_proof(leaves: &[TranscriptDigest], index: usize) -> Option<MerkleProof> {
    if leaves.is_empty() || index >= leaves.len() {
        return None;
    }
    let mut layer: Vec<TranscriptDigest> = leaves.iter().map(hash_leaf).collect();
    let mut idx = index;
    let mut path = Vec::new();
    while layer.len() > 1 {
        if idx % 2 == 0 {
            if idx + 1 < layer.len() {
                path.push(MerkleProofNode {
                    sibling: layer[idx + 1],
                    left: false,
                });
            }
        } else {
            path.push(MerkleProofNode {
                sibling: layer[idx - 1],
                left: true,
            });
        }
        let mut next = Vec::with_capacity((layer.len() + 1) / 2);
        for chunk in layer.chunks(2) {
            if chunk.len() == 1 {
                next.push(chunk[0]);
            } else {
                next.push(hash_pair(&chunk[0], &chunk[1]));
            }
        }
        layer = next;
        idx /= 2;
    }
    Some(MerkleProof {
        root: layer[0],
        leaf: leaves[index],
        index,
        path,
    })
}

/// Checks whether the proof recomputes the advertised Merkle root.
pub fn verify_proof(proof: &MerkleProof) -> bool {
    let mut hash = hash_leaf(&proof.leaf);
    for node in &proof.path {
        if node.left {
            hash = hash_pair(&node.sibling, &hash);
        } else {
            hash = hash_pair(&hash, &node.sibling);
        }
    }
    hash == proof.root
}

impl MerkleProof {
    /// Serialises the proof to a JSON string with hex-encoded digests.
    pub fn to_json_string(&self) -> String {
        let path: Vec<_> = self
            .path
            .iter()
            .map(|node| {
                json!({
                    "direction": if node.left { "L" } else { "R" },
                    "sibling": digest_to_hex(&node.sibling)
                })
            })
            .collect();
        json!({
            "root": digest_to_hex(&self.root),
            "leaf": digest_to_hex(&self.leaf),
            "index": self.index,
            "path": path
        })
        .to_string()
    }

    /// Parses a proof previously emitted by [`MerkleProof::to_json_string`].
    pub fn from_json_str(input: &str) -> Result<Self, String> {
        let value: serde_json::Value =
            serde_json::from_str(input).map_err(|err| format!("invalid proof JSON: {err}"))?;
        let root = digest_from_hex(
            value
                .get("root")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "missing root".to_string())?,
        )?;
        let leaf = digest_from_hex(
            value
                .get("leaf")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "missing leaf".to_string())?,
        )?;
        let index = value
            .get("index")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| "missing index".to_string())? as usize;
        let mut path = Vec::new();
        if let Some(array) = value.get("path").and_then(|v| v.as_array()) {
            for node in array {
                let direction = node
                    .get("direction")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "invalid direction".to_string())?;
                let sibling_hex = node
                    .get("sibling")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "invalid sibling".to_string())?;
                let sibling = digest_from_hex(sibling_hex)?;
                let left = matches!(direction, "L" | "l");
                path.push(MerkleProofNode { sibling, left });
            }
        }
        Ok(Self {
            root,
            leaf,
            index,
            path,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(n: u8) -> TranscriptDigest {
        let mut out = [0u8; 32];
        out[0] = n;
        out
    }

    #[test]
    fn merkle_roundtrip() {
        let leaves = vec![leaf(1), leaf(2), leaf(3), leaf(4)];
        let root = merkle_root(&leaves);
        let proof = build_proof(&leaves, 2).unwrap();
        assert_eq!(proof.root, root);
        assert!(verify_proof(&proof));
    }

    #[test]
    fn proof_serialization() {
        let leaves = vec![leaf(1), leaf(2)];
        let proof = build_proof(&leaves, 1).unwrap();
        let json = proof.to_json_string();
        let parsed = MerkleProof::from_json_str(&json).unwrap();
        assert!(verify_proof(&parsed));
    }
}
