const state = {
  kind: "observer",
  lastPackage: "",
  lastData: null,
  lastAnalysis: null,
  lastProbe: null,
  probeRun: 0,
};

const OBSERVER_BOOTSTRAPS = [
  "/ip4/159.203.109.128/tcp/7002/p2p/12D3KooWMCyR9gXPXCGAMNCVJDKbisohRRq8oaTHNiR91HZ67cSR",
  "/ip4/64.23.182.213/tcp/7002/p2p/12D3KooWGEHbPAQ9ZVB9Uqg1j8CnsNqKvS2xmAe5cmT4w3idUtmQ",
  "/ip4/164.92.150.22/tcp/7002/p2p/12D3KooWFNv4sZfDKypMeWqRetghHxXzkhPTc4PvynDZKSETJqd8",
];

const el = {
  kindButtons: [...document.querySelectorAll("[data-kind]")],
  nodeId: document.querySelector("#node-id"),
  publicHost: document.querySelector("#public-host"),
  operator: document.querySelector("#operator"),
  region: document.querySelector("#region"),
  p2pPort: document.querySelector("#p2p-port"),
  metricsPort: document.querySelector("#metrics-port"),
  keyPath: document.querySelector("#key-path"),
  command: document.querySelector("#register-command"),
  copyCommand: document.querySelector("#copy-command"),
  fileInput: document.querySelector("#registration-file"),
  dropZone: document.querySelector("#drop-zone"),
  fileState: document.querySelector("#file-state"),
  report: document.querySelector("#registration-report"),
  package: document.querySelector("#submission-package"),
  copyPackage: document.querySelector("#copy-package"),
  githubSubmit: document.querySelector("#github-submit"),
  probeState: document.querySelector("#probe-state"),
  probeReport: document.querySelector("#probe-report"),
};

function shell(value) {
  const raw = String(value || "").trim();
  if (raw === "$HOME/.powerhouse/node.key") {
    return '"$HOME/.powerhouse/node.key"';
  }
  if (/^[A-Za-z0-9._:/@+-]+$/.test(raw)) {
    return raw || "VALUE";
  }
  return `'${raw.replace(/'/g, "'\\''")}'`;
}

function renderCommand() {
  const kind = state.kind;
  const nodeId = el.nodeId.value.trim() || "mynode";
  const host = el.publicHost.value.trim() || "<public-ip-or-dns>";
  const operator = el.operator.value.trim();
  const region = el.region.value.trim() || "self-hosted";
  const p2pPort = el.p2pPort.value.trim() || "7001";
  const metricsPort = el.metricsPort.value.trim() || "9102";
  const keyPath = el.keyPath.value.trim() || "$HOME/.powerhouse/node.key";
  const output = `${nodeId}.${kind}.registration.json`;
  const common = [
    `  --node-id ${shell(nodeId)} \\`,
    `  --public-host ${shell(host)} \\`,
    `  --key ${shell(keyPath)} \\`,
  ];
  if (operator) common.push(`  --operator ${shell(operator)} \\`);
  common.push(
    `  --region ${shell(region)} \\`,
    `  --p2p-port ${shell(p2pPort)} \\`,
    `  --metrics-port ${shell(metricsPort)} \\`
  );

  if (kind === "observer") {
    const setup = [
      "julian observer setup \\",
      ...common,
      `  --output ${shell(output)}`,
    ].join("\n");
    const start = [
      "julian net start \\",
      `  --node-id ${shell(nodeId)} \\`,
      `  --log-dir ./logs/${shell(`${nodeId}-observer`)} \\`,
      `  --blob-dir ./data/${shell(`${nodeId}-observer`)} \\`,
      `  --listen /ip4/0.0.0.0/tcp/${shell(p2pPort)} \\`,
      ...OBSERVER_BOOTSTRAPS.map((addr) => `  --bootstrap ${shell(addr)} \\`),
      `  --key ${shell(keyPath)} \\`,
      `  --metrics 0.0.0.0:${shell(metricsPort)}`,
    ].join("\n");
    const doctor = [
      "julian observer doctor \\",
      ...common.slice(0, -1),
      `  --metrics-port ${shell(metricsPort)}`,
    ].join("\n");
    const submit = `julian observer submit ${shell(output)}`;
    el.command.textContent = `${setup}\n\n${start}\n\n${doctor}\n\n${submit}`;
    return;
  }

  const lines = [
    "julian validator-registry register \\",
    ...common,
    "  --system-metrics-port 9101 \\",
    `  --output ${shell(output)}`,
  ];
  el.command.textContent = lines.join("\n");
}

function setKind(kind) {
  if (kind !== state.kind) {
    const metricsValue = el.metricsPort.value.trim();
    if (kind === "validator" && metricsValue === "9102") el.metricsPort.value = "9100";
    if (kind === "observer" && metricsValue === "9100") el.metricsPort.value = "9102";
  }
  state.kind = kind;
  document.body.dataset.kind = kind;
  el.kindButtons.forEach((button) => {
    button.classList.toggle("active", button.dataset.kind === kind);
  });
  renderCommand();
}

function card(label, value, mode = "") {
  return `<div class="report-card ${mode}"><span>${escapeHtml(label)}</span><b>${escapeHtml(value)}</b></div>`;
}

function escapeHtml(value) {
  return String(value ?? "")
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

function p2pHost(address) {
  const match = String(address || "").match(/\/(?:ip4|ip6|dns|dns4|dns6)\/([^/]+)/);
  return match ? match[1].toLowerCase() : "";
}

function urlHost(url) {
  try {
    return new URL(url).hostname.toLowerCase();
  } catch (_) {
    return "";
  }
}

function p2pPort(address) {
  const match = String(address || "").match(/\/tcp\/([0-9]+)/);
  return match ? Number(match[1]) : 7001;
}

function metricsPort(url) {
  try {
    const parsed = new URL(url);
    if (parsed.port) return Number(parsed.port);
    return parsed.protocol === "https:" ? 443 : 80;
  } catch (_) {
    return 9102;
  }
}

function privateHost(host) {
  const value = String(host || "").toLowerCase();
  if (!value || value === "localhost" || value === "::1") return true;
  if (value.startsWith("127.") || value.startsWith("10.") || value.startsWith("192.168.")) {
    return true;
  }
  const parts = value.split(".").map((part) => Number(part));
  if (parts.length === 4 && parts.every((part) => Number.isInteger(part))) {
    return parts[0] === 172 && parts[1] >= 16 && parts[1] <= 31;
  }
  return value.startsWith("fc") || value.startsWith("fd");
}

function analyzeRegistration(data) {
  const errors = [];
  const warnings = [];
  const schema = String(data.schema || "");
  const type = schema.includes("observer")
    ? "observer"
    : schema.includes("validator")
      ? "validator"
      : "unknown";
  const required = [
    "schema",
    "chain_id",
    "node_id",
    "operator",
    "region",
    "peer_id",
    "public_key_b64",
    "p2p_address",
    "metrics_url",
    "issued_at_unix",
    "valid_until_unix",
    "signature_b64",
  ];
  required.forEach((field) => {
    if (data[field] === undefined || data[field] === null || data[field] === "") {
      errors.push(`missing ${field}`);
    }
  });
  if (type === "unknown") errors.push("unsupported registration schema");
  if (Number(data.chain_id) !== 177155) warnings.push("chain_id is not 177155");
  if (!String(data.peer_id || "").startsWith("12D3KooW")) warnings.push("peer_id format is unexpected");
  if (String(data.signature_b64 || "").length < 32) errors.push("signature is missing or too short");
  if (String(data.public_key_b64 || "").length < 32) errors.push("public key is missing or too short");

  const hostFromP2p = p2pHost(data.p2p_address);
  const hostFromMetrics = urlHost(data.metrics_url);
  if (!hostFromP2p) errors.push("p2p address has no host");
  if (!hostFromMetrics) errors.push("metrics URL is invalid");
  if (hostFromP2p && hostFromMetrics && hostFromP2p !== hostFromMetrics) {
    errors.push("p2p and metrics hosts differ");
  }
  if (privateHost(hostFromMetrics)) {
    warnings.push("metrics host is private or local");
  }
  const now = Math.floor(Date.now() / 1000);
  if (Number(data.valid_until_unix) <= now) errors.push("registration is expired");
  if (Number(data.issued_at_unix) > now + 300) errors.push("issued_at is in the future");

  return { type, errors, warnings, hostFromP2p, hostFromMetrics };
}

function renderAnalysis(data, analysis) {
  const status = analysis.errors.length
    ? "ERROR"
    : analysis.warnings.length
      ? "REVIEW"
      : "READY";
  el.fileState.textContent = status;
  const mode = analysis.errors.length ? "error" : analysis.warnings.length ? "warn" : "ok";
  const cards = [
    card("STATUS", status, mode),
    card("TYPE", analysis.type.toUpperCase(), analysis.type === "unknown" ? "error" : "ok"),
    card("NODE", data.node_id || "MISSING", data.node_id ? "" : "error"),
    card("PEER", data.peer_id || "MISSING", data.peer_id ? "" : "error"),
    card("P2P HOST", analysis.hostFromP2p || "MISSING", analysis.hostFromP2p ? "" : "error"),
    card("METRICS HOST", analysis.hostFromMetrics || "MISSING", analysis.hostFromMetrics ? "" : "error"),
    card("SIGNATURE", data.signature_b64 ? "PRESENT" : "MISSING", data.signature_b64 ? "ok" : "error"),
    card(
      "NOTES",
      [...analysis.errors, ...analysis.warnings].join(" / ") || "NO CLIENT-SIDE ISSUES",
      mode
    ),
  ];
  el.report.innerHTML = cards.join("");
  state.lastData = data;
  state.lastAnalysis = analysis;
  updateSubmission(data, analysis, null);
}

function updateSubmission(data, analysis, probe) {
  const status = analysis.errors.length
    ? "ERROR"
    : analysis.warnings.length
      ? "REVIEW"
      : "READY";
  const submission = {
    schema: "mfenx-node-registration-submission-v1",
    created_at: new Date().toISOString(),
    registration_type: analysis.type,
    client_side_status: status.toLowerCase(),
    client_side_errors: analysis.errors,
    client_side_warnings: analysis.warnings,
    external_probe: probe,
    registration: data,
  };
  state.lastPackage = JSON.stringify(submission, null, 2);
  el.package.value = state.lastPackage;
  el.copyPackage.disabled = false;
  const title = encodeURIComponent(`Node registration: ${data.node_id || "unknown"}`);
  const body = encodeURIComponent(
    [
      `Registration type: ${analysis.type}`,
      `Node ID: ${data.node_id || "unknown"}`,
      `Peer ID: ${data.peer_id || "unknown"}`,
      `Client-side status: ${status}`,
      "",
      "Attach or paste the signed registration JSON generated by julian.",
      "Do not include a private key.",
    ].join("\n")
  );
  el.githubSubmit.href = `https://github.com/JROChub/power_house/issues/new?title=${title}&body=${body}`;
  el.githubSubmit.classList.remove("disabled");
}

function setProbe(status, text, mode = "") {
  el.probeState.textContent = status;
  el.probeState.className = mode;
  el.probeReport.textContent = text;
}

function probeSummary(probe) {
  if (probe.error) return probe.error;
  const target = probe.target || {};
  const metrics = probe.metrics || {};
  const p2p = probe.p2p || {};
  const lines = [
    `target: ${target.host || "unknown"} metrics:${target.metrics_port || "?"} p2p:${target.p2p_port || "?"}`,
    `metrics: ${metrics.reachable ? "reachable" : "blocked"} identity:${metrics.identity_found ? "found" : "missing"} peers:${metrics.connected_peers ?? 0}`,
    `p2p: ${p2p.reachable ? "reachable" : "blocked"}`,
  ];
  if (metrics.error) lines.push(`metrics error: ${metrics.error}`);
  if (p2p.error) lines.push(`p2p error: ${p2p.error}`);
  if (metrics.identity) {
    lines.push(`identity node: ${metrics.identity.node_id || "unknown"}`);
    lines.push(`identity peer: ${metrics.identity.peer_id || "unknown"}`);
  }
  return lines.join("\n");
}

async function runExternalProbe(data, analysis) {
  const run = ++state.probeRun;
  if (analysis.type !== "observer") {
    setProbe("SKIPPED", "The external reachability probe is only used for public observer registrations.");
    updateSubmission(data, analysis, null);
    return;
  }
  if (analysis.errors.length) {
    setProbe("SKIPPED", "Fix the signed registration errors before testing external reachability.");
    updateSubmission(data, analysis, null);
    return;
  }
  const host = analysis.hostFromMetrics || analysis.hostFromP2p;
  if (!host || privateHost(host)) {
    setProbe("SKIPPED", "Registration uses a private/local host. Use a public IPv4 or DNS name before submission.", "error");
    updateSubmission(data, analysis, null);
    return;
  }
  const params = new URLSearchParams({
    host,
    metrics_port: String(metricsPort(data.metrics_url)),
    p2p_port: String(p2pPort(data.p2p_address)),
  });
  setProbe("CHECKING", "Production is testing the public metrics and p2p ports now.");
  try {
    const response = await fetch(`https://rpc.mfenx.com/observer-probe?${params}`, {
      cache: "no-store",
    });
    const probe = await response.json();
    if (run !== state.probeRun) return;
    state.lastProbe = probe;
    setProbe(probe.ok ? "PASS" : "FIX", probeSummary(probe), probe.ok ? "ok" : "error");
    updateSubmission(data, analysis, probe);
  } catch (error) {
    if (run !== state.probeRun) return;
    const probe = { ok: false, error: error.message };
    state.lastProbe = probe;
    setProbe("RETRY", `External probe request failed: ${error.message}`, "error");
    updateSubmission(data, analysis, probe);
  }
}

async function readRegistration(file) {
  try {
    state.probeRun += 1;
    state.lastProbe = null;
    setProbe("WAITING", "Upload parsed. External reachability test will run after client-side checks.");
    const text = await file.text();
    const data = JSON.parse(text);
    const analysis = analyzeRegistration(data);
    renderAnalysis(data, analysis);
    runExternalProbe(data, analysis);
  } catch (error) {
    state.probeRun += 1;
    el.fileState.textContent = "ERROR";
    el.report.innerHTML = card("STATUS", `INVALID JSON: ${error.message}`, "error");
    setProbe("ERROR", "Upload must be valid signed registration JSON.", "error");
    el.package.value = "";
    el.copyPackage.disabled = true;
    el.githubSubmit.classList.add("disabled");
  }
}

async function copyText(value, button, label) {
  await navigator.clipboard.writeText(value);
  const previous = button.textContent;
  button.textContent = label;
  window.setTimeout(() => {
    button.textContent = previous;
  }, 1100);
}

el.kindButtons.forEach((button) => {
  button.addEventListener("click", () => setKind(button.dataset.kind));
});

[el.nodeId, el.publicHost, el.operator, el.region, el.p2pPort, el.metricsPort, el.keyPath].forEach((input) => {
  input.addEventListener("input", renderCommand);
});

el.copyCommand.addEventListener("click", () => {
  copyText(el.command.textContent, el.copyCommand, "COPIED");
});

el.copyPackage.addEventListener("click", () => {
  if (state.lastPackage) copyText(state.lastPackage, el.copyPackage, "COPIED");
});

el.fileInput.addEventListener("change", () => {
  const [file] = el.fileInput.files;
  if (file) readRegistration(file);
});

["dragenter", "dragover"].forEach((eventName) => {
  el.dropZone.addEventListener(eventName, (event) => {
    event.preventDefault();
    el.dropZone.classList.add("dragging");
  });
});

["dragleave", "drop"].forEach((eventName) => {
  el.dropZone.addEventListener(eventName, (event) => {
    event.preventDefault();
    el.dropZone.classList.remove("dragging");
  });
});

el.dropZone.addEventListener("drop", (event) => {
  const [file] = event.dataTransfer.files;
  if (file) readRegistration(file);
});

setKind("observer");
