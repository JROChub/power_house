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

const state = {
  workloads: [],
  selected: 0,
  online: false,
  running: false,
  pulse: 0,
  history: JSON.parse(localStorage.getItem("mfenx-lightsout-history") || "[]").slice(0, 6),
};

const $ = (selector) => document.querySelector(selector);
const fields = {
  bridge: $("#bridge-state"),
  list: $("#workload-list"),
  source: $("#source"),
  lines: $("#line-numbers"),
  title: $("#program-title"),
  sequence: $("#program-sequence"),
  lanes: $("#lanes"),
  laneReadout: $("#lane-readout"),
  run: $("#run"),
  idle: $("#idle-readout"),
  active: $("#active-readout"),
  history: $("#history"),
  dialog: $("#connect-dialog"),
};

function updateClock() {
  $("#utc-clock").textContent = `${new Date().toISOString().slice(11, 19)} UTC`;
}

function lineNumbers() {
  const count = fields.source.value.split("\n").length;
  fields.lines.textContent = Array.from({ length: count }, (_, index) => index + 1).join("\n");
  fields.lines.scrollTop = fields.source.scrollTop;
}

function formatSource(source) {
  return `${JSON.stringify(source, null, 2)}\n`;
}

function selectWorkload(index) {
  state.selected = index;
  const item = state.workloads[index];
  if (!item) return;
  fields.source.value = formatSource(item.source);
  fields.title.textContent = names[item.id] || item.id.replaceAll("-", " ");
  fields.sequence.textContent = `${String(index + 1).padStart(2, "0")} / ${String(state.workloads.length).padStart(2, "0")}`;
  fields.list.querySelectorAll("button").forEach((button, buttonIndex) => {
    button.setAttribute("aria-current", String(buttonIndex === index));
  });
  lineNumbers();
  state.pulse = .35;
}

function renderWorkloads() {
  fields.list.replaceChildren(...state.workloads.map((item, index) => {
    const entry = document.createElement("li");
    const button = document.createElement("button");
    button.type = "button";
    button.innerHTML = `<span>${String(index + 1).padStart(2, "0")}</span><b>${names[item.id] || item.id}</b>`;
    button.addEventListener("click", () => selectWorkload(index));
    entry.append(button);
    return entry;
  }));
  selectWorkload(0);
}

function bridgeState(kind, copy) {
  fields.bridge.className = `bridge-state ${kind}`;
  fields.bridge.querySelector("span").textContent = copy;
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
    state.online = true;
    state.workloads = payload.workloads;
    fields.lanes.max = Math.max(1, health.logical_cores);
    fields.lanes.value = Math.max(1, health.logical_cores);
    fields.laneReadout.textContent = fields.lanes.value;
    $("#host-arch").textContent = health.architecture;
    $("#host-cores").textContent = `${health.logical_cores} physical lanes`;
    $("#host-fabric").textContent = health.physical_workers ? `${health.physical_workers} native workers` : "hosted execution";
    bridgeState("online", health.physical_workers
      ? `machine present · ${health.physical_workers} nodes / ${health.logical_cores} lanes`
      : `machine present · ${health.logical_cores} lanes`);
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

function decodeTensor(output) {
  if (!output?.tensor) return "no tensor returned";
  const { element, shape } = output.tensor.ty;
  const bytes = Uint8Array.from(output.tensor.data);
  const view = new DataView(bytes.buffer);
  const values = [];
  const readers = {
    i32: [4, (offset) => view.getInt32(offset, true)],
    u32: [4, (offset) => view.getUint32(offset, true)],
    i64: [8, (offset) => view.getBigInt64(offset, true).toString()],
    u64: [8, (offset) => view.getBigUint64(offset, true).toString()],
    f32: [4, (offset) => Number(view.getFloat32(offset, true).toPrecision(7))],
    f64: [8, (offset) => Number(view.getFloat64(offset, true).toPrecision(12))],
    bool: [1, (offset) => Boolean(view.getUint8(offset))],
  };
  const [width, read] = readers[element] || [1, (offset) => view.getUint8(offset)];
  for (let offset = 0; offset + width <= bytes.length; offset += width) values.push(read(offset));
  return `${element} [${shape.join(" × ")}]\n${JSON.stringify(values)}`;
}

function showResult(payload) {
  const metrics = payload.result.metrics;
  fields.idle.hidden = true;
  fields.active.hidden = false;
  $("#result-time").textContent = `${(Number(metrics.end_to_end_ns) / 1e6).toFixed(3)} ms`;
  $("#result-rate").textContent = humanRate(metrics.integer_operations_per_second, "op/s");
  $("#result-bandwidth").textContent = humanRate(metrics.memory_bandwidth_bytes_per_second, "B/s");
  const quorumNodes = payload.cluster?.workers_in_quorum?.length;
  $("#result-lanes").textContent = quorumNodes ? `${payload.lanes} · ${quorumNodes} nodes` : payload.lanes;
  $("#result-instructions").textContent = payload.instruction_count;
  $("#result-tensor").textContent = decodeTensor(payload.result.outputs[0]);
  $("#result-digest").textContent = payload.image_digest;
  const record = {
    name: payload.program,
    elapsed: Number(metrics.end_to_end_ns),
    digest: payload.image_digest,
    checkpoint: payload.cluster?.checkpoint_id,
    at: Date.now(),
  };
  state.history.unshift(record);
  state.history = state.history.slice(0, 6);
  localStorage.setItem("mfenx-lightsout-history", JSON.stringify(state.history));
  renderHistory();
  state.pulse = 1;
}

function renderHistory() {
  fields.history.replaceChildren(...state.history.map((item) => {
    const row = document.createElement("li");
    row.innerHTML = `<span>${item.name}</span><time>${(item.elapsed / 1e6).toFixed(3)} ms</time>`;
    row.title = item.checkpoint ? `${item.digest} · checkpoint ${item.checkpoint}` : item.digest;
    return row;
  }));
}

async function execute() {
  if (state.running) return;
  if (!state.online) {
    fields.dialog.showModal();
    return;
  }
  let source;
  try {
    source = JSON.parse(fields.source.value);
  } catch (error) {
    bridgeState("error", `source error · ${error.message}`);
    return;
  }
  state.running = true;
  fields.run.disabled = true;
  fields.run.querySelector("b").textContent = "executing";
  bridgeState("online", "physical lanes active");
  state.pulse = .75;
  try {
    const response = await fetch(`${BRIDGE}/v1/run`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ source, lanes: Number(fields.lanes.value) }),
    });
    const payload = await response.json();
    if (!response.ok || !payload.verified) throw new Error(payload.error || "machine rejected workload");
    showResult(payload);
    bridgeState("online", "machine present · result verified");
  } catch (error) {
    bridgeState("error", error.message);
  } finally {
    state.running = false;
    fields.run.disabled = false;
    fields.run.querySelector("b").textContent = "execute";
  }
}

fields.source.addEventListener("input", lineNumbers);
fields.source.addEventListener("scroll", () => { fields.lines.scrollTop = fields.source.scrollTop; });
fields.lanes.addEventListener("input", () => { fields.laneReadout.textContent = fields.lanes.value; });
fields.run.addEventListener("click", execute);
fields.bridge.addEventListener("click", () => { if (!state.online) fields.dialog.showModal(); });
$("#retry-bridge").addEventListener("click", () => connect(true));
$("#copy-digest").addEventListener("click", () => navigator.clipboard.writeText($("#result-digest").textContent));
$("#clear-history").addEventListener("click", () => {
  state.history = [];
  localStorage.removeItem("mfenx-lightsout-history");
  renderHistory();
});
$("#new-program").addEventListener("click", () => {
  fields.source.value = "{\n  \"program\": {\n    \"version\": 1,\n    \"name\": \"untitled-machine\",\n    \"instructions\": [],\n    \"outputs\": []\n  },\n  \"inputs\": {}\n}\n";
  fields.title.textContent = "untitled machine";
  fields.sequence.textContent = "— / —";
  fields.list.querySelectorAll("button").forEach((button) => button.setAttribute("aria-current", "false"));
  lineNumbers();
  fields.source.focus();
});
document.addEventListener("keydown", (event) => {
  if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
    event.preventDefault();
    execute();
  }
  if (!event.metaKey && !event.ctrlKey && !event.altKey && /^[1-6]$/.test(event.key) && document.activeElement !== fields.source) {
    selectWorkload(Number(event.key) - 1);
  }
});

function machineField() {
  const canvas = $("#machine-field");
  const context = canvas.getContext("2d");
  const nodes = Array.from({ length: 22 }, (_, index) => ({
    x: ((index * 47) % 101) / 100,
    y: ((index * 31 + 13) % 97) / 100,
    phase: index * .71,
  }));
  let width = 0;
  let height = 0;
  let frame = 0;
  function resize() {
    const ratio = Math.min(devicePixelRatio || 1, 2);
    width = innerWidth;
    height = innerHeight;
    canvas.width = Math.floor(width * ratio);
    canvas.height = Math.floor(height * ratio);
    canvas.style.width = `${width}px`;
    canvas.style.height = `${height}px`;
    context.setTransform(ratio, 0, 0, ratio, 0, 0);
  }
  function draw() {
    frame += .009;
    state.pulse *= .972;
    context.clearRect(0, 0, width, height);
    context.lineWidth = 1;
    for (let index = 0; index < nodes.length; index += 1) {
      const node = nodes[index];
      const x = node.x * width + Math.sin(frame + node.phase) * 8;
      const y = node.y * height + Math.cos(frame * .7 + node.phase) * 6;
      const next = nodes[(index * 7 + 5) % nodes.length];
      const nx = next.x * width + Math.sin(frame + next.phase) * 8;
      const ny = next.y * height + Math.cos(frame * .7 + next.phase) * 6;
      context.strokeStyle = `rgba(198,255,54,${.025 + state.pulse * .09})`;
      context.beginPath();
      context.moveTo(x, y);
      context.lineTo(nx, y);
      context.lineTo(nx, ny);
      context.stroke();
      context.fillStyle = index % 4 === 0 ? `rgba(198,255,54,${.12 + state.pulse * .55})` : "rgba(217,221,214,.11)";
      context.fillRect(x - 1, y - 1, 2, 2);
      if (state.running) {
        const t = (frame * 1.8 + index * .13) % 1;
        context.fillStyle = "rgba(198,255,54,.8)";
        context.fillRect(x + (nx - x) * t - 1, y - 1, 3, 3);
      }
    }
    requestAnimationFrame(draw);
  }
  addEventListener("resize", resize);
  resize();
  draw();
}

updateClock();
setInterval(updateClock, 1000);
renderHistory();
machineField();
connect(true);
