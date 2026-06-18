const state = {
  kind: "observer",
  lastPackage: "",
};

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
  const metricsPort = el.metricsPort.value.trim() || "9100";
  const keyPath = el.keyPath.value.trim() || "$HOME/.powerhouse/node.key";
  const output = `${nodeId}.${kind}.registration.json`;
  const lines = [
    `julian ${kind}-registry register \\`,
    `  --node-id ${shell(nodeId)} \\`,
    `  --public-host ${shell(host)} \\`,
    `  --key ${shell(keyPath)} \\`,
  ];
  if (operator) {
    lines.push(`  --operator ${shell(operator)} \\`);
  }
  lines.push(
    `  --region ${shell(region)} \\`,
    `  --p2p-port ${shell(p2pPort)} \\`,
    `  --metrics-port ${shell(metricsPort)} \\`
  );
  if (kind === "validator") {
    lines.push(`  --system-metrics-port 9101 \\`);
  }
  lines.push(`  --output ${shell(output)}`);
  el.command.textContent = lines.join("\n");
}

function setKind(kind) {
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

  const submission = {
    schema: "mfenx-node-registration-submission-v1",
    created_at: new Date().toISOString(),
    registration_type: analysis.type,
    client_side_status: status.toLowerCase(),
    client_side_errors: analysis.errors,
    client_side_warnings: analysis.warnings,
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

async function readRegistration(file) {
  try {
    const text = await file.text();
    const data = JSON.parse(text);
    const analysis = analyzeRegistration(data);
    renderAnalysis(data, analysis);
  } catch (error) {
    el.fileState.textContent = "ERROR";
    el.report.innerHTML = card("STATUS", `INVALID JSON: ${error.message}`, "error");
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
