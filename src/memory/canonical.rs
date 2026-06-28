//! Canonical JSON and digest helpers for Memory Capsules.

use super::errors::MemoryError;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest as _, Sha256};
use std::collections::BTreeSet;

/// Prefix used for SHA-256 digest strings.
pub const SHA256_PREFIX: &str = "sha256:";

/// Hashes bytes with a domain separator and returns `sha256:<hex>`.
pub fn digest_bytes(domain: &[u8], bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(bytes);
    format!("{SHA256_PREFIX}{}", hex::encode(hasher.finalize()))
}

/// Serializes a value as canonical JSON bytes after rejecting float numbers.
pub fn canonical_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, MemoryError> {
    let value = serde_json::to_value(value)?;
    reject_noncanonical_numbers(&value, "")?;
    serde_json::to_vec(&value).map_err(MemoryError::Json)
}

/// Computes a domain-separated digest over canonical JSON.
pub fn digest_json<T: Serialize>(domain: &[u8], value: &T) -> Result<String, MemoryError> {
    Ok(digest_bytes(domain, &canonical_bytes(value)?))
}

/// Parses JSON while rejecting duplicate object keys and float numbers.
pub fn parse_strict_value(input: &str) -> Result<Value, MemoryError> {
    DuplicateKeyScanner::new(input).scan()?;
    let value: Value = serde_json::from_str(input)?;
    reject_noncanonical_numbers(&value, "")?;
    Ok(value)
}

/// Validates a `sha256:<64 lowercase hex>` digest.
pub fn validate_sha256(value: &str) -> Result<(), MemoryError> {
    let Some(hex_digest) = value.strip_prefix(SHA256_PREFIX) else {
        return Err(MemoryError::InvalidDigest(value.to_string()));
    };
    if hex_digest.len() != 64
        || !hex_digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(MemoryError::InvalidDigest(value.to_string()));
    }
    Ok(())
}

pub(crate) fn reject_noncanonical_numbers(value: &Value, path: &str) -> Result<(), MemoryError> {
    match value {
        Value::Number(number) if !(number.is_i64() || number.is_u64()) => {
            Err(MemoryError::Canonical(format!(
                "non-integer JSON number at {}",
                if path.is_empty() { "/" } else { path }
            )))
        }
        Value::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                reject_noncanonical_numbers(value, &format!("{path}/{index}"))?;
            }
            Ok(())
        }
        Value::Object(values) => {
            for (key, value) in values {
                reject_noncanonical_numbers(value, &format!("{path}/{}", escape_pointer(key)))?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn escape_pointer(value: &str) -> String {
    value.replace('~', "~0").replace('/', "~1")
}

struct DuplicateKeyScanner<'a> {
    input: &'a [u8],
    cursor: usize,
}

impl<'a> DuplicateKeyScanner<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input: input.as_bytes(),
            cursor: 0,
        }
    }

    fn scan(mut self) -> Result<(), MemoryError> {
        self.skip_ws();
        self.value()?;
        self.skip_ws();
        if self.cursor != self.input.len() {
            return Err(MemoryError::Canonical(
                "trailing content after JSON value".to_string(),
            ));
        }
        Ok(())
    }

    fn value(&mut self) -> Result<(), MemoryError> {
        self.skip_ws();
        match self.peek() {
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'"') => {
                self.string()?;
                Ok(())
            }
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(b't') => self.literal(b"true"),
            Some(b'f') => self.literal(b"false"),
            Some(b'n') => self.literal(b"null"),
            Some(other) => Err(MemoryError::Canonical(format!(
                "unexpected JSON byte {other} at {}",
                self.cursor
            ))),
            None => Err(MemoryError::Canonical("unexpected end of JSON".to_string())),
        }
    }

    fn object(&mut self) -> Result<(), MemoryError> {
        self.expect(b'{')?;
        self.skip_ws();
        let mut keys = BTreeSet::new();
        if self.consume_if(b'}') {
            return Ok(());
        }
        loop {
            self.skip_ws();
            let key = self.string()?;
            if !keys.insert(key.clone()) {
                return Err(MemoryError::Canonical(format!(
                    "duplicate object key {key:?}"
                )));
            }
            self.skip_ws();
            self.expect(b':')?;
            self.value()?;
            self.skip_ws();
            if self.consume_if(b'}') {
                return Ok(());
            }
            self.expect(b',')?;
        }
    }

    fn array(&mut self) -> Result<(), MemoryError> {
        self.expect(b'[')?;
        self.skip_ws();
        if self.consume_if(b']') {
            return Ok(());
        }
        loop {
            self.value()?;
            self.skip_ws();
            if self.consume_if(b']') {
                return Ok(());
            }
            self.expect(b',')?;
        }
    }

    fn string(&mut self) -> Result<String, MemoryError> {
        let start = self.cursor;
        self.expect(b'"')?;
        let mut escaped = false;
        while let Some(byte) = self.next() {
            if escaped {
                escaped = false;
                continue;
            }
            match byte {
                b'\\' => escaped = true,
                b'"' => {
                    let end = self.cursor;
                    return serde_json::from_slice(&self.input[start..end])
                        .map_err(MemoryError::Json);
                }
                0x00..=0x1f => {
                    return Err(MemoryError::Canonical(
                        "unescaped control byte in JSON string".to_string(),
                    ));
                }
                _ => {}
            }
        }
        Err(MemoryError::Canonical(
            "unterminated JSON string".to_string(),
        ))
    }

    fn number(&mut self) -> Result<(), MemoryError> {
        if self.consume_if(b'-') && !self.peek_is_digit() {
            return Err(MemoryError::Canonical("invalid JSON number".to_string()));
        }
        if self.consume_if(b'0') {
            if self.peek_is_digit() {
                return Err(MemoryError::Canonical(
                    "leading zero in JSON number".to_string(),
                ));
            }
        } else {
            self.digits()?;
        }
        if matches!(self.peek(), Some(b'.' | b'e' | b'E')) {
            return Err(MemoryError::Canonical(
                "floating-point JSON number is forbidden".to_string(),
            ));
        }
        Ok(())
    }

    fn digits(&mut self) -> Result<(), MemoryError> {
        if !self.peek_is_digit() {
            return Err(MemoryError::Canonical("expected digit".to_string()));
        }
        while self.peek_is_digit() {
            self.cursor += 1;
        }
        Ok(())
    }

    fn literal(&mut self, expected: &[u8]) -> Result<(), MemoryError> {
        if self.input.get(self.cursor..self.cursor + expected.len()) == Some(expected) {
            self.cursor += expected.len();
            Ok(())
        } else {
            Err(MemoryError::Canonical(format!(
                "invalid JSON literal at {}",
                self.cursor
            )))
        }
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\n' | b'\r' | b'\t')) {
            self.cursor += 1;
        }
    }

    fn expect(&mut self, expected: u8) -> Result<(), MemoryError> {
        match self.next() {
            Some(found) if found == expected => Ok(()),
            Some(found) => Err(MemoryError::Canonical(format!(
                "expected byte {expected}, found {found} at {}",
                self.cursor.saturating_sub(1)
            ))),
            None => Err(MemoryError::Canonical(format!(
                "expected byte {expected}, found end of JSON"
            ))),
        }
    }

    fn consume_if(&mut self, byte: u8) -> bool {
        if self.peek() == Some(byte) {
            self.cursor += 1;
            true
        } else {
            false
        }
    }

    fn peek_is_digit(&self) -> bool {
        matches!(self.peek(), Some(b'0'..=b'9'))
    }

    fn peek(&self) -> Option<u8> {
        self.input.get(self.cursor).copied()
    }

    fn next(&mut self) -> Option<u8> {
        let byte = self.peek()?;
        self.cursor += 1;
        Some(byte)
    }
}
