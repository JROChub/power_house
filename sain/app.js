"use strict";

const $ = (selector, root = document) => root.querySelector(selector);
const conversation = $("#conversation");
const composer = $("#composer");
const question = $("#question");
const send = $("#send");
const evidenceLens = $("#evidenceLens");
const lensScrim = $("#lensScrim");
const remoteConfig = window.SAIN_REMOTE_CONFIG;
const apiBase = remoteConfig?.apiBase?.replace(/\/$/, "") || "";
let accessToken = remoteConfig ? sessionStorage.getItem("sainAccessToken") || "" : "";
let eventNumber = 1;
let activeReport = null;
let thinkingTimer = null;
let webResearchEnabled = false;

function endpoint(path) { return `${apiBase}${path}`; }

async function apiJSON(path, options = {}) {
  const headers = new Headers(options.headers || {});
  if (accessToken) headers.set("Authorization", `Bearer ${accessToken}`);
  const response = await fetch(endpoint(path), { ...options, headers, cache: "no-store" });
  let payload;
  try { payload = await response.json(); } catch { payload = { ok: false, error: `HTTP ${response.status}` }; }
  if (!response.ok || !payload.ok) {
    if (response.status === 401 && remoteConfig) lockObservatory("Session expired. Enter the access key again.");
    throw new Error(payload.error || `Request failed with HTTP ${response.status}`);
  }
  return payload;
}

function lockObservatory(message = "") {
  if (!remoteConfig) return;
  accessToken = "";
  sessionStorage.removeItem("sainAccessToken");
  document.documentElement.classList.remove("sain-authorized");
  $("#accessGate").hidden = false;
  $(".shell").setAttribute("aria-hidden", "true");
  $("#accessError").textContent = message;
  setTimeout(() => $("#accessPassword").focus(), 0);
}

function unlockObservatory(token) {
  accessToken = token;
  sessionStorage.setItem("sainAccessToken", token);
  document.documentElement.classList.add("sain-authorized");
  $("#accessGate").hidden = true;
  $(".shell").removeAttribute("aria-hidden");
}

function text(id, value) {
  const node = document.getElementById(id);
  if (node) node.textContent = value ?? "—";
}

function short(value, size = 14) {
  if (!value) return "—";
  const clean = String(value).replace(/^sha256:/, "");
  return clean.length > size ? `${clean.slice(0, size)}…` : clean;
}

function percent(value) {
  return typeof value === "number" ? `${(value * 100).toFixed(1)}%` : "—";
}

function stamp() {
  return new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" });
}

function toast(message) {
  const node = $("#toast");
  node.textContent = message;
  node.classList.add("show");
  clearTimeout(toast.timer);
  toast.timer = setTimeout(() => node.classList.remove("show"), 2600);
}

async function refreshStatus() {
  try {
    const payload = await apiJSON("/api/status");
    const state = payload.snapshot;
    text("identityShort", short(state.identity_id, 20));
    text("generation", state.generation);
    text("integrity", percent(state.integrity));
    text("memoryCount", state.memory_entries);
    text("causalCount", state.causal_models);
    text("energy", percent(state.energy));
    text("stress", Number(state.stress || 0).toExponential(1));
    text("capabilities", state.capabilities);
    text("genomes", state.cognitive_genomes);
    text("currentGoal", state.current_goal);
    text("connectionLabel", state.cortex_ready ? "LOCAL CORTEX READY" : "VERIFYING CORTEX");
    $("#signalDot").classList.toggle("live", state.cortex_ready);
    if (!state.cortex_ready) setTimeout(refreshStatus, 3000);
  } catch (error) {
    text("connectionLabel", "FIELD INTERRUPTED");
    $("#signalDot").classList.remove("live");
    toast(error.message);
  }
}

async function requestChat(prompt) {
  const options = {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ question: prompt, web: webResearchEnabled }),
  };
  if (!remoteConfig) return apiJSON("/api/chat", options);
  const started = await apiJSON("/api/chat/start", options);
  for (let attempt = 0; attempt < 300; attempt += 1) {
    await new Promise((resolve) => setTimeout(resolve, 3000));
    const status = await apiJSON(`/api/chat/jobs/${encodeURIComponent(started.job_id)}`);
    if (status.job.state === "complete") return status.job.result;
    if (status.job.state === "failed") throw new Error(status.job.error || "Sain's remote job failed");
  }
  throw new Error("Sain's remote job exceeded the 15-minute boundary");
}

function element(tag, className, content) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (content !== undefined) node.textContent = content;
  return node;
}

function proofStrip(labels) {
  const strip = element("div", "proof-strip");
  labels.forEach((label) => strip.append(element("span", "", label)));
  return strip;
}

function addEvent(kind, heading, body, labels = []) {
  const article = element("article", `event ${kind}-event`);
  const node = element("div", "event-node");
  node.append(element("i"));
  article.append(node);
  const eventBody = element("div", "event-body");
  const meta = element("div", "event-meta");
  meta.append(element("span", "", kind === "user" ? "HUMAN TRANSMISSION" : kind === "sain" ? "SAIN / ACCEPTED RESPONSE" : "FIELD EVENT"));
  meta.append(element("time", "", stamp()));
  eventBody.append(meta, element("h2", "", heading), element("p", "", body));
  if (labels.length) eventBody.append(proofStrip(labels));
  article.append(eventBody);
  conversation.append(article);
  eventNumber += 1;
  text("eventCount", String(eventNumber).padStart(2, "0"));
  text("inputIndex", String(Math.floor(eventNumber / 2) + 1).padStart(2, "0"));
  conversation.scrollTo({ top: conversation.scrollHeight, behavior: "smooth" });
  return article;
}

function addThinking() {
  const article = addEvent("thinking", webResearchEnabled ? "WEB RESEARCH ACTIVE" : "LOCAL CORTEX ACTIVE", webResearchEnabled ? "Searching and crawling public evidence…" : "Verifying native identity…", [webResearchEnabled ? "BOUNDED HTTPS RETRIEVAL" : "LOCAL INFERENCE"]);
  const message = $("p", article);
  const phases = [
    webResearchEnabled ? "Retrieving bounded public HTTPS sources…" : "Verifying native identity and journal chain…",
    "Loading the local Qwen3 cortex into memory…",
    "Reasoning inside the bounded local context…",
    "Preparing a provenance-bound response…",
    "Binding memory, epistemic claim, and proof event…",
  ];
  let phase = 0;
  thinkingTimer = setInterval(() => {
    phase = Math.min(phase + 1, phases.length - 1);
    message.textContent = phases[phase];
  }, 6500);
  return article;
}

function addWebSources(article, sources = []) {
  if (!sources.length) return;
  const list = element("div", "web-sources");
  sources.forEach((source, index) => {
    const link = element("a");
    link.href = source.url;
    link.target = "_blank";
    link.rel = "noopener noreferrer";
    link.append(element("b", "", source.source_id ? short(source.source_id, 12) : `[${index + 1}]`), element("span", "", source.title || source.url), element("small", "", short(source.content_sha256, 8)));
    list.append(link);
  });
  $(".event-body", article).append(list);
}

function renderResearchAudit(external = null) {
  const audit = $("#researchAudit");
  const metricsNode = $("#researchMetrics");
  const claimsNode = $("#claimFindings");
  const tribunalNode = $("#tribunalFindings");
  metricsNode.replaceChildren(); claimsNode.replaceChildren(); tribunalNode.replaceChildren();
  if (!external || external.schema !== 2) { audit.hidden = true; return; }
  audit.hidden = false;
  const metrics = external.research_metrics || {};
  ["sources", "independent_origins", "passages", "claims", "contradictions", "world_changes"].forEach((name) => {
    const cell = element("div");
    cell.append(element("span", "", name.replaceAll("_", " ").toUpperCase()), element("b", "", String(metrics[name] ?? 0)));
    metricsNode.append(cell);
  });
  (external.claims || []).slice(0, 8).forEach((claim) => {
    const item = element("div", `claim-${claim.status || "insufficient"}`);
    item.append(element("b", "", String(claim.status || "unknown").toUpperCase()), element("span", "", short(claim.claim_id, 14)), element("p", "", claim.text || "Untitled claim"));
    const citations = [...(claim.support_passage_ids || []), ...(claim.refute_passage_ids || [])];
    if (citations.length) item.append(element("small", "", citations.map((id) => `[${id}]`).join(" ")));
    claimsNode.append(item);
  });
  (external.tribunal || []).forEach((finding) => {
    const item = element("div", `tribunal-${finding.disposition || "qualify"}`);
    item.append(element("b", "", String(finding.role || "role").toUpperCase()), element("span", "", String(finding.disposition || "unknown").toUpperCase()), element("p", "", (finding.reasons || []).join(" · ")));
    tribunalNode.append(item);
  });
}

function setValidity(validity = {}) {
  ["computational", "provenance", "epistemic", "normative"].forEach((name) => {
    const cell = document.querySelector(`[data-validity="${name}"]`);
    const status = String(validity[name] || "waiting").toLowerCase();
    cell.className = status;
    $("b", cell).textContent = status.toUpperCase();
  });
}

function setLensOpen(open) {
  evidenceLens.classList.toggle("open", open);
  lensScrim.hidden = !open;
  if (open && matchMedia("(max-width: 880px)").matches) $("#closeLens").focus({ preventScroll: true });
}

function inspect(report, article, reveal = true) {
  document.querySelectorAll(".sain-event.selected").forEach((node) => node.classList.remove("selected"));
  article?.classList.add("selected");
  activeReport = report;
  const claim = report.epistemic_claim || {};
  const validity = claim.validity || {};
  setValidity(validity);
  text("lensState", "EVENT LOCKED");
  text("receiptEvent", report.event_id);
  text("receiptProof", report.proof_digest);
  text("receiptLineage", report.lineage_digest);
  text("receiptModel", report.cortex?.model || "local cortex");
  text("receiptExecution", report.cortex?.execution || "LOCAL / PROVIDER FREE");
  text("receiptLatency", report.cortex?.elapsed_ms != null ? `${report.cortex.elapsed_ms} ms` : "—");
  text("scopeText", validity.scope_statement || "Execution and provenance are bound; external factual truth remains independently challengeable.");
  renderResearchAudit(report.cortex?.external_evidence);
  $("#copyReceipt").disabled = false;
  if (reveal) setLensOpen(true);
}

async function transmit() {
  const prompt = question.value.trim();
  if (!prompt || send.disabled) return;
  addEvent("user", "QUESTION", prompt);
  question.value = "";
  question.style.height = "auto";
  send.disabled = true;
  const thinking = addThinking();
  const started = performance.now();
  try {
    const payload = await requestChat(prompt);
    thinking.remove();
    clearInterval(thinkingTimer);
    const report = payload.report;
    const labels = [
      `GEN ${report.generation}`,
      `PROOF ${short(report.proof_digest, 10)}`,
      "MEMORY COMMITTED",
      report.cortex?.provider_required ? "PROVIDER" : "PROVIDER FREE",
    ];
    if (report.cortex?.external_evidence?.sources?.length) labels.push(`WEB ${report.cortex.external_evidence.sources.length} SOURCES`);
    const spoken = report.cortex?.inference_audit?.accepted
      ? report.cortex?.inference_response
      : "Sain quarantined the generated answer because it conflicted with verified state. Inspect the evidence lens for details.";
    const article = addEvent("sain", "PROOF-CARRYING RESPONSE", spoken || "No response text returned.", labels);
    addWebSources(article, report.cortex?.external_evidence?.sources);
    article.tabIndex = 0;
    article.setAttribute("role", "button");
    article.setAttribute("aria-label", "Inspect evidence for this Sain response");
    article.addEventListener("click", () => inspect(report, article));
    article.addEventListener("keydown", (event) => {
      if (event.key === "Enter" || event.key === " ") inspect(report, article);
    });
    inspect(report, article, !matchMedia("(max-width: 880px)").matches);
    toast(`Response sealed in ${((performance.now() - started) / 1000).toFixed(1)} seconds`);
    refreshStatus();
  } catch (error) {
    thinking.remove();
    clearInterval(thinkingTimer);
    addEvent("error", "CORTEX INTERRUPTED", error.message, ["NO RESPONSE COMMITTED"]);
    toast(error.message);
  } finally {
    send.disabled = false;
    question.focus();
  }
}

composer.addEventListener("submit", (event) => { event.preventDefault(); transmit(); });
question.addEventListener("keydown", (event) => {
  if (event.key === "Enter" && !event.shiftKey) { event.preventDefault(); transmit(); }
});
question.addEventListener("input", () => {
  question.style.height = "auto";
  question.style.height = `${Math.min(question.scrollHeight, 150)}px`;
});
$("#focusComposer").addEventListener("click", () => question.focus());
$("#webMode").addEventListener("click", () => {
  webResearchEnabled = !webResearchEnabled;
  $("#webMode").setAttribute("aria-pressed", String(webResearchEnabled));
  question.placeholder = webResearchEnabled ? "Ask Sain to research the live web…" : "Hello Sain…";
  toast(webResearchEnabled ? "Web research enabled for new questions" : "Web research disabled");
  question.focus();
});
$("#historyTop").addEventListener("click", () => conversation.scrollTo({ top: 0, behavior: "smooth" }));
$("#historyBottom").addEventListener("click", () => conversation.scrollTo({ top: conversation.scrollHeight, behavior: "smooth" }));
$("#newSession").addEventListener("click", () => {
  conversation.querySelectorAll(".event:not(.system-event)").forEach((node) => node.remove());
  eventNumber = 1; activeReport = null; setValidity(); text("eventCount", "01"); text("lensState", "STANDBY");
  $("#copyReceipt").disabled = true; setLensOpen(false); question.focus(); toast("New local field opened");
});
$("#closeLens").addEventListener("click", () => setLensOpen(false));
lensScrim.addEventListener("click", () => setLensOpen(false));
document.addEventListener("keydown", (event) => {
  if (event.key === "Escape" && evidenceLens.classList.contains("open")) setLensOpen(false);
});
$("#copyReceipt").addEventListener("click", async () => {
  if (!activeReport) return;
  const receipt = JSON.stringify({
    event_id: activeReport.event_id,
    proof_digest: activeReport.proof_digest,
    event_head: activeReport.event_head,
    lineage_digest: activeReport.lineage_digest,
    generation: activeReport.generation,
    validity: activeReport.epistemic_claim?.validity,
    external_evidence: activeReport.cortex?.external_evidence,
  }, null, 2);
  await navigator.clipboard.writeText(receipt);
  toast("Proof receipt copied");
});

setInterval(() => text("missionClock", new Date().toLocaleTimeString("en-GB")), 1000);

$("#accessForm").addEventListener("submit", async (event) => {
  event.preventDefault();
  const password = $("#accessPassword").value;
  const button = $("#accessSubmit");
  button.disabled = true;
  $("#accessError").textContent = "";
  try {
    const response = await fetch(endpoint("/auth"), {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ password }),
      cache: "no-store",
    });
    const payload = await response.json();
    if (!response.ok || !payload.ok || !payload.token) throw new Error(payload.error || "Access denied");
    $("#accessPassword").value = "";
    unlockObservatory(payload.token);
    await refreshStatus();
    question.focus();
  } catch (error) {
    $("#accessError").textContent = error.message;
    $("#accessPassword").select();
  } finally { button.disabled = false; }
});

if (remoteConfig) {
  if (accessToken) {
    unlockObservatory(accessToken);
    refreshStatus();
  } else lockObservatory();
} else refreshStatus();

// A restrained living proof field: deterministic geometry, pointer-responsive depth.
const canvas = $("#field");
const context = canvas.getContext("2d");
let width = 0, height = 0, points = [], pointer = { x: .62, y: .35 };
function resizeField() {
  const ratio = Math.min(devicePixelRatio || 1, 2);
  width = innerWidth; height = innerHeight;
  canvas.width = width * ratio; canvas.height = height * ratio;
  canvas.style.width = `${width}px`; canvas.style.height = `${height}px`;
  context.setTransform(ratio, 0, 0, ratio, 0, 0);
  points = Array.from({ length: Math.min(58, Math.floor(width / 24)) }, (_, index) => ({
    x: ((index * 131) % 997) / 997 * width,
    y: ((index * 277 + 83) % 991) / 991 * height,
    phase: index * .71,
  }));
}
function drawField(now) {
  context.clearRect(0, 0, width, height);
  const time = now * .00012;
  points.forEach((point, index) => {
    const x = point.x + Math.sin(time + point.phase) * 9 + (pointer.x - .5) * (index % 5) * 3;
    const y = point.y + Math.cos(time * .8 + point.phase) * 7 + (pointer.y - .5) * (index % 4) * 3;
    for (let next = index + 1; next < Math.min(points.length, index + 5); next += 1) {
      const other = points[next]; const distance = Math.hypot(x - other.x, y - other.y);
      if (distance < 170) { context.strokeStyle = `rgba(62,228,213,${(1 - distance / 170) * .09})`; context.beginPath(); context.moveTo(x, y); context.lineTo(other.x, other.y); context.stroke(); }
    }
    context.fillStyle = index % 9 === 0 ? "rgba(186,255,53,.55)" : "rgba(62,228,213,.26)";
    context.fillRect(x, y, index % 9 === 0 ? 2 : 1, index % 9 === 0 ? 2 : 1);
  });
  requestAnimationFrame(drawField);
}
addEventListener("resize", resizeField);
addEventListener("pointermove", (event) => { pointer = { x: event.clientX / width, y: event.clientY / height }; }, { passive: true });
resizeField(); requestAnimationFrame(drawField);
