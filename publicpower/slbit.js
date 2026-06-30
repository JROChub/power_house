const enc = new TextEncoder();
const dec = new TextDecoder();
const domains = {
  seed: "slbit:transcript-seed:v2\0",
  payload: "slbit:round-payload:v2\0",
  transcript: "slbit:transcript:v2\0",
  graph: "slbit:semantic-graph:v2\0",
  packetId: "slbit:packet-id:v2\0",
  packet: "slbit:viz-packet:v2\0",
};

const el = Object.fromEntries(
  [
    "verify-state",
    "packet-id",
    "packet-digest",
    "transcript-digest",
    "graph-digest",
    "gate-stack",
    "graph-count",
    "semantic-graph",
    "claim-kind",
    "claim-title",
    "claim-subtitle",
    "summary-tabs",
    "summary-text",
    "anchor-list",
    "round-count",
    "timeline",
    "json-preview",
    "json-size",
    "packet-file",
    "export-markdown",
    "copy-llm",
    "boundary-state",
    "boundary-detail",
    "authority-strip",
    "ask-grid",
    "toast",
  ].map((id) => [id, document.getElementById(id)]),
);

const samples = {
  drone: {
    claim: ["drone-camera-frame-7842", 4096],
    seed: "drone-seed-7842",
    producer: ["mfenx-slbit-demo", "3.1.0", "browser"],
    visualization: { color: [0, 200, 255], icon: "camera", layer_name: "perception-conv3" },
    rounds: [
      [0, "sensor-frame", "sensor-processing", "Raw camera frame converted into normalized features"],
      [1, "attention-7", "attention-head-7", "Stop-sign feature strongly activated"],
      [2, "policy-stop", "safety-policy", "Stop-required policy fired"],
    ],
    nodes: [
      ["frame-7842", "input", "Camera frame 7842"],
      ["conv3", "model-layer", "perception-conv3"],
      ["attention-head-7", "attention", "attention-head-7"],
      ["stop-sign-detected", "decision", "stop-sign-detected"],
      ["stop-required", "policy-output", "stop-required"],
    ],
    edges: [
      ["frame-7842", "conv3", "processed-by"],
      ["conv3", "attention-head-7", "activated"],
      ["attention-head-7", "stop-sign-detected", "supports"],
      ["stop-sign-detected", "stop-required", "triggers"],
    ],
    anchors: [
      {
        anchor_type: "power-house/rootprint",
        label: "drone-perception-7842",
        reference: "rootprint-branch-7842",
        digest: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        metadata: {
          branch_id: "rootprint-branch-7842",
          replay_fingerprint: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
          sidecar_digest: "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        },
      },
    ],
    summaries: {
      operator: {
        author: null,
        generated: false,
        signed: false,
        source_scope: "transcript-plus-anchors",
        text: "The drone detected a stop sign and triggered the stop-required safety policy.",
      },
      auditor: {
        author: null,
        generated: false,
        signed: false,
        source_scope: "transcript-plus-anchors",
        text: "The packet binds the perception transcript to a Rootprint anchor while keeping SLBIT outside proof identity.",
      },
      llm_context: {
        author: null,
        generated: true,
        signed: false,
        source_scope: "transcript-plus-anchors",
        text: "Frame 7842 passed through perception-conv3, activated attention-head-7, produced stop-sign-detected, and triggered stop-required.",
      },
    },
  },
  agent: {
    claim: ["agent-session-deploy-review-42", 2048],
    seed: "agent-session-deploy-review-42",
    producer: ["agent-audit-demo", "3.1.0", null],
    visualization: { color: [185, 255, 61], icon: "robot", layer_name: "agent-session" },
    rounds: [
      [0, "issue-brief", "observation", "User requested a production deployment review"],
      [1, "repo-status", "tool-call:git-status", "Repository status was checked before proposing changes"],
      [2, "ci-result", "policy-check", "Release gate required tests to pass before merge"],
      [3, "human-approval", "human-approval", "Human approval was required before publishing"],
    ],
    nodes: [
      ["request", "observation", "Deployment review request"],
      ["git-status", "tool-call", "git status"],
      ["ci-gate", "policy-check", "CI release gate"],
      ["approval", "human-approval", "Human approval"],
      ["decision", "decision", "Proceed after checks"],
    ],
    edges: [
      ["request", "git-status", "requires"],
      ["git-status", "ci-gate", "feeds"],
      ["ci-gate", "approval", "requires"],
      ["approval", "decision", "authorizes"],
    ],
    anchors: [
      { anchor_type: "git-commit", label: "reviewed commit", reference: "f275025", digest: null, metadata: { repository: "JROChub/power_house" } },
      { anchor_type: "human-approval", label: "operator approval", reference: "approval-ticket-42", digest: null, metadata: {} },
    ],
    summaries: {
      auditor: {
        author: null,
        generated: false,
        signed: false,
        source_scope: "transcript-only",
        text: "The agent inspected repository state, respected the CI gate, and required human approval before the deployment decision.",
      },
      llm_context: {
        author: null,
        generated: true,
        signed: false,
        source_scope: "transcript-only",
        text: "Agent trace with observation, tool call, policy check, human approval, and final decision nodes.",
      },
    },
  },
  zkml: {
    claim: ["zkml-image-classification-17", 8192],
    seed: "zkml-image-classification-17",
    producer: ["zkml-demo", "3.1.0", null],
    visualization: { color: [130, 180, 255], icon: "brain", layer_name: "classifier-transformer" },
    rounds: [
      [0, "embedding-digest", "embedding", "Image embedding committed with fixed-point activations"],
      [1, "attention-digest", "attention-head-3", "Vehicle-like feature cluster received strongest attention"],
      [2, "classification-digest", "classification", "Classification output selected vehicle with confidence band 9200/10000"],
    ],
    nodes: [
      ["image-17", "input", "Image 17"],
      ["embedding", "embedding", "Fixed-point embedding"],
      ["attention-head-3", "attention", "attention-head-3"],
      ["vehicle", "classification", "vehicle"],
    ],
    edges: [
      ["image-17", "embedding", "encoded-as"],
      ["embedding", "attention-head-3", "attended-by"],
      ["attention-head-3", "vehicle", "supports"],
    ],
    anchors: [
      { anchor_type: "zk-proof", label: "classification proof", reference: "proofs/classification-17.zk", digest: "sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd", metadata: {} },
      { anchor_type: "model-card", label: "classifier-transformer-v4", reference: "models/classifier-transformer-v4/card.json", digest: null, metadata: {} },
    ],
    summaries: {
      operator: {
        author: null,
        generated: false,
        signed: false,
        source_scope: "transcript-plus-anchors",
        text: "The verified model classified the image as vehicle with confidence band 9200/10000.",
      },
      developer: {
        author: null,
        generated: false,
        signed: false,
        source_scope: "transcript-plus-anchors",
        text: "The packet explains a zkML image classification without changing the zk proof.",
      },
    },
  },
  finance: {
    claim: ["risk-model-audit-2026-06", 1024],
    seed: "risk-model-audit-2026-06",
    producer: ["risk-audit-demo", "3.1.0", null],
    visualization: { color: [255, 193, 77], icon: "database", layer_name: "credit-risk-model" },
    rounds: [
      [0, "dataset-fingerprint", "dataset-binding", "Input portfolio dataset matched approved fingerprint"],
      [1, "risk-score-output", "risk-model", "Risk score bucket was computed from committed features"],
      [2, "policy-control", "regulatory-control", "Adverse-action review control was applied"],
    ],
    nodes: [
      ["dataset", "dataset", "Approved portfolio dataset"],
      ["risk-model", "model", "Credit risk model"],
      ["risk-score", "decision", "Risk score bucket"],
      ["control", "regulatory-control", "Adverse-action review"],
    ],
    edges: [
      ["dataset", "risk-model", "input-to"],
      ["risk-model", "risk-score", "produces"],
      ["risk-score", "control", "checked-by"],
    ],
    anchors: [
      { anchor_type: "dataset-fingerprint", label: "portfolio dataset", reference: null, digest: "sha256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee", metadata: {} },
      { anchor_type: "regulatory-control", label: "adverse-action review", reference: "control-aa-2026-06", digest: null, metadata: {} },
    ],
    summaries: {
      auditor: {
        author: null,
        generated: false,
        signed: false,
        source_scope: "transcript-plus-anchors",
        text: "The packet records the dataset binding, model decision path, and regulatory control applied to the risk score.",
      },
      executive: {
        author: null,
        generated: false,
        signed: false,
        source_scope: "transcript-plus-anchors",
        text: "The audit trail shows the risk model used the approved dataset and applied the required control.",
      },
    },
  },
};

let currentPacket = null;
let currentJson = "";
let currentAudience = "";
let selectedNode = "";

function quote(value) {
  return JSON.stringify(String(value));
}

function utf8(value) {
  return enc.encode(value);
}

function concatBytes(left, right) {
  const out = new Uint8Array(left.length + right.length);
  out.set(left);
  out.set(right, left.length);
  return out;
}

function u64(value) {
  const bytes = new Uint8Array(8);
  const view = new DataView(bytes.buffer);
  view.setBigUint64(0, BigInt(value));
  return bytes;
}

function absorb(target, value) {
  target.push(u64(value.length), value);
}

async function sha256(bytes) {
  const digest = await crypto.subtle.digest("SHA-256", bytes);
  return [...new Uint8Array(digest)]
    .map((byte) => byte.toString(16).padStart(2, "0"))
    .join("");
}

async function digestWithDomain(domain, value) {
  const bytes = value instanceof Uint8Array ? value : utf8(value);
  return `sha256:${await sha256(concatBytes(utf8(domain), bytes))}`;
}

function metadataJson(metadata = {}) {
  const entries = Object.entries(metadata || {}).sort(([a], [b]) => a.localeCompare(b));
  return `{${entries.map(([key, value]) => `${quote(key)}:${quote(value)}`).join(",")}}`;
}

function hintsJson(hints = {}) {
  const parts = [];
  if (Array.isArray(hints.color)) parts.push(`${quote("color")}:[${hints.color.map(Number).join(",")}]`);
  if (hints.icon != null) parts.push(`${quote("icon")}:${quote(hints.icon)}`);
  if (hints.layer_name != null) parts.push(`${quote("layer_name")}:${quote(hints.layer_name)}`);
  return `{${parts.join(",")}}`;
}

function producerJson(producer = {}) {
  return `{${quote("environment")}:${producer.environment == null ? "null" : quote(producer.environment)},${quote("metadata")}:${metadataJson(producer.metadata)},${quote("name")}:${quote(producer.name)},${quote("version")}:${quote(producer.version)}}`;
}

function claimJson(claim) {
  return `{${quote("bit_width")}:${Number(claim.bit_width)},${quote("id")}:${quote(claim.id)},${quote("viz_hints")}:${hintsJson(claim.viz_hints)}}`;
}

function sortedAnchors(anchors = []) {
  return [...anchors].sort((a, b) =>
    String(a.anchor_type).localeCompare(String(b.anchor_type))
    || String(a.label).localeCompare(String(b.label))
    || String(a.reference ?? "").localeCompare(String(b.reference ?? ""))
    || String(a.digest ?? "").localeCompare(String(b.digest ?? "")),
  );
}

function anchorJson(anchor) {
  return `{${quote("anchor_type")}:${quote(anchor.anchor_type)},${quote("digest")}:${anchor.digest == null ? "null" : quote(anchor.digest)},${quote("label")}:${quote(anchor.label)},${quote("metadata")}:${metadataJson(anchor.metadata)},${quote("reference")}:${anchor.reference == null ? "null" : quote(anchor.reference)}}`;
}

function anchorsJson(anchors = []) {
  return `[${sortedAnchors(anchors).map(anchorJson).join(",")}]`;
}

function transcriptJson(rounds = []) {
  return `[${rounds.map((round) => `{${quote("component")}:${quote(round.component)},${quote("note")}:${quote(round.note)},${quote("payload_sha256")}:${quote(round.payload_sha256)},${quote("round")}:${Number(round.round)}}`).join(",")}]`;
}

function sortedNodes(nodes = []) {
  return [...nodes].sort((a, b) => String(a.id).localeCompare(String(b.id)) || String(a.kind).localeCompare(String(b.kind)));
}

function sortedEdges(edges = []) {
  return [...edges].sort((a, b) =>
    String(a.from).localeCompare(String(b.from))
    || String(a.to).localeCompare(String(b.to))
    || String(a.kind).localeCompare(String(b.kind)),
  );
}

function semanticGraphJson(graph = {}) {
  const edges = sortedEdges(graph.edges).map((edge) => {
    const label = edge.label == null ? "" : `,${quote("label")}:${quote(edge.label)}`;
    return `{${quote("from")}:${quote(edge.from)},${quote("kind")}:${quote(edge.kind)}${label},${quote("to")}:${quote(edge.to)}}`;
  });
  const nodes = sortedNodes(graph.nodes).map((node) => {
    const label = node.label == null ? "" : `,${quote("label")}:${quote(node.label)}`;
    const digest = node.digest == null ? "" : `,${quote("digest")}:${quote(node.digest)}`;
    return `{${quote("id")}:${quote(node.id)},${quote("kind")}:${quote(node.kind)}${label}${digest}}`;
  });
  return `{${quote("edges")}:[${edges.join(",")}],${quote("nodes")}:[${nodes.join(",")}]}`;
}

function summariesJson(summaries = {}) {
  return `{${Object.keys(summaries).sort().map((audience) => {
    const summary = summaries[audience];
    return `${quote(audience)}:{${quote("author")}:${summary.author == null ? "null" : quote(summary.author)},${quote("generated")}:${summary.generated ? "true" : "false"},${quote("signed")}:${summary.signed ? "true" : "false"},${quote("source_scope")}:${quote(summary.source_scope)},${quote("text")}:${quote(summary.text)}}`;
  }).join(",")}}`;
}

function redactionsJson(redactions = []) {
  return `[${[...redactions].sort((a, b) => String(a.redaction_id).localeCompare(String(b.redaction_id))).map((redaction) => `{${quote("field")}:${quote(redaction.field)},${quote("original_digest")}:${quote(redaction.original_digest)},${quote("reason")}:${quote(redaction.reason)},${quote("redaction_id")}:${quote(redaction.redaction_id)},${quote("replacement")}:${quote(redaction.replacement)}}`).join(",")}]`;
}

function signaturesJson(signatures = []) {
  return `[${[...signatures].sort((a, b) =>
    String(a.signer).localeCompare(String(b.signer))
    || String(a.signature_type).localeCompare(String(b.signature_type))
    || String(a.signature).localeCompare(String(b.signature)),
  ).map((signature) => `{${quote("signature")}:${quote(signature.signature)},${quote("signature_type")}:${quote(signature.signature_type)},${quote("signer")}:${quote(signature.signer)}}`).join(",")}]`;
}

function digestsJson(digests, includePacket = true) {
  const packet = includePacket ? `${quote("packet")}:${quote(digests.packet)},` : "";
  return `{${packet}${quote("seed_commitment")}:${quote(digests.seed_commitment)},${quote("semantic_graph")}:${quote(digests.semantic_graph)},${quote("transcript")}:${quote(digests.transcript)}}`;
}

function identityJson(packet) {
  return `{${quote("claim")}:${claimJson(packet.claim)},${quote("anchors")}:${anchorsJson(packet.anchors)},${quote("semantic_graph_digest")}:${quote(packet.digests.semantic_graph)},${quote("transcript_digest")}:${quote(packet.digests.transcript)}}`;
}

function packetJson(packet, includePacketDigest = true) {
  return `{${quote("schema")}:${quote(packet.schema)},${quote("packet_id")}:${quote(packet.packet_id)},${quote("producer")}:${producerJson(packet.producer)},${quote("claim")}:${claimJson(packet.claim)},${quote("anchors")}:${anchorsJson(packet.anchors)},${quote("transcript")}:${transcriptJson(packet.transcript)},${quote("semantic_graph")}:${semanticGraphJson(packet.semantic_graph)},${quote("visualization")}:${hintsJson(packet.visualization)},${quote("summaries")}:${summariesJson(packet.summaries)},${quote("redactions")}:${redactionsJson(packet.redactions)},${quote("signatures")}:${signaturesJson(packet.signatures)},${quote("digests")}:${digestsJson(packet.digests, includePacketDigest)}}`;
}

async function transcriptDigest(seedCommitment, rounds) {
  const chunks = [];
  absorb(chunks, utf8(seedCommitment));
  absorb(chunks, u64(rounds.length));
  for (const round of rounds) {
    absorb(chunks, u64(round.round));
    absorb(chunks, utf8(round.component));
    absorb(chunks, utf8(round.note));
    absorb(chunks, utf8(round.payload_sha256));
  }
  return digestWithDomain(domains.transcript, concatAll(chunks));
}

function concatAll(chunks) {
  const size = chunks.reduce((sum, chunk) => sum + chunk.length, 0);
  const out = new Uint8Array(size);
  let offset = 0;
  for (const chunk of chunks) {
    out.set(chunk, offset);
    offset += chunk.length;
  }
  return out;
}

async function buildSample(name) {
  const spec = samples[name];
  const rounds = [];
  for (const [round, payload, component, note] of spec.rounds) {
    rounds.push({
      round,
      component,
      note,
      payload_sha256: await digestWithDomain(domains.payload, utf8(payload)),
    });
  }
  const packet = {
    schema: "slbit/viz-packet/v2",
    packet_id: "",
    producer: {
      environment: spec.producer[2],
      metadata: {},
      name: spec.producer[0],
      version: spec.producer[1],
    },
    claim: {
      id: spec.claim[0],
      bit_width: spec.claim[1],
      viz_hints: spec.visualization,
    },
    anchors: spec.anchors,
    transcript: rounds,
    semantic_graph: {
      nodes: spec.nodes.map(([id, kind, label]) => ({ id, kind, label })),
      edges: spec.edges.map(([from, to, kind]) => ({ from, to, kind })),
    },
    visualization: spec.visualization,
    summaries: spec.summaries,
    redactions: [],
    signatures: [],
    digests: {
      seed_commitment: await digestWithDomain(domains.seed, utf8(spec.seed)),
      transcript: "",
      semantic_graph: "",
      packet: "",
    },
  };
  packet.digests.transcript = await transcriptDigest(packet.digests.seed_commitment, packet.transcript);
  packet.digests.semantic_graph = await digestWithDomain(domains.graph, semanticGraphJson(packet.semantic_graph));
  packet.packet_id = await digestWithDomain(domains.packetId, identityJson(packet));
  packet.digests.packet = await digestWithDomain(domains.packet, packetJson(packet, false));
  return packet;
}

function validateGraph(graph) {
  const ids = new Set();
  const gates = [];
  for (const node of graph.nodes || []) {
    if (ids.has(node.id)) gates.push(["Unique graph nodes", false, node.id]);
    ids.add(node.id);
  }
  if (!gates.length) gates.push(["Unique graph nodes", true, `${ids.size} nodes`]);
  const adjacency = new Map();
  const edgeIds = new Set();
  for (const edge of graph.edges || []) {
    const edgeId = `${edge.from}\0${edge.to}\0${edge.kind}`;
    if (edgeIds.has(edgeId)) gates.push(["Unique graph edges", false, edge.kind]);
    edgeIds.add(edgeId);
    if (!ids.has(edge.from) || !ids.has(edge.to)) gates.push(["Graph references", false, `${edge.from} -> ${edge.to}`]);
    if (!adjacency.has(edge.from)) adjacency.set(edge.from, []);
    adjacency.get(edge.from).push(edge.to);
  }
  if (!gates.some(([name]) => name === "Unique graph edges")) gates.push(["Unique graph edges", true, `${edgeIds.size} edges`]);
  if (!gates.some(([name]) => name === "Graph references")) gates.push(["Graph references", true, "all endpoints resolved"]);
  const temporary = new Set();
  const permanent = new Set();
  const visit = (node) => {
    if (permanent.has(node)) return true;
    if (temporary.has(node)) return false;
    temporary.add(node);
    for (const child of adjacency.get(node) || []) {
      if (!visit(child)) return false;
    }
    temporary.delete(node);
    permanent.add(node);
    return true;
  };
  const acyclic = [...ids].every(visit);
  gates.push(["Acyclic semantic graph", acyclic, acyclic ? "DAG accepted" : "cycle detected"]);
  return gates;
}

function isSha(value) {
  return /^sha256:[0-9a-f]{64}$/.test(String(value));
}

async function verifyPacket(packet) {
  const gates = [];
  gates.push(["Schema", packet.schema === "slbit/viz-packet/v2", packet.schema || "missing"]);
  gates.push(["Packet ID shape", isSha(packet.packet_id), short(packet.packet_id)]);
  gates.push(["Digest shapes", ["seed_commitment", "transcript", "semantic_graph", "packet"].every((key) => isSha(packet.digests?.[key])), "sha256 fields"]);
  gates.push(...validateGraph(packet.semantic_graph || {}));
  const expectedTranscript = await transcriptDigest(packet.digests.seed_commitment, packet.transcript || []);
  gates.push(["Transcript digest", expectedTranscript === packet.digests.transcript, short(expectedTranscript)]);
  const expectedGraph = await digestWithDomain(domains.graph, semanticGraphJson(packet.semantic_graph || {}));
  gates.push(["Semantic graph digest", expectedGraph === packet.digests.semantic_graph, short(expectedGraph)]);
  const expectedId = await digestWithDomain(domains.packetId, identityJson(packet));
  gates.push(["Packet identity", expectedId === packet.packet_id, short(expectedId)]);
  const expectedPacket = await digestWithDomain(domains.packet, packetJson(packet, false));
  gates.push(["Packet digest", expectedPacket === packet.digests.packet, short(expectedPacket)]);
  return gates;
}

function short(value) {
  if (!value) return "--";
  return String(value).replace("sha256:", "").slice(0, 16).toUpperCase();
}

function setText(id, value) {
  el[id].textContent = value;
}

function toast(message) {
  el.toast.textContent = message;
  el.toast.classList.add("show");
  clearTimeout(toast.timer);
  toast.timer = setTimeout(() => el.toast.classList.remove("show"), 2600);
}

function renderGates(gates) {
  el["gate-stack"].replaceChildren(...gates.map(([name, ok, detail]) => {
    const row = document.createElement("div");
    row.className = `gate ${ok ? "ok" : "fail"}`;
    const dot = document.createElement("i");
    const label = document.createElement("span");
    const value = document.createElement("b");
    label.textContent = name;
    value.textContent = detail;
    row.append(dot, label, value);
    return row;
  }));
}

function gatesPassed(gates) {
  return gates.every(([, ok]) => ok);
}

function countBy(items, selector) {
  const counts = new Map();
  for (const item of items || []) {
    const key = selector(item);
    counts.set(key, (counts.get(key) || 0) + 1);
  }
  return counts;
}

function shortestDependencyPath(graph = {}) {
  const nodes = graph.nodes || [];
  const edges = graph.edges || [];
  if (nodes.length < 2) return nodes.map((node) => node.id);
  const incoming = new Set(edges.map((edge) => edge.to));
  const outgoing = new Set(edges.map((edge) => edge.from));
  const start = nodes.find((node) => !incoming.has(node.id))?.id || nodes[0].id;
  const end = [...nodes].reverse().find((node) => !outgoing.has(node.id))?.id || nodes[nodes.length - 1].id;
  const queue = [[start]];
  const seen = new Set([start]);
  while (queue.length) {
    const path = queue.shift();
    const last = path[path.length - 1];
    if (last === end) return path;
    for (const edge of edges.filter((item) => item.from === last)) {
      if (seen.has(edge.to)) continue;
      seen.add(edge.to);
      queue.push([...path, edge.to]);
    }
  }
  return [start, end].filter(Boolean);
}

function inspectPacket(packet, gates) {
  const nodes = packet.semantic_graph?.nodes || [];
  const edges = packet.semantic_graph?.edges || [];
  const anchors = packet.anchors || [];
  const summaries = Object.values(packet.summaries || {});
  const nodeKinds = countBy(nodes, (node) => node.kind || "unknown");
  const anchorKinds = countBy(anchors, (anchor) => anchor.anchor_type || "external");
  const rootprintAnchors = anchors.filter((anchor) => String(anchor.anchor_type).includes("rootprint"));
  const generatedSummaries = summaries.filter((summary) => summary.generated).length;
  const signedSummaries = summaries.filter((summary) => summary.signed).length;
  const path = shortestDependencyPath(packet.semantic_graph || {});
  return {
    valid: gatesPassed(gates),
    schema: packet.schema,
    packetId: packet.packet_id,
    rootprintAnchors,
    nodeKinds,
    anchorKinds,
    transcriptRounds: packet.transcript?.length || 0,
    semanticNodes: nodes.length,
    semanticEdges: edges.length,
    anchors: anchors.length,
    generatedSummaries,
    signedSummaries,
    dependencyPath: path,
    semanticChangesAffectCore: false,
    digest: packet.digests?.packet || "",
  };
}

function authorityCards(report) {
  const graphKinds = [...report.nodeKinds.entries()]
    .sort(([a], [b]) => a.localeCompare(b))
    .slice(0, 3)
    .map(([kind, count]) => `${kind}:${count}`)
    .join(" / ");
  const anchorKinds = [...report.anchorKinds.entries()]
    .sort(([a], [b]) => a.localeCompare(b))
    .slice(0, 2)
    .map(([kind, count]) => `${kind}:${count}`)
    .join(" / ");
  return [
    ["CORE ANCHORS", String(report.rootprintAnchors.length), anchorKinds || "no external anchors"],
    ["SEMANTIC NODES", String(report.semanticNodes), graphKinds || "no graph nodes"],
    ["TRANSCRIPT", String(report.transcriptRounds), `${report.semanticEdges} graph edges`],
    ["GENERATED TEXT", String(report.generatedSummaries), `${report.signedSummaries} signed summaries`],
  ];
}

function askAnswers(packet, report) {
  const path = report.dependencyPath.length ? report.dependencyPath.join(" -> ") : "no dependency path published";
  const rootprint = report.rootprintAnchors[0];
  const rootprintText = rootprint
    ? `${rootprint.label} / ${rootprint.metadata?.branch_id || rootprint.reference || "branch binding"}`
    : "No Rootprint anchor is present in this packet.";
  return [
    [
      "WHAT DID IT PROVE?",
      "SLBIT locally verifies semantic packet integrity, transcript digest consistency, graph identity, and packet identity. External proof validity remains with the bound proof system.",
    ],
    [
      "WHAT IS CORE?",
      rootprintText,
    ],
    [
      "WHAT IS SEMANTIC?",
      `${report.semanticNodes} semantic nodes and ${report.transcriptRounds} transcript rounds explain the claim without becoming proof identity.`,
    ],
    [
      "WHAT DEPENDS ON WHAT?",
      path,
    ],
    [
      "WHAT CAN CHANGE?",
      "Presentation text, summaries, and graph labels may change only by producing a new packet digest; core proof identity remains unchanged.",
    ],
    [
      "FAILURE BOUNDARY",
      report.valid
        ? "All local packet gates passed. A semantic mutation would reject at the semantic packet layer, not silently alter core proof validity."
        : "At least one local packet gate failed. Treat the semantic packet as rejected until the failed gate is corrected.",
    ],
  ];
}

function renderMeaningConsole(packet, gates) {
  const report = inspectPacket(packet, gates);
  setText("boundary-state", report.valid ? "SEMANTIC VALID / CORE UNCHANGED" : "SEMANTIC REJECTED / CORE UNCHANGED");
  setText(
    "boundary-detail",
    `${report.schema} inspected locally. Semantic changes affect this packet digest and sidecar binding, but they do not rewrite proof validity, Rootprint lineage, replay fingerprints, or core identity.`,
  );
  el["authority-strip"].replaceChildren(...authorityCards(report).map(([label, value, detail]) => {
    const card = document.createElement("article");
    card.className = "authority-card";
    const span = document.createElement("span");
    const strong = document.createElement("b");
    const small = document.createElement("small");
    span.textContent = label;
    strong.textContent = value;
    small.textContent = detail;
    card.append(span, strong, small);
    return card;
  }));
  el["ask-grid"].replaceChildren(...askAnswers(packet, report).map(([question, answer]) => {
    const card = document.createElement("article");
    card.className = "ask-card";
    const span = document.createElement("span");
    const strong = document.createElement("b");
    const paragraph = document.createElement("p");
    span.textContent = "DETERMINISTIC ASK";
    strong.textContent = question;
    paragraph.textContent = answer;
    card.append(span, strong, paragraph);
    return card;
  }));
}

function renderGraph(packet) {
  const svg = el["semantic-graph"];
  svg.replaceChildren();
  const nodes = packet.semantic_graph.nodes || [];
  const edges = packet.semantic_graph.edges || [];
  setText("graph-count", `${nodes.length} NODES / ${edges.length} EDGES`);
  if (!selectedNode && nodes[0]) selectedNode = nodes[0].id;
  const levels = graphLevels(nodes, edges);
  const grouped = new Map();
  for (const node of nodes) {
    const level = levels.get(node.id) || 0;
    if (!grouped.has(level)) grouped.set(level, []);
    grouped.get(level).push(node);
  }
  const maxLevel = Math.max(1, ...grouped.keys());
  const positions = new Map();
  for (const [level, group] of grouped) {
    group.sort((a, b) => String(a.id).localeCompare(String(b.id)));
    group.forEach((node, index) => {
      const x = 44 + (level / maxLevel) * 620;
      const y = 40 + ((index + 1) / (group.length + 1)) * 340;
      positions.set(node.id, { x, y });
    });
  }
  const defs = document.createElementNS("http://www.w3.org/2000/svg", "defs");
  defs.innerHTML = `
    <linearGradient id="edge-gradient" x1="0%" y1="0%" x2="100%" y2="0%">
      <stop offset="0%" stop-color="#3ee4d5" stop-opacity=".2"/>
      <stop offset="52%" stop-color="#baff35" stop-opacity=".78"/>
      <stop offset="100%" stop-color="#3ee4d5" stop-opacity=".2"/>
    </linearGradient>
    <radialGradient id="node-halo" cx="50%" cy="50%" r="70%">
      <stop offset="0%" stop-color="#baff35" stop-opacity=".35"/>
      <stop offset="100%" stop-color="#baff35" stop-opacity="0"/>
    </radialGradient>
    <pattern id="graph-grid" width="42" height="42" patternUnits="userSpaceOnUse">
      <path d="M 42 0 L 0 0 0 42" fill="none" stroke="#3ee4d5" stroke-opacity=".07" stroke-width="1"/>
    </pattern>
    <filter id="graph-glow" x="-40%" y="-80%" width="180%" height="260%">
      <feGaussianBlur stdDeviation="3.5" result="blur"/>
      <feMerge><feMergeNode in="blur"/><feMergeNode in="SourceGraphic"/></feMerge>
    </filter>`;
  const background = document.createElementNS("http://www.w3.org/2000/svg", "rect");
  background.setAttribute("class", "graph-backplane");
  background.setAttribute("width", "820");
  background.setAttribute("height", "460");
  background.setAttribute("fill", "url(#graph-grid)");
  svg.append(defs, background);
  edges.forEach((edge, index) => {
    const from = positions.get(edge.from);
    const to = positions.get(edge.to);
    if (!from || !to) return;
    const startX = from.x + 150;
    const startY = from.y + 26;
    const endX = to.x;
    const endY = to.y + 26;
    const curve = Math.max(44, Math.abs(endX - startX) * 0.42);
    const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
    path.setAttribute("class", "graph-edge");
    path.setAttribute("d", `M ${startX} ${startY} C ${startX + curve} ${startY}, ${endX - curve} ${endY}, ${endX} ${endY}`);
    svg.append(path);
    const pulse = document.createElementNS("http://www.w3.org/2000/svg", "circle");
    pulse.setAttribute("class", "graph-pulse");
    pulse.setAttribute("r", "3.4");
    const motion = document.createElementNS("http://www.w3.org/2000/svg", "animateMotion");
    motion.setAttribute("dur", `${4.5 + (index % 4) * 0.4}s`);
    motion.setAttribute("repeatCount", "indefinite");
    motion.setAttribute("path", path.getAttribute("d"));
    pulse.append(motion);
    svg.append(pulse);
  });
  for (const node of nodes) {
    const position = positions.get(node.id);
    const group = document.createElementNS("http://www.w3.org/2000/svg", "g");
    group.setAttribute("class", `graph-node ${node.id === selectedNode ? "selected" : ""}`);
    group.setAttribute("transform", `translate(${position.x} ${position.y})`);
    group.addEventListener("click", () => {
      selectedNode = node.id;
      renderGraph(packet);
      renderInspector(packet);
    });
    const halo = document.createElementNS("http://www.w3.org/2000/svg", "ellipse");
    halo.setAttribute("class", "node-halo");
    halo.setAttribute("cx", "75");
    halo.setAttribute("cy", "26");
    halo.setAttribute("rx", "96");
    halo.setAttribute("ry", "44");
    const rect = document.createElementNS("http://www.w3.org/2000/svg", "rect");
    rect.setAttribute("width", "150");
    rect.setAttribute("height", "52");
    const kind = document.createElementNS("http://www.w3.org/2000/svg", "text");
    kind.setAttribute("x", "12");
    kind.setAttribute("y", "18");
    kind.setAttribute("class", "kind");
    kind.textContent = String(node.kind || "").toUpperCase();
    const label = document.createElementNS("http://www.w3.org/2000/svg", "text");
    label.setAttribute("x", "12");
    label.setAttribute("y", "37");
    label.textContent = clip(node.label || node.id, 18);
    group.append(halo, rect, kind, label);
    svg.append(group);
  }
}

function graphLevels(nodes, edges) {
  const levels = new Map(nodes.map((node) => [node.id, 0]));
  for (let pass = 0; pass < nodes.length; pass += 1) {
    let changed = false;
    for (const edge of edges) {
      const next = Math.max(levels.get(edge.to) || 0, (levels.get(edge.from) || 0) + 1);
      if (next !== levels.get(edge.to)) {
        levels.set(edge.to, next);
        changed = true;
      }
    }
    if (!changed) break;
  }
  return levels;
}

function clip(value, max) {
  value = String(value);
  return value.length > max ? `${value.slice(0, max - 1)}...` : value;
}

function renderInspector(packet) {
  const node = (packet.semantic_graph.nodes || []).find((item) => item.id === selectedNode);
  setText("claim-kind", node ? String(node.kind).toUpperCase() : "CLAIM");
  setText("claim-title", node?.label || node?.id || packet.claim.id);
  setText("claim-subtitle", `${packet.claim.id} / ${packet.producer.name} ${packet.producer.version}`);
  const audiences = Object.keys(packet.summaries || {}).sort();
  if (!currentAudience || !audiences.includes(currentAudience)) currentAudience = audiences[0] || "";
  el["summary-tabs"].replaceChildren(...audiences.map((audience) => {
    const button = document.createElement("button");
    button.type = "button";
    button.className = audience === currentAudience ? "active" : "";
    button.textContent = audience.toUpperCase();
    button.addEventListener("click", () => {
      currentAudience = audience;
      renderInspector(packet);
    });
    return button;
  }));
  const summary = packet.summaries?.[currentAudience];
  el["summary-text"].textContent = summary?.text || "No audience summary published.";
  el["anchor-list"].replaceChildren(...sortedAnchors(packet.anchors).map((anchor) => {
    const card = document.createElement("div");
    card.className = "anchor-card";
    const kind = document.createElement("span");
    const label = document.createElement("b");
    const reference = document.createElement("code");
    kind.textContent = anchor.anchor_type;
    label.textContent = anchor.label;
    reference.textContent = anchor.reference || anchor.digest || "REFERENCELESS ANCHOR";
    card.append(kind, label, reference);
    return card;
  }));
}

function renderTimeline(packet) {
  setText("round-count", `${packet.transcript.length} ROUNDS`);
  el.timeline.replaceChildren(...packet.transcript.map((round, index) => {
    const button = document.createElement("button");
    button.type = "button";
    button.className = index === 0 ? "active" : "";
    const label = document.createElement("span");
    const component = document.createElement("b");
    const note = document.createElement("p");
    label.textContent = `ROUND ${round.round}`;
    component.textContent = round.component;
    note.textContent = round.note;
    button.append(label, component, note);
    button.addEventListener("click", () => {
      el.timeline.querySelectorAll("button").forEach((item) => item.classList.remove("active"));
      button.classList.add("active");
      const match = (packet.semantic_graph.nodes || []).find((node) => node.id === round.component || node.label === round.component);
      if (match) {
        selectedNode = match.id;
        renderGraph(packet);
        renderInspector(packet);
      }
    });
    return button;
  }));
}

function renderJson(packet) {
  currentJson = packetJson(packet, true);
  const pretty = JSON.stringify(JSON.parse(currentJson), null, 2);
  el["json-preview"].textContent = pretty;
  setText("json-size", `${enc.encode(currentJson).length.toLocaleString("en-US")} B`);
}

function toMarkdown(packet) {
  const summary = packet.summaries?.[currentAudience] || Object.values(packet.summaries || {})[0];
  const report = inspectPacket(packet, [["export", true, ""]]);
  const lines = [
    "# SLBIT Meaning Observatory Packet",
    "",
    `- Schema: \`${packet.schema}\``,
    `- Packet ID: \`${packet.packet_id}\``,
    `- Claim: \`${packet.claim.id}\``,
    `- Packet digest: \`${packet.digests.packet}\``,
    `- Transcript digest: \`${packet.digests.transcript}\``,
    `- Semantic graph digest: \`${packet.digests.semantic_graph}\``,
    `- Semantic changes affect core identity: \`${report.semanticChangesAffectCore}\``,
    "",
    "## Truth Boundary",
    "",
    "SLBIT verifies semantic packet integrity and explains bound proof state. It does not change proof validity, Rootprint lineage, replay fingerprints, or core identity.",
    "",
    "## Summary",
    "",
    summary?.text || "No summary published.",
    "",
    "## Transcript",
    "",
    ...packet.transcript.map((round) => `- Round ${round.round} \`${round.component}\`: ${round.note}`),
    "",
    "## Anchors",
    "",
    ...sortedAnchors(packet.anchors).map((anchor) => `- \`${anchor.anchor_type}\`: ${anchor.label}${anchor.reference ? ` (\`${anchor.reference}\`)` : ""}`),
  ];
  return `${lines.join("\n")}\n`;
}

function renderPacket(packet, gates) {
  currentPacket = packet;
  setText("packet-id", packet.packet_id);
  setText("packet-digest", packet.digests.packet);
  setText("transcript-digest", packet.digests.transcript);
  setText("graph-digest", packet.digests.semantic_graph);
  const ok = gates.every(([, gate]) => gate);
  setText("verify-state", ok ? "VERIFIED" : "REJECTED");
  el["verify-state"].style.color = ok ? "var(--acid)" : "var(--coral)";
  renderGates(gates);
  renderGraph(packet);
  renderInspector(packet);
  renderTimeline(packet);
  renderMeaningConsole(packet, gates);
  renderJson(packet);
}

async function loadPacket(packet, message = "Packet loaded") {
  try {
    const gates = await verifyPacket(packet);
    renderPacket(packet, gates);
    toast(message);
  } catch (error) {
    toast(`Packet rejected: ${error.message}`);
  }
}

async function loadSample(name) {
  selectedNode = "";
  currentAudience = "";
  const packet = await buildSample(name);
  await loadPacket(packet, `${name.toUpperCase()} packet verified locally`);
}

async function readFile(file) {
  const text = await file.text();
  await loadPacket(JSON.parse(text), `${file.name} loaded`);
}

function download(name, body, type = "text/plain") {
  const blob = new Blob([body], { type });
  const href = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = href;
  link.download = name;
  link.click();
  URL.revokeObjectURL(href);
}

function bind() {
  document.getElementById("sample-drone").addEventListener("click", () => loadSample("drone"));
  document.getElementById("sample-agent").addEventListener("click", () => loadSample("agent"));
  document.getElementById("sample-zkml").addEventListener("click", () => loadSample("zkml"));
  document.getElementById("sample-finance").addEventListener("click", () => loadSample("finance"));
  el["packet-file"].addEventListener("change", (event) => {
    const file = event.target.files?.[0];
    if (file) readFile(file);
  });
  el["export-markdown"].addEventListener("click", () => {
    if (!currentPacket) return;
    download(`${currentPacket.claim.id}.md`, toMarkdown(currentPacket), "text/markdown");
  });
  el["copy-llm"].addEventListener("click", async () => {
    if (!currentPacket) return;
    const summary = currentPacket.summaries?.llm_context?.text || toMarkdown(currentPacket);
    await navigator.clipboard.writeText(summary);
    toast("LLM context copied");
  });
  window.addEventListener("dragenter", () => document.body.classList.add("dragging"));
  window.addEventListener("dragover", (event) => event.preventDefault());
  window.addEventListener("dragleave", (event) => {
    if (event.clientX <= 0 || event.clientY <= 0) document.body.classList.remove("dragging");
  });
  window.addEventListener("drop", (event) => {
    event.preventDefault();
    document.body.classList.remove("dragging");
    const file = event.dataTransfer.files?.[0];
    if (file) readFile(file);
  });
}

bind();
loadSample("drone");
