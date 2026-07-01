//! Deterministic Rust-subset frontend for SFCS zkVM profiles.
//!
//! This compiler intentionally starts with a small safe Rust subset: a single
//! `u32 + u32 -> u32` function. It emits the RV32I program consumed by the
//! private-add ZK profile and a deterministic semantic packet for observability.

use super::{
    vm::SfcsVmProgram,
    zk::{encode_rv32_add, SFCS_ZK_PRIVATE_ADD_PROTOCOL_V1_DRAFT},
};
use crate::memory::{semantic_packet_digest, MemoryError};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::error::Error;
use std::fmt;

const SOURCE_DIGEST_DOMAIN: &[u8] = b"power-house:sfcs-compiler:v1-draft:source\0";

/// Compiler output for the first Rust-subset private-add profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcsCompiledPrivateAdd {
    /// Compiler schema.
    pub schema: String,
    /// Source language.
    pub language: String,
    /// Source digest after deterministic normalization.
    pub source_digest: String,
    /// Function name.
    pub function_name: String,
    /// Left parameter name.
    pub lhs_name: String,
    /// Right parameter name.
    pub rhs_name: String,
    /// Return type.
    pub return_type: String,
    /// Left private register.
    pub lhs_register: u8,
    /// Right private register.
    pub rhs_register: u8,
    /// Public output register.
    pub output_register: u8,
    /// Emitted RV32I program.
    pub program: SfcsVmProgram,
    /// Non-core semantic packet for Observatory/slbit-style display.
    pub semantic_packet: Value,
}

impl SfcsCompiledPrivateAdd {
    /// Returns the emitted VM program digest.
    pub fn program_digest(&self) -> Result<String, SfcsCompilerError> {
        Ok(self.program.program_digest()?)
    }
}

/// Compiles a safe Rust add function into the private-add profile.
pub fn compile_private_add_source(
    source: &str,
) -> Result<SfcsCompiledPrivateAdd, SfcsCompilerError> {
    let normalized = normalize_rust_source(source)?;
    let parsed = parse_private_add(&normalized)?;
    let program =
        SfcsVmProgram::rv32i(vec![encode_rv32_add(3, 10, 11), 0x0000_0073]).with_max_steps(8);
    let source_digest = source_digest(&normalized);
    let program_digest = program.program_digest()?;
    let mut packet = json!({
        "schema": "slbit/viz-packet/v3",
        "packet_id": format!("slp_sfcs_compiler_{}", &source_digest["sha256:".len()..18]),
        "packet_digest": "",
        "claim": {
            "claim_id": format!("claim_{}", parsed.function_name),
            "label": format!("Rust subset function {} compiled to SFCS private add", parsed.function_name),
            "domain": "sfcs-zkvm-compiler",
            "status": "explained",
            "bound_core": {
                "source_digest": source_digest,
                "program_digest": program_digest,
                "zk_profile": SFCS_ZK_PRIVATE_ADD_PROTOCOL_V1_DRAFT
            }
        },
        "transcript": {
            "rounds": [
                {
                    "index": 0,
                    "component": "rust-subset-parser",
                    "note": "Accepted one u32 add function with two parameters."
                },
                {
                    "index": 1,
                    "component": "rv32i-emitter",
                    "note": "Emitted add output_register,lhs_register,rhs_register followed by ecall."
                },
                {
                    "index": 2,
                    "component": "sfcs-zk-profile",
                    "note": "Bound emitted program to the private no-overflow add proof profile."
                }
            ]
        },
        "semantic_dag": {
            "nodes": [
                {"id": "source", "type": "artifact", "label": "Rust source"},
                {"id": "program", "type": "artifact", "label": "RV32I program"},
                {"id": "proof-profile", "type": "claim", "label": "Private add ZK profile"}
            ],
            "edges": [
                {"from": "source", "to": "program", "kind": "compiled-to"},
                {"from": "program", "to": "proof-profile", "kind": "proven-by"}
            ]
        },
        "views": {
            "timeline": [],
            "claim_cards": [],
            "graphs": [],
            "diffs": []
        },
        "explanation_constraints": {
            "allowed_sources": ["packet_nodes", "transcript_rounds", "bound_core_metadata"],
            "forbid_unbound_claims": true,
            "mark_generated_text_non_authoritative": true
        }
    });
    packet["packet_digest"] = json!(semantic_packet_digest(&packet)?);
    Ok(SfcsCompiledPrivateAdd {
        schema: "power-house/sfcs-rust-private-add/v1-draft".to_string(),
        language: "rust-subset".to_string(),
        source_digest,
        function_name: parsed.function_name,
        lhs_name: parsed.lhs_name,
        rhs_name: parsed.rhs_name,
        return_type: "u32".to_string(),
        lhs_register: 10,
        rhs_register: 11,
        output_register: 3,
        program,
        semantic_packet: packet,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedPrivateAdd {
    function_name: String,
    lhs_name: String,
    rhs_name: String,
}

fn normalize_rust_source(source: &str) -> Result<String, SfcsCompilerError> {
    if source
        .chars()
        .any(|ch| ch.is_control() && ch != '\n' && ch != '\r' && ch != '\t')
    {
        return Err(SfcsCompilerError::InvalidSource(
            "source contains unsupported control characters".to_string(),
        ));
    }
    let without_line_comments = source
        .lines()
        .map(|line| line.split_once("//").map(|(head, _)| head).unwrap_or(line))
        .collect::<Vec<_>>()
        .join("\n");
    let normalized = without_line_comments
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if normalized.is_empty() {
        return Err(SfcsCompilerError::InvalidSource(
            "source is empty".to_string(),
        ));
    }
    Ok(normalized)
}

fn parse_private_add(source: &str) -> Result<ParsedPrivateAdd, SfcsCompilerError> {
    let source = source.strip_prefix("pub ").unwrap_or(source);
    let rest = source.strip_prefix("fn ").ok_or_else(|| {
        SfcsCompilerError::InvalidSource("source must start with fn or pub fn".to_string())
    })?;
    let (function_name, after_name) = split_identifier(rest)?;
    let after_name = after_name.trim_start();
    let after_name = after_name.strip_prefix('(').ok_or_else(|| {
        SfcsCompilerError::InvalidSource("function must declare parameters".to_string())
    })?;
    let (params, after_params) = after_name.split_once(')').ok_or_else(|| {
        SfcsCompilerError::InvalidSource("function parameter list is not closed".to_string())
    })?;
    let params = params.split(',').map(str::trim).collect::<Vec<_>>();
    if params.len() != 2 {
        return Err(SfcsCompilerError::InvalidSource(
            "private-add profile requires exactly two parameters".to_string(),
        ));
    }
    let lhs_name = parse_u32_param(params[0])?;
    let rhs_name = parse_u32_param(params[1])?;
    if lhs_name == rhs_name {
        return Err(SfcsCompilerError::InvalidSource(
            "parameters must be distinct".to_string(),
        ));
    }
    let after_params = after_params.trim_start();
    let after_return = after_params
        .strip_prefix("-> u32")
        .ok_or_else(|| SfcsCompilerError::InvalidSource("function must return u32".to_string()))?;
    let after_return = after_return.trim_start();
    let body = after_return
        .strip_prefix('{')
        .and_then(|value| value.strip_suffix('}'))
        .ok_or_else(|| {
            SfcsCompilerError::InvalidSource("function body must use braces".to_string())
        })?
        .trim();
    let expression = body
        .strip_prefix("return ")
        .unwrap_or(body)
        .trim_end_matches(';')
        .trim();
    let expected_left = format!("{lhs_name} + {rhs_name}");
    let expected_right = format!("{rhs_name} + {lhs_name}");
    if expression != expected_left && expression != expected_right {
        return Err(SfcsCompilerError::InvalidSource(format!(
            "private-add profile only accepts `{lhs_name} + {rhs_name}`"
        )));
    }
    Ok(ParsedPrivateAdd {
        function_name: function_name.to_string(),
        lhs_name,
        rhs_name,
    })
}

fn split_identifier(value: &str) -> Result<(&str, &str), SfcsCompilerError> {
    let mut end = 0;
    for (index, ch) in value.char_indices() {
        let allowed = if index == 0 {
            ch == '_' || ch.is_ascii_alphabetic()
        } else {
            ch == '_' || ch.is_ascii_alphanumeric()
        };
        if !allowed {
            break;
        }
        end = index + ch.len_utf8();
    }
    if end == 0 {
        return Err(SfcsCompilerError::InvalidSource(
            "expected identifier".to_string(),
        ));
    }
    Ok((&value[..end], &value[end..]))
}

fn parse_u32_param(param: &str) -> Result<String, SfcsCompilerError> {
    let (name, ty) = param.split_once(':').ok_or_else(|| {
        SfcsCompilerError::InvalidSource("parameter must use name: u32".to_string())
    })?;
    let name = name.trim();
    let ty = ty.trim();
    validate_identifier(name)?;
    if ty != "u32" {
        return Err(SfcsCompilerError::InvalidSource(
            "parameter type must be u32".to_string(),
        ));
    }
    Ok(name.to_string())
}

fn validate_identifier(value: &str) -> Result<(), SfcsCompilerError> {
    let (name, rest) = split_identifier(value)?;
    if name != value || !rest.is_empty() {
        return Err(SfcsCompilerError::InvalidSource(format!(
            "invalid identifier {value}"
        )));
    }
    Ok(())
}

fn source_digest(source: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(SOURCE_DIGEST_DOMAIN);
    hasher.update(source.as_bytes());
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

/// Errors returned by the Rust-subset compiler.
#[derive(Debug)]
pub enum SfcsCompilerError {
    /// Source cannot be compiled by this profile.
    InvalidSource(String),
    /// VM program construction failed.
    Vm(super::vm::SfcsVmError),
    /// Memory semantic packet digest failed.
    Memory(MemoryError),
}

impl fmt::Display for SfcsCompilerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSource(message) => {
                write!(formatter, "invalid SFCS compiler source: {message}")
            }
            Self::Vm(error) => write!(formatter, "SFCS compiler VM error: {error}"),
            Self::Memory(error) => {
                write!(formatter, "SFCS compiler semantic packet error: {error}")
            }
        }
    }
}

impl Error for SfcsCompilerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Vm(error) => Some(error),
            Self::Memory(error) => Some(error),
            _ => None,
        }
    }
}

impl From<super::vm::SfcsVmError> for SfcsCompilerError {
    fn from(error: super::vm::SfcsVmError) -> Self {
        Self::Vm(error)
    }
}

impl From<MemoryError> for SfcsCompilerError {
    fn from(error: MemoryError) -> Self {
        Self::Memory(error)
    }
}
