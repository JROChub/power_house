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
const TRACE_STEP_DOMAIN: &[u8] = b"power-house:sfcs:v1-draft:trace-step\0";
const TRACE_DOMAIN: &[u8] = b"power-house:sfcs:v1-draft:execution-trace\0";
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
    /// Integer constant stored in `params["value"]`.
    Const,
    /// Integer addition over ordered inputs.
    Add,
    /// Integer multiplication over ordered inputs.
    Mul,
    /// Deterministic branch: input 0 is the condition, input 1 true, input 2 false.
    Branch,
    /// Dense opaque step that is not eligible for the Sovereign fast path.
    DenseStep,
    /// Explicit memory read placeholder. Not executable in the draft evaluator.
    MemoryRead,
    /// Explicit memory write placeholder. Not executable in the draft evaluator.
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
        }
    }

    /// Sets a metadata label.
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    fn is_fast_path_eligible(&self) -> bool {
        matches!(
            self.op,
            SfcsOp::Input | SfcsOp::Const | SfcsOp::Add | SfcsOp::Mul | SfcsOp::FastPathClaim
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
    /// - `const <id> <integer>`
    /// - `add <id> <input> <input> [input...]`
    /// - `mul <id> <input> <input> [input...]`
    /// - `branch <id> <condition> <true> <false>`
    /// - `dense <id> <input> [input...]`
    /// - `memory_read <id> <input> [input...]`
    /// - `memory_write <id> <input> [input...]`
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
        Ok(SfcsDiscoveryReport {
            graph_digest,
            node_count: self.nodes.len(),
            fast_path_nodes,
            dense_nodes,
            fast_path_workload_digest: workload.workload_digest()?,
        })
    }

    /// Evaluates the executable arithmetic subset deterministically.
    ///
    /// Memory and dense opaque operations are deliberately rejected in this
    /// draft evaluator rather than silently accepted.
    pub fn evaluate(
        &self,
        inputs: &BTreeMap<String, i64>,
    ) -> Result<BTreeMap<String, i64>, SfcsError> {
        Ok(self.execution_trace(inputs)?.outputs)
    }

    /// Executes the arithmetic subset and returns a deterministic trace.
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
        let mut values = BTreeMap::new();
        let mut steps = Vec::new();
        for node_id in self.topological_order()? {
            let node = &self.nodes[&node_id];
            let input_values = node
                .inputs
                .iter()
                .map(|input| (input.clone(), values[input]))
                .collect::<BTreeMap<_, _>>();
            let value = match node.op {
                SfcsOp::Input => *inputs
                    .get(&node.id)
                    .ok_or_else(|| SfcsError::MissingInput(node.id.clone()))?,
                SfcsOp::Const => *node.params.get("value").ok_or_else(|| {
                    SfcsError::InvalidGraph(format!("const {} missing value", node.id))
                })?,
                SfcsOp::Add => node.inputs.iter().try_fold(0_i64, |acc, input| {
                    Ok::<i64, SfcsError>(acc.wrapping_add(values[input]))
                })?,
                SfcsOp::Mul => node.inputs.iter().try_fold(1_i64, |acc, input| {
                    Ok::<i64, SfcsError>(acc.wrapping_mul(values[input]))
                })?,
                SfcsOp::Branch => {
                    let condition = values[&node.inputs[0]];
                    if condition != 0 {
                        values[&node.inputs[1]]
                    } else {
                        values[&node.inputs[2]]
                    }
                }
                SfcsOp::FastPathClaim
                | SfcsOp::DenseStep
                | SfcsOp::MemoryRead
                | SfcsOp::MemoryWrite => {
                    return Err(SfcsError::UnsupportedEvaluation(node.id.clone()));
                }
            };
            let mut step = SfcsTraceStep {
                step_index: steps.len() as u64,
                node_id: node.id.clone(),
                op: node.op.clone(),
                input_nodes: node.inputs.clone(),
                input_values,
                output_value: value,
                fast_path_eligible: node.is_fast_path_eligible(),
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
        let mut pending_fast = Vec::new();
        for node_id in self.topological_order()? {
            let node = &self.nodes[&node_id];
            if node.is_fast_path_eligible() {
                pending_fast.push(node_id);
            } else {
                if !pending_fast.is_empty() {
                    operations.push(SfcsRewriteOperation::new(
                        operations.len() as u64,
                        SfcsRewriteKind::FastPathExtract,
                        std::mem::take(&mut pending_fast),
                        graph_digest.clone(),
                    )?);
                }
                operations.push(SfcsRewriteOperation::new(
                    operations.len() as u64,
                    SfcsRewriteKind::DenseBoundary,
                    vec![node_id],
                    graph_digest.clone(),
                )?);
            }
        }
        if !pending_fast.is_empty() {
            operations.push(SfcsRewriteOperation::new(
                operations.len() as u64,
                SfcsRewriteKind::FastPathExtract,
                pending_fast,
                graph_digest.clone(),
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
            fast_path_workload_digest,
            dense_nodes,
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
    if expected_node_count != discovery.node_count as u64
        || expected_fast != discovery.fast_path_nodes.len() as u64
        || expected_dense != discovery.dense_nodes.len() as u64
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
    /// Digest of the extracted fast-path workload descriptor.
    pub fast_path_workload_digest: String,
}

/// Deterministic execution trace for the arithmetic SFCS subset.
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
    /// Digest of the complete fast-path workload descriptor.
    pub fast_path_workload_digest: String,
    /// Dense/general nodes left outside the fast path.
    pub dense_nodes: Vec<String>,
}

impl SfcsSynthesisPlan {
    fn preimage(&self) -> serde_json::Value {
        serde_json::json!({
            "graph_digest": self.graph_digest,
            "operations": self.operations,
            "operation_digests": self.operation_digests,
            "fast_path_workload_digest": self.fast_path_workload_digest,
            "dense_nodes": self.dense_nodes,
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
    /// Domain-separated digest of the operation.
    pub operation_digest: String,
}

impl SfcsRewriteOperation {
    fn new(
        index: u64,
        kind: SfcsRewriteKind,
        node_ids: Vec<String>,
        graph_digest: String,
    ) -> Result<Self, SfcsError> {
        validate_sha256(&graph_digest)?;
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
        SfcsOp::MemoryRead | SfcsOp::MemoryWrite => {
            if node.inputs.is_empty() {
                return Err(SfcsError::InvalidGraph(format!(
                    "memory node {} requires at least one input",
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
