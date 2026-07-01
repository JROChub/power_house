//! Deterministic Rust-subset frontends for SFCS profiles.
//!
//! The public compiler lowers a small safe Rust expression subset directly
//! into SFCS fractal graphs. With `sfcs-zk`, the module also exposes the
//! narrower private-add compiler that emits the RV32I program consumed by the
//! private-add ZK profile.

#[cfg(feature = "sfcs-zk")]
use super::{
    vm::SfcsVmProgram,
    zk::{encode_rv32_add, SFCS_ZK_PRIVATE_ADD_PROTOCOL_V1_DRAFT},
};
use super::{SfcsGraph, SfcsNode, SfcsOp};
use crate::memory::{semantic_packet_digest, MemoryError};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

const SOURCE_DIGEST_DOMAIN: &[u8] = b"power-house:sfcs-compiler:v1-draft:source\0";
const PUBLIC_RUST_SCHEMA: &str = "power-house/sfcs-rust-public/v1-draft";
const LLVM_IR_SCHEMA: &str = "power-house/sfcs-llvm-ir/v1-draft";
const WASM_STACK_SCHEMA: &str = "power-house/sfcs-wasm-stack/v1-draft";
#[cfg(feature = "sfcs-zk")]
const PRIVATE_ADD_SCHEMA: &str = "power-house/sfcs-rust-private-add/v1-draft";

/// Compiler output for the public Rust-subset-to-SFCS path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcsCompiledPublicRust {
    /// Compiler schema.
    pub schema: String,
    /// Source language.
    pub language: String,
    /// Source digest after deterministic normalization.
    pub source_digest: String,
    /// Function name.
    pub function_name: String,
    /// Ordered parameter names.
    pub parameters: Vec<String>,
    /// Return type.
    pub return_type: String,
    /// Generated SFCS source.
    pub sfcs_source: String,
    /// Generated SFCS graph.
    pub graph: SfcsGraph,
    /// Non-core semantic packet for Observatory/slbit-style display.
    pub semantic_packet: Value,
}

/// Compiler output for the LLVM-style SSA IR to SFCS path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcsCompiledLlvmIr {
    /// Compiler schema.
    pub schema: String,
    /// Source language.
    pub language: String,
    /// Source digest after deterministic normalization.
    pub source_digest: String,
    /// Function name.
    pub function_name: String,
    /// Ordered parameter names without `%`.
    pub parameters: Vec<String>,
    /// Generated SFCS graph.
    pub graph: SfcsGraph,
    /// Non-core semantic packet for Observatory/slbit-style display.
    pub semantic_packet: Value,
}

/// Compiler output for the WASM-style stack IR to SFCS path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcsCompiledWasmStack {
    /// Compiler schema.
    pub schema: String,
    /// Source language.
    pub language: String,
    /// Source digest after deterministic normalization.
    pub source_digest: String,
    /// Ordered parameter names.
    pub parameters: Vec<String>,
    /// Generated SFCS graph.
    pub graph: SfcsGraph,
    /// Non-core semantic packet for Observatory/slbit-style display.
    pub semantic_packet: Value,
}

impl SfcsCompiledWasmStack {
    /// Returns the generated SFCS graph digest.
    pub fn graph_digest(&self) -> Result<String, SfcsCompilerError> {
        Ok(self.graph.fractal_digest()?)
    }
}

impl SfcsCompiledLlvmIr {
    /// Returns the generated SFCS graph digest.
    pub fn graph_digest(&self) -> Result<String, SfcsCompilerError> {
        Ok(self.graph.fractal_digest()?)
    }
}

impl SfcsCompiledPublicRust {
    /// Returns the generated SFCS graph digest.
    pub fn graph_digest(&self) -> Result<String, SfcsCompilerError> {
        Ok(self.graph.fractal_digest()?)
    }
}

/// Compiler output for the first Rust-subset private-add profile.
#[cfg(feature = "sfcs-zk")]
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

#[cfg(feature = "sfcs-zk")]
impl SfcsCompiledPrivateAdd {
    /// Returns the emitted VM program digest.
    pub fn program_digest(&self) -> Result<String, SfcsCompilerError> {
        Ok(self.program.program_digest()?)
    }
}

/// Compiles a public safe Rust expression function directly into an SFCS graph.
pub fn compile_public_rust_source(
    source: &str,
) -> Result<SfcsCompiledPublicRust, SfcsCompilerError> {
    let normalized = normalize_rust_source(source)?;
    let parsed = parse_public_rust_function(&normalized)?;
    let mut sfcs_source = String::new();
    for parameter in &parsed.parameters {
        sfcs_source.push_str("input ");
        sfcs_source.push_str(parameter);
        sfcs_source.push('\n');
    }
    sfcs_source.push_str("output ");
    sfcs_source.push_str(&parsed.expression);
    sfcs_source.push_str(" as return\n");
    let graph = SfcsGraph::from_source(&sfcs_source)?;
    let source_digest = source_digest(&normalized);
    let graph_digest = graph.fractal_digest()?;
    let mut packet = json!({
        "schema": "slbit/viz-packet/v3",
        "packet_id": format!("slp_sfcs_rust_public_{}", &source_digest["sha256:".len()..18]),
        "packet_digest": "",
        "claim": {
            "claim_id": format!("claim_{}", parsed.function_name),
            "label": format!("Rust subset function {} compiled directly to SFCS", parsed.function_name),
            "domain": "sfcs-public-rust-compiler",
            "status": "explained",
            "bound_core": {
                "source_digest": source_digest,
                "graph_digest": graph_digest,
                "compiler_schema": PUBLIC_RUST_SCHEMA
            }
        },
        "transcript": {
            "rounds": [
                {
                    "index": 0,
                    "component": "rust-subset-parser",
                    "note": "Accepted a public u32 expression function."
                },
                {
                    "index": 1,
                    "component": "sfcs-fractal-emitter",
                    "note": "Lowered parameters and expression directly into SFCS source and graph nodes."
                },
                {
                    "index": 2,
                    "component": "truth-boundary",
                    "note": "Generated semantic packet is non-core and cannot alter graph or .pha identity."
                }
            ]
        },
        "semantic_dag": {
            "nodes": [
                {"id": "source", "type": "artifact", "label": "Rust source"},
                {"id": "sfcs-source", "type": "artifact", "label": "SFCS source"},
                {"id": "sfcs-graph", "type": "artifact", "label": "SFCS graph"}
            ],
            "edges": [
                {"from": "source", "to": "sfcs-source", "kind": "lowered-to"},
                {"from": "sfcs-source", "to": "sfcs-graph", "kind": "parsed-as"}
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
    Ok(SfcsCompiledPublicRust {
        schema: PUBLIC_RUST_SCHEMA.to_string(),
        language: "rust-subset".to_string(),
        source_digest,
        function_name: parsed.function_name,
        parameters: parsed.parameters,
        return_type: "u32".to_string(),
        sfcs_source,
        graph,
        semantic_packet: packet,
    })
}

/// Compiles a deterministic LLVM-style SSA subset directly into an SFCS graph.
///
/// Supported line-oriented IR:
///
/// - `define i32 @name(i32 %a, i32 %b) {`
/// - `%x = add|sub|mul|and|or|xor|shl|lshr i32 <lhs>, <rhs>`
/// - `%x = icmp eq|ult|ugt|ule|uge i32 <lhs>, <rhs>`
/// - `%x = select i1 <cond>, i32 <true>, i32 <false>`
/// - `ret i32 <value>`
pub fn compile_llvm_ir_source(source: &str) -> Result<SfcsCompiledLlvmIr, SfcsCompilerError> {
    let normalized = normalize_llvm_ir_source(source)?;
    let source_digest = source_digest(&normalized);
    let mut compiler = LlvmIrCompiler::new();
    for (line_index, line) in normalized.lines().enumerate() {
        compiler.process_line(line, line_index + 1)?;
    }
    let (function_name, parameters, graph) = compiler.finish()?;
    let graph_digest = graph.fractal_digest()?;
    let mut packet = json!({
        "schema": "slbit/viz-packet/v3",
        "packet_id": format!("slp_sfcs_llvm_ir_{}", &source_digest["sha256:".len()..18]),
        "packet_digest": "",
        "claim": {
            "claim_id": format!("claim_{}", function_name),
            "label": format!("LLVM-style SSA function {} compiled directly to SFCS", function_name),
            "domain": "sfcs-llvm-ir-compiler",
            "status": "explained",
            "bound_core": {
                "source_digest": source_digest,
                "graph_digest": graph_digest,
                "compiler_schema": LLVM_IR_SCHEMA
            }
        },
        "transcript": {
            "rounds": [
                {
                    "index": 0,
                    "component": "llvm-ir-parser",
                    "note": "Accepted a deterministic i32 SSA subset with explicit return."
                },
                {
                    "index": 1,
                    "component": "sfcs-fractal-emitter",
                    "note": "Lowered SSA instructions directly into SFCS graph nodes without circuit flattening."
                },
                {
                    "index": 2,
                    "component": "truth-boundary",
                    "note": "Generated semantic packet is non-core and cannot alter graph or .pha identity."
                }
            ]
        },
        "semantic_dag": {
            "nodes": [
                {"id": "llvm-ir-source", "type": "artifact", "label": "LLVM-style IR"},
                {"id": "sfcs-graph", "type": "artifact", "label": "SFCS graph"}
            ],
            "edges": [
                {"from": "llvm-ir-source", "to": "sfcs-graph", "kind": "lowered-to"}
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
    Ok(SfcsCompiledLlvmIr {
        schema: LLVM_IR_SCHEMA.to_string(),
        language: "llvm-ir-subset".to_string(),
        source_digest,
        function_name,
        parameters,
        graph,
        semantic_packet: packet,
    })
}

/// Compiles a deterministic WASM-style stack IR directly into an SFCS graph.
///
/// Supported line-oriented instructions:
///
/// - `param <name> i32`
/// - `local.get <name>`
/// - `i32.const <value>`
/// - `i32.add`, `i32.sub`, `i32.mul`, `i32.and`, `i32.or`, `i32.xor`
/// - `i32.shl`, `i32.shr_u`, `i32.eq`, `i32.lt_u`, `i32.gt_u`
/// - `select`
/// - `return`
pub fn compile_wasm_stack_source(source: &str) -> Result<SfcsCompiledWasmStack, SfcsCompilerError> {
    let normalized = normalize_wasm_stack_source(source)?;
    let source_digest = source_digest(&normalized);
    let mut compiler = WasmStackCompiler::new();
    for (line_index, line) in normalized.lines().enumerate() {
        compiler.process_line(line, line_index + 1)?;
    }
    let (parameters, graph) = compiler.finish()?;
    let graph_digest = graph.fractal_digest()?;
    let mut packet = json!({
        "schema": "slbit/viz-packet/v3",
        "packet_id": format!("slp_sfcs_wasm_stack_{}", &source_digest["sha256:".len()..18]),
        "packet_digest": "",
        "claim": {
            "claim_id": "claim_wasm_stack",
            "label": "WASM-style stack IR compiled directly to SFCS",
            "domain": "sfcs-wasm-stack-compiler",
            "status": "explained",
            "bound_core": {
                "source_digest": source_digest,
                "graph_digest": graph_digest,
                "compiler_schema": WASM_STACK_SCHEMA
            }
        },
        "transcript": {
            "rounds": [
                {
                    "index": 0,
                    "component": "wasm-stack-parser",
                    "note": "Accepted deterministic i32 stack instructions."
                },
                {
                    "index": 1,
                    "component": "sfcs-fractal-emitter",
                    "note": "Lowered stack operations directly into SFCS graph nodes."
                }
            ]
        },
        "semantic_dag": {
            "nodes": [
                {"id": "wasm-stack-source", "type": "artifact", "label": "WASM-style source"},
                {"id": "sfcs-graph", "type": "artifact", "label": "SFCS graph"}
            ],
            "edges": [
                {"from": "wasm-stack-source", "to": "sfcs-graph", "kind": "compiled-to"}
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
    Ok(SfcsCompiledWasmStack {
        schema: WASM_STACK_SCHEMA.to_string(),
        language: "wasm-stack-subset".to_string(),
        source_digest,
        parameters,
        graph,
        semantic_packet: packet,
    })
}

/// Compiles a safe Rust add function into the private-add profile.
#[cfg(feature = "sfcs-zk")]
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
        schema: PRIVATE_ADD_SCHEMA.to_string(),
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

#[cfg(feature = "sfcs-zk")]
#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedPrivateAdd {
    function_name: String,
    lhs_name: String,
    rhs_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedPublicRustFunction {
    function_name: String,
    parameters: Vec<String>,
    expression: String,
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

fn parse_public_rust_function(source: &str) -> Result<ParsedPublicRustFunction, SfcsCompilerError> {
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
    let parameters = if params.trim().is_empty() {
        Vec::new()
    } else {
        params
            .split(',')
            .map(str::trim)
            .map(parse_u32_param)
            .collect::<Result<Vec<_>, _>>()?
    };
    let mut seen = std::collections::BTreeSet::new();
    for parameter in &parameters {
        if !seen.insert(parameter) {
            return Err(SfcsCompilerError::InvalidSource(
                "parameters must be distinct".to_string(),
            ));
        }
    }
    let after_params = after_params.trim_start();
    let after_return = after_params
        .strip_prefix("-> u32")
        .ok_or_else(|| SfcsCompilerError::InvalidSource("function must return u32".to_string()))?;
    let body = braced_body(after_return.trim_start())?;
    let expression = normalize_rust_return_expression(body)?;
    Ok(ParsedPublicRustFunction {
        function_name: function_name.to_string(),
        parameters,
        expression,
    })
}

#[cfg(feature = "sfcs-zk")]
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

fn braced_body(value: &str) -> Result<&str, SfcsCompilerError> {
    value
        .strip_prefix('{')
        .and_then(|body| body.strip_suffix('}'))
        .ok_or_else(|| {
            SfcsCompilerError::InvalidSource("function body must use braces".to_string())
        })
        .map(str::trim)
}

fn normalize_rust_return_expression(body: &str) -> Result<String, SfcsCompilerError> {
    let expression = body
        .strip_prefix("return ")
        .unwrap_or(body)
        .trim_end_matches(';')
        .trim();
    rust_expression_to_sfcs(expression)
}

fn rust_expression_to_sfcs(expression: &str) -> Result<String, SfcsCompilerError> {
    let expression = expression.trim();
    if expression.is_empty() {
        return Err(SfcsCompilerError::InvalidSource(
            "function expression is empty".to_string(),
        ));
    }
    if let Some(rest) = expression.strip_prefix("if ") {
        return rust_if_expression_to_sfcs(rest);
    }
    if expression.contains('{') || expression.contains('}') {
        return Err(SfcsCompilerError::InvalidSource(
            "braces are only supported in if expressions".to_string(),
        ));
    }
    Ok(expression.to_string())
}

fn rust_if_expression_to_sfcs(rest: &str) -> Result<String, SfcsCompilerError> {
    let Some(open_then) = rest.find('{') else {
        return Err(SfcsCompilerError::InvalidSource(
            "if expression requires then block".to_string(),
        ));
    };
    let condition = rest[..open_then].trim();
    let close_then = matching_brace(rest, open_then)?;
    let then_body = rest[open_then + 1..close_then].trim();
    let after_then = rest[close_then + 1..].trim();
    let after_else = after_then.strip_prefix("else").ok_or_else(|| {
        SfcsCompilerError::InvalidSource("if expression requires else block".to_string())
    })?;
    let after_else = after_else.trim_start();
    if !after_else.starts_with('{') {
        return Err(SfcsCompilerError::InvalidSource(
            "else block must use braces".to_string(),
        ));
    }
    let close_else = matching_brace(after_else, 0)?;
    if !after_else[close_else + 1..].trim().is_empty() {
        return Err(SfcsCompilerError::InvalidSource(
            "unexpected tokens after else block".to_string(),
        ));
    }
    let else_body = after_else[1..close_else].trim();
    Ok(format!(
        "if {} then {} else {}",
        rust_expression_to_sfcs(condition)?,
        rust_expression_to_sfcs(then_body)?,
        rust_expression_to_sfcs(else_body)?
    ))
}

fn matching_brace(value: &str, open_index: usize) -> Result<usize, SfcsCompilerError> {
    let bytes = value.as_bytes();
    if bytes.get(open_index) != Some(&b'{') {
        return Err(SfcsCompilerError::InvalidSource(
            "expected opening brace".to_string(),
        ));
    }
    let mut depth = 0_u32;
    for (index, byte) in bytes.iter().enumerate().skip(open_index) {
        match byte {
            b'{' => depth += 1,
            b'}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Ok(index);
                }
            }
            _ => {}
        }
    }
    Err(SfcsCompilerError::InvalidSource(
        "unclosed brace in if expression".to_string(),
    ))
}

fn normalize_llvm_ir_source(source: &str) -> Result<String, SfcsCompilerError> {
    if source
        .chars()
        .any(|ch| ch.is_control() && ch != '\n' && ch != '\r' && ch != '\t')
    {
        return Err(SfcsCompilerError::InvalidSource(
            "LLVM IR source contains unsupported control characters".to_string(),
        ));
    }
    let lines = source
        .lines()
        .filter_map(|line| {
            let line = line.split_once(';').map(|(head, _)| head).unwrap_or(line);
            let normalized = line.split_whitespace().collect::<Vec<_>>().join(" ");
            if normalized.is_empty() {
                None
            } else {
                Some(normalized)
            }
        })
        .collect::<Vec<_>>();
    if lines.is_empty() {
        return Err(SfcsCompilerError::InvalidSource(
            "LLVM IR source is empty".to_string(),
        ));
    }
    Ok(lines.join("\n"))
}

struct LlvmIrCompiler {
    graph: SfcsGraph,
    function_name: Option<String>,
    parameters: Vec<String>,
    values: BTreeMap<String, String>,
    temp_counter: u64,
    function_open: bool,
    returned: bool,
    closed: bool,
}

impl LlvmIrCompiler {
    fn new() -> Self {
        Self {
            graph: SfcsGraph::new(Vec::new()),
            function_name: None,
            parameters: Vec::new(),
            values: BTreeMap::new(),
            temp_counter: 0,
            function_open: false,
            returned: false,
            closed: false,
        }
    }

    fn process_line(&mut self, line: &str, line_number: usize) -> Result<(), SfcsCompilerError> {
        if line.starts_with("define ") {
            return self.define(line, line_number);
        }
        if line == "}" {
            return self.close(line_number);
        }
        if self.closed {
            return Err(SfcsCompilerError::InvalidSource(format!(
                "line {line_number}: instructions after closing brace are not supported"
            )));
        }
        if !self.function_open {
            return Err(SfcsCompilerError::InvalidSource(format!(
                "line {line_number}: LLVM IR must start with a define line"
            )));
        }
        if line.ends_with(':') {
            validate_identifier(line.trim_end_matches(':'))?;
            return Ok(());
        }
        if self.returned {
            return Err(SfcsCompilerError::InvalidSource(format!(
                "line {line_number}: instructions after ret are not supported"
            )));
        }
        if line.starts_with("ret ") {
            return self.ret(line, line_number);
        }
        let Some((lhs, rhs)) = line.split_once(" = ") else {
            return Err(SfcsCompilerError::InvalidSource(format!(
                "line {line_number}: expected SSA assignment or ret"
            )));
        };
        self.assignment(lhs.trim(), rhs.trim(), line_number)
    }

    fn finish(self) -> Result<(String, Vec<String>, SfcsGraph), SfcsCompilerError> {
        if !self.function_open {
            return Err(SfcsCompilerError::InvalidSource(
                "LLVM IR did not define a function".to_string(),
            ));
        }
        if !self.returned {
            return Err(SfcsCompilerError::InvalidSource(
                "LLVM IR function must return one i32 value".to_string(),
            ));
        }
        if !self.closed {
            return Err(SfcsCompilerError::InvalidSource(
                "LLVM IR function must close with }".to_string(),
            ));
        }
        self.graph.verify()?;
        Ok((
            self.function_name.unwrap_or_default(),
            self.parameters,
            self.graph,
        ))
    }

    fn define(&mut self, line: &str, line_number: usize) -> Result<(), SfcsCompilerError> {
        if self.function_open {
            return Err(SfcsCompilerError::InvalidSource(format!(
                "line {line_number}: only one LLVM IR function is supported"
            )));
        }
        let rest = line.strip_prefix("define i32 @").ok_or_else(|| {
            SfcsCompilerError::InvalidSource(format!(
                "line {line_number}: only `define i32 @name(...) {{` is supported"
            ))
        })?;
        let (name, after_name) = rest.split_once('(').ok_or_else(|| {
            SfcsCompilerError::InvalidSource(format!(
                "line {line_number}: define line must include parameter list"
            ))
        })?;
        validate_identifier(name)?;
        let (params, after_params) = after_name.split_once(')').ok_or_else(|| {
            SfcsCompilerError::InvalidSource(format!(
                "line {line_number}: define parameter list is not closed"
            ))
        })?;
        if after_params.trim() != "{" {
            return Err(SfcsCompilerError::InvalidSource(format!(
                "line {line_number}: define line must end with opening brace"
            )));
        }
        self.function_name = Some(name.to_string());
        let mut seen = BTreeSet::new();
        if !params.trim().is_empty() {
            for param in params.split(',') {
                let parts = param.split_whitespace().collect::<Vec<_>>();
                if parts.len() != 2 || parts[0] != "i32" {
                    return Err(SfcsCompilerError::InvalidSource(format!(
                        "line {line_number}: parameters must use `i32 %name`"
                    )));
                }
                let source_name = parts[1];
                if !source_name.starts_with('%') {
                    return Err(SfcsCompilerError::InvalidSource(format!(
                        "line {line_number}: parameter names must start with %"
                    )));
                }
                let node_id = llvm_value_id(source_name)?;
                if !seen.insert(node_id.clone()) {
                    return Err(SfcsCompilerError::InvalidSource(format!(
                        "line {line_number}: duplicate parameter {source_name}"
                    )));
                }
                self.values.insert(source_name.to_string(), node_id.clone());
                self.parameters
                    .push(source_name.trim_start_matches('%').to_string());
                self.graph
                    .insert_node(SfcsNode::new(&node_id, SfcsOp::Input, Vec::new()))?;
            }
        }
        self.function_open = true;
        Ok(())
    }

    fn close(&mut self, line_number: usize) -> Result<(), SfcsCompilerError> {
        if self.closed {
            return Err(SfcsCompilerError::InvalidSource(format!(
                "line {line_number}: duplicate closing brace"
            )));
        }
        if !self.function_open {
            return Err(SfcsCompilerError::InvalidSource(format!(
                "line {line_number}: unexpected closing brace"
            )));
        }
        if !self.returned {
            return Err(SfcsCompilerError::InvalidSource(format!(
                "line {line_number}: closing brace before ret"
            )));
        }
        self.closed = true;
        Ok(())
    }

    fn assignment(
        &mut self,
        lhs: &str,
        rhs: &str,
        line_number: usize,
    ) -> Result<(), SfcsCompilerError> {
        if !lhs.starts_with('%') {
            return Err(SfcsCompilerError::InvalidSource(format!(
                "line {line_number}: assignment target must be an SSA value"
            )));
        }
        if self.values.contains_key(lhs) {
            return Err(SfcsCompilerError::InvalidSource(format!(
                "line {line_number}: SSA value {lhs} is already defined"
            )));
        }
        let parts = rhs.split_whitespace().collect::<Vec<_>>();
        match parts.as_slice() {
            [op, "i32", left, right] => {
                let op = match *op {
                    "add" => SfcsOp::Add,
                    "sub" => SfcsOp::Sub,
                    "mul" => SfcsOp::Mul,
                    "and" => SfcsOp::BitAnd,
                    "or" => SfcsOp::BitOr,
                    "xor" => SfcsOp::BitXor,
                    "shl" => SfcsOp::Shl,
                    "lshr" => SfcsOp::Shr,
                    other => {
                        return Err(SfcsCompilerError::InvalidSource(format!(
                            "line {line_number}: unsupported LLVM i32 op `{other}`"
                        )));
                    }
                };
                let left = self.operand(left, line_number)?;
                let right = self.operand(right, line_number)?;
                self.define_node(lhs, op, vec![left, right], line_number)
            }
            ["icmp", predicate, "i32", left, right] => {
                let op = match *predicate {
                    "eq" => SfcsOp::Eq,
                    "ult" => SfcsOp::Lt,
                    "ugt" => SfcsOp::Gt,
                    "ule" => SfcsOp::Le,
                    "uge" => SfcsOp::Ge,
                    other => {
                        return Err(SfcsCompilerError::InvalidSource(format!(
                            "line {line_number}: unsupported LLVM icmp predicate `{other}`"
                        )));
                    }
                };
                let left = self.operand(left, line_number)?;
                let right = self.operand(right, line_number)?;
                self.define_node(lhs, op, vec![left, right], line_number)
            }
            ["select", "i1", condition, "i32", true_value, "i32", false_value] => {
                let condition = self.operand(condition, line_number)?;
                let true_value = self.operand(true_value, line_number)?;
                let false_value = self.operand(false_value, line_number)?;
                self.define_node(
                    lhs,
                    SfcsOp::Branch,
                    vec![condition, true_value, false_value],
                    line_number,
                )
            }
            _ => Err(SfcsCompilerError::InvalidSource(format!(
                "line {line_number}: unsupported LLVM IR assignment `{rhs}`"
            ))),
        }
    }

    fn define_node(
        &mut self,
        lhs: &str,
        op: SfcsOp,
        inputs: Vec<String>,
        line_number: usize,
    ) -> Result<(), SfcsCompilerError> {
        let node_id = llvm_value_id(lhs)?;
        if self.graph.nodes.contains_key(&node_id) {
            return Err(SfcsCompilerError::InvalidSource(format!(
                "line {line_number}: generated node id {node_id} collides"
            )));
        }
        let inputs = self.distinct_inputs(inputs)?;
        self.graph.insert_node(
            SfcsNode::new(&node_id, op, inputs).with_metadata("source_op", "llvm-ir"),
        )?;
        self.values.insert(lhs.to_string(), node_id);
        Ok(())
    }

    fn ret(&mut self, line: &str, line_number: usize) -> Result<(), SfcsCompilerError> {
        let parts = line.split_whitespace().collect::<Vec<_>>();
        let ["ret", "i32", value] = parts.as_slice() else {
            return Err(SfcsCompilerError::InvalidSource(format!(
                "line {line_number}: only `ret i32 <value>` is supported"
            )));
        };
        let output = self.operand(value, line_number)?;
        self.graph.outputs = vec![output];
        self.returned = true;
        Ok(())
    }

    fn operand(&mut self, value: &str, line_number: usize) -> Result<String, SfcsCompilerError> {
        let value = value.trim_end_matches(',').trim();
        if value.starts_with('%') {
            return self.values.get(value).cloned().ok_or_else(|| {
                SfcsCompilerError::InvalidSource(format!(
                    "line {line_number}: unknown LLVM value {value}"
                ))
            });
        }
        let parsed = value.parse::<u64>().map_err(|error| {
            SfcsCompilerError::InvalidSource(format!(
                "line {line_number}: invalid LLVM integer operand `{value}`: {error}"
            ))
        })?;
        if parsed > u32::MAX as u64 {
            return Err(SfcsCompilerError::InvalidSource(format!(
                "line {line_number}: LLVM i32 constant is outside u32 range"
            )));
        }
        let id = self.next_temp("const");
        self.graph
            .insert_node(SfcsNode::constant(&id, parsed as i64))?;
        Ok(id)
    }

    fn distinct_inputs(&mut self, inputs: Vec<String>) -> Result<Vec<String>, SfcsCompilerError> {
        let mut seen = BTreeSet::new();
        let mut distinct = Vec::new();
        for input in inputs {
            if seen.insert(input.clone()) {
                distinct.push(input);
                continue;
            }
            let alias_id = self.next_temp("alias");
            self.graph
                .insert_node(SfcsNode::new(&alias_id, SfcsOp::Alias, vec![input]))?;
            distinct.push(alias_id);
        }
        Ok(distinct)
    }

    fn next_temp(&mut self, label: &str) -> String {
        loop {
            let id = format!("__llvm_{label}_{:06}", self.temp_counter);
            self.temp_counter += 1;
            if !self.graph.nodes.contains_key(&id) {
                return id;
            }
        }
    }
}

fn llvm_value_id(value: &str) -> Result<String, SfcsCompilerError> {
    let raw = value.trim_start_matches('%');
    if raw.is_empty() {
        return Err(SfcsCompilerError::InvalidSource(
            "LLVM SSA value cannot be empty".to_string(),
        ));
    }
    let id = if raw.chars().next().unwrap().is_ascii_digit() {
        format!("v_{raw}")
    } else {
        raw.to_string()
    };
    validate_identifier(&id)?;
    Ok(id)
}

fn normalize_wasm_stack_source(source: &str) -> Result<String, SfcsCompilerError> {
    if source
        .chars()
        .any(|ch| ch.is_control() && ch != '\n' && ch != '\r' && ch != '\t')
    {
        return Err(SfcsCompilerError::InvalidSource(
            "WASM stack source contains unsupported control characters".to_string(),
        ));
    }
    let lines = source
        .lines()
        .filter_map(|line| {
            let line = line.split_once(";;").map(|(head, _)| head).unwrap_or(line);
            let normalized = line.split_whitespace().collect::<Vec<_>>().join(" ");
            if normalized.is_empty() {
                None
            } else {
                Some(normalized)
            }
        })
        .collect::<Vec<_>>();
    if lines.is_empty() {
        return Err(SfcsCompilerError::InvalidSource(
            "WASM stack source is empty".to_string(),
        ));
    }
    Ok(lines.join("\n"))
}

struct WasmStackCompiler {
    graph: SfcsGraph,
    parameters: Vec<String>,
    parameter_set: BTreeSet<String>,
    stack: Vec<String>,
    temp_counter: u64,
    returned: bool,
}

impl WasmStackCompiler {
    fn new() -> Self {
        Self {
            graph: SfcsGraph::new(Vec::new()),
            parameters: Vec::new(),
            parameter_set: BTreeSet::new(),
            stack: Vec::new(),
            temp_counter: 0,
            returned: false,
        }
    }

    fn process_line(&mut self, line: &str, line_number: usize) -> Result<(), SfcsCompilerError> {
        if self.returned {
            return Err(SfcsCompilerError::InvalidSource(format!(
                "line {line_number}: instructions after return are not supported"
            )));
        }
        let parts = line.split_whitespace().collect::<Vec<_>>();
        match parts.as_slice() {
            ["param", name, "i32"] => self.param(name, line_number),
            ["local.get", name] => self.local_get(name, line_number),
            ["i32.const", value] => self.i32_const(value, line_number),
            ["i32.add"] => self.binary(SfcsOp::Add, "i32_add", line_number),
            ["i32.sub"] => self.binary(SfcsOp::Sub, "i32_sub", line_number),
            ["i32.mul"] => self.binary(SfcsOp::Mul, "i32_mul", line_number),
            ["i32.and"] => self.binary(SfcsOp::BitAnd, "i32_and", line_number),
            ["i32.or"] => self.binary(SfcsOp::BitOr, "i32_or", line_number),
            ["i32.xor"] => self.binary(SfcsOp::BitXor, "i32_xor", line_number),
            ["i32.shl"] => self.binary(SfcsOp::Shl, "i32_shl", line_number),
            ["i32.shr_u"] => self.binary(SfcsOp::Shr, "i32_shr_u", line_number),
            ["i32.eq"] => self.binary(SfcsOp::Eq, "i32_eq", line_number),
            ["i32.lt_u"] => self.binary(SfcsOp::Lt, "i32_lt_u", line_number),
            ["i32.gt_u"] => self.binary(SfcsOp::Gt, "i32_gt_u", line_number),
            ["select"] => self.select(line_number),
            ["return"] => self.return_top(line_number),
            _ => Err(SfcsCompilerError::InvalidSource(format!(
                "line {line_number}: unsupported WASM stack instruction `{line}`"
            ))),
        }
    }

    fn finish(mut self) -> Result<(Vec<String>, SfcsGraph), SfcsCompilerError> {
        if !self.returned {
            if self.stack.len() == 1 {
                self.graph.outputs = vec![self.stack.pop().unwrap()];
            } else {
                return Err(SfcsCompilerError::InvalidSource(
                    "WASM stack program must return exactly one value".to_string(),
                ));
            }
        }
        self.graph.verify()?;
        Ok((self.parameters, self.graph))
    }

    fn param(&mut self, name: &str, line_number: usize) -> Result<(), SfcsCompilerError> {
        validate_identifier(name)?;
        if !self.parameter_set.insert(name.to_string()) {
            return Err(SfcsCompilerError::InvalidSource(format!(
                "line {line_number}: duplicate parameter {name}"
            )));
        }
        self.parameters.push(name.to_string());
        self.graph
            .insert_node(SfcsNode::new(name, SfcsOp::Input, Vec::new()))?;
        Ok(())
    }

    fn local_get(&mut self, name: &str, line_number: usize) -> Result<(), SfcsCompilerError> {
        validate_identifier(name)?;
        if !self.parameter_set.contains(name) {
            return Err(SfcsCompilerError::InvalidSource(format!(
                "line {line_number}: unknown local {name}"
            )));
        }
        self.stack.push(name.to_string());
        Ok(())
    }

    fn i32_const(&mut self, value: &str, line_number: usize) -> Result<(), SfcsCompilerError> {
        let value = value.parse::<i64>().map_err(|error| {
            SfcsCompilerError::InvalidSource(format!(
                "line {line_number}: invalid i32.const value: {error}"
            ))
        })?;
        if !(0..=u32::MAX as i64).contains(&value) {
            return Err(SfcsCompilerError::InvalidSource(format!(
                "line {line_number}: i32.const is outside u32 range"
            )));
        }
        let id = self.next_temp("const");
        self.graph.insert_node(SfcsNode::constant(&id, value))?;
        self.stack.push(id);
        Ok(())
    }

    fn binary(
        &mut self,
        op: SfcsOp,
        label: &str,
        line_number: usize,
    ) -> Result<(), SfcsCompilerError> {
        let right = self.pop(line_number)?;
        let left = self.pop(line_number)?;
        let inputs = self.distinct_inputs(vec![left, right])?;
        let id = self.next_temp(label);
        self.graph
            .insert_node(SfcsNode::new(&id, op, inputs).with_metadata("source_op", label))?;
        self.stack.push(id);
        Ok(())
    }

    fn select(&mut self, line_number: usize) -> Result<(), SfcsCompilerError> {
        let false_value = self.pop(line_number)?;
        let true_value = self.pop(line_number)?;
        let condition = self.pop(line_number)?;
        let inputs = self.distinct_inputs(vec![condition, true_value, false_value])?;
        let id = self.next_temp("select");
        self.graph.insert_node(
            SfcsNode::new(&id, SfcsOp::Branch, inputs).with_metadata("source_op", "select"),
        )?;
        self.stack.push(id);
        Ok(())
    }

    fn return_top(&mut self, line_number: usize) -> Result<(), SfcsCompilerError> {
        let value = self.pop(line_number)?;
        if !self.stack.is_empty() {
            return Err(SfcsCompilerError::InvalidSource(format!(
                "line {line_number}: return requires exactly one stack value"
            )));
        }
        self.graph.outputs = vec![value];
        self.returned = true;
        Ok(())
    }

    fn pop(&mut self, line_number: usize) -> Result<String, SfcsCompilerError> {
        self.stack.pop().ok_or_else(|| {
            SfcsCompilerError::InvalidSource(format!("line {line_number}: stack underflow"))
        })
    }

    fn distinct_inputs(&mut self, inputs: Vec<String>) -> Result<Vec<String>, SfcsCompilerError> {
        let mut seen = BTreeSet::new();
        let mut distinct = Vec::new();
        for input in inputs {
            if seen.insert(input.clone()) {
                distinct.push(input);
                continue;
            }
            let alias_id = self.next_temp("alias");
            self.graph
                .insert_node(SfcsNode::new(&alias_id, SfcsOp::Alias, vec![input]))?;
            distinct.push(alias_id);
        }
        Ok(distinct)
    }

    fn next_temp(&mut self, label: &str) -> String {
        loop {
            let id = format!("__wasm_{label}_{:06}", self.temp_counter);
            self.temp_counter += 1;
            if !self.graph.nodes.contains_key(&id) {
                return id;
            }
        }
    }
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
    /// SFCS graph construction failed.
    Sfcs(super::SfcsError),
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
            Self::Sfcs(error) => write!(formatter, "SFCS compiler graph error: {error}"),
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
            Self::Sfcs(error) => Some(error),
            Self::Vm(error) => Some(error),
            Self::Memory(error) => Some(error),
            _ => None,
        }
    }
}

impl From<super::SfcsError> for SfcsCompilerError {
    fn from(error: super::SfcsError) -> Self {
        Self::Sfcs(error)
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
