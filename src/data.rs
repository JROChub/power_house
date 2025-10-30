//! Lightweight, deterministic serialization helpers for ledger artifacts.
//!
//! The routines in this module emit newline-delimited, ASCII-only records that
//! capture the Fiat–Shamir transcripts, per-round sums and final evaluations of
//! generalized sum-check proofs.  The format is stable and does not rely on
//! external crates, keeping the project dependency-free.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Escapes a slice of `u64` values into a single line of ASCII text.
fn encode_u64_slice(values: &[u64]) -> String {
    values
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Computes the deterministic digest used for transcript records.
pub fn compute_digest(transcript: &[u64], round_sums: &[u64], final_value: u64) -> u64 {
    let mut hasher = DefaultHasher::new();
    transcript.hash(&mut hasher);
    round_sums.hash(&mut hasher);
    final_value.hash(&mut hasher);
    hasher.finish()
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
    write_line(&format!("hash:{digest}"))
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
pub fn parse_record<'a, I>(lines: I) -> Result<(Vec<u64>, Vec<u64>, u64, u64), String>
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
    let stored_hash = parse_u64(hash_line, "hash:")?;
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
    use super::{compute_digest, parse_record, verify_record_lines, write_record};

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
        assert_ne!(lines[3], "hash:0");
    }

    #[test]
    fn test_parse_and_verify() {
        let lines = vec![
            "transcript:10 20".to_string(),
            "round_sums:5 7".to_string(),
            "final:9".to_string(),
            format!("hash:{}", compute_digest(&[10, 20], &[5, 7], 9)),
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
            "hash:0".to_string(),
        ];
        assert!(verify_record_lines(lines.iter().map(|s| s.as_str())).is_err());
    }
}
