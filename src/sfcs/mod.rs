//! Experimental Sovereign Fractal Computation Substrate primitives.
//!
//! SFCS is intentionally feature-gated behind `sfcs` and does not alter
//! `.pha`, Rootprint, Memory Capsule, or slbit semantics. A computational
//! fractal can be committed as ordinary `.pha` core data and then anchored by
//! Rootprint, but Rootprint v1 branch identity remains unchanged.

use crate::provenance::{PhaArtifact, PhaError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

/// Draft schema identifier for SFCS computational fractals.
pub const SFCS_SCHEMA_V1_DRAFT: &str = "power-house/sfcs-fractal/v1-draft";

const FRACTAL_DIGEST_DOMAIN: &[u8] = b"power-house:sfcs:v1-draft:fractal\0";
const FAST_PATH_DOMAIN: &[u8] = b"power-house:sfcs:v1-draft:fast-path-workload\0";
const TRACE_INPUT_DOMAIN: &[u8] = b"power-house:sfcs:v1-draft:trace-inputs\0";
const TRACE_OUTPUT_DOMAIN: &[u8] = b"power-house:sfcs:v1-draft:trace-outputs\0";
const TRACE_MEMORY_DOMAIN: &[u8] = b"power-house:sfcs:v1-draft:trace-memory\0";
const TRACE_STEP_DOMAIN: &[u8] = b"power-house:sfcs:v1-draft:trace-step\0";
const TRACE_DOMAIN: &[u8] = b"power-house:sfcs:v1-draft:execution-trace\0";
const STRUCTURE_REGION_DOMAIN: &[u8] = b"power-house:sfcs:v1-draft:structure-region\0";
const SYNTHESIS_OPERATION_DOMAIN: &[u8] = b"power-house:sfcs:v1-draft:synthesis-operation\0";
const SYNTHESIS_PLAN_DOMAIN: &[u8] = b"power-house:sfcs:v1-draft:synthesis-plan\0";
const EMBEDDING_INVARIANT_DOMAIN: &[u8] = b"power-house:sfcs:v1-draft:embedding-invariant\0";
const SHA256_PREFIX: &str = "sha256:";

/// A deterministic computational operation carried by an SFCS node.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SfcsOp {
    /// Public input value supplied to evaluation.
    Input,
    /// Deterministic identity edge used by direct source lowering.
    Alias,
    /// Integer constant stored in `params["value"]`.
    Const,
    /// Integer addition over ordered inputs.
    Add,
    /// Integer subtraction over two ordered inputs.
    Sub,
    /// Integer multiplication over ordered inputs.
    Mul,
    /// Integer division over two ordered inputs. Division by zero rejects execution.
    Div,
    /// Integer remainder over two ordered inputs. Division by zero rejects execution.
    Mod,
    /// Deterministic equality predicate. Returns 1 for equality and 0 otherwise.
    Eq,
    /// Deterministic less-than predicate. Returns 1 when left < right.
    Lt,
    /// Deterministic less-than-or-equal predicate. Returns 1 when left <= right.
    Le,
    /// Deterministic greater-than predicate. Returns 1 when left > right.
    Gt,
    /// Deterministic greater-than-or-equal predicate. Returns 1 when left >= right.
    Ge,
    /// Deterministic nonzero conjunction. Returns 1 when both inputs are nonzero.
    And,
    /// Deterministic nonzero disjunction. Returns 1 when either input is nonzero.
    Or,
    /// Deterministic nonzero negation. Returns 1 when the input is zero.
    Not,
    /// Deterministic bitwise conjunction over two ordered inputs.
    BitAnd,
    /// Deterministic bitwise disjunction over two ordered inputs.
    BitOr,
    /// Deterministic bitwise exclusive-or over two ordered inputs.
    BitXor,
    /// Deterministic wrapping left shift. Negative or oversized shifts reject execution.
    Shl,
    /// Deterministic wrapping right shift. Negative or oversized shifts reject execution.
    Shr,
    /// Deterministic branch: input 0 is the condition, input 1 true, input 2 false.
    Branch,
    /// Dense opaque step that is not eligible for the Sovereign fast path.
    DenseStep,
    /// Deterministic memory read. Input 0 is the address; input 1 optionally orders memory state.
    MemoryRead,
    /// Deterministic memory write. Inputs 0 and 1 are address and value; input 2 optionally orders memory state.
    MemoryWrite,
    /// Node already rewritten to an external fast-path workload certificate.
    FastPathClaim,
}

/// One computational fractal node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcsNode {
    /// Stable node identifier within the fractal graph.
    pub id: String,
    /// Operation kind.
    pub op: SfcsOp,
    /// Ordered parent node identifiers. Order is semantic and is committed.
    pub inputs: Vec<String>,
    /// Deterministic integer parameters.
    pub params: BTreeMap<String, i64>,
    /// Optional human label. Labels are metadata and are committed by the draft digest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Deterministic source or structure metadata committed by the draft digest.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, String>,
}

impl SfcsNode {
    /// Creates a node with no parameters.
    pub fn new(id: impl Into<String>, op: SfcsOp, inputs: Vec<String>) -> Self {
        Self {
            id: id.into(),
            op,
            inputs,
            params: BTreeMap::new(),
            label: None,
            metadata: BTreeMap::new(),
        }
    }

    /// Creates an integer constant node.
    pub fn constant(id: impl Into<String>, value: i64) -> Self {
        Self {
            id: id.into(),
            op: SfcsOp::Const,
            inputs: Vec::new(),
            params: BTreeMap::from([("value".to_string(), value)]),
            label: None,
            metadata: BTreeMap::new(),
        }
    }

    /// Sets a metadata label.
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Adds deterministic source or structural metadata.
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    fn is_fast_path_eligible(&self) -> bool {
        matches!(
            self.op,
            SfcsOp::Input
                | SfcsOp::Alias
                | SfcsOp::Const
                | SfcsOp::Add
                | SfcsOp::Sub
                | SfcsOp::Mul
                | SfcsOp::FastPathClaim
        )
    }
}

/// A canonical draft computational fractal graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcsGraph {
    /// SFCS schema identifier.
    pub schema: String,
    /// Nodes keyed by stable node ID.
    pub nodes: BTreeMap<String, SfcsNode>,
    /// Output node identifiers in producer-declared order.
    pub outputs: Vec<String>,
}

impl SfcsGraph {
    /// Parses a small deterministic SFCS program directly into a fractal graph.
    ///
    /// This is intentionally not a circuit compiler. Each source line maps to
    /// one first-class fractal node or output declaration.
    ///
    /// Supported lines:
    ///
    /// - `input <id>`
    /// - `alias <id> <input>`
    /// - `const <id> <integer>`
    /// - `add <id> <input> <input> [input...]`
    /// - `sub <id> <left> <right>`
    /// - `mul <id> <input> <input> [input...]`
    /// - `div <id> <left> <right>`
    /// - `mod <id> <left> <right>`
    /// - `eq <id> <left> <right>`
    /// - `lt <id> <left> <right>`
    /// - `le <id> <left> <right>`
    /// - `gt <id> <left> <right>`
    /// - `ge <id> <left> <right>`
    /// - `and <id> <left> <right>`
    /// - `or <id> <left> <right>`
    /// - `bit_and <id> <left> <right>`
    /// - `bit_or <id> <left> <right>`
    /// - `bit_xor <id> <left> <right>`
    /// - `shl <id> <left> <right>`
    /// - `shr <id> <left> <right>`
    /// - `not <id> <input>`
    /// - `branch <id> <condition> <true> <false>`
    /// - `dense <id> <input> [input...]`
    /// - `memory_read <id> <input> [input...]`
    /// - `memory_write <id> <input> [input...]`
    /// - `label <id> <text...>`
    /// - `meta <id> <key> <value...>`
    /// - `output <id> [id...]`
    pub fn from_program(source: &str) -> Result<Self, SfcsError> {
        let mut graph = Self::new(Vec::new());
        let mut outputs = Vec::new();

        for (line_index, raw_line) in source.lines().enumerate() {
            let line_number = line_index + 1;
            let line = raw_line.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let parts = line.split_whitespace().collect::<Vec<_>>();
            let Some((keyword, rest)) = parts.split_first() else {
                continue;
            };
            match *keyword {
                "input" => {
                    require_program_arity(line_number, keyword, rest, 1)?;
                    graph.insert_node(SfcsNode::new(rest[0], SfcsOp::Input, Vec::new()))?;
                }
                "alias" => {
                    require_program_arity(line_number, keyword, rest, 2)?;
                    graph.insert_node(SfcsNode::new(
                        rest[0],
                        SfcsOp::Alias,
                        vec![rest[1].to_string()],
                    ))?;
                }
                "const" => {
                    require_program_arity(line_number, keyword, rest, 2)?;
                    let value = rest[1].parse::<i64>().map_err(|error| {
                        SfcsError::InvalidProgram(format!(
                            "line {line_number}: invalid const value: {error}"
                        ))
                    })?;
                    graph.insert_node(SfcsNode::constant(rest[0], value))?;
                }
                "add" | "mul" => {
                    if rest.len() < 3 {
                        return Err(SfcsError::InvalidProgram(format!(
                            "line {line_number}: {keyword} requires id and at least two inputs"
                        )));
                    }
                    let op = if *keyword == "add" {
                        SfcsOp::Add
                    } else {
                        SfcsOp::Mul
                    };
                    graph.insert_node(SfcsNode::new(
                        rest[0],
                        op,
                        rest[1..].iter().map(|value| (*value).to_string()).collect(),
                    ))?;
                }
                "sub" | "div" | "mod" | "eq" | "lt" | "le" | "gt" | "ge" | "and" | "or"
                | "bit_and" | "bit_or" | "bit_xor" | "shl" | "shr" => {
                    require_program_arity(line_number, keyword, rest, 3)?;
                    let op = match *keyword {
                        "sub" => SfcsOp::Sub,
                        "div" => SfcsOp::Div,
                        "mod" => SfcsOp::Mod,
                        "eq" => SfcsOp::Eq,
                        "lt" => SfcsOp::Lt,
                        "le" => SfcsOp::Le,
                        "gt" => SfcsOp::Gt,
                        "ge" => SfcsOp::Ge,
                        "and" => SfcsOp::And,
                        "or" => SfcsOp::Or,
                        "bit_and" => SfcsOp::BitAnd,
                        "bit_or" => SfcsOp::BitOr,
                        "bit_xor" => SfcsOp::BitXor,
                        "shl" => SfcsOp::Shl,
                        "shr" => SfcsOp::Shr,
                        _ => unreachable!(),
                    };
                    graph.insert_node(SfcsNode::new(
                        rest[0],
                        op,
                        rest[1..].iter().map(|value| (*value).to_string()).collect(),
                    ))?;
                }
                "not" => {
                    require_program_arity(line_number, keyword, rest, 2)?;
                    graph.insert_node(SfcsNode::new(
                        rest[0],
                        SfcsOp::Not,
                        vec![rest[1].to_string()],
                    ))?;
                }
                "branch" => {
                    require_program_arity(line_number, keyword, rest, 4)?;
                    graph.insert_node(SfcsNode::new(
                        rest[0],
                        SfcsOp::Branch,
                        rest[1..].iter().map(|value| (*value).to_string()).collect(),
                    ))?;
                }
                "dense" | "memory_read" | "memory_write" => {
                    if rest.len() < 2 {
                        return Err(SfcsError::InvalidProgram(format!(
                            "line {line_number}: {keyword} requires id and at least one input"
                        )));
                    }
                    let op = match *keyword {
                        "dense" => SfcsOp::DenseStep,
                        "memory_read" => SfcsOp::MemoryRead,
                        "memory_write" => SfcsOp::MemoryWrite,
                        _ => unreachable!(),
                    };
                    graph.insert_node(SfcsNode::new(
                        rest[0],
                        op,
                        rest[1..].iter().map(|value| (*value).to_string()).collect(),
                    ))?;
                }
                "label" => {
                    if rest.len() < 2 {
                        return Err(SfcsError::InvalidProgram(format!(
                            "line {line_number}: label requires id and text"
                        )));
                    }
                    let node = graph.nodes.get_mut(rest[0]).ok_or_else(|| {
                        SfcsError::InvalidProgram(format!(
                            "line {line_number}: label references unknown node {}",
                            rest[0]
                        ))
                    })?;
                    node.label = Some(rest[1..].join(" "));
                }
                "meta" => {
                    if rest.len() < 3 {
                        return Err(SfcsError::InvalidProgram(format!(
                            "line {line_number}: meta requires id, key, and value"
                        )));
                    }
                    validate_id(rest[1])?;
                    let node = graph.nodes.get_mut(rest[0]).ok_or_else(|| {
                        SfcsError::InvalidProgram(format!(
                            "line {line_number}: meta references unknown node {}",
                            rest[0]
                        ))
                    })?;
                    node.metadata
                        .insert(rest[1].to_string(), rest[2..].join(" "));
                }
                "output" => {
                    if rest.is_empty() {
                        return Err(SfcsError::InvalidProgram(format!(
                            "line {line_number}: output requires at least one node id"
                        )));
                    }
                    outputs.extend(rest.iter().map(|value| (*value).to_string()));
                }
                _ => {
                    return Err(SfcsError::InvalidProgram(format!(
                        "line {line_number}: unknown SFCS program directive {keyword}"
                    )));
                }
            }
        }

        if outputs.is_empty() {
            return Err(SfcsError::InvalidProgram(
                "program did not declare outputs".to_string(),
            ));
        }
        graph.outputs = outputs;
        graph.verify()?;
        Ok(graph)
    }

    /// Parses a deterministic expression source directly into an SFCS graph.
    ///
    /// This is the higher-level native SFCS frontend. It does not lower source
    /// into a circuit language. Each assignment becomes a committed fractal
    /// node, and nested expressions become deterministic intermediate nodes.
    ///
    /// Supported statements:
    ///
    /// - `input <id>`
    /// - `let <id> = <expr>`
    /// - `output <id> [id...]`
    /// - `output <expr> as <id>`
    ///
    /// Supported expressions:
    ///
    /// - integer constants
    /// - identifiers
    /// - `+`, `-`, `*`
    /// - `/`, `%`
    /// - `<`, `<=`, `>`, `>=`
    /// - `==`, `&&`, `||`, `!`
    /// - `&`, `|`, `^`, `<<`, `>>`
    /// - parentheses
    /// - `load(<address>)`, `store(<address>, <value>)`
    /// - `if <expr> then <expr> else <expr>`
    pub fn from_source(source: &str) -> Result<Self, SfcsError> {
        let mut builder = SfcsSourceBuilder::new();
        for (line_index, raw_line) in source.lines().enumerate() {
            let line_number = line_index + 1;
            let line = raw_line.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            if let Some(rest) = line.strip_prefix("input ") {
                let parts = rest.split_whitespace().collect::<Vec<_>>();
                require_program_arity(line_number, "input", &parts, 1)?;
                builder.insert_input(parts[0], line_number)?;
                continue;
            }
            if let Some(rest) = line.strip_prefix("let ") {
                let (id, expression) = rest.split_once('=').ok_or_else(|| {
                    SfcsError::InvalidProgram(format!("line {line_number}: let requires '='"))
                })?;
                let id = id.trim();
                if id.split_whitespace().count() != 1 {
                    return Err(SfcsError::InvalidProgram(format!(
                        "line {line_number}: let target must be one identifier"
                    )));
                }
                builder.insert_let(id, expression.trim(), line_number)?;
                continue;
            }
            if let Some(rest) = line.strip_prefix("output ") {
                builder.insert_output(rest.trim(), line_number)?;
                continue;
            }
            return Err(SfcsError::InvalidProgram(format!(
                "line {line_number}: unknown SFCS source statement"
            )));
        }
        builder.finish()
    }

    /// Strictly parses a graph from JSON bytes.
    ///
    /// This parser rejects duplicate object keys before serde decoding so
    /// canonical digest inputs cannot be ambiguous.
    pub fn from_slice(bytes: &[u8]) -> Result<Self, SfcsError> {
        let text = std::str::from_utf8(bytes)
            .map_err(|error| SfcsError::Canonical(format!("SFCS JSON is not UTF-8: {error}")))?;
        DuplicateKeyScanner::new(text).scan()?;
        let graph: Self = serde_json::from_str(text)?;
        graph.verify()?;
        Ok(graph)
    }

    /// Creates an empty draft SFCS graph.
    pub fn new(outputs: Vec<String>) -> Self {
        Self {
            schema: SFCS_SCHEMA_V1_DRAFT.to_string(),
            nodes: BTreeMap::new(),
            outputs,
        }
    }

    /// Inserts a node. Structural validation is performed by [`SfcsGraph::verify`].
    pub fn insert_node(&mut self, node: SfcsNode) -> Result<(), SfcsError> {
        validate_id(&node.id)?;
        if self.nodes.contains_key(&node.id) {
            return Err(SfcsError::DuplicateNode(node.id));
        }
        self.nodes.insert(node.id.clone(), node);
        Ok(())
    }

    /// Verifies deterministic structural invariants.
    pub fn verify(&self) -> Result<(), SfcsError> {
        if self.schema != SFCS_SCHEMA_V1_DRAFT {
            return Err(SfcsError::UnsupportedSchema(self.schema.clone()));
        }
        if self.nodes.is_empty() {
            return Err(SfcsError::InvalidGraph("graph has no nodes".to_string()));
        }
        if self.outputs.is_empty() {
            return Err(SfcsError::InvalidGraph("graph has no outputs".to_string()));
        }
        let mut seen_outputs = BTreeSet::new();
        for output in &self.outputs {
            validate_id(output)?;
            if !seen_outputs.insert(output) {
                return Err(SfcsError::InvalidGraph(format!(
                    "output {output} is repeated"
                )));
            }
            if !self.nodes.contains_key(output) {
                return Err(SfcsError::UnknownNode(output.clone()));
            }
        }
        for (key, node) in &self.nodes {
            if key != &node.id {
                return Err(SfcsError::InvalidGraph(format!(
                    "node key {key} does not match stored id {}",
                    node.id
                )));
            }
            validate_id(&node.id)?;
            let mut seen_inputs = BTreeSet::new();
            for input in &node.inputs {
                validate_id(input)?;
                if !seen_inputs.insert(input) {
                    return Err(SfcsError::InvalidGraph(format!(
                        "node {} repeats input {input}",
                        node.id
                    )));
                }
                if !self.nodes.contains_key(input) {
                    return Err(SfcsError::UnknownNode(input.clone()));
                }
            }
            validate_node_shape(node)?;
            if let Some(label) = &node.label {
                validate_metadata_value("label", label)?;
            }
            for (key, value) in &node.metadata {
                validate_id(key)?;
                validate_metadata_value(key, value)?;
            }
        }
        self.topological_order()?;
        Ok(())
    }

    /// Returns the domain-separated canonical digest of the fractal graph.
    pub fn fractal_digest(&self) -> Result<String, SfcsError> {
        self.verify()?;
        digest_json(FRACTAL_DIGEST_DOMAIN, self)
    }

    /// Returns canonical compact JSON bytes for this graph.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, SfcsError> {
        self.verify()?;
        serde_json::to_vec(self).map_err(SfcsError::Json)
    }

    /// Deterministically analyzes nodes eligible for the Sovereign fast path.
    pub fn discover_structure(&self) -> Result<SfcsDiscoveryReport, SfcsError> {
        self.verify()?;
        let mut fast_path_nodes = Vec::new();
        let mut dense_nodes = Vec::new();
        for node in self.nodes.values() {
            if node.is_fast_path_eligible() {
                fast_path_nodes.push(node.id.clone());
            } else {
                dense_nodes.push(node.id.clone());
            }
        }
        let graph_digest = self.fractal_digest()?;
        let workload = SfcsFastPathWorkload::new(graph_digest.clone(), fast_path_nodes.clone())?;
        let regions = self.structure_regions(&graph_digest)?;
        let fast_path_regions = regions
            .iter()
            .filter(|region| region.kind == SfcsRegionKind::FastPath)
            .count();
        let dense_regions = regions.len().saturating_sub(fast_path_regions);
        Ok(SfcsDiscoveryReport {
            graph_digest,
            node_count: self.nodes.len(),
            fast_path_nodes,
            dense_nodes,
            regions,
            fast_path_regions,
            dense_regions,
            fast_path_workload_digest: workload.workload_digest()?,
        })
    }

    /// Returns deterministic connected structure regions inside the fractal.
    pub fn structure_regions(
        &self,
        graph_digest: &str,
    ) -> Result<Vec<SfcsStructureRegion>, SfcsError> {
        self.verify()?;
        validate_sha256(graph_digest)?;
        let order = self.topological_order()?;
        let order_index = order
            .iter()
            .enumerate()
            .map(|(index, node_id)| (node_id.clone(), index))
            .collect::<BTreeMap<_, _>>();
        let mut consumers = BTreeMap::<String, Vec<String>>::new();
        for node in self.nodes.values() {
            for input in &node.inputs {
                consumers
                    .entry(input.clone())
                    .or_default()
                    .push(node.id.clone());
            }
        }
        for values in consumers.values_mut() {
            values.sort_by_key(|node_id| order_index[node_id]);
        }

        let mut assigned = BTreeSet::new();
        let mut raw_regions = Vec::<(usize, SfcsRegionKind, Vec<String>)>::new();
        for node_id in &order {
            if assigned.contains(node_id) {
                continue;
            }
            let kind = if self.nodes[node_id].is_fast_path_eligible() {
                SfcsRegionKind::FastPath
            } else {
                SfcsRegionKind::DenseBoundary
            };
            let mut stack = vec![node_id.clone()];
            let mut region_nodes = BTreeSet::new();
            while let Some(current) = stack.pop() {
                if !assigned.insert(current.clone()) {
                    continue;
                }
                region_nodes.insert(current.clone());
                let current_node = &self.nodes[&current];
                for neighbor in current_node
                    .inputs
                    .iter()
                    .chain(consumers.get(&current).into_iter().flatten())
                {
                    if assigned.contains(neighbor) {
                        continue;
                    }
                    let neighbor_kind = if self.nodes[neighbor].is_fast_path_eligible() {
                        SfcsRegionKind::FastPath
                    } else {
                        SfcsRegionKind::DenseBoundary
                    };
                    if neighbor_kind == kind {
                        stack.push(neighbor.clone());
                    }
                }
            }
            let mut node_ids = region_nodes.into_iter().collect::<Vec<_>>();
            node_ids.sort_by_key(|node_id| order_index[node_id]);
            let completion = node_ids
                .iter()
                .map(|node_id| order_index[node_id])
                .max()
                .unwrap_or(0);
            raw_regions.push((completion, kind, node_ids));
        }
        raw_regions.sort_by_key(|(start, kind, node_ids)| (*start, kind.clone(), node_ids.clone()));

        let mut regions = Vec::new();
        for (index, (_, kind, node_ids)) in raw_regions.into_iter().enumerate() {
            let node_set = node_ids.iter().cloned().collect::<BTreeSet<_>>();
            let mut entry_nodes = BTreeSet::new();
            let mut output_nodes = BTreeSet::new();
            for node_id in &node_ids {
                let node = &self.nodes[node_id];
                if node.inputs.iter().any(|input| !node_set.contains(input)) {
                    entry_nodes.insert(node_id.clone());
                }
                let leaves_region = consumers
                    .get(node_id)
                    .into_iter()
                    .flatten()
                    .any(|consumer| !node_set.contains(consumer));
                if leaves_region || self.outputs.contains(node_id) {
                    output_nodes.insert(node_id.clone());
                }
            }
            let mut region = SfcsStructureRegion {
                region_id: format!(
                    "region_{index:04}_{}",
                    match kind {
                        SfcsRegionKind::FastPath => "fast_path",
                        SfcsRegionKind::DenseBoundary => "dense_boundary",
                    }
                ),
                kind,
                node_ids,
                entry_nodes: entry_nodes.into_iter().collect(),
                output_nodes: output_nodes.into_iter().collect(),
                graph_digest: graph_digest.to_string(),
                region_digest: String::new(),
            };
            region.region_digest = digest_json(STRUCTURE_REGION_DOMAIN, &region.preimage())?;
            regions.push(region);
        }
        Ok(regions)
    }

    /// Evaluates the executable source-to-fractal subset deterministically.
    ///
    /// Explicit opaque dense steps are rejected rather than silently accepted
    /// until a proof profile defines their semantics.
    pub fn evaluate(
        &self,
        inputs: &BTreeMap<String, i64>,
    ) -> Result<BTreeMap<String, i64>, SfcsError> {
        Ok(self.execution_trace(inputs)?.outputs)
    }

    /// Executes the source-to-fractal subset and returns a deterministic trace.
    ///
    /// The trace is a first-class digestible object. It records the node order,
    /// operation, inputs, outputs, and per-step digests so replay can distinguish
    /// graph truth from later display or semantic layers.
    pub fn execution_trace(
        &self,
        inputs: &BTreeMap<String, i64>,
    ) -> Result<SfcsExecutionTrace, SfcsError> {
        self.verify()?;
        let graph_digest = self.fractal_digest()?;
        let input_digest = digest_json(TRACE_INPUT_DOMAIN, inputs)?;
        let mut memory = BTreeMap::<i64, i64>::new();
        let mut values = BTreeMap::new();
        let mut steps = Vec::new();
        for node_id in self.topological_order()? {
            let node = &self.nodes[&node_id];
            let input_values = node
                .inputs
                .iter()
                .map(|input| (input.clone(), values[input]))
                .collect::<BTreeMap<_, _>>();
            let memory_before_digest = digest_json(TRACE_MEMORY_DOMAIN, &memory)?;
            let value = match node.op {
                SfcsOp::Input => *inputs
                    .get(&node.id)
                    .ok_or_else(|| SfcsError::MissingInput(node.id.clone()))?,
                SfcsOp::Alias => values[&node.inputs[0]],
                SfcsOp::Const => *node.params.get("value").ok_or_else(|| {
                    SfcsError::InvalidGraph(format!("const {} missing value", node.id))
                })?,
                SfcsOp::Add => node.inputs.iter().try_fold(0_i64, |acc, input| {
                    Ok::<i64, SfcsError>(acc.wrapping_add(values[input]))
                })?,
                SfcsOp::Sub => values[&node.inputs[0]].wrapping_sub(values[&node.inputs[1]]),
                SfcsOp::Mul => node.inputs.iter().try_fold(1_i64, |acc, input| {
                    Ok::<i64, SfcsError>(acc.wrapping_mul(values[input]))
                })?,
                SfcsOp::Div => {
                    let divisor = values[&node.inputs[1]];
                    if divisor == 0 {
                        return Err(SfcsError::Execution(format!(
                            "node {} attempted division by zero",
                            node.id
                        )));
                    }
                    values[&node.inputs[0]].wrapping_div(divisor)
                }
                SfcsOp::Mod => {
                    let divisor = values[&node.inputs[1]];
                    if divisor == 0 {
                        return Err(SfcsError::Execution(format!(
                            "node {} attempted remainder by zero",
                            node.id
                        )));
                    }
                    values[&node.inputs[0]].wrapping_rem(divisor)
                }
                SfcsOp::Eq => i64::from(values[&node.inputs[0]] == values[&node.inputs[1]]),
                SfcsOp::Lt => i64::from(values[&node.inputs[0]] < values[&node.inputs[1]]),
                SfcsOp::Le => i64::from(values[&node.inputs[0]] <= values[&node.inputs[1]]),
                SfcsOp::Gt => i64::from(values[&node.inputs[0]] > values[&node.inputs[1]]),
                SfcsOp::Ge => i64::from(values[&node.inputs[0]] >= values[&node.inputs[1]]),
                SfcsOp::And => {
                    i64::from(values[&node.inputs[0]] != 0 && values[&node.inputs[1]] != 0)
                }
                SfcsOp::Or => {
                    i64::from(values[&node.inputs[0]] != 0 || values[&node.inputs[1]] != 0)
                }
                SfcsOp::Not => i64::from(values[&node.inputs[0]] == 0),
                SfcsOp::BitAnd => values[&node.inputs[0]] & values[&node.inputs[1]],
                SfcsOp::BitOr => values[&node.inputs[0]] | values[&node.inputs[1]],
                SfcsOp::BitXor => values[&node.inputs[0]] ^ values[&node.inputs[1]],
                SfcsOp::Shl => {
                    let shift = checked_shift_amount(values[&node.inputs[1]], &node.id)?;
                    values[&node.inputs[0]].wrapping_shl(shift)
                }
                SfcsOp::Shr => {
                    let shift = checked_shift_amount(values[&node.inputs[1]], &node.id)?;
                    values[&node.inputs[0]].wrapping_shr(shift)
                }
                SfcsOp::Branch => {
                    let condition = values[&node.inputs[0]];
                    if condition != 0 {
                        values[&node.inputs[1]]
                    } else {
                        values[&node.inputs[2]]
                    }
                }
                SfcsOp::MemoryRead => {
                    let address = values[&node.inputs[0]];
                    *memory.get(&address).unwrap_or(&0)
                }
                SfcsOp::MemoryWrite => {
                    let address = values[&node.inputs[0]];
                    let stored = values[&node.inputs[1]];
                    memory.insert(address, stored);
                    stored
                }
                SfcsOp::FastPathClaim | SfcsOp::DenseStep => {
                    return Err(SfcsError::UnsupportedEvaluation(node.id.clone()));
                }
            };
            let memory_after_digest = digest_json(TRACE_MEMORY_DOMAIN, &memory)?;
            let mut step = SfcsTraceStep {
                step_index: steps.len() as u64,
                node_id: node.id.clone(),
                op: node.op.clone(),
                input_nodes: node.inputs.clone(),
                input_values,
                output_value: value,
                fast_path_eligible: node.is_fast_path_eligible(),
                memory_before_digest,
                memory_after_digest,
                step_digest: String::new(),
            };
            step.step_digest = digest_json(TRACE_STEP_DOMAIN, &step.preimage())?;
            values.insert(node.id.clone(), value);
            steps.push(step);
        }
        let outputs = self
            .outputs
            .iter()
            .map(|id| Ok((id.clone(), values[id])))
            .collect::<Result<BTreeMap<_, _>, SfcsError>>()?;
        let output_digest = digest_json(TRACE_OUTPUT_DOMAIN, &outputs)?;
        let mut trace = SfcsExecutionTrace {
            graph_digest,
            input_digest,
            output_digest,
            trace_digest: String::new(),
            steps,
            outputs,
        };
        trace.trace_digest = digest_json(TRACE_DOMAIN, &trace.preimage())?;
        Ok(trace)
    }

    /// Creates a deterministic synthesis plan for fast-path extraction.
    ///
    /// The plan records where structured sub-fractals can be routed to the
    /// Sovereign fast path and where dense/general boundaries remain. It does
    /// not mutate Rootprint v1 or `.pha` core rules.
    pub fn synthesis_plan(&self) -> Result<SfcsSynthesisPlan, SfcsError> {
        let discovery = self.discover_structure()?;
        let graph_digest = discovery.graph_digest.clone();
        let mut operations = Vec::new();
        for region in &discovery.regions {
            let kind = match region.kind {
                SfcsRegionKind::FastPath => SfcsRewriteKind::FastPathExtract,
                SfcsRegionKind::DenseBoundary => SfcsRewriteKind::DenseBoundary,
            };
            operations.push(SfcsRewriteOperation::new(
                operations.len() as u64,
                kind,
                region.node_ids.clone(),
                graph_digest.clone(),
                region.region_digest.clone(),
            )?);
        }
        let operation_digests = operations
            .iter()
            .map(|operation| operation.operation_digest.clone())
            .collect::<Vec<_>>();
        let fast_path_workload_digest = discovery.fast_path_workload_digest.clone();
        let dense_nodes = discovery.dense_nodes.clone();
        let mut plan = SfcsSynthesisPlan {
            graph_digest: graph_digest.clone(),
            synthesis_digest: String::new(),
            embedding_invariant_digest: String::new(),
            operations,
            operation_digests,
            regions: discovery.regions.clone(),
            fast_path_workload_digest,
            dense_nodes,
            fast_path_regions: discovery.fast_path_regions,
            dense_regions: discovery.dense_regions,
        };
        plan.synthesis_digest = digest_json(SYNTHESIS_PLAN_DOMAIN, &plan.preimage())?;
        plan.embedding_invariant_digest = digest_json(
            EMBEDDING_INVARIANT_DOMAIN,
            &serde_json::json!({
                "graph_digest": graph_digest,
                "synthesis_digest": plan.synthesis_digest,
            }),
        )?;
        Ok(plan)
    }

    /// Commits the SFCS graph as ordinary `.pha` core data.
    ///
    /// This is the safe bridge to current Power House identity: the resulting
    /// artifact can be anchored by Rootprint without changing Rootprint v1.
    pub fn to_pha_artifact(&self, label: impl Into<String>) -> Result<PhaArtifact, SfcsError> {
        let report = self.discover_structure()?;
        PhaArtifact::new(
            serde_json::json!({
                "producer": "power_house_sfcs",
                "label": label.into(),
                "fractal_digest": report.graph_digest,
                "schema": self.schema,
            }),
            "power-house/sfcs/v1-draft",
            serde_json::json!({
                "outputs": self.outputs,
                "node_count": report.node_count,
                "fast_path_nodes": report.fast_path_nodes.len(),
                "dense_nodes": report.dense_nodes.len(),
                "structure_regions": report.regions.len(),
                "fast_path_regions": report.fast_path_regions,
                "dense_regions": report.dense_regions,
            }),
            serde_json::to_value(self)?,
        )
        .map_err(SfcsError::Pha)
    }

    /// Commits graph, synthesis plan, and execution trace as `.pha` core data.
    ///
    /// This is the draft "program + trace + synthesis" artifact. It remains
    /// ordinary Power House core data and can be anchored by Rootprint without
    /// adding new Rootprint rules.
    pub fn to_execution_pha_artifact(
        &self,
        label: impl Into<String>,
        inputs: &BTreeMap<String, i64>,
    ) -> Result<PhaArtifact, SfcsError> {
        let trace = self.execution_trace(inputs)?;
        let synthesis = self.synthesis_plan()?;
        PhaArtifact::new(
            serde_json::json!({
                "producer": "power_house_sfcs",
                "label": label.into(),
                "fractal_digest": trace.graph_digest,
                "trace_digest": trace.trace_digest,
                "synthesis_digest": synthesis.synthesis_digest,
                "embedding_invariant_digest": synthesis.embedding_invariant_digest,
                "schema": self.schema,
            }),
            "power-house/sfcs-execution/v1-draft",
            serde_json::json!({
                "inputs": inputs,
                "outputs": trace.outputs,
                "node_count": self.nodes.len(),
                "trace_steps": trace.steps.len(),
                "synthesis_operations": synthesis.operations.len(),
                "dense_nodes": synthesis.dense_nodes.len(),
                "structure_regions": synthesis.regions.len(),
                "fast_path_regions": synthesis.fast_path_regions,
                "dense_regions": synthesis.dense_regions,
                "fast_path_workload_digest": synthesis.fast_path_workload_digest,
            }),
            serde_json::json!({
                "graph": self,
                "trace": trace,
                "synthesis": synthesis,
            }),
        )
        .map_err(SfcsError::Pha)
    }

    fn topological_order(&self) -> Result<Vec<String>, SfcsError> {
        let mut temporary = BTreeSet::new();
        let mut permanent = BTreeSet::new();
        let mut order = Vec::new();
        for node_id in self.nodes.keys() {
            visit_node(node_id, self, &mut temporary, &mut permanent, &mut order)?;
        }
        Ok(order)
    }
}

/// Verifies that a `.pha` artifact carries a valid SFCS draft graph.
pub fn verify_pha_embedding(artifact: &PhaArtifact) -> Result<SfcsEmbeddingReport, SfcsError> {
    artifact.verify().map_err(SfcsError::Pha)?;
    if artifact.embedded_proof.protocol != "power-house/sfcs/v1-draft" {
        return Err(SfcsError::InvalidEmbedding(
            "embedded proof protocol is not SFCS".to_string(),
        ));
    }
    let graph: SfcsGraph = serde_json::from_value(artifact.embedded_proof.proof.clone())?;
    graph.verify()?;
    let discovery = graph.discover_structure()?;
    let provenance_digest = artifact
        .provenance
        .get("fractal_digest")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            SfcsError::InvalidEmbedding("missing provenance fractal_digest".to_string())
        })?;
    if provenance_digest != discovery.graph_digest {
        return Err(SfcsError::InvalidEmbedding(
            "provenance fractal_digest does not match graph".to_string(),
        ));
    }
    let public_inputs = &artifact.embedded_proof.public_inputs;
    let expected_node_count = public_inputs
        .get("node_count")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| SfcsError::InvalidEmbedding("missing public node_count".to_string()))?;
    let expected_fast = public_inputs
        .get("fast_path_nodes")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| SfcsError::InvalidEmbedding("missing public fast_path_nodes".to_string()))?;
    let expected_dense = public_inputs
        .get("dense_nodes")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| SfcsError::InvalidEmbedding("missing public dense_nodes".to_string()))?;
    let expected_regions = public_inputs
        .get("structure_regions")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| {
            SfcsError::InvalidEmbedding("missing public structure_regions".to_string())
        })?;
    let expected_fast_regions = public_inputs
        .get("fast_path_regions")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| {
            SfcsError::InvalidEmbedding("missing public fast_path_regions".to_string())
        })?;
    let expected_dense_regions = public_inputs
        .get("dense_regions")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| SfcsError::InvalidEmbedding("missing public dense_regions".to_string()))?;
    if expected_node_count != discovery.node_count as u64
        || expected_fast != discovery.fast_path_nodes.len() as u64
        || expected_dense != discovery.dense_nodes.len() as u64
        || expected_regions != discovery.regions.len() as u64
        || expected_fast_regions != discovery.fast_path_regions as u64
        || expected_dense_regions != discovery.dense_regions as u64
    {
        return Err(SfcsError::InvalidEmbedding(
            "public SFCS counters do not match graph discovery".to_string(),
        ));
    }
    Ok(SfcsEmbeddingReport {
        graph_digest: discovery.graph_digest,
        artifact_phx_fingerprint: artifact.phx_fingerprint.clone(),
        node_count: discovery.node_count,
        fast_path_nodes: discovery.fast_path_nodes.len(),
        dense_nodes: discovery.dense_nodes.len(),
        structure_regions: discovery.regions.len(),
        fast_path_regions: discovery.fast_path_regions,
        dense_regions: discovery.dense_regions,
        fast_path_workload_digest: discovery.fast_path_workload_digest,
    })
}

/// Verifies a `.pha` artifact that carries an SFCS execution trace and synthesis plan.
pub fn verify_execution_embedding(
    artifact: &PhaArtifact,
) -> Result<SfcsExecutionEmbeddingReport, SfcsError> {
    artifact.verify().map_err(SfcsError::Pha)?;
    if artifact.embedded_proof.protocol != "power-house/sfcs-execution/v1-draft" {
        return Err(SfcsError::InvalidEmbedding(
            "embedded proof protocol is not SFCS execution".to_string(),
        ));
    }
    let proof: SfcsExecutionProof = serde_json::from_value(artifact.embedded_proof.proof.clone())?;
    proof.graph.verify()?;
    let inputs = artifact
        .embedded_proof
        .public_inputs
        .get("inputs")
        .ok_or_else(|| SfcsError::InvalidEmbedding("missing execution inputs".to_string()))?;
    let inputs = serde_json::from_value::<BTreeMap<String, i64>>(inputs.clone())?;
    let expected_trace = proof.graph.execution_trace(&inputs)?;
    let expected_synthesis = proof.graph.synthesis_plan()?;
    if proof.trace != expected_trace {
        return Err(SfcsError::InvalidEmbedding(
            "execution trace does not replay from graph and inputs".to_string(),
        ));
    }
    if proof.synthesis != expected_synthesis {
        return Err(SfcsError::InvalidEmbedding(
            "synthesis plan does not replay from graph".to_string(),
        ));
    }
    let provenance = &artifact.provenance;
    for (field, expected) in [
        ("fractal_digest", &expected_trace.graph_digest),
        ("trace_digest", &expected_trace.trace_digest),
        ("synthesis_digest", &expected_synthesis.synthesis_digest),
        (
            "embedding_invariant_digest",
            &expected_synthesis.embedding_invariant_digest,
        ),
    ] {
        let found = provenance
            .get(field)
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| SfcsError::InvalidEmbedding(format!("missing provenance {field}")))?;
        if found != expected {
            return Err(SfcsError::InvalidEmbedding(format!(
                "provenance {field} does not match replay"
            )));
        }
    }
    let public_inputs = &artifact.embedded_proof.public_inputs;
    let expected_outputs = public_inputs
        .get("outputs")
        .ok_or_else(|| SfcsError::InvalidEmbedding("missing public outputs".to_string()))?;
    if expected_outputs != &serde_json::to_value(&expected_trace.outputs)? {
        return Err(SfcsError::InvalidEmbedding(
            "public outputs do not match trace".to_string(),
        ));
    }
    for (field, expected) in [
        ("node_count", proof.graph.nodes.len() as u64),
        ("trace_steps", expected_trace.steps.len() as u64),
        (
            "synthesis_operations",
            expected_synthesis.operations.len() as u64,
        ),
        ("dense_nodes", expected_synthesis.dense_nodes.len() as u64),
        ("structure_regions", expected_synthesis.regions.len() as u64),
        (
            "fast_path_regions",
            expected_synthesis.fast_path_regions as u64,
        ),
        ("dense_regions", expected_synthesis.dense_regions as u64),
    ] {
        let found = public_inputs
            .get(field)
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| SfcsError::InvalidEmbedding(format!("missing public {field}")))?;
        if found != expected {
            return Err(SfcsError::InvalidEmbedding(format!(
                "public {field} does not match replay"
            )));
        }
    }
    Ok(SfcsExecutionEmbeddingReport {
        graph_digest: expected_trace.graph_digest,
        artifact_phx_fingerprint: artifact.phx_fingerprint.clone(),
        trace_digest: expected_trace.trace_digest,
        synthesis_digest: expected_synthesis.synthesis_digest,
        embedding_invariant_digest: expected_synthesis.embedding_invariant_digest,
        output_digest: expected_trace.output_digest,
        node_count: proof.graph.nodes.len(),
        trace_steps: expected_trace.steps.len(),
        synthesis_operations: expected_synthesis.operations.len(),
        dense_nodes: expected_synthesis.dense_nodes.len(),
        structure_regions: expected_synthesis.regions.len(),
        fast_path_regions: expected_synthesis.fast_path_regions,
        dense_regions: expected_synthesis.dense_regions,
    })
}

/// Verified SFCS `.pha` embedding summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcsEmbeddingReport {
    /// Digest of the embedded SFCS graph.
    pub graph_digest: String,
    /// Core `.pha` fingerprint that commits to the graph payload.
    pub artifact_phx_fingerprint: String,
    /// Total graph nodes.
    pub node_count: usize,
    /// Fast-path eligible node count.
    pub fast_path_nodes: usize,
    /// Dense/general node count.
    pub dense_nodes: usize,
    /// Total deterministic structure regions.
    pub structure_regions: usize,
    /// Fast-path structure region count.
    pub fast_path_regions: usize,
    /// Dense/general structure region count.
    pub dense_regions: usize,
    /// Digest of the extracted fast-path workload descriptor.
    pub fast_path_workload_digest: String,
}

/// Verified SFCS execution `.pha` embedding summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcsExecutionEmbeddingReport {
    /// Digest of the embedded SFCS graph.
    pub graph_digest: String,
    /// Core `.pha` fingerprint that commits to graph, trace, and synthesis plan.
    pub artifact_phx_fingerprint: String,
    /// Digest of the replayed execution trace.
    pub trace_digest: String,
    /// Digest of the deterministic synthesis plan.
    pub synthesis_digest: String,
    /// Digest binding graph identity to synthesis identity.
    pub embedding_invariant_digest: String,
    /// Digest of public outputs.
    pub output_digest: String,
    /// Total graph nodes.
    pub node_count: usize,
    /// Total trace steps.
    pub trace_steps: usize,
    /// Total synthesis operations.
    pub synthesis_operations: usize,
    /// Dense/general node count.
    pub dense_nodes: usize,
    /// Total deterministic structure regions.
    pub structure_regions: usize,
    /// Fast-path structure region count.
    pub fast_path_regions: usize,
    /// Dense/general structure region count.
    pub dense_regions: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct SfcsExecutionProof {
    graph: SfcsGraph,
    trace: SfcsExecutionTrace,
    synthesis: SfcsSynthesisPlan,
}

/// Deterministic structure-discovery report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcsDiscoveryReport {
    /// Digest of the analyzed graph.
    pub graph_digest: String,
    /// Total nodes.
    pub node_count: usize,
    /// Nodes eligible for the structured fast path.
    pub fast_path_nodes: Vec<String>,
    /// Nodes requiring dense/general handling.
    pub dense_nodes: Vec<String>,
    /// Deterministically discovered connected structure regions.
    pub regions: Vec<SfcsStructureRegion>,
    /// Count of fast-path regions.
    pub fast_path_regions: usize,
    /// Count of dense/general regions.
    pub dense_regions: usize,
    /// Digest of the extracted fast-path workload descriptor.
    pub fast_path_workload_digest: String,
}

/// Kind of deterministic SFCS structure region.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SfcsRegionKind {
    /// Connected structured arithmetic region eligible for the Sovereign fast path.
    FastPath,
    /// Connected dense/control/memory boundary that remains outside the fast path.
    DenseBoundary,
}

/// Connected sub-fractal discovered during deterministic structure analysis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcsStructureRegion {
    /// Stable region identifier assigned by deterministic topological order.
    pub region_id: String,
    /// Region kind.
    pub kind: SfcsRegionKind,
    /// Nodes contained in the region in topological order.
    pub node_ids: Vec<String>,
    /// Region nodes with at least one dependency outside the region.
    pub entry_nodes: Vec<String>,
    /// Region nodes consumed outside the region or exported as graph outputs.
    pub output_nodes: Vec<String>,
    /// Source graph digest before extraction.
    pub graph_digest: String,
    /// Domain-separated digest of the region preimage.
    pub region_digest: String,
}

impl SfcsStructureRegion {
    fn preimage(&self) -> serde_json::Value {
        serde_json::json!({
            "region_id": self.region_id,
            "kind": self.kind,
            "node_ids": self.node_ids,
            "entry_nodes": self.entry_nodes,
            "output_nodes": self.output_nodes,
            "graph_digest": self.graph_digest,
        })
    }
}

/// Deterministic execution trace for the source-to-fractal SFCS subset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcsExecutionTrace {
    /// Digest of the source graph.
    pub graph_digest: String,
    /// Digest of the public execution inputs.
    pub input_digest: String,
    /// Digest of the public execution outputs.
    pub output_digest: String,
    /// Digest of the full trace preimage.
    pub trace_digest: String,
    /// Deterministic node execution steps.
    pub steps: Vec<SfcsTraceStep>,
    /// Public output values.
    pub outputs: BTreeMap<String, i64>,
}

impl SfcsExecutionTrace {
    fn preimage(&self) -> serde_json::Value {
        serde_json::json!({
            "graph_digest": self.graph_digest,
            "input_digest": self.input_digest,
            "output_digest": self.output_digest,
            "steps": self.steps,
            "outputs": self.outputs,
        })
    }
}

/// One deterministic execution trace step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcsTraceStep {
    /// Zero-based execution index.
    pub step_index: u64,
    /// Executed node ID.
    pub node_id: String,
    /// Executed operation.
    pub op: SfcsOp,
    /// Ordered input node IDs.
    pub input_nodes: Vec<String>,
    /// Input values observed at this step.
    pub input_values: BTreeMap<String, i64>,
    /// Output value produced by this step.
    pub output_value: i64,
    /// Whether this node is eligible for the Sovereign fast path.
    pub fast_path_eligible: bool,
    /// Digest of deterministic memory state before this step.
    pub memory_before_digest: String,
    /// Digest of deterministic memory state after this step.
    pub memory_after_digest: String,
    /// Domain-separated digest of this step.
    pub step_digest: String,
}

impl SfcsTraceStep {
    fn preimage(&self) -> serde_json::Value {
        serde_json::json!({
            "step_index": self.step_index,
            "node_id": self.node_id,
            "op": self.op,
            "input_nodes": self.input_nodes,
            "input_values": self.input_values,
            "output_value": self.output_value,
            "fast_path_eligible": self.fast_path_eligible,
            "memory_before_digest": self.memory_before_digest,
            "memory_after_digest": self.memory_after_digest,
        })
    }
}

/// Deterministic synthesis plan that records fast-path and dense boundaries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcsSynthesisPlan {
    /// Digest of the source graph.
    pub graph_digest: String,
    /// Digest of this synthesis plan.
    pub synthesis_digest: String,
    /// Digest binding graph and synthesis identities.
    pub embedding_invariant_digest: String,
    /// Ordered rewrite/extraction operations.
    pub operations: Vec<SfcsRewriteOperation>,
    /// Ordered operation digests.
    pub operation_digests: Vec<String>,
    /// Structure regions used to create the synthesis operations.
    pub regions: Vec<SfcsStructureRegion>,
    /// Digest of the complete fast-path workload descriptor.
    pub fast_path_workload_digest: String,
    /// Dense/general nodes left outside the fast path.
    pub dense_nodes: Vec<String>,
    /// Number of fast-path regions.
    pub fast_path_regions: usize,
    /// Number of dense/general regions.
    pub dense_regions: usize,
}

impl SfcsSynthesisPlan {
    fn preimage(&self) -> serde_json::Value {
        serde_json::json!({
            "graph_digest": self.graph_digest,
            "operations": self.operations,
            "operation_digests": self.operation_digests,
            "regions": self.regions,
            "fast_path_workload_digest": self.fast_path_workload_digest,
            "dense_nodes": self.dense_nodes,
            "fast_path_regions": self.fast_path_regions,
            "dense_regions": self.dense_regions,
        })
    }
}

/// Kind of deterministic SFCS synthesis operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SfcsRewriteKind {
    /// Extract a contiguous structured sub-fractal for the Sovereign fast path.
    FastPathExtract,
    /// Record a dense/general computation boundary.
    DenseBoundary,
}

/// One deterministic synthesis operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcsRewriteOperation {
    /// Zero-based operation index.
    pub index: u64,
    /// Operation kind.
    pub kind: SfcsRewriteKind,
    /// Nodes covered by the operation in deterministic order.
    pub node_ids: Vec<String>,
    /// Source graph digest before the operation.
    pub graph_digest: String,
    /// Optional workload digest for fast-path extraction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workload_digest: Option<String>,
    /// Digest of the structure region that produced this operation.
    pub region_digest: String,
    /// Domain-separated digest of the operation.
    pub operation_digest: String,
}

impl SfcsRewriteOperation {
    fn new(
        index: u64,
        kind: SfcsRewriteKind,
        node_ids: Vec<String>,
        graph_digest: String,
        region_digest: String,
    ) -> Result<Self, SfcsError> {
        validate_sha256(&graph_digest)?;
        validate_sha256(&region_digest)?;
        if node_ids.is_empty() {
            return Err(SfcsError::InvalidGraph(
                "synthesis operation cannot be empty".to_string(),
            ));
        }
        for node_id in &node_ids {
            validate_id(node_id)?;
        }
        let workload_digest = if kind == SfcsRewriteKind::FastPathExtract {
            Some(
                SfcsFastPathWorkload::new(graph_digest.clone(), node_ids.clone())?
                    .workload_digest()?,
            )
        } else {
            None
        };
        let mut operation = Self {
            index,
            kind,
            node_ids,
            graph_digest,
            workload_digest,
            region_digest,
            operation_digest: String::new(),
        };
        operation.operation_digest =
            digest_json(SYNTHESIS_OPERATION_DOMAIN, &operation.preimage())?;
        Ok(operation)
    }

    fn preimage(&self) -> serde_json::Value {
        serde_json::json!({
            "index": self.index,
            "kind": self.kind,
            "node_ids": self.node_ids,
            "graph_digest": self.graph_digest,
            "workload_digest": self.workload_digest,
            "region_digest": self.region_digest,
        })
    }
}

/// Descriptor for the privileged structured proving path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcsFastPathWorkload {
    /// Source fractal graph digest.
    pub graph_digest: String,
    /// Canonically sorted node IDs included in the workload.
    pub node_ids: Vec<String>,
    /// Draft strategy label.
    pub strategy: String,
}

impl SfcsFastPathWorkload {
    /// Creates a deterministic fast-path workload descriptor.
    pub fn new(graph_digest: String, mut node_ids: Vec<String>) -> Result<Self, SfcsError> {
        validate_sha256(&graph_digest)?;
        node_ids.sort();
        node_ids.dedup();
        for node_id in &node_ids {
            validate_id(node_id)?;
        }
        Ok(Self {
            graph_digest,
            node_ids,
            strategy: "structured-arithmetic-draft".to_string(),
        })
    }

    /// Returns the domain-separated workload digest.
    pub fn workload_digest(&self) -> Result<String, SfcsError> {
        digest_json(FAST_PATH_DOMAIN, self)
    }
}

/// Interface for future structured SFCS proof engines.
pub trait SovereignFastPath {
    /// Engine-specific error type.
    type Error;

    /// Proves an extracted structured workload.
    fn prove_workload(
        &self,
        workload: &SfcsFastPathWorkload,
    ) -> Result<SfcsFastPathCertificate, Self::Error>;
}

/// Opaque certificate returned by a future fast-path prover.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SfcsFastPathCertificate {
    /// Digest of the workload this certificate answers.
    pub workload_digest: String,
    /// Certificate schema or proof profile.
    pub profile: String,
    /// Opaque deterministic proof bytes or encoded proof payload.
    pub payload_sha256: String,
}

/// Errors returned by SFCS draft operations.
#[derive(Debug)]
pub enum SfcsError {
    /// Unsupported SFCS schema.
    UnsupportedSchema(String),
    /// Node identifier is malformed.
    InvalidId(String),
    /// Duplicate node ID.
    DuplicateNode(String),
    /// Referenced node is absent.
    UnknownNode(String),
    /// Graph shape is invalid.
    InvalidGraph(String),
    /// Textual SFCS program is invalid.
    InvalidProgram(String),
    /// A graph cycle was found.
    CycleDetected(String),
    /// Canonical JSON validation failed.
    Canonical(String),
    /// A `.pha` SFCS embedding is structurally inconsistent.
    InvalidEmbedding(String),
    /// Required input value is missing.
    MissingInput(String),
    /// Node cannot be evaluated by the draft arithmetic evaluator.
    UnsupportedEvaluation(String),
    /// Deterministic execution failed.
    Execution(String),
    /// Digest is malformed.
    InvalidDigest(String),
    /// JSON serialization failed.
    Json(serde_json::Error),
    /// PHA construction failed.
    Pha(PhaError),
}

impl fmt::Display for SfcsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedSchema(schema) => {
                write!(formatter, "unsupported SFCS schema: {schema}")
            }
            Self::InvalidId(id) => write!(formatter, "invalid SFCS node id: {id}"),
            Self::DuplicateNode(id) => write!(formatter, "duplicate SFCS node: {id}"),
            Self::UnknownNode(id) => write!(formatter, "unknown SFCS node: {id}"),
            Self::InvalidGraph(message) => write!(formatter, "invalid SFCS graph: {message}"),
            Self::InvalidProgram(message) => write!(formatter, "invalid SFCS program: {message}"),
            Self::CycleDetected(id) => write!(formatter, "SFCS graph contains a cycle at {id}"),
            Self::Canonical(message) => write!(formatter, "SFCS canonical JSON error: {message}"),
            Self::InvalidEmbedding(message) => {
                write!(formatter, "invalid SFCS embedding: {message}")
            }
            Self::MissingInput(id) => write!(formatter, "missing SFCS input: {id}"),
            Self::UnsupportedEvaluation(id) => {
                write!(
                    formatter,
                    "SFCS node {id} is not executable by the draft evaluator"
                )
            }
            Self::Execution(message) => write!(formatter, "SFCS execution error: {message}"),
            Self::InvalidDigest(value) => write!(formatter, "invalid digest: {value}"),
            Self::Json(error) => write!(formatter, "SFCS JSON error: {error}"),
            Self::Pha(error) => write!(formatter, "SFCS PHA error: {error}"),
        }
    }
}

impl Error for SfcsError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Json(error) => Some(error),
            Self::Pha(error) => Some(error),
            _ => None,
        }
    }
}

impl From<serde_json::Error> for SfcsError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

struct SfcsSourceBuilder {
    graph: SfcsGraph,
    bindings: BTreeMap<String, String>,
    outputs: Vec<String>,
    temp_counter: u64,
    memory_head: Option<String>,
}

impl SfcsSourceBuilder {
    fn new() -> Self {
        Self {
            graph: SfcsGraph::new(Vec::new()),
            bindings: BTreeMap::new(),
            outputs: Vec::new(),
            temp_counter: 0,
            memory_head: None,
        }
    }

    fn insert_input(&mut self, id: &str, line_number: usize) -> Result<(), SfcsError> {
        self.reserve_binding(id, line_number)?;
        self.graph
            .insert_node(SfcsNode::new(id, SfcsOp::Input, Vec::new()))?;
        self.bindings.insert(id.to_string(), id.to_string());
        Ok(())
    }

    fn insert_let(
        &mut self,
        id: &str,
        expression: &str,
        line_number: usize,
    ) -> Result<(), SfcsError> {
        self.reserve_binding(id, line_number)?;
        let expression = SfcsExprParser::parse(expression, line_number)?;
        let node_id = self.lower_expr(&expression, Some(id.to_string()), line_number)?;
        self.bindings.insert(id.to_string(), node_id);
        Ok(())
    }

    fn insert_output(&mut self, body: &str, line_number: usize) -> Result<(), SfcsError> {
        if body.is_empty() {
            return Err(SfcsError::InvalidProgram(format!(
                "line {line_number}: output requires at least one value"
            )));
        }
        if let Some((expression, id)) = split_output_as(body) {
            let id = id.trim();
            self.reserve_binding(id, line_number)?;
            let expression = SfcsExprParser::parse(expression.trim(), line_number)?;
            let node_id = self.lower_expr(&expression, Some(id.to_string()), line_number)?;
            self.bindings.insert(id.to_string(), node_id.clone());
            self.outputs.push(node_id);
            return Ok(());
        }
        for value in body.split_whitespace() {
            let node_id = self.resolve(value, line_number)?;
            self.outputs.push(node_id);
        }
        Ok(())
    }

    fn finish(mut self) -> Result<SfcsGraph, SfcsError> {
        if self.outputs.is_empty() {
            return Err(SfcsError::InvalidProgram(
                "source did not declare outputs".to_string(),
            ));
        }
        self.graph.outputs = self.outputs;
        self.graph.verify()?;
        Ok(self.graph)
    }

    fn reserve_binding(&self, id: &str, line_number: usize) -> Result<(), SfcsError> {
        validate_id(id)?;
        if self.bindings.contains_key(id) || self.graph.nodes.contains_key(id) {
            return Err(SfcsError::InvalidProgram(format!(
                "line {line_number}: source identifier {id} is already defined"
            )));
        }
        Ok(())
    }

    fn resolve(&self, id: &str, line_number: usize) -> Result<String, SfcsError> {
        validate_id(id)?;
        self.bindings.get(id).cloned().ok_or_else(|| {
            SfcsError::InvalidProgram(format!(
                "line {line_number}: source references unknown identifier {id}"
            ))
        })
    }

    fn next_temp(&mut self) -> String {
        loop {
            let id = format!("__sfcs_tmp_{:06}", self.temp_counter);
            self.temp_counter += 1;
            if !self.graph.nodes.contains_key(&id) && !self.bindings.contains_key(&id) {
                return id;
            }
        }
    }

    fn lower_expr(
        &mut self,
        expression: &SfcsExpr,
        preferred_id: Option<String>,
        line_number: usize,
    ) -> Result<String, SfcsError> {
        match expression {
            SfcsExpr::Ident(id) => {
                let resolved = self.resolve(id, line_number)?;
                if let Some(id) = preferred_id {
                    self.insert_generated_node(
                        SfcsNode::new(&id, SfcsOp::Alias, vec![resolved]),
                        "alias",
                    )?;
                    Ok(id)
                } else {
                    Ok(resolved)
                }
            }
            SfcsExpr::Number(value) => {
                let id = preferred_id.unwrap_or_else(|| self.next_temp());
                self.insert_generated_node(SfcsNode::constant(&id, *value), "const")?;
                Ok(id)
            }
            SfcsExpr::UnaryNot(inner) => {
                let input = self.lower_expr(inner, None, line_number)?;
                let id = preferred_id.unwrap_or_else(|| self.next_temp());
                self.insert_generated_node(SfcsNode::new(&id, SfcsOp::Not, vec![input]), "not")?;
                Ok(id)
            }
            SfcsExpr::Load(address) => {
                let address = self.lower_expr(address, None, line_number)?;
                let id = preferred_id.unwrap_or_else(|| self.next_temp());
                let mut inputs = vec![address];
                if let Some(memory_head) = self.memory_head.clone() {
                    inputs.push(memory_head);
                }
                self.insert_generated_node(SfcsNode::new(&id, SfcsOp::MemoryRead, inputs), "load")?;
                self.memory_head = Some(id.clone());
                Ok(id)
            }
            SfcsExpr::Store { address, value } => {
                let address = self.lower_expr(address, None, line_number)?;
                let value = self.lower_expr(value, None, line_number)?;
                let id = preferred_id.unwrap_or_else(|| self.next_temp());
                let mut inputs = self.distinct_inputs(vec![address, value], line_number)?;
                if let Some(memory_head) = self.memory_head.clone() {
                    if inputs.iter().any(|input| input == &memory_head) {
                        let alias_id = self.next_temp();
                        self.insert_generated_node(
                            SfcsNode::new(&alias_id, SfcsOp::Alias, vec![memory_head]),
                            "alias",
                        )?;
                        inputs.push(alias_id);
                    } else {
                        inputs.push(memory_head);
                    }
                }
                self.insert_generated_node(
                    SfcsNode::new(&id, SfcsOp::MemoryWrite, inputs),
                    "store",
                )?;
                self.memory_head = Some(id.clone());
                Ok(id)
            }
            SfcsExpr::Binary { op, left, right } => {
                let left = self.lower_expr(left, None, line_number)?;
                let right = self.lower_expr(right, None, line_number)?;
                let id = preferred_id.unwrap_or_else(|| self.next_temp());
                let inputs = self.distinct_inputs(vec![left, right], line_number)?;
                self.insert_generated_node(SfcsNode::new(&id, op.to_sfcs_op(), inputs), op.name())?;
                Ok(id)
            }
            SfcsExpr::If {
                condition,
                then_expr,
                else_expr,
            } => {
                let condition = self.lower_expr(condition, None, line_number)?;
                let then_id = self.lower_expr(then_expr, None, line_number)?;
                let else_id = self.lower_expr(else_expr, None, line_number)?;
                let id = preferred_id.unwrap_or_else(|| self.next_temp());
                let inputs =
                    self.distinct_inputs(vec![condition, then_id, else_id], line_number)?;
                self.insert_generated_node(
                    SfcsNode::new(&id, SfcsOp::Branch, inputs),
                    "if_then_else",
                )?;
                Ok(id)
            }
        }
    }

    fn insert_generated_node(
        &mut self,
        mut node: SfcsNode,
        source_op: &str,
    ) -> Result<(), SfcsError> {
        node.metadata
            .insert("source".to_string(), "sfcs-expression".to_string());
        node.metadata
            .insert("source_op".to_string(), source_op.to_string());
        self.graph.insert_node(node)
    }

    fn distinct_inputs(
        &mut self,
        inputs: Vec<String>,
        line_number: usize,
    ) -> Result<Vec<String>, SfcsError> {
        let mut seen = BTreeSet::new();
        let mut distinct = Vec::new();
        for input in inputs {
            if seen.insert(input.clone()) {
                distinct.push(input);
                continue;
            }
            let alias_id = self.next_temp();
            self.insert_generated_node(
                SfcsNode::new(&alias_id, SfcsOp::Alias, vec![input]),
                "alias",
            )
            .map_err(|error| {
                SfcsError::InvalidProgram(format!(
                    "line {line_number}: could not create deterministic alias: {error}"
                ))
            })?;
            distinct.push(alias_id);
        }
        Ok(distinct)
    }
}

fn split_output_as(body: &str) -> Option<(&str, &str)> {
    let (expression, id) = body.rsplit_once(" as ")?;
    if expression.trim().is_empty() || id.trim().is_empty() || id.split_whitespace().count() != 1 {
        return None;
    }
    Some((expression, id))
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SfcsExpr {
    Ident(String),
    Number(i64),
    UnaryNot(Box<SfcsExpr>),
    Load(Box<SfcsExpr>),
    Store {
        address: Box<SfcsExpr>,
        value: Box<SfcsExpr>,
    },
    Binary {
        op: SfcsExprBinaryOp,
        left: Box<SfcsExpr>,
        right: Box<SfcsExpr>,
    },
    If {
        condition: Box<SfcsExpr>,
        then_expr: Box<SfcsExpr>,
        else_expr: Box<SfcsExpr>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SfcsExprBinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
}

impl SfcsExprBinaryOp {
    fn to_sfcs_op(self) -> SfcsOp {
        match self {
            Self::Add => SfcsOp::Add,
            Self::Sub => SfcsOp::Sub,
            Self::Mul => SfcsOp::Mul,
            Self::Div => SfcsOp::Div,
            Self::Mod => SfcsOp::Mod,
            Self::Eq => SfcsOp::Eq,
            Self::Lt => SfcsOp::Lt,
            Self::Le => SfcsOp::Le,
            Self::Gt => SfcsOp::Gt,
            Self::Ge => SfcsOp::Ge,
            Self::And => SfcsOp::And,
            Self::Or => SfcsOp::Or,
            Self::BitAnd => SfcsOp::BitAnd,
            Self::BitOr => SfcsOp::BitOr,
            Self::BitXor => SfcsOp::BitXor,
            Self::Shl => SfcsOp::Shl,
            Self::Shr => SfcsOp::Shr,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::Sub => "sub",
            Self::Mul => "mul",
            Self::Div => "div",
            Self::Mod => "mod",
            Self::Eq => "eq",
            Self::Lt => "lt",
            Self::Le => "le",
            Self::Gt => "gt",
            Self::Ge => "ge",
            Self::And => "and",
            Self::Or => "or",
            Self::BitAnd => "bit_and",
            Self::BitOr => "bit_or",
            Self::BitXor => "bit_xor",
            Self::Shl => "shl",
            Self::Shr => "shr",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SfcsExprToken {
    Ident(String),
    Number(i64),
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    EqEq,
    Lt,
    Le,
    Gt,
    Ge,
    AndAnd,
    OrOr,
    Amp,
    Pipe,
    Caret,
    ShiftLeft,
    ShiftRight,
    Bang,
    Comma,
    LParen,
    RParen,
    If,
    Then,
    Else,
    Load,
    Store,
    End,
}

struct SfcsExprParser {
    tokens: Vec<SfcsExprToken>,
    index: usize,
    line_number: usize,
}

impl SfcsExprParser {
    fn parse(source: &str, line_number: usize) -> Result<SfcsExpr, SfcsError> {
        let mut parser = Self {
            tokens: tokenize_expr(source, line_number)?,
            index: 0,
            line_number,
        };
        let expression = parser.parse_expression()?;
        parser.expect_end()?;
        Ok(expression)
    }

    fn parse_expression(&mut self) -> Result<SfcsExpr, SfcsError> {
        self.parse_if()
    }

    fn parse_if(&mut self) -> Result<SfcsExpr, SfcsError> {
        if self.consume_if() {
            let condition = self.parse_expression()?;
            self.expect_then()?;
            let then_expr = self.parse_expression()?;
            self.expect_else()?;
            let else_expr = self.parse_expression()?;
            return Ok(SfcsExpr::If {
                condition: Box::new(condition),
                then_expr: Box::new(then_expr),
                else_expr: Box::new(else_expr),
            });
        }
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<SfcsExpr, SfcsError> {
        let mut expression = self.parse_and()?;
        while self.consume_or_or() {
            let right = self.parse_and()?;
            expression = SfcsExpr::Binary {
                op: SfcsExprBinaryOp::Or,
                left: Box::new(expression),
                right: Box::new(right),
            };
        }
        Ok(expression)
    }

    fn parse_and(&mut self) -> Result<SfcsExpr, SfcsError> {
        let mut expression = self.parse_bit_or()?;
        while self.consume_and_and() {
            let right = self.parse_bit_or()?;
            expression = SfcsExpr::Binary {
                op: SfcsExprBinaryOp::And,
                left: Box::new(expression),
                right: Box::new(right),
            };
        }
        Ok(expression)
    }

    fn parse_bit_or(&mut self) -> Result<SfcsExpr, SfcsError> {
        let mut expression = self.parse_bit_xor()?;
        while self.consume_pipe() {
            let right = self.parse_bit_xor()?;
            expression = SfcsExpr::Binary {
                op: SfcsExprBinaryOp::BitOr,
                left: Box::new(expression),
                right: Box::new(right),
            };
        }
        Ok(expression)
    }

    fn parse_bit_xor(&mut self) -> Result<SfcsExpr, SfcsError> {
        let mut expression = self.parse_bit_and()?;
        while self.consume_caret() {
            let right = self.parse_bit_and()?;
            expression = SfcsExpr::Binary {
                op: SfcsExprBinaryOp::BitXor,
                left: Box::new(expression),
                right: Box::new(right),
            };
        }
        Ok(expression)
    }

    fn parse_bit_and(&mut self) -> Result<SfcsExpr, SfcsError> {
        let mut expression = self.parse_eq()?;
        while self.consume_amp() {
            let right = self.parse_eq()?;
            expression = SfcsExpr::Binary {
                op: SfcsExprBinaryOp::BitAnd,
                left: Box::new(expression),
                right: Box::new(right),
            };
        }
        Ok(expression)
    }

    fn parse_eq(&mut self) -> Result<SfcsExpr, SfcsError> {
        let mut expression = self.parse_cmp()?;
        while self.consume_eq_eq() {
            let right = self.parse_cmp()?;
            expression = SfcsExpr::Binary {
                op: SfcsExprBinaryOp::Eq,
                left: Box::new(expression),
                right: Box::new(right),
            };
        }
        Ok(expression)
    }

    fn parse_cmp(&mut self) -> Result<SfcsExpr, SfcsError> {
        let mut expression = self.parse_shift()?;
        loop {
            let op = if self.consume_le() {
                SfcsExprBinaryOp::Le
            } else if self.consume_lt() {
                SfcsExprBinaryOp::Lt
            } else if self.consume_ge() {
                SfcsExprBinaryOp::Ge
            } else if self.consume_gt() {
                SfcsExprBinaryOp::Gt
            } else {
                return Ok(expression);
            };
            let right = self.parse_shift()?;
            expression = SfcsExpr::Binary {
                op,
                left: Box::new(expression),
                right: Box::new(right),
            };
        }
    }

    fn parse_shift(&mut self) -> Result<SfcsExpr, SfcsError> {
        let mut expression = self.parse_add_sub()?;
        loop {
            let op = if self.consume_shift_left() {
                SfcsExprBinaryOp::Shl
            } else if self.consume_shift_right() {
                SfcsExprBinaryOp::Shr
            } else {
                return Ok(expression);
            };
            let right = self.parse_add_sub()?;
            expression = SfcsExpr::Binary {
                op,
                left: Box::new(expression),
                right: Box::new(right),
            };
        }
    }

    fn parse_add_sub(&mut self) -> Result<SfcsExpr, SfcsError> {
        let mut expression = self.parse_mul_div_mod()?;
        loop {
            if self.consume_plus() {
                let right = self.parse_mul_div_mod()?;
                expression = SfcsExpr::Binary {
                    op: SfcsExprBinaryOp::Add,
                    left: Box::new(expression),
                    right: Box::new(right),
                };
            } else if self.consume_minus() {
                let right = self.parse_mul_div_mod()?;
                expression = SfcsExpr::Binary {
                    op: SfcsExprBinaryOp::Sub,
                    left: Box::new(expression),
                    right: Box::new(right),
                };
            } else {
                return Ok(expression);
            }
        }
    }

    fn parse_mul_div_mod(&mut self) -> Result<SfcsExpr, SfcsError> {
        let mut expression = self.parse_unary()?;
        loop {
            let op = if self.consume_star() {
                SfcsExprBinaryOp::Mul
            } else if self.consume_slash() {
                SfcsExprBinaryOp::Div
            } else if self.consume_percent() {
                SfcsExprBinaryOp::Mod
            } else {
                return Ok(expression);
            };
            let right = self.parse_unary()?;
            expression = SfcsExpr::Binary {
                op,
                left: Box::new(expression),
                right: Box::new(right),
            };
        }
    }

    fn parse_unary(&mut self) -> Result<SfcsExpr, SfcsError> {
        if self.consume_bang() {
            return Ok(SfcsExpr::UnaryNot(Box::new(self.parse_unary()?)));
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<SfcsExpr, SfcsError> {
        match self.next().clone() {
            SfcsExprToken::Ident(id) => Ok(SfcsExpr::Ident(id)),
            SfcsExprToken::Number(value) => Ok(SfcsExpr::Number(value)),
            SfcsExprToken::Load => {
                self.expect_lparen()?;
                let address = self.parse_expression()?;
                self.expect_rparen()?;
                Ok(SfcsExpr::Load(Box::new(address)))
            }
            SfcsExprToken::Store => {
                self.expect_lparen()?;
                let address = self.parse_expression()?;
                self.expect_comma()?;
                let value = self.parse_expression()?;
                self.expect_rparen()?;
                Ok(SfcsExpr::Store {
                    address: Box::new(address),
                    value: Box::new(value),
                })
            }
            SfcsExprToken::LParen => {
                let expression = self.parse_expression()?;
                self.expect_rparen()?;
                Ok(expression)
            }
            token => Err(self.error(format!("expected expression, found {token:?}"))),
        }
    }

    fn next(&mut self) -> &SfcsExprToken {
        let token = &self.tokens[self.index];
        self.index += 1;
        token
    }

    fn peek(&self) -> &SfcsExprToken {
        &self.tokens[self.index]
    }

    fn consume_if(&mut self) -> bool {
        self.consume(|token| matches!(token, SfcsExprToken::If))
    }

    fn consume_then(&mut self) -> bool {
        self.consume(|token| matches!(token, SfcsExprToken::Then))
    }

    fn consume_else(&mut self) -> bool {
        self.consume(|token| matches!(token, SfcsExprToken::Else))
    }

    fn consume_or_or(&mut self) -> bool {
        self.consume(|token| matches!(token, SfcsExprToken::OrOr))
    }

    fn consume_and_and(&mut self) -> bool {
        self.consume(|token| matches!(token, SfcsExprToken::AndAnd))
    }

    fn consume_eq_eq(&mut self) -> bool {
        self.consume(|token| matches!(token, SfcsExprToken::EqEq))
    }

    fn consume_le(&mut self) -> bool {
        self.consume(|token| matches!(token, SfcsExprToken::Le))
    }

    fn consume_lt(&mut self) -> bool {
        self.consume(|token| matches!(token, SfcsExprToken::Lt))
    }

    fn consume_ge(&mut self) -> bool {
        self.consume(|token| matches!(token, SfcsExprToken::Ge))
    }

    fn consume_gt(&mut self) -> bool {
        self.consume(|token| matches!(token, SfcsExprToken::Gt))
    }

    fn consume_shift_left(&mut self) -> bool {
        self.consume(|token| matches!(token, SfcsExprToken::ShiftLeft))
    }

    fn consume_shift_right(&mut self) -> bool {
        self.consume(|token| matches!(token, SfcsExprToken::ShiftRight))
    }

    fn consume_plus(&mut self) -> bool {
        self.consume(|token| matches!(token, SfcsExprToken::Plus))
    }

    fn consume_minus(&mut self) -> bool {
        self.consume(|token| matches!(token, SfcsExprToken::Minus))
    }

    fn consume_star(&mut self) -> bool {
        self.consume(|token| matches!(token, SfcsExprToken::Star))
    }

    fn consume_slash(&mut self) -> bool {
        self.consume(|token| matches!(token, SfcsExprToken::Slash))
    }

    fn consume_percent(&mut self) -> bool {
        self.consume(|token| matches!(token, SfcsExprToken::Percent))
    }

    fn consume_amp(&mut self) -> bool {
        self.consume(|token| matches!(token, SfcsExprToken::Amp))
    }

    fn consume_pipe(&mut self) -> bool {
        self.consume(|token| matches!(token, SfcsExprToken::Pipe))
    }

    fn consume_caret(&mut self) -> bool {
        self.consume(|token| matches!(token, SfcsExprToken::Caret))
    }

    fn consume_bang(&mut self) -> bool {
        self.consume(|token| matches!(token, SfcsExprToken::Bang))
    }

    fn consume<F>(&mut self, predicate: F) -> bool
    where
        F: FnOnce(&SfcsExprToken) -> bool,
    {
        if predicate(self.peek()) {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn expect_then(&mut self) -> Result<(), SfcsError> {
        if self.consume_then() {
            Ok(())
        } else {
            Err(self.error("expected then".to_string()))
        }
    }

    fn expect_else(&mut self) -> Result<(), SfcsError> {
        if self.consume_else() {
            Ok(())
        } else {
            Err(self.error("expected else".to_string()))
        }
    }

    fn expect_lparen(&mut self) -> Result<(), SfcsError> {
        if self.consume(|token| matches!(token, SfcsExprToken::LParen)) {
            Ok(())
        } else {
            Err(self.error("expected '('".to_string()))
        }
    }

    fn expect_rparen(&mut self) -> Result<(), SfcsError> {
        if self.consume(|token| matches!(token, SfcsExprToken::RParen)) {
            Ok(())
        } else {
            Err(self.error("expected ')'".to_string()))
        }
    }

    fn expect_comma(&mut self) -> Result<(), SfcsError> {
        if self.consume(|token| matches!(token, SfcsExprToken::Comma)) {
            Ok(())
        } else {
            Err(self.error("expected ','".to_string()))
        }
    }

    fn expect_end(&mut self) -> Result<(), SfcsError> {
        if matches!(self.peek(), SfcsExprToken::End) {
            Ok(())
        } else {
            Err(self.error(format!("unexpected token {:?}", self.peek())))
        }
    }

    fn error(&self, message: String) -> SfcsError {
        SfcsError::InvalidProgram(format!(
            "line {}: invalid source expression: {message}",
            self.line_number
        ))
    }
}

fn tokenize_expr(source: &str, line_number: usize) -> Result<Vec<SfcsExprToken>, SfcsError> {
    let bytes = source.as_bytes();
    let mut index = 0;
    let mut tokens = Vec::new();
    while index < bytes.len() {
        let byte = bytes[index];
        if byte.is_ascii_whitespace() {
            index += 1;
            continue;
        }
        match byte {
            b'+' => {
                tokens.push(SfcsExprToken::Plus);
                index += 1;
            }
            b'-' => {
                tokens.push(SfcsExprToken::Minus);
                index += 1;
            }
            b'*' => {
                tokens.push(SfcsExprToken::Star);
                index += 1;
            }
            b'/' => {
                tokens.push(SfcsExprToken::Slash);
                index += 1;
            }
            b'%' => {
                tokens.push(SfcsExprToken::Percent);
                index += 1;
            }
            b'!' => {
                tokens.push(SfcsExprToken::Bang);
                index += 1;
            }
            b',' => {
                tokens.push(SfcsExprToken::Comma);
                index += 1;
            }
            b'(' => {
                tokens.push(SfcsExprToken::LParen);
                index += 1;
            }
            b')' => {
                tokens.push(SfcsExprToken::RParen);
                index += 1;
            }
            b'=' if bytes.get(index + 1) == Some(&b'=') => {
                tokens.push(SfcsExprToken::EqEq);
                index += 2;
            }
            b'&' if bytes.get(index + 1) == Some(&b'&') => {
                tokens.push(SfcsExprToken::AndAnd);
                index += 2;
            }
            b'&' => {
                tokens.push(SfcsExprToken::Amp);
                index += 1;
            }
            b'|' if bytes.get(index + 1) == Some(&b'|') => {
                tokens.push(SfcsExprToken::OrOr);
                index += 2;
            }
            b'|' => {
                tokens.push(SfcsExprToken::Pipe);
                index += 1;
            }
            b'^' => {
                tokens.push(SfcsExprToken::Caret);
                index += 1;
            }
            b'<' if bytes.get(index + 1) == Some(&b'<') => {
                tokens.push(SfcsExprToken::ShiftLeft);
                index += 2;
            }
            b'<' if bytes.get(index + 1) == Some(&b'=') => {
                tokens.push(SfcsExprToken::Le);
                index += 2;
            }
            b'<' => {
                tokens.push(SfcsExprToken::Lt);
                index += 1;
            }
            b'>' if bytes.get(index + 1) == Some(&b'>') => {
                tokens.push(SfcsExprToken::ShiftRight);
                index += 2;
            }
            b'>' if bytes.get(index + 1) == Some(&b'=') => {
                tokens.push(SfcsExprToken::Ge);
                index += 2;
            }
            b'>' => {
                tokens.push(SfcsExprToken::Gt);
                index += 1;
            }
            b'0'..=b'9' => {
                let start = index;
                while bytes.get(index).is_some_and(u8::is_ascii_digit) {
                    index += 1;
                }
                let value = source[start..index].parse::<i64>().map_err(|error| {
                    SfcsError::InvalidProgram(format!(
                        "line {line_number}: invalid integer literal: {error}"
                    ))
                })?;
                tokens.push(SfcsExprToken::Number(value));
            }
            _ if byte.is_ascii_alphabetic() || byte == b'_' => {
                let start = index;
                while bytes.get(index).is_some_and(|value| {
                    value.is_ascii_alphanumeric() || matches!(*value, b'_' | b'.' | b':')
                }) {
                    index += 1;
                }
                let id = &source[start..index];
                match id {
                    "if" => tokens.push(SfcsExprToken::If),
                    "then" => tokens.push(SfcsExprToken::Then),
                    "else" => tokens.push(SfcsExprToken::Else),
                    "load" => tokens.push(SfcsExprToken::Load),
                    "store" => tokens.push(SfcsExprToken::Store),
                    _ => {
                        validate_id(id)?;
                        tokens.push(SfcsExprToken::Ident(id.to_string()));
                    }
                }
            }
            _ => {
                return Err(SfcsError::InvalidProgram(format!(
                    "line {line_number}: unsupported expression character {:?}",
                    byte as char
                )));
            }
        }
    }
    if tokens.is_empty() {
        return Err(SfcsError::InvalidProgram(format!(
            "line {line_number}: empty expression"
        )));
    }
    tokens.push(SfcsExprToken::End);
    Ok(tokens)
}

fn visit_node(
    node_id: &str,
    graph: &SfcsGraph,
    temporary: &mut BTreeSet<String>,
    permanent: &mut BTreeSet<String>,
    order: &mut Vec<String>,
) -> Result<(), SfcsError> {
    if permanent.contains(node_id) {
        return Ok(());
    }
    if !temporary.insert(node_id.to_string()) {
        return Err(SfcsError::CycleDetected(node_id.to_string()));
    }
    let node = graph
        .nodes
        .get(node_id)
        .ok_or_else(|| SfcsError::UnknownNode(node_id.to_string()))?;
    for input in &node.inputs {
        visit_node(input, graph, temporary, permanent, order)?;
    }
    temporary.remove(node_id);
    permanent.insert(node_id.to_string());
    order.push(node_id.to_string());
    Ok(())
}

fn validate_node_shape(node: &SfcsNode) -> Result<(), SfcsError> {
    match node.op {
        SfcsOp::Input => require_inputs(node, 0),
        SfcsOp::Alias => require_inputs(node, 1),
        SfcsOp::Const => {
            require_inputs(node, 0)?;
            if !node.params.contains_key("value") {
                return Err(SfcsError::InvalidGraph(format!(
                    "const node {} missing value parameter",
                    node.id
                )));
            }
            Ok(())
        }
        SfcsOp::Add | SfcsOp::Mul => {
            if node.inputs.len() < 2 {
                return Err(SfcsError::InvalidGraph(format!(
                    "node {} requires at least two inputs",
                    node.id
                )));
            }
            Ok(())
        }
        SfcsOp::Sub
        | SfcsOp::Div
        | SfcsOp::Mod
        | SfcsOp::Eq
        | SfcsOp::Lt
        | SfcsOp::Le
        | SfcsOp::Gt
        | SfcsOp::Ge
        | SfcsOp::And
        | SfcsOp::Or
        | SfcsOp::BitAnd
        | SfcsOp::BitOr
        | SfcsOp::BitXor
        | SfcsOp::Shl
        | SfcsOp::Shr => require_inputs(node, 2),
        SfcsOp::Not => require_inputs(node, 1),
        SfcsOp::Branch => require_inputs(node, 3),
        SfcsOp::DenseStep | SfcsOp::FastPathClaim => {
            if node.inputs.is_empty() {
                return Err(SfcsError::InvalidGraph(format!(
                    "node {} requires at least one input",
                    node.id
                )));
            }
            Ok(())
        }
        SfcsOp::MemoryRead => {
            if !(1..=2).contains(&node.inputs.len()) {
                return Err(SfcsError::InvalidGraph(format!(
                    "memory read {} requires address and optional memory dependency",
                    node.id
                )));
            }
            Ok(())
        }
        SfcsOp::MemoryWrite => {
            if !(2..=3).contains(&node.inputs.len()) {
                return Err(SfcsError::InvalidGraph(format!(
                    "memory write {} requires address, value, and optional memory dependency",
                    node.id
                )));
            }
            Ok(())
        }
    }
}

fn require_inputs(node: &SfcsNode, count: usize) -> Result<(), SfcsError> {
    if node.inputs.len() == count {
        Ok(())
    } else {
        Err(SfcsError::InvalidGraph(format!(
            "node {} requires exactly {count} inputs",
            node.id
        )))
    }
}

fn require_program_arity(
    line_number: usize,
    keyword: &str,
    values: &[&str],
    count: usize,
) -> Result<(), SfcsError> {
    if values.len() == count {
        Ok(())
    } else {
        Err(SfcsError::InvalidProgram(format!(
            "line {line_number}: {keyword} requires {count} argument(s)"
        )))
    }
}

fn validate_id(value: &str) -> Result<(), SfcsError> {
    if value.is_empty() || value.len() > 96 {
        return Err(SfcsError::InvalidId(value.to_string()));
    }
    if !value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':'))
    {
        return Err(SfcsError::InvalidId(value.to_string()));
    }
    Ok(())
}

fn validate_metadata_value(key: &str, value: &str) -> Result<(), SfcsError> {
    if value.len() > 512 {
        return Err(SfcsError::InvalidGraph(format!(
            "metadata value for {key} exceeds 512 bytes"
        )));
    }
    if value.chars().any(char::is_control) {
        return Err(SfcsError::InvalidGraph(format!(
            "metadata value for {key} contains a control character"
        )));
    }
    Ok(())
}

fn checked_shift_amount(value: i64, node_id: &str) -> Result<u32, SfcsError> {
    if !(0..=63).contains(&value) {
        return Err(SfcsError::Execution(format!(
            "node {node_id} used invalid shift amount {value}"
        )));
    }
    Ok(value as u32)
}

fn validate_sha256(value: &str) -> Result<(), SfcsError> {
    let Some(hex_digest) = value.strip_prefix(SHA256_PREFIX) else {
        return Err(SfcsError::InvalidDigest(value.to_string()));
    };
    if hex_digest.len() != 64
        || !hex_digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(SfcsError::InvalidDigest(value.to_string()));
    }
    Ok(())
}

fn digest_json<T: Serialize>(domain: &[u8], value: &T) -> Result<String, SfcsError> {
    let encoded = serde_json::to_vec(value)?;
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(encoded);
    Ok(format!("{SHA256_PREFIX}{}", hex::encode(hasher.finalize())))
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

    fn scan(mut self) -> Result<(), SfcsError> {
        self.skip_ws();
        self.value()?;
        self.skip_ws();
        if self.cursor != self.input.len() {
            return Err(SfcsError::Canonical(
                "trailing content after JSON value".to_string(),
            ));
        }
        Ok(())
    }

    fn value(&mut self) -> Result<(), SfcsError> {
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
            Some(other) => Err(SfcsError::Canonical(format!(
                "unexpected JSON byte {other} at {}",
                self.cursor
            ))),
            None => Err(SfcsError::Canonical("unexpected end of JSON".to_string())),
        }
    }

    fn object(&mut self) -> Result<(), SfcsError> {
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
                return Err(SfcsError::Canonical(format!(
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

    fn array(&mut self) -> Result<(), SfcsError> {
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

    fn string(&mut self) -> Result<String, SfcsError> {
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
                        .map_err(SfcsError::Json);
                }
                0x00..=0x1f => {
                    return Err(SfcsError::Canonical(
                        "unescaped control byte in JSON string".to_string(),
                    ));
                }
                _ => {}
            }
        }
        Err(SfcsError::Canonical("unterminated JSON string".to_string()))
    }

    fn number(&mut self) -> Result<(), SfcsError> {
        if self.consume_if(b'-') && !self.peek_is_digit() {
            return Err(SfcsError::Canonical("invalid JSON number".to_string()));
        }
        if self.consume_if(b'0') {
            if self.peek_is_digit() {
                return Err(SfcsError::Canonical(
                    "leading zero in JSON number".to_string(),
                ));
            }
        } else {
            self.digits()?;
        }
        if matches!(self.peek(), Some(b'.' | b'e' | b'E')) {
            return Err(SfcsError::Canonical(
                "floating-point JSON number is forbidden".to_string(),
            ));
        }
        Ok(())
    }

    fn digits(&mut self) -> Result<(), SfcsError> {
        if !self.peek_is_digit() {
            return Err(SfcsError::Canonical("expected digit".to_string()));
        }
        while self.peek_is_digit() {
            self.cursor += 1;
        }
        Ok(())
    }

    fn literal(&mut self, expected: &[u8]) -> Result<(), SfcsError> {
        if self.input.get(self.cursor..self.cursor + expected.len()) == Some(expected) {
            self.cursor += expected.len();
            Ok(())
        } else {
            Err(SfcsError::Canonical(format!(
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

    fn expect(&mut self, expected: u8) -> Result<(), SfcsError> {
        match self.next() {
            Some(found) if found == expected => Ok(()),
            Some(found) => Err(SfcsError::Canonical(format!(
                "expected byte {expected}, found {found} at {}",
                self.cursor.saturating_sub(1)
            ))),
            None => Err(SfcsError::Canonical(format!(
                "expected byte {expected}, found end of JSON"
            ))),
        }
    }

    fn consume_if(&mut self, expected: u8) -> bool {
        if self.peek() == Some(expected) {
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
