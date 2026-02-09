//! Lightweight, deterministic serialization helpers for ledger artifacts.
//!
//! The routines in this module emit newline-delimited, ASCII-only records that
//! capture the Fiat–Shamir transcripts, per-round sums and final evaluations of
//! generalized sum-check proofs.  The records are deterministic and use a
//! domain-separated BLAKE2b-256 digest to ensure tamper resistance while
//! remaining human-auditable.

use blake2::digest::{consts::U32, Digest};

type Blake2b256 = blake2::Blake2b<U32>;

/// Domain tag applied to every transcript digest.
const DIGEST_DOMAIN: &[u8] = b"MFENX_TRANSCRIPT";

/// Fixed-width transcript digest.
pub type TranscriptDigest = [u8; 32];

fn write_u64_be(hasher: &mut Blake2b256, value: u64) {
    hasher.update(value.to_be_bytes());
}

fn write_slice(hasher: &mut Blake2b256, values: &[u64]) {
    write_u64_be(hasher, values.len() as u64);
    for &value in values {
        write_u64_be(hasher, value);
    }
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn decode_hex(input: &str) -> Result<Vec<u8>, String> {
    if !input.len().is_multiple_of(2) {
        return Err("hex digest must contain an even number of characters".to_string());
    }
    let mut out = Vec::with_capacity(input.len() / 2);
    let bytes = input.as_bytes();
    for chunk in bytes.chunks(2) {
        let hi = (chunk[0] as char).to_digit(16);
        let lo = (chunk[1] as char).to_digit(16);
        match (hi, lo) {
            (Some(hi), Some(lo)) => out.push(((hi << 4) | lo) as u8),
            _ => return Err("invalid hex digit in digest".to_string()),
        }
    }
    Ok(out)
}

/// Converts a digest into a lowercase hex string.
pub fn digest_to_hex(digest: &TranscriptDigest) -> String {
    encode_hex(digest)
}

/// Parses a lowercase or uppercase hex string into a transcript digest.
pub fn digest_from_hex(input: &str) -> Result<TranscriptDigest, String> {
    let bytes = decode_hex(input)?;
    if bytes.len() != 32 {
        return Err("digest must be 32 bytes (64 hex chars)".to_string());
    }
    let mut digest = [0u8; 32];
    digest.copy_from_slice(&bytes);
    Ok(digest)
}

/// Escapes a slice of `u64` values into a single line of ASCII text.
fn encode_u64_slice(values: &[u64]) -> String {
    values
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Computes the deterministic digest used for transcript records.
pub fn compute_digest(
    transcript: &[u64],
    round_sums: &[u64],
    final_value: u64,
) -> TranscriptDigest {
    let mut hasher = Blake2b256::new();
    hasher.update(DIGEST_DOMAIN);
    write_slice(&mut hasher, transcript);
    write_slice(&mut hasher, round_sums);
    write_u64_be(&mut hasher, final_value);
    let output = hasher.finalize();
    let mut digest = [0u8; 32];
    digest.copy_from_slice(&output);
    digest
}

/// Writes a transcript record using the provided writer function.
pub fn write_record<W>(
    mut write_line: W,
    transcript: &[u64],
    round_sums: &[u64],
    final_value: u64,
) -> std::io::Result<()>
where
    W: FnMut(&str) -> std::io::Result<()>,
{
    let digest = compute_digest(transcript, round_sums, final_value);
    write_line(&format!("transcript:{}", encode_u64_slice(transcript)))?;
    write_line(&format!("round_sums:{}", encode_u64_slice(round_sums)))?;
    write_line(&format!("final:{}", final_value))?;
    write_line(&format!("hash:{}", digest_to_hex(&digest)))
}

fn parse_vec_u64(input: &str, prefix: &str) -> Result<Vec<u64>, String> {
    let tail = input
        .strip_prefix(prefix)
        .ok_or_else(|| format!("missing {prefix} prefix"))?
        .trim();
    if tail.is_empty() {
        return Ok(Vec::new());
    }
    tail.split_whitespace()
        .map(|tok| {
            tok.parse::<u64>()
                .map_err(|_| format!("invalid integer in {prefix}"))
        })
        .collect()
}

fn parse_u64(input: &str, prefix: &str) -> Result<u64, String> {
    let tail = input
        .strip_prefix(prefix)
        .ok_or_else(|| format!("missing {prefix} prefix"))?
        .trim();
    tail.parse::<u64>()
        .map_err(|_| format!("invalid integer in {prefix}"))
}

/// Parses a transcript record and returns its components and stored hash.
pub fn parse_record<'a, I>(lines: I) -> Result<(Vec<u64>, Vec<u64>, u64, TranscriptDigest), String>
where
    I: IntoIterator<Item = &'a str>,
{
    let mut iter = lines.into_iter();
    let transcript_line = iter
        .next()
        .ok_or_else(|| "missing transcript line".to_string())?;
    let round_sums_line = iter
        .next()
        .ok_or_else(|| "missing round_sums line".to_string())?;
    let final_line = iter
        .next()
        .ok_or_else(|| "missing final line".to_string())?;
    let hash_line = iter.next().ok_or_else(|| "missing hash line".to_string())?;
    let transcript = parse_vec_u64(transcript_line, "transcript:")?;
    let round_sums = parse_vec_u64(round_sums_line, "round_sums:")?;
    let final_value = parse_u64(final_line, "final:")?;
    let stored_hash = digest_from_hex(
        hash_line
            .strip_prefix("hash:")
            .ok_or_else(|| "missing hash prefix".to_string())?
            .trim(),
    )?;
    Ok((transcript, round_sums, final_value, stored_hash))
}

/// Verifies that a transcript record matches its stored hash digest.
pub fn verify_record_lines<'a, I>(lines: I) -> Result<(), String>
where
    I: IntoIterator<Item = &'a str> + Clone,
{
    let (transcript, round_sums, final_value, stored_hash) = parse_record(lines.clone())?;
    let computed = compute_digest(&transcript, &round_sums, final_value);
    if computed == stored_hash {
        Ok(())
    } else {
        Err("hash mismatch".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::{compute_digest, digest_to_hex, parse_record, verify_record_lines, write_record};

    #[test]
    fn test_write_record_format() {
        let mut lines = Vec::new();
        write_record(
            |line| {
                lines.push(line.to_string());
                Ok(())
            },
            &[1, 2, 3],
            &[4, 5],
            6,
        )
        .unwrap();
        assert_eq!(lines[0], "transcript:1 2 3");
        assert_eq!(lines[1], "round_sums:4 5");
        assert_eq!(lines[2], "final:6");
        assert!(lines[3].starts_with("hash:"));
        assert_eq!(lines[3].len(), 5 + 64);
    }

    #[test]
    fn test_parse_and_verify() {
        let lines = vec![
            "transcript:10 20".to_string(),
            "round_sums:5 7".to_string(),
            "final:9".to_string(),
            format!(
                "hash:{}",
                digest_to_hex(&compute_digest(&[10, 20], &[5, 7], 9))
            ),
        ];
        let parsed = parse_record(lines.iter().map(|s| s.as_str())).unwrap();
        assert_eq!(parsed.0, vec![10, 20]);
        assert!(verify_record_lines(lines.iter().map(|s| s.as_str())).is_ok());
    }

    #[test]
    fn test_verify_rejects_tampering() {
        let lines = vec![
            "transcript:1".to_string(),
            "round_sums:2".to_string(),
            "final:3".to_string(),
            "hash:deadbeef".to_string(),
        ];
        assert!(verify_record_lines(lines.iter().map(|s| s.as_str())).is_err());
    }
}
