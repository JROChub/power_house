"use strict";

const BRIDGE = "https://rarecomp-mfenx-api.jrochub-resonance.workers.dev";
const names = {
  "exact-gemm": "exact gemm",
  "exact-batched-ntt": "batched ntt",
  "exact-graph-bfs": "graph traversal",
  "exact-transition-power": "powered transition",
  "exact-tensor-contract": "tensor contraction",
  "exact-find-first": "parallel search",
};

const parseStored = (key, fallback) => {
  try { return JSON.parse(localStorage.getItem(key) || JSON.stringify(fallback)); }
  catch { return fallback; }
};

const state = {
  workloads: [],
  selected: -1,
  online: false,
  busy: false,
  pulse: 0,
  health: null,
  compilation: null,
  compilationKey: "",
  result: null,
  checkpoint: null,
  history: parseStored("mfenx-console-history", []).slice(0, 12),
};

const $ = (selector) => document.querySelector(selector);
const $$ = (selector) => [...document.querySelectorAll(selector)];
const fields = {
  bridge: $("#bridge-state"), list: $("#workload-list"), source: $("#source"),
  lines: $("#line-numbers"), title: $("#program-title"), sequence: $("#program-sequence"),
  lanes: $("#lanes"), laneReadout: $("#lane-readout"), run: $("#run"),
  transmute: $("#transmute"), idle: $("#idle-readout"), active: $("#active-readout"),
  history: $("#history"), dialog: $("#connect-dialog"), checkpoint: $("#checkpoint-id"),
};

function updateClock() {
  $("#utc-clock").textContent = `${new Date().toISOString().slice(11, 19)} UTC`;
}

function lineNumbers() {
  fields.lines.textContent = Array.from({ length: fields.source.value.split("\n").length }, (_, index) => index + 1).join("\n");
  fields.lines.scrollTop = fields.source.scrollTop;
}

const formatSource = (source) => `${JSON.stringify(source, null, 2)}\n`;
const machineKey = () => `${fields.lanes.value}:${fields.source.value}`;

function invalidateMachine() {
  state.compilation = null;
  state.compilationKey = "";
  $("#machine-count").textContent = "—";
  $("#empty-machine").hidden = false;
  $("#machine-inspection").hidden = true;
  $("#typed-ir").textContent = "Compile the program to inspect its validated typed IR.";
  downloadState();
}

function selectWorkload(index) {
  const item = state.workloads[index];
  if (!item) return;
  state.selected = index;
  fields.source.value = formatSource(item.source);
  fields.title.textContent = names[item.id] || item.id.replaceAll("-", " ");
  fields.sequence.textContent = `${String(index + 1).padStart(2, "0")} / ${String(state.workloads.length).padStart(2, "0")}`;
  fields.list.querySelectorAll("button").forEach((button, buttonIndex) => button.setAttribute("aria-current", String(buttonIndex === index)));
  localStorage.removeItem("mfenx-console-draft");
  invalidateMachine();
  lineNumbers();
  state.pulse = .35;
}

function renderWorkloads() {
  const draft = localStorage.getItem("mfenx-console-draft");
  fields.list.replaceChildren(...state.workloads.map((item, index) => {
    const entry = document.createElement("li");
    const button = document.createElement("button");
    button.type = "button";
    const number = document.createElement("span");
    const label = document.createElement("b");
    number.textContent = String(index + 1).padStart(2, "0");
    label.textContent = names[item.id] || item.id;
    button.append(number, label);
    button.addEventListener("click", () => selectWorkload(index));
    entry.append(button);
    return entry;
  }));
  $("#program-count").textContent = String(state.workloads.length).padStart(2, "0");
  selectWorkload(0);
  if (draft) loadCustomSource(draft, "recovered draft");
}

function bridgeState(kind, copy) {
  fields.bridge.className = `bridge-state ${kind}`;
  fields.bridge.querySelector("span").textContent = copy;
}

function renderFabric(health) {
  $("#host-arch").textContent = health.architecture;
  $("#host-cores").textContent = `${health.logical_cores} cluster lanes`;
  $("#host-fabric").textContent = `${health.physical_workers} native workers`;
  $("#fabric-readout").textContent = `${health.physical_workers} × ${Math.max(1, health.logical_cores / health.physical_workers)}`;
  $("#fabric-health").textContent = health.ok ? "quorum ready" : "degraded";
  $("#cluster-mini").replaceChildren(...health.nodes.map(node => {
    const row = document.createElement("span");
    row.className = node.state;
    row.textContent = `${node.name.replace("native-west-", "cell ")} / ${node.lanes} lanes`;
    return row;
  }));
  $("#node-list").replaceChildren(...health.nodes.map((node, index) => {
    const item = document.createElement("li");
    const id = document.createElement("span");
    const body = document.createElement("div");
    const title = document.createElement("b");
    const detail = document.createElement("small");
    id.textContent = `0${index + 1}`;
    title.textContent = node.name;
    detail.textContent = node.state === "ready" ? `${node.architecture} · ${node.lanes} native lanes` : node.error || "unavailable";
    body.append(title, detail);
    item.className = node.state;
    item.append(id, body);
    return item;
  }));
  const backends = [
    ["rarecomp gpu", "active", `${health.logical_cores} physical native lanes`],
    ["exact oracle", "active", "independent scalar replay"],
    ["fpga fabric", "detached", "not attached to hosted fabric"],
    ["qpu provider", "detached", "not attached to hosted fabric"],
    ["near storage", "detached", "not attached to hosted fabric"],
  ];
  const matrix = $("#backend-matrix");
  matrix.replaceChildren();
  for (const [name, status, detail] of backends) {
    const row = document.createElement("div");
    row.className = status;
    const label = document.createElement("b");
    const copy = document.createElement("small");
    label.textContent = name;
    copy.textContent = detail;
    row.append(label, copy);
    matrix.append(row);
  }
}

async function connect(showDialog = false) {
  bridgeState("", "looking for the machine");
  try {
    const [healthResponse, workloadResponse] = await Promise.all([
      fetch(`${BRIDGE}/v1/health`, { cache: "no-store" }),
      fetch(`${BRIDGE}/v1/workloads`, { cache: "no-store" }),
    ]);
    if (!healthResponse.ok || !workloadResponse.ok) throw new Error("machine rejected handshake");
    const health = await healthResponse.json();
    const payload = await workloadResponse.json();
    if (!health.ok || !Array.isArray(payload.workloads)) throw new Error("machine quorum unavailable");
    state.online = true;
    state.health = health;
    state.workloads = payload.workloads;
    fields.lanes.max = Math.max(1, health.logical_cores);
    fields.lanes.value = Math.max(1, health.logical_cores);
    fields.laneReadout.textContent = fields.lanes.value;
    renderFabric(health);
    bridgeState("online", `machine present · ${health.physical_workers} nodes / ${health.logical_cores} lanes`);
    renderWorkloads();
    if (fields.dialog.open) fields.dialog.close();
  } catch (error) {
    state.online = false;
    bridgeState("error", "hosted machine unavailable");
    if (showDialog && !fields.dialog.open) fields.dialog.showModal();
  }
}

function humanRate(value, unit) {
  const number = Number(value || 0);
  if (number >= 1e9) return `${(number / 1e9).toFixed(2)} G${unit}`;
  if (number >= 1e6) return `${(number / 1e6).toFixed(2)} M${unit}`;
  if (number >= 1e3) return `${(number / 1e3).toFixed(1)} K${unit}`;
  return `${number} ${unit}`;
}

function humanBytes(value) {
  const number = Number(value || 0);
  if (number >= 1024 ** 3) return `${(number / 1024 ** 3).toFixed(2)} GiB`;
  if (number >= 1024 ** 2) return `${(number / 1024 ** 2).toFixed(2)} MiB`;
  if (number >= 1024) return `${(number / 1024).toFixed(1)} KiB`;
  return `${number} B`;
}

function decodeTensor(output) {
  if (!output?.tensor) return "no tensor returned";
  const { element, shape } = output.tensor.ty;
  const bytes = Uint8Array.from(output.tensor.data);
  const view = new DataView(bytes.buffer);
  const values = [];
  const readers = {
    i32: [4, offset => view.getInt32(offset, true)], u32: [4, offset => view.getUint32(offset, true)],
    i64: [8, offset => view.getBigInt64(offset, true).toString()], u64: [8, offset => view.getBigUint64(offset, true).toString()],
    f32: [4, offset => Number(view.getFloat32(offset, true).toPrecision(7))],
    f64: [8, offset => Number(view.getFloat64(offset, true).toPrecision(12))], bool: [1, offset => Boolean(view.getUint8(offset))],
    u8: [1, offset => view.getUint8(offset)],
  };
  const [width, read] = readers[element] || readers.u8;
  const limit = Math.min(bytes.length, width * 512);
  for (let offset = 0; offset + width <= limit; offset += width) values.push(read(offset));
  const clipped = bytes.length > limit ? `\n… ${bytes.length - limit} additional bytes` : "";
  return `${element} [${shape.join(" × ")}]\n${JSON.stringify(values)}${clipped}`;
}

function switchView(name) {
  $$("[data-view]").forEach(button => button.setAttribute("aria-current", String(button.dataset.view === name)));
  $$("[data-panel]").forEach(panel => { panel.hidden = panel.dataset.panel !== name; });
}

function switchInspector(name) {
  $$("[data-inspector]").forEach(button => button.setAttribute("aria-current", String(button.dataset.inspector === name)));
  $$("[data-inspector-panel]").forEach(panel => { panel.hidden = panel.dataset.inspectorPanel !== name; });
}

function opcodeLabel(opcode) {
  return String(opcode?.opcode || "unknown").replaceAll("_", " ");
}

function renderMachine(payload) {
  const machine = payload.machine;
  $("#empty-machine").hidden = true;
  $("#machine-inspection").hidden = false;
  $("#machine-count").textContent = String(machine.instructions.length).padStart(2, "0");
  $("#machine-isa").textContent = `v${machine.isa_version}`;
  $("#machine-lanes").textContent = machine.lanes;
  $("#machine-cluster-lanes").textContent = payload.cluster?.aggregate_lanes || machine.lanes;
  $("#machine-tile").textContent = `${machine.tile_elements} elements`;
  $("#machine-peak").textContent = humanBytes(machine.estimated_peak_bytes);
  $("#machine-quorum").textContent = payload.cluster ? `${payload.cluster.workers_in_quorum.length}/${payload.cluster.workers_requested}` : "local";
  $("#instruction-stream").replaceChildren(...machine.instructions.map((instruction, index) => {
    const row = document.createElement("li");
    const number = document.createElement("span");
    const operation = document.createElement("div");
    const label = document.createElement("b");
    const shape = document.createElement("small");
    const tiles = document.createElement("code");
    number.textContent = String(index).padStart(3, "0");
    label.textContent = opcodeLabel(instruction.opcode);
    shape.textContent = `${instruction.ty.element} [${instruction.ty.shape.join(" × ")}] → %${instruction.result}`;
    tiles.textContent = `${instruction.tiles} tile${instruction.tiles === 1 ? "" : "s"}`;
    operation.append(label, shape);
    row.append(number, operation, tiles);
    return row;
  }));
  $("#typed-ir").textContent = JSON.stringify(payload.typed_ir, null, 2);
  fields.title.textContent = payload.program;
  downloadState();
}

function sourceRequest() {
  let source;
  try { source = JSON.parse(fields.source.value); }
  catch (error) { throw new Error(`source error · ${error.message}`); }
  return { source, lanes: Number(fields.lanes.value) };
}

function setBusy(action, busy) {
  state.busy = busy;
  fields.run.disabled = busy;
  fields.transmute.disabled = busy;
  if (busy) {
    action.querySelector("b").dataset.label = action.querySelector("b").textContent;
    action.querySelector("b").textContent = action === fields.run ? "executing" : "synthesizing";
  } else {
    fields.run.querySelector("b").textContent = "execute";
    fields.transmute.querySelector("b").textContent = "transmute";
  }
}

async function transmute(openMachine = true) {
  if (state.busy || !state.online) {
    if (!state.online && !fields.dialog.open) fields.dialog.showModal();
    return false;
  }
  let request;
  try { request = sourceRequest(); }
  catch (error) { bridgeState("error", error.message); return false; }
  setBusy(fields.transmute, true);
  bridgeState("online", "TRANSMUTE · synthesizing native machine");
  state.pulse = .72;
  try {
    const response = await fetch(`${BRIDGE}/v1/transmute`, {
      method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(request),
    });
    const payload = await response.json();
    if (!response.ok || !payload.ok) throw new Error(payload.error || "compiler rejected source");
    state.compilation = payload;
    state.compilationKey = machineKey();
    renderMachine(payload);
    if (openMachine) switchView("machine");
    bridgeState("online", `machine synthesized · ${payload.cluster?.workers_in_quorum?.length || 1} image quorum`);
    state.pulse = 1;
    return true;
  } catch (error) {
    bridgeState("error", error.message);
    return false;
  } finally {
    setBusy(fields.transmute, false);
  }
}

function addProof(label, value) {
  const group = document.createElement("div");
  const term = document.createElement("dt");
  const detail = document.createElement("dd");
  term.textContent = label;
  detail.textContent = String(value ?? "unavailable");
  group.append(term, detail);
  $("#proof-list").append(group);
}

function renderProof(payload) {
  const metrics = payload.result.metrics;
  const cluster = payload.cluster || {};
  $("#proof-state").textContent = payload.verified ? "accepted" : "rejected";
  $("#proof-list").replaceChildren();
  addProof("execution", metrics.backend);
  addProof("quorum", `${cluster.workers_in_quorum?.length || 1}/${cluster.workers_requested || 1} matching workers`);
  addProof("checkpoint", cluster.checkpoint_id || "not issued");
  addProof("artifact", payload.result.artifact_digest);
  addProof("execution time", `${(Number(metrics.execution_ns) / 1e6).toFixed(3)} ms`);
  addProof("verification", `${(Number(metrics.verification_ns) / 1e6).toFixed(3)} ms`);
  addProof("integer work", Number(metrics.integer_operations).toLocaleString());
  addProof("materialized", humanBytes(metrics.materialized_bytes));
  $("#telemetry-evidence").textContent = JSON.stringify(metrics.telemetry, null, 2);
}

function showResult(payload, recovered = false) {
  const metrics = payload.result.metrics;
  state.result = payload;
  fields.idle.hidden = true;
  fields.active.hidden = false;
  $("#result-time").textContent = `${(Number(metrics.end_to_end_ns) / 1e6).toFixed(3)} ms`;
  $("#result-rate").textContent = humanRate(metrics.integer_operations_per_second, "op/s");
  $("#result-bandwidth").textContent = humanRate(metrics.memory_bandwidth_bytes_per_second, "B/s");
  const quorumNodes = payload.cluster?.workers_in_quorum?.length;
  $("#result-lanes").textContent = quorumNodes ? `${payload.lanes} · ${quorumNodes} nodes` : payload.lanes;
  $("#result-instructions").textContent = payload.instruction_count;
  $("#result-tensor").textContent = payload.result.outputs.map((output, index) => `output ${index}\n${decodeTensor(output)}`).join("\n\n");
  $("#result-digest").textContent = payload.image_digest;
  fields.checkpoint.value = payload.cluster?.checkpoint_id || "";
  renderProof(payload);
  const record = {
    name: payload.program, elapsed: Number(metrics.end_to_end_ns), digest: payload.image_digest,
    checkpoint: payload.cluster?.checkpoint_id, recovered, at: Date.now(),
  };
  state.history = [record, ...state.history.filter(item => item.checkpoint !== record.checkpoint)].slice(0, 12);
  localStorage.setItem("mfenx-console-history", JSON.stringify(state.history));
  renderHistory();
  downloadState();
  state.pulse = 1;
}

function renderHistory() {
  fields.history.replaceChildren(...state.history.map(item => {
    const row = document.createElement("li");
    const body = document.createElement("button");
    const name = document.createElement("span");
    const elapsed = document.createElement("time");
    name.textContent = item.name;
    elapsed.textContent = `${(item.elapsed / 1e6).toFixed(3)} ms`;
    body.type = "button";
    body.title = item.checkpoint ? `recover checkpoint ${item.checkpoint}` : item.digest;
    body.append(name, elapsed);
    if (item.checkpoint) body.addEventListener("click", () => recover(item.checkpoint));
    row.append(body);
    return row;
  }));
}

async function execute() {
  if (state.busy) return;
  if (!state.online) { fields.dialog.showModal(); return; }
  if (state.compilationKey !== machineKey()) {
    const compiled = await transmute(false);
    if (!compiled) return;
  }
  let request;
  try { request = sourceRequest(); }
  catch (error) { bridgeState("error", error.message); return; }
  setBusy(fields.run, true);
  bridgeState("online", "physical fabric active · awaiting quorum");
  state.pulse = .8;
  try {
    const response = await fetch(`${BRIDGE}/v1/run`, {
      method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(request),
    });
    const payload = await response.json();
    if (!response.ok || !payload.verified) throw new Error(payload.error || "machine rejected workload");
    showResult(payload);
    switchInspector("result");
    bridgeState("online", "machine present · result verified");
  } catch (error) {
    bridgeState("error", error.message);
  } finally {
    setBusy(fields.run, false);
  }
}

async function recover(checkpointId = fields.checkpoint.value.trim()) {
  if (!checkpointId || state.busy) return;
  setBusy(fields.run, true);
  bridgeState("online", "recovering durable checkpoint");
  try {
    const response = await fetch(`${BRIDGE}/v1/run`, {
      method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ checkpoint_id: checkpointId }),
    });
    const payload = await response.json();
    if (!response.ok || !payload.verified) throw new Error(payload.error || "checkpoint recovery failed");
    showResult(payload, true);
    switchInspector("result");
    bridgeState("online", "checkpoint recovered · result verified");
  } catch (error) {
    bridgeState("error", error.message);
  } finally {
    setBusy(fields.run, false);
  }
}

function safeName(value) {
  return String(value || "rarecomp-machine").toLowerCase().replace(/[^a-z0-9_-]+/g, "-").replace(/^-|-$/g, "") || "rarecomp-machine";
}

function downloadJson(filename, value) {
  const blob = new Blob([`${JSON.stringify(value, null, 2)}\n`], { type: "application/json" });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = filename;
  document.body.append(anchor);
  anchor.click();
  anchor.remove();
  URL.revokeObjectURL(url);
}

function downloadState() {
  $("[data-download='image']").disabled = !state.compilation?.image;
  $("[data-download='result']").disabled = !state.result;
  $("[data-download='checkpoint']").disabled = !state.result?.cluster?.checkpoint_id;
}

async function download(kind) {
  let source;
  try { source = JSON.parse(fields.source.value); } catch { source = fields.source.value; }
  const name = safeName(state.result?.program || state.compilation?.program || source?.program?.name);
  if (kind === "source") downloadJson(`${name}.json`, source);
  if (kind === "image" && state.compilation?.image) downloadJson(`${name}.mfx`, state.compilation.image);
  if (kind === "result" && state.result) downloadJson(`${name}.result.json`, state.result);
  if (kind === "checkpoint" && state.result?.cluster?.checkpoint_id) {
    const id = state.result.cluster.checkpoint_id;
    const response = await fetch(`${BRIDGE}/v1/checkpoint/${encodeURIComponent(id)}`, { cache: "no-store" });
    if (!response.ok) { bridgeState("error", "checkpoint export failed"); return; }
    state.checkpoint = await response.json();
    downloadJson(`${name}.${id}.checkpoint.json`, state.checkpoint);
  }
}

function loadCustomSource(text, title = "untitled machine") {
  try {
    const parsed = JSON.parse(text);
    fields.source.value = formatSource(parsed);
    fields.title.textContent = parsed?.program?.name || title;
    fields.sequence.textContent = "custom / source";
    state.selected = -1;
    fields.list.querySelectorAll("button").forEach(button => button.setAttribute("aria-current", "false"));
    localStorage.setItem("mfenx-console-draft", fields.source.value);
    invalidateMachine();
    lineNumbers();
  } catch (error) {
    bridgeState("error", `source error · ${error.message}`);
  }
}

fields.source.addEventListener("input", () => {
  lineNumbers();
  localStorage.setItem("mfenx-console-draft", fields.source.value);
  if (state.compilationKey && state.compilationKey !== machineKey()) invalidateMachine();
});
fields.source.addEventListener("scroll", () => { fields.lines.scrollTop = fields.source.scrollTop; });
fields.lanes.addEventListener("input", () => {
  fields.laneReadout.textContent = fields.lanes.value;
  if (state.compilationKey && state.compilationKey !== machineKey()) invalidateMachine();
});
fields.run.addEventListener("click", execute);
fields.transmute.addEventListener("click", () => transmute(true));
fields.bridge.addEventListener("click", () => { if (!state.online) fields.dialog.showModal(); else switchInspector("fabric"); });
$("#retry-bridge").addEventListener("click", () => connect(true));
$("#copy-digest").addEventListener("click", () => navigator.clipboard.writeText($("#result-digest").textContent));
$("#clear-history").addEventListener("click", () => {
  state.history = [];
  localStorage.removeItem("mfenx-console-history");
  renderHistory();
});
$("#new-program").addEventListener("click", () => loadCustomSource(JSON.stringify({
  program: { version: 1, name: "untitled-machine", instructions: [], outputs: [] }, inputs: {},
}), "untitled machine"));
$("#open-program").addEventListener("click", () => $("#program-file").click());
$("#program-file").addEventListener("change", async event => {
  const file = event.target.files?.[0];
  if (file) loadCustomSource(await file.text(), file.name.replace(/\.json$/i, ""));
  event.target.value = "";
});
$("#recover").addEventListener("click", () => recover());
fields.checkpoint.addEventListener("keydown", event => { if (event.key === "Enter") recover(); });
$$('[data-view]').forEach(button => button.addEventListener("click", () => switchView(button.dataset.view)));
$$('[data-inspector]').forEach(button => button.addEventListener("click", () => switchInspector(button.dataset.inspector)));
$$('[data-download]').forEach(button => button.addEventListener("click", () => download(button.dataset.download)));

document.addEventListener("keydown", event => {
  if ((event.metaKey || event.ctrlKey) && event.key === "Enter") { event.preventDefault(); execute(); }
  if ((event.metaKey || event.ctrlKey) && event.shiftKey && event.key.toLowerCase() === "t") { event.preventDefault(); transmute(true); }
  if (!event.metaKey && !event.ctrlKey && !event.altKey && /^[1-6]$/.test(event.key) && document.activeElement !== fields.source) selectWorkload(Number(event.key) - 1);
});

function machineField() {
  const canvas = $("#machine-field");
  const context = canvas.getContext("2d");
  const nodes = Array.from({ length: 28 }, (_, index) => ({ x: ((index * 47) % 101) / 100, y: ((index * 31 + 13) % 97) / 100, phase: index * .71 }));
  let width = 0; let height = 0; let frame = 0;
  function resize() {
    const ratio = Math.min(devicePixelRatio || 1, 2);
    width = innerWidth; height = innerHeight;
    canvas.width = Math.floor(width * ratio); canvas.height = Math.floor(height * ratio);
    canvas.style.width = `${width}px`; canvas.style.height = `${height}px`;
    context.setTransform(ratio, 0, 0, ratio, 0, 0);
  }
  function draw() {
    frame += .009; state.pulse *= .972; context.clearRect(0, 0, width, height); context.lineWidth = 1;
    for (let index = 0; index < nodes.length; index += 1) {
      const node = nodes[index];
      const x = node.x * width + Math.sin(frame + node.phase) * 8;
      const y = node.y * height + Math.cos(frame * .7 + node.phase) * 6;
      const next = nodes[(index * 7 + 5) % nodes.length];
      const nx = next.x * width + Math.sin(frame + next.phase) * 8;
      const ny = next.y * height + Math.cos(frame * .7 + next.phase) * 6;
      context.strokeStyle = `rgba(198,255,54,${.025 + state.pulse * .09})`;
      context.beginPath(); context.moveTo(x, y); context.lineTo(nx, y); context.lineTo(nx, ny); context.stroke();
      context.fillStyle = index % 4 === 0 ? `rgba(198,255,54,${.12 + state.pulse * .55})` : "rgba(217,221,214,.11)";
      context.fillRect(x - 1, y - 1, 2, 2);
      if (state.busy) {
        const t = (frame * 1.8 + index * .13) % 1;
        context.fillStyle = "rgba(198,255,54,.8)";
        context.fillRect(x + (nx - x) * t - 1, y - 1, 3, 3);
      }
    }
    requestAnimationFrame(draw);
  }
  addEventListener("resize", resize); resize(); draw();
}

updateClock();
setInterval(updateClock, 1000);
renderHistory();
downloadState();
machineField();
connect(true);
