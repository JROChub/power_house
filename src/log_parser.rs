use crate::{
    parse_transcript_record, transcript_digest, verify_transcript_lines, TranscriptDigest,
};
use std::{fs, path::Path};

/// Metadata captured from optional comment lines in a ledger log file.
#[derive(Debug, Clone, Default)]
pub struct LogRecordMetadata {
    /// Optional challenge derivation mode (e.g., `mod`, `rejection`).
    pub challenge_mode: Option<String>,
    /// Optional fold digest provided alongside the transcript.
    pub fold_digest: Option<TranscriptDigest>,
}

/// Parsed contents of a ledger log file.
#[derive(Debug, Clone)]
pub struct ParsedLogFile {
    /// Statement string extracted from the log.
    pub statement: String,
    /// Deterministic transcript digest verified against the stored hash.
    pub digest: TranscriptDigest,
    /// Metadata surfaced from comment lines.
    pub metadata: LogRecordMetadata,
}

/// Parses a ledger log file, tolerating optional comment lines that begin with `#`.
pub fn parse_log_file(path: &Path) -> Result<ParsedLogFile, String> {
    let contents = fs::read_to_string(path)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    parse_log_contents(path, &contents)
}

fn parse_log_contents(path: &Path, contents: &str) -> Result<ParsedLogFile, String> {
    let mut metadata = LogRecordMetadata::default();
    let mut lines: Vec<String> = Vec::new();
    for raw in contents.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix('#') {
            if let Some((key, value)) = rest.trim().split_once(':') {
                let key = key.trim();
                let value = value.trim();
                if key.eq_ignore_ascii_case("challenge_mode") && !value.is_empty() {
                    metadata.challenge_mode = Some(value.to_string());
                } else if key.eq_ignore_ascii_case("fold_digest") && !value.is_empty() {
                    metadata.fold_digest = Some(parse_fold_digest(value)?);
                }
            }
            continue;
        }
        lines.push(line.to_string());
    }
    if lines.is_empty() {
        return Err(format!("{} is empty", path.display()));
    }
    let statement_line = lines.remove(0);
    if !statement_line.starts_with("statement:") {
        return Err(format!("{} missing statement prefix", path.display()));
    }
    let statement = statement_line[10..].to_string();
    verify_transcript_lines(lines.iter().map(|s| s.as_str()))
        .map_err(|err| format!("{} verification failed: {err}", path.display()))?;
    let (challenges, round_sums, final_value, stored_hash) =
        parse_transcript_record(lines.iter().map(|s| s.as_str()))
            .map_err(|err| format!("{} parse error: {err}", path.display()))?;
    let computed = transcript_digest(&challenges, &round_sums, final_value);
    if computed != stored_hash {
        return Err(format!(
            "{} hash mismatch: stored={}, computed={}",
            path.display(),
            crate::transcript_digest_to_hex(&stored_hash),
            crate::transcript_digest_to_hex(&computed)
        ));
    }
    Ok(ParsedLogFile {
        statement,
        digest: computed,
        metadata,
    })
}

fn parse_fold_digest(value: &str) -> Result<TranscriptDigest, String> {
    let cleaned = value.trim();
    if cleaned.is_empty() {
        return Err("fold digest value is empty".to_string());
    }
    crate::transcript_digest_from_hex(cleaned)
}

/// Attempts to load a fold digest hint from `fold_digest.txt` inside `dir`.
pub fn read_fold_digest_hint(dir: &Path) -> Result<Option<TranscriptDigest>, String> {
    let path = dir.join("fold_digest.txt");
    let contents = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(_) => return Ok(None),
    };
    let value = contents.trim();
    if value.is_empty() {
        return Ok(None);
    }
    crate::transcript_digest_from_hex(value)
        .map(Some)
        .map_err(|err| format!("invalid fold_digest.txt value: {err}"))
}
