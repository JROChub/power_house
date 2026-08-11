"use strict";

(function (global) {
  const LIMITS = Object.freeze({ model: 64 * 1024 * 1024, dataset: 64 * 1024 * 1024, contract: 1024 * 1024, samples: 25000, outputs: 250000, depth: 32 });
  const SHA = /^sha256:[0-9a-f]{64}$/;
  const CLAIM_KEYS = ["boundary", "criticality", "evidence_required", "id", "parameters", "relation"];
  let cancelRequested = false;
  let lastReport = null;
  let lastCandidate = null;
  let lastGeneration = null;
  let lastContract = null;
  let localContract = false;

  class GateError extends Error {}
  class CancelledError extends Error {}

  class StrictJsonParser {
    constructor(text) { this.text = text; this.i = 0; this.depth = 0; }
    parse() { const value = this.value(); this.ws(); if (this.i !== this.text.length) throw new GateError("contract has trailing input"); return value; }
    ws() { while (/[\x20\t\r\n]/.test(this.text[this.i] || "")) this.i += 1; }
    value() {
      this.ws();
      if (++this.depth > LIMITS.depth) throw new GateError("contract nesting exceeds limit");
      const c = this.text[this.i]; let value;
      if (c === "{") value = this.object();
      else if (c === "[") value = this.array();
      else if (c === '"') value = this.string();
      else if (c === "t" && this.text.slice(this.i, this.i + 4) === "true") { this.i += 4; value = true; }
      else if (c === "f" && this.text.slice(this.i, this.i + 5) === "false") { this.i += 5; value = false; }
      else if (c === "n" && this.text.slice(this.i, this.i + 4) === "null") { this.i += 4; value = null; }
      else value = this.number();
      this.depth -= 1; return value;
    }
    object() {
      const result = Object.create(null); const keys = new Set(); this.i += 1; this.ws();
      if (this.text[this.i] === "}") { this.i += 1; return result; }
      while (true) {
        this.ws(); if (this.text[this.i] !== '"') throw new GateError("object key must be a string");
        const key = this.string(); if (keys.has(key)) throw new GateError(`duplicate JSON key: ${key}`); keys.add(key);
        this.ws(); if (this.text[this.i++] !== ":") throw new GateError("missing colon after object key"); result[key] = this.value(); this.ws();
        const c = this.text[this.i++]; if (c === "}") return result; if (c !== ",") throw new GateError("object is not comma separated");
      }
    }
    array() {
      const result = []; this.i += 1; this.ws(); if (this.text[this.i] === "]") { this.i += 1; return result; }
      while (true) { result.push(this.value()); this.ws(); const c = this.text[this.i++]; if (c === "]") return result; if (c !== ",") throw new GateError("array is not comma separated"); }
    }
    string() {
      const start = this.i; this.i += 1;
      while (this.i < this.text.length) {
        const c = this.text[this.i++];
        if (c === '"') { try { return JSON.parse(this.text.slice(start, this.i)); } catch { throw new GateError("invalid JSON string"); } }
        if (c === "\\") { const e = this.text[this.i++]; if (e === "u") { if (!/^[0-9a-fA-F]{4}$/.test(this.text.slice(this.i, this.i + 4))) throw new GateError("invalid Unicode escape"); this.i += 4; } else if (!'"\\/bfnrt'.includes(e)) throw new GateError("invalid string escape"); }
        else if (c.charCodeAt(0) < 0x20) throw new GateError("control character in string");
      }
      throw new GateError("unterminated string");
    }
    number() {
      const match = this.text.slice(this.i).match(/^-?(?:0|[1-9][0-9]*)/); if (!match) throw new GateError("invalid JSON value");
      const end = this.i + match[0].length; if (/[.eE]/.test(this.text[end] || "")) throw new GateError("JSON floats are forbidden in contracts");
      this.i = end; const value = Number(match[0]); if (!Number.isSafeInteger(value)) throw new GateError("contract integer exceeds safe range"); return value;
    }
  }

  function parseStrictJson(text) { if (typeof text !== "string") throw new GateError("contract must be UTF-8 text"); return new StrictJsonParser(text).parse(); }
  function plainObject(value, name) { if (!value || typeof value !== "object" || Array.isArray(value)) throw new GateError(`${name} must be an object`); return value; }
  function exactKeys(value, expected, name) { plainObject(value, name); const actual = Object.keys(value).sort(); const wanted = [...expected].sort(); if (actual.join("\0") !== wanted.join("\0")) throw new GateError(`${name} fields are not the supported profile`); }
  function integer(value, name, min, max) { if (!Number.isSafeInteger(value) || value < min || value > max) throw new GateError(`${name} is out of range`); return value; }
  function string(value, name) { if (typeof value !== "string" || !value) throw new GateError(`${name} must be a nonempty string`); return value; }
  function digest(value, name) { if (typeof value !== "string" || !SHA.test(value)) throw new GateError(`${name} must be canonical sha256`); return value; }

  function validateContract(c) {
    exactKeys(c, ["candidate", "claims", "contract_id", "dataset", "output", "policy_profile", "schema", "source"], "contract");
    if (c.schema !== "mfenx/ckodmk-browser-contract/v1" || c.policy_profile !== "ckodmk/browser-wasm-f32-batch1-classification/v1") throw new GateError("unsupported browser contract profile");
    string(c.contract_id, "contract_id");
    for (const side of ["source", "candidate"]) { exactKeys(c[side], ["format", "sha256"], side); if (c[side].format !== "onnx") throw new GateError(`${side} format must be onnx`); digest(c[side].sha256, `${side}.sha256`); }
    exactKeys(c.dataset, ["batch_size", "format", "input_dtype", "input_key", "input_layout", "label_key", "sample_shape", "samples", "sha256"], "dataset");
    if (c.dataset.batch_size !== 1 || c.dataset.format !== "numpy-npz-stored-canonical-v1" || c.dataset.input_dtype !== "float32" || c.dataset.input_key !== "inputs" || c.dataset.input_layout !== "NC" || c.dataset.label_key !== "labels") throw new GateError("unsupported dataset profile");
    integer(c.dataset.samples, "dataset.samples", 1, LIMITS.samples); digest(c.dataset.sha256, "dataset.sha256");
    if (!Array.isArray(c.dataset.sample_shape) || c.dataset.sample_shape.length < 1 || c.dataset.sample_shape.length > 4) throw new GateError("sample_shape is unsupported");
    let elements = 1; for (const [i, n] of c.dataset.sample_shape.entries()) { elements *= integer(n, `sample_shape[${i}]`, 1, 100000); if (!Number.isSafeInteger(elements) || elements * c.dataset.samples > LIMITS.outputs * 64) throw new GateError("dataset shape exceeds work limit"); }
    exactKeys(c.output, ["class_count", "decision_rule", "dtype", "semantic_role"], "output");
    integer(c.output.class_count, "output.class_count", 2, 10000); if (c.output.decision_rule !== "argmax-first" || c.output.dtype !== "float32" || c.output.semantic_role !== "classification-logits") throw new GateError("unsupported output profile");
    if (c.dataset.samples * c.output.class_count > LIMITS.outputs) throw new GateError("output evidence exceeds work limit");
    if (!Array.isArray(c.claims) || c.claims.length < 1 || c.claims.length > 64) throw new GateError("claims count is unsupported");
    const ids = new Set(); for (const [i, claim] of c.claims.entries()) { exactKeys(claim, CLAIM_KEYS, `claims[${i}]`); string(claim.id, "claim.id"); if (ids.has(claim.id)) throw new GateError(`duplicate claim id: ${claim.id}`); ids.add(claim.id); if (!["required", "advisory"].includes(claim.criticality)) throw new GateError("unsupported claim criticality"); plainObject(claim.parameters, "claim.parameters"); }
    return c;
  }

  async function sha256(buffer) { const hash = await global.crypto.subtle.digest("SHA-256", buffer); return "sha256:" + [...new Uint8Array(hash)].map((x) => x.toString(16).padStart(2, "0")).join(""); }
  function crcTable() { const table = new Uint32Array(256); for (let n = 0; n < 256; n += 1) { let c = n; for (let k = 0; k < 8; k += 1) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1; table[n] = c >>> 0; } return table; }
  const CRC_TABLE = crcTable();
  function crc32(bytes) { let c = 0xffffffff; for (const b of bytes) c = CRC_TABLE[(c ^ b) & 255] ^ (c >>> 8); return (c ^ 0xffffffff) >>> 0; }
  function u16(view, at) { return view.getUint16(at, true); } function u32(view, at) { return view.getUint32(at, true); }
  function parseStoredNpz(buffer) {
    const bytes = new Uint8Array(buffer); const view = new DataView(buffer); if (bytes.length < 22) throw new GateError("dataset is not a ZIP container");
    let eocd = -1; for (let i = bytes.length - 22, floor = Math.max(0, bytes.length - 65557); i >= floor; i -= 1) if (u32(view, i) === 0x06054b50) { eocd = i; break; }
    if (eocd < 0 || u16(view, eocd + 4) !== 0 || u16(view, eocd + 6) !== 0 || u16(view, eocd + 20) !== 0) throw new GateError("dataset ZIP footer is unsupported");
    const count = u16(view, eocd + 10); if (count !== 2 || u16(view, eocd + 8) !== count) throw new GateError("dataset must contain exactly two members");
    const decoder = new TextDecoder("utf-8", { fatal: true }); let at = u32(view, eocd + 16); const members = Object.create(null); const expected = ["inputs.npy", "labels.npy"];
    for (let index = 0; index < count; index += 1) {
      if (at + 46 > eocd || u32(view, at) !== 0x02014b50) throw new GateError("invalid ZIP central directory");
      const flags = u16(view, at + 8), method = u16(view, at + 10), expectedCrc = u32(view, at + 16), compressed = u32(view, at + 20), size = u32(view, at + 24), nameLength = u16(view, at + 28), extraLength = u16(view, at + 30), commentLength = u16(view, at + 32), local = u32(view, at + 42);
      if (flags !== 0 || method !== 0 || compressed !== size || size === 0xffffffff || local === 0xffffffff) throw new GateError("dataset ZIP member is not canonical stored form");
      const name = decoder.decode(bytes.slice(at + 46, at + 46 + nameLength)); if (name !== expected[index]) throw new GateError("dataset member order or name is invalid");
      if (local + 30 > bytes.length || u32(view, local) !== 0x04034b50 || u16(view, local + 6) !== 0 || u16(view, local + 8) !== 0) throw new GateError("invalid local ZIP member");
      const localNameLength = u16(view, local + 26), localExtraLength = u16(view, local + 28); const localName = decoder.decode(bytes.slice(local + 30, local + 30 + localNameLength)); if (localName !== name) throw new GateError("ZIP member names disagree");
      const dataAt = local + 30 + localNameLength + localExtraLength; if (dataAt + size > bytes.length) throw new GateError("truncated dataset member"); const data = bytes.slice(dataAt, dataAt + size); if (crc32(data) !== expectedCrc) throw new GateError("dataset member CRC mismatch"); members[name] = data;
      at += 46 + nameLength + extraLength + commentLength;
    }
    if (at !== eocd) throw new GateError("unexpected ZIP bytes before footer"); return members;
  }

  function parseNpy(bytes, expectedDescr) {
    if (bytes.length < 12 || bytes[0] !== 0x93 || new TextDecoder().decode(bytes.slice(1, 6)) !== "NUMPY") throw new GateError("invalid NPY magic");
    const major = bytes[6], minor = bytes[7]; let headerLength, headerAt;
    const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    if (major === 1 && minor === 0) { headerLength = view.getUint16(8, true); headerAt = 10; } else if (major === 2 && minor === 0) { headerLength = view.getUint32(8, true); headerAt = 12; } else throw new GateError("unsupported NPY version");
    if (headerLength > 4096 || headerAt + headerLength > bytes.length) throw new GateError("invalid NPY header length");
    const header = new TextDecoder("ascii", { fatal: true }).decode(bytes.slice(headerAt, headerAt + headerLength));
    const descr = header.match(/['"]descr['"]\s*:\s*['"]([^'"]+)['"]/); const order = header.match(/['"]fortran_order['"]\s*:\s*(True|False)/); const shapeMatch = header.match(/['"]shape['"]\s*:\s*\(([^)]*)\)/);
    if (!descr || descr[1] !== expectedDescr || !order || order[1] !== "False" || !shapeMatch) throw new GateError("NPY profile does not match the contract");
    const shape = shapeMatch[1].split(",").map((x) => x.trim()).filter(Boolean).map((x) => { if (!/^[1-9][0-9]*$/.test(x)) throw new GateError("invalid NPY shape"); return Number(x); });
    let count = 1; for (const n of shape) { count *= n; if (!Number.isSafeInteger(count)) throw new GateError("NPY shape overflow"); }
    const itemBytes = expectedDescr === "<f4" ? 4 : 8; const dataAt = headerAt + headerLength; if (dataAt + count * itemBytes !== bytes.length) throw new GateError("NPY payload length mismatch");
    return { view, dataAt, shape, count };
  }

  function parseDataset(buffer, contract) {
    const members = parseStoredNpz(buffer); const input = parseNpy(members["inputs.npy"], "<f4"); const labelsNpy = parseNpy(members["labels.npy"], "<i8");
    const wantedInputShape = [contract.dataset.samples, ...contract.dataset.sample_shape]; if (input.shape.join(",") !== wantedInputShape.join(",") || labelsNpy.shape.join(",") !== String(contract.dataset.samples)) throw new GateError("dataset shape disagrees with contract");
    const inputs = new Float32Array(input.count); for (let i = 0; i < input.count; i += 1) { const value = input.view.getFloat32(input.dataAt + i * 4, true); if (!Number.isFinite(value)) throw new GateError("dataset contains non-finite input"); inputs[i] = value; }
    const labels = new Int32Array(labelsNpy.count); for (let i = 0; i < labelsNpy.count; i += 1) { const value = labelsNpy.view.getBigInt64(labelsNpy.dataAt + i * 8, true); if (value < 0n || value >= BigInt(contract.output.class_count)) throw new GateError("dataset label is outside class range"); labels[i] = Number(value); }
    return { inputs, labels, sampleElements: contract.dataset.sample_shape.reduce((a, b) => a * b, 1) };
  }

  function decimalFraction(text) { if (typeof text !== "string" || !/^-?(?:0|[1-9][0-9]*)(?:\.[0-9]+)?$/.test(text)) throw new GateError("policy decimal is noncanonical"); const negative = text[0] === "-"; const body = negative ? text.slice(1) : text; const parts = body.split("."); const denominator = 10n ** BigInt(parts[1]?.length || 0); const numerator = BigInt(parts[0]) * denominator + BigInt(parts[1] || "0"); return [negative ? -numerator : numerator, denominator]; }
  function doubleFraction(value) { if (!Number.isFinite(value)) throw new GateError("non-finite observed metric"); if (value === 0) return [0n, 1n]; const buffer = new ArrayBuffer(8), view = new DataView(buffer); view.setFloat64(0, value, false); const bits = view.getBigUint64(0, false); const sign = bits >> 63n ? -1n : 1n, exponent = Number((bits >> 52n) & 0x7ffn), fraction = bits & ((1n << 52n) - 1n); let numerator, power; if (exponent === 0) { numerator = fraction; power = -1074; } else { numerator = (1n << 52n) | fraction; power = exponent - 1023 - 52; } numerator *= sign; return power >= 0 ? [numerator << BigInt(power), 1n] : [numerator, 1n << BigInt(-power)]; }
  function fractionGte(a, b) { return a[0] * b[1] >= b[0] * a[1]; }
  function fractionLte(a, b) { return a[0] * b[1] <= b[0] * a[1]; }
  function argmax(values) { let best = 0; for (let i = 1; i < values.length; i += 1) if (values[i] > values[best]) best = i; return best; }
  function floatHex(values) { const buffer = new ArrayBuffer(4), view = new DataView(buffer); return Array.from(values, (value) => { view.setFloat32(0, value, false); return view.getUint32(0, false).toString(16).padStart(8, "0"); }); }

  async function execute(sourceBytes, candidateBytes, dataset, contract, progress) {
    if (!global.ort) throw new GateError("local ONNX Runtime Web failed to load");
    global.ort.env.wasm.numThreads = 1; global.ort.env.wasm.proxy = false; global.ort.env.wasm.wasmPaths = new URL("vendor/ort/", global.location.href).href;
    const options = { executionProviders: ["wasm"], executionMode: "sequential", graphOptimizationLevel: "disabled" };
    const source = await global.ort.InferenceSession.create(sourceBytes, options); const candidate = await global.ort.InferenceSession.create(candidateBytes, options);
    try {
      if (source.inputNames.length !== 1 || candidate.inputNames.length !== 1 || source.outputNames.length !== 1 || candidate.outputNames.length !== 1) throw new GateError("models must each expose exactly one input and output");
      const n = contract.dataset.samples, classes = contract.output.class_count, sampleShape = [1, ...contract.dataset.sample_shape]; const sourceBits = [], candidateBits = [], sourceDecisions = [], candidateDecisions = []; let sourceCorrect = 0, candidateCorrect = 0, maxLinf = 0;
      for (let row = 0; row < n; row += 1) {
        if (cancelRequested) throw new CancelledError("verification cancelled"); const start = row * dataset.sampleElements; const sample = dataset.inputs.slice(start, start + dataset.sampleElements); const tensorA = new global.ort.Tensor("float32", sample, sampleShape); const tensorB = new global.ort.Tensor("float32", sample.slice(), sampleShape);
        const [sourceResult, candidateResult] = await Promise.all([source.run({ [source.inputNames[0]]: tensorA }), candidate.run({ [candidate.inputNames[0]]: tensorB })]); const a = sourceResult[source.outputNames[0]], b = candidateResult[candidate.outputNames[0]];
        if (a.type !== "float32" || b.type !== "float32" || a.data.length !== classes || b.data.length !== classes) throw new GateError("model output does not match Float32 class profile");
        for (let c = 0; c < classes; c += 1) { if (!Number.isFinite(a.data[c]) || !Number.isFinite(b.data[c])) throw new GateError("model emitted non-finite output"); maxLinf = Math.max(maxLinf, Math.abs(a.data[c] - b.data[c])); }
        const da = argmax(a.data), db = argmax(b.data); sourceDecisions.push(da); candidateDecisions.push(db); if (da === dataset.labels[row]) sourceCorrect += 1; if (db === dataset.labels[row]) candidateCorrect += 1; sourceBits.push(...floatHex(a.data)); candidateBits.push(...floatHex(b.data));
        if ((row + 1) % 20 === 0 || row + 1 === n) { progress(row + 1, n); await new Promise((resolve) => global.setTimeout(resolve, 0)); }
      }
      return { sourceBits, candidateBits, sourceDecisions, candidateDecisions, sourceCorrect, candidateCorrect, maxLinf };
    } finally { await source.release(); await candidate.release(); }
  }

  function adjudicate(contract, observed) {
    const changes = observed.sourceDecisions.reduce((n, value, i) => n + (value === observed.candidateDecisions[i] ? 0 : 1), 0); const accuracyDelta = [BigInt(observed.candidateCorrect - observed.sourceCorrect), BigInt(contract.dataset.samples)]; const results = [];
    for (const claim of contract.claims) {
      let result = "NOT_EVALUATED", detail = "relation is outside the browser profile";
      if (claim.relation === "artifact_identity" && claim.boundary === "artifact" && claim.evidence_required === "digest") { result = "PASS"; detail = "all caller-pinned artifact digests matched"; }
      else if (claim.boundary === "onnxruntime-web-wasm-observation" && claim.evidence_required === "finite-dataset-exhaustive") {
        if (claim.relation === "decision_invariant" && Object.keys(claim.parameters).length === 0) { result = changes === 0 ? "PASS" : "FAIL"; detail = `${changes} of ${contract.dataset.samples} decisions changed`; }
        else if (claim.relation === "finite_set_accuracy_delta" && claim.parameters.metric === "accuracy" && Object.keys(claim.parameters).sort().join() === "metric,minimum") { const minimum = decimalFraction(claim.parameters.minimum); result = fractionGte(accuracyDelta, minimum) ? "PASS" : "FAIL"; detail = `${observed.candidateCorrect - observed.sourceCorrect}/${contract.dataset.samples} complete-set accuracy delta`; }
        else if (claim.relation === "bounded_deviation" && claim.parameters.metric === "linf" && Object.keys(claim.parameters).sort().join() === "maximum,metric") { const maximum = decimalFraction(claim.parameters.maximum); result = fractionLte(doubleFraction(observed.maxLinf), maximum) ? "PASS" : "FAIL"; detail = `${observed.maxLinf} maximum observed L∞`; }
      }
      results.push({ id: claim.id, relation: claim.relation, criticality: claim.criticality, result, detail });
    }
    const required = results.filter((x) => x.criticality === "required"); const decision = required.some((x) => x.result === "FAIL") ? "BLOCK" : required.some((x) => x.result !== "PASS") ? "INCONCLUSIVE" : "PASS";
    return { decision, changes, accuracyDeltaNumerator: Number(accuracyDelta[0]), accuracyDeltaDenominator: Number(accuracyDelta[1]), claims: results };
  }

  async function readBoundFile(file, maximum, name) { if (!(file instanceof Blob) || file.size < 1 || file.size > maximum) throw new GateError(`${name} must be a nonempty file no larger than ${maximum} bytes`); return file.arrayBuffer(); }
  function setText(id, value) { const node = global.document.getElementById(id); if (node) node.textContent = value; }
  function updateProgress(done, total) { setText("gate-rows", `${done.toLocaleString()} / ${total.toLocaleString()}`); const bar = global.document.getElementById("gate-progress"); if (bar) bar.style.width = `${total ? done / total * 100 : 0}%`; }
  function showDecision(decision, message) { const node = global.document.getElementById("gate-decision"); node.textContent = decision; node.className = decision === "BLOCK" ? "block" : decision === "INCONCLUSIVE" ? "inconclusive" : ""; setText("gate-status", message); }
  function renderClaims(claims) { const list = global.document.getElementById("gate-claims"); list.replaceChildren(...claims.map((claim) => { const item = global.document.createElement("li"), span = global.document.createElement("span"), b = global.document.createElement("b"); span.textContent = `${claim.id} / ${claim.detail}`; b.textContent = claim.result; b.className = claim.result.toLowerCase().replace("_", "-"); item.append(span, b); return item; })); }
  function chosen(id) { return global.document.getElementById(id).files[0]; }

  async function runFromUi(event) {
    event.preventDefault(); cancelRequested = false; lastReport = null; const run = global.document.getElementById("run-gate"), cancel = global.document.getElementById("cancel-gate"), download = global.document.getElementById("download-report"); run.disabled = true; cancel.disabled = false; download.disabled = true; updateProgress(0, 0); renderClaims([]); showDecision("RUNNING", "Freezing and hashing local inputs…");
    let wakeLock = null;
    try {
      if (global.navigator?.wakeLock?.request) { try { wakeLock = await global.navigator.wakeLock.request("screen"); } catch { wakeLock = null; } }
      const sourceFile = chosen("gate-source"), candidateFile = chosen("gate-candidate"), datasetFile = chosen("gate-dataset"), contractFile = chosen("gate-contract"); if (!sourceFile || !candidateFile || !datasetFile || !contractFile) throw new GateError("select all four input files"); setText("gate-size", `${sourceFile.size.toLocaleString()} to ${candidateFile.size.toLocaleString()}`);
      const [sourceBuffer, candidateBuffer, datasetBuffer, contractBuffer] = await Promise.all([readBoundFile(sourceFile, LIMITS.model, "source"), readBoundFile(candidateFile, LIMITS.model, "candidate"), readBoundFile(datasetFile, LIMITS.dataset, "dataset"), readBoundFile(contractFile, LIMITS.contract, "contract")]);
      const expected = global.document.getElementById("gate-contract-digest").value.trim(); if (!SHA.test(expected)) throw new GateError("expected contract digest must use sha256: plus 64 lowercase hexadecimal characters");
      const [sourceDigest, candidateDigest, datasetDigest, contractDigest] = await Promise.all([sha256(sourceBuffer), sha256(candidateBuffer), sha256(datasetBuffer), sha256(contractBuffer)]); if (contractDigest !== expected) throw new GateError("contract digest does not match caller authority");
      const contract = validateContract(parseStrictJson(new TextDecoder("utf-8", { fatal: true }).decode(contractBuffer))); if (sourceDigest !== contract.source.sha256 || candidateDigest !== contract.candidate.sha256 || datasetDigest !== contract.dataset.sha256) throw new GateError("artifact digest does not match contract");
      const dataset = parseDataset(datasetBuffer, contract); showDecision("RUNNING", "Executing source and candidate locally at batch one…"); const observed = await execute(new Uint8Array(sourceBuffer), new Uint8Array(candidateBuffer), dataset, contract, updateProgress); const adjudication = adjudicate(contract, observed);
      setText("gate-changes", `${adjudication.changes.toLocaleString()} / ${contract.dataset.samples.toLocaleString()}`); setText("gate-accuracy", `${adjudication.accuracyDeltaNumerator}/${adjudication.accuracyDeltaDenominator}`); setText("gate-linf", String(observed.maxLinf)); renderClaims(adjudication.claims); showDecision(adjudication.decision, adjudication.decision === "PASS" ? "Every required check passed." : adjudication.decision === "BLOCK" ? "At least one required check failed." : "A required check could not be completed in this browser.");
      lastReport = { schema: "mfenx/ckodmk-browser-report/v1", created_at: new Date().toISOString(), authentication: "none", contract_authority: localContract ? "local-self-issued-test-contract" : "caller-pinned-digest", runtime: { name: "onnxruntime-web", version: "1.27.0", backend: "wasm", batch_size: 1 }, candidate_generation: lastGeneration, contract: { id: contract.contract_id, sha256: contractDigest }, artifacts: { source_sha256: sourceDigest, candidate_sha256: candidateDigest, dataset_sha256: datasetDigest }, measurements: { samples: contract.dataset.samples, class_count: contract.output.class_count, source_correct: observed.sourceCorrect, candidate_correct: observed.candidateCorrect, decision_changes: adjudication.changes, max_linf: observed.maxLinf }, raw_observation_evidence: { labels: Array.from(dataset.labels), source_decisions: observed.sourceDecisions, candidate_decisions: observed.candidateDecisions, source_outputs_float32_hex: observed.sourceBits, candidate_outputs_float32_hex: observed.candidateBits }, claims: adjudication.claims, decision: adjudication.decision, nonclaims: ["native runtime behavior", "device identity", "latency", "power", "thermal behavior", "universal equivalence"] }; download.disabled = false; const share = global.document.getElementById("share-report"); if (share) share.disabled = !(global.navigator?.share && global.navigator?.canShare?.({ files: [reportFile()] })); global.dispatchEvent(new CustomEvent("ckodmk:verification-complete", { detail: { decision: adjudication.decision, contract_sha256: contractDigest } }));
    } catch (error) {
      const cancelled = error instanceof CancelledError; showDecision(cancelled ? "CANCELLED" : "BLOCK", cancelled ? "The local run was cancelled; no decision was authorized." : `Fail closed: ${error.message || "unknown browser-gate error"}`); renderClaims([]);
    } finally { if (wakeLock) { try { await wakeLock.release(); } catch {} } run.disabled = false; cancel.disabled = true; }
  }

  async function fetchFile(path, name, type) { const response = await fetch(path, { cache: "no-store", credentials: "same-origin" }); if (!response.ok) throw new GateError(`demo asset failed to load: ${name}`); return new File([await response.blob()], name, { type }); }
  async function fetchChunkedFile(parts, expectedBytes, expectedDigest, name, type) {
    if (!Array.isArray(parts) || parts.length < 1 || parts.length > 32 || !Number.isSafeInteger(expectedBytes) || expectedBytes < 1 || expectedBytes > LIMITS.model || !SHA.test(expectedDigest)) throw new GateError(`invalid chunk manifest for ${name}`);
    const merged = new Uint8Array(expectedBytes);
    let offset = 0;
    for (const [index, part] of parts.entries()) {
      if (!part || Object.keys(part).sort().join(",") !== "bytes,path,sha256" || typeof part.path !== "string" || !Number.isSafeInteger(part.bytes) || part.bytes < 1 || part.bytes > 8 * 1024 * 1024 || !SHA.test(part.sha256)) throw new GateError(`invalid chunk ${index + 1} for ${name}`);
      const response = await fetch(part.path, { cache: "no-store", credentials: "same-origin" });
      if (!response.ok) throw new GateError(`chunk ${index + 1} failed to load for ${name}`);
      const bytes = new Uint8Array(await response.arrayBuffer());
      if (bytes.length !== part.bytes || await sha256(bytes) !== part.sha256 || offset + bytes.length > merged.length) throw new GateError(`chunk ${index + 1} failed integrity validation for ${name}`);
      merged.set(bytes, offset);
      offset += bytes.length;
    }
    if (offset !== merged.length || await sha256(merged) !== expectedDigest) throw new GateError(`reconstructed file failed integrity validation for ${name}`);
    return new File([merged], name, { type });
  }
  function fetchDemoArtifact(demo, side) {
    const name = demo[`${side}Name`];
    const parts = demo[`${side}Parts`];
    if (parts) return fetchChunkedFile(parts, demo[`${side}Bytes`], demo[`${side}Digest`], name, "application/octet-stream");
    return fetchFile(demo[side], name, "application/octet-stream");
  }
  function setInputFile(id, file) { const transfer = new DataTransfer(); transfer.items.add(file); const input = global.document.getElementById(id); input.files = transfer.files; input.dispatchEvent(new Event("change")); }
  async function buildCandidate(sourceFile = chosen("gate-source")) { const build = global.document.getElementById("build-candidate"), download = global.document.getElementById("download-candidate"); if (!sourceFile) throw new GateError("select a supported source ONNX model first"); if (!global.CKODMKBrowserOptimizer || !global.ort) throw new GateError("browser optimizer or ONNX runtime failed to load"); build.disabled = true; download.disabled = true; showDecision("BUILDING", "Creating a deterministic per channel INT8 candidate on this device…"); try { const sourceBytes = new Uint8Array(await readBoundFile(sourceFile, LIMITS.model, "source")); const generated = global.CKODMKBrowserOptimizer.buildCandidate(sourceBytes); const candidateBytes = generated.bytes; global.ort.env.wasm.numThreads = 1; global.ort.env.wasm.proxy = false; global.ort.env.wasm.wasmPaths = new URL("vendor/ort/", global.location.href).href; const options = { executionProviders: ["wasm"], executionMode: "sequential", graphOptimizationLevel: "disabled" }; const sourceSession = await global.ort.InferenceSession.create(sourceBytes, options); let candidateSession = null; try { candidateSession = await global.ort.InferenceSession.create(candidateBytes, options); if (sourceSession.inputNames.length !== 1 || sourceSession.outputNames.length !== 1 || candidateSession.inputNames.length !== 1 || candidateSession.outputNames.length !== 1 || sourceSession.inputNames[0] !== candidateSession.inputNames[0] || sourceSession.outputNames[0] !== candidateSession.outputNames[0]) throw new GateError("generated candidate interface does not match the source"); } finally { await sourceSession.release(); if (candidateSession) await candidateSession.release(); } lastCandidate = new File([candidateBytes], `${sourceFile.name.replace(/\.onnx$/i, "") || "model"}.ckodmk-int8.onnx`, { type: "application/octet-stream" }); lastGeneration = { implementation: "mfenx-ckodmk-browser", version: "0.2.1", profile: generated.profile, source_bytes: sourceBytes.length, candidate_bytes: candidateBytes.length }; setInputFile("gate-candidate", lastCandidate); setText("gate-size", `${sourceBytes.length.toLocaleString()} to ${candidateBytes.length.toLocaleString()}`); download.disabled = false; showDecision("READY", `Candidate built locally: ${sourceBytes.length.toLocaleString()} to ${candidateBytes.length.toLocaleString()} bytes. Run verification next.`); return lastCandidate; } catch (error) { lastCandidate = null; lastGeneration = null; throw error; } finally { build.disabled = false; } }
  async function buildCandidateFromUi() { try { await buildCandidate(); } catch (error) { showDecision("BLOCK", `Candidate build stopped: ${error.message || "unknown optimizer error"}`); } }

  function inspectDatasetForContract(buffer) {
    const members = parseStoredNpz(buffer);
    const input = parseNpy(members["inputs.npy"], "<f4");
    const labels = parseNpy(members["labels.npy"], "<i8");
    if (input.shape.length !== 2 || input.shape[0] !== labels.count || labels.shape.length !== 1 || input.shape[0] < 1 || input.shape[0] > LIMITS.samples) throw new GateError("data set must use the supported two dimensional NC shape");
    const sampleShape = input.shape.slice(1);
    const sampleElements = sampleShape.reduce((total, value) => total * value, 1);
    if (!Number.isSafeInteger(sampleElements) || sampleElements < 1 || input.count !== input.shape[0] * sampleElements || input.count > LIMITS.outputs) throw new GateError("data set work exceeds the browser contract profile");
    const inputs = new Float32Array(input.count);
    for (let index = 0; index < input.count; index += 1) {
      const value = input.view.getFloat32(input.dataAt + index * 4, true);
      if (!Number.isFinite(value)) throw new GateError("data set contains a nonfinite input");
      inputs[index] = value;
    }
    const labelValues = new BigInt64Array(labels.count);
    for (let index = 0; index < labels.count; index += 1) {
      const value = labels.view.getBigInt64(labels.dataAt + index * 8, true);
      if (value < 0n || value > 65535n) throw new GateError("data set label is outside the supported range");
      labelValues[index] = value;
    }
    return { samples: input.shape[0], sampleShape, sampleElements, inputs, labels: labelValues };
  }

  async function createTestContract() {
    const sourceFile = chosen("gate-source"), candidateFile = chosen("gate-candidate"), datasetFile = chosen("gate-dataset");
    if (!sourceFile || !candidateFile || !datasetFile) throw new GateError("select a source and data set, then build or select a candidate");
    const button = global.document.getElementById("create-contract"), download = global.document.getElementById("download-contract");
    button.disabled = true; download.disabled = true; showDecision("BUILDING", "Inspecting the local model interface and creating a test contract…");
    try {
      const [sourceBuffer, candidateBuffer, datasetBuffer] = await Promise.all([readBoundFile(sourceFile, LIMITS.model, "source"), readBoundFile(candidateFile, LIMITS.model, "candidate"), readBoundFile(datasetFile, LIMITS.dataset, "dataset")]);
      const data = inspectDatasetForContract(datasetBuffer);
      global.ort.env.wasm.numThreads = 1; global.ort.env.wasm.proxy = false; global.ort.env.wasm.wasmPaths = new URL("vendor/ort/", global.location.href).href;
      const options = { executionProviders: ["wasm"], executionMode: "sequential", graphOptimizationLevel: "disabled" };
      const sourceSession = await global.ort.InferenceSession.create(new Uint8Array(sourceBuffer), options), candidateSession = await global.ort.InferenceSession.create(new Uint8Array(candidateBuffer), options);
      let classCount;
      try {
        if (sourceSession.inputNames.length !== 1 || sourceSession.outputNames.length !== 1 || candidateSession.inputNames.length !== 1 || candidateSession.outputNames.length !== 1) throw new GateError("models must each expose exactly one input and output");
        const first = data.inputs.slice(0, data.sampleElements), shape = [1, ...data.sampleShape];
        const [sourceResult, candidateResult] = await Promise.all([sourceSession.run({ [sourceSession.inputNames[0]]: new global.ort.Tensor("float32", first, shape) }), candidateSession.run({ [candidateSession.inputNames[0]]: new global.ort.Tensor("float32", first.slice(), shape) })]);
        const sourceOutput = sourceResult[sourceSession.outputNames[0]], candidateOutput = candidateResult[candidateSession.outputNames[0]];
        if (sourceOutput.type !== "float32" || candidateOutput.type !== "float32" || sourceOutput.data.length !== candidateOutput.data.length || sourceOutput.data.length < 2 || sourceOutput.data.length > 65536) throw new GateError("model outputs are outside the browser classification profile");
        classCount = sourceOutput.data.length;
      } finally { await sourceSession.release(); await candidateSession.release(); }
      for (const label of data.labels) if (label >= BigInt(classCount)) throw new GateError("data set label exceeds the model class count");
      const [sourceDigest, candidateDigest, datasetDigest] = await Promise.all([sha256(sourceBuffer), sha256(candidateBuffer), sha256(datasetBuffer)]);
      const contract = {
        candidate: { format: "onnx", sha256: candidateDigest },
        claims: [
          { boundary: "artifact", criticality: "required", evidence_required: "digest", id: "artifact-binding", parameters: {}, relation: "artifact_identity" },
          { boundary: "onnxruntime-web-wasm-observation", criticality: "advisory", evidence_required: "finite-dataset-exhaustive", id: "data-set-decisions", parameters: {}, relation: "decision_invariant" },
          { boundary: "onnxruntime-web-wasm-observation", criticality: "required", evidence_required: "finite-dataset-exhaustive", id: "data-set-accuracy", parameters: { metric: "accuracy", minimum: "-0.01" }, relation: "finite_set_accuracy_delta" },
          { boundary: "onnxruntime-web-wasm-observation", criticality: "advisory", evidence_required: "finite-dataset-exhaustive", id: "data-set-logits", parameters: { maximum: "0.25", metric: "linf" }, relation: "bounded_deviation" }
        ],
        contract_id: `local-test/${sourceDigest.slice(7, 19)}/${candidateDigest.slice(7, 19)}`,
        dataset: { batch_size: 1, format: "numpy-npz-stored-canonical-v1", input_dtype: "float32", input_key: "inputs", input_layout: "NC", label_key: "labels", sample_shape: data.sampleShape, samples: data.samples, sha256: datasetDigest },
        output: { class_count: classCount, decision_rule: "argmax-first", dtype: "float32", semantic_role: "classification-logits" },
        policy_profile: "ckodmk/browser-wasm-f32-batch1-classification/v1",
        schema: "mfenx/ckodmk-browser-contract/v1",
        source: { format: "onnx", sha256: sourceDigest }
      };
      const bytes = new TextEncoder().encode(JSON.stringify(contract, null, 2) + "\n");
      validateContract(parseStrictJson(new TextDecoder("utf-8", { fatal: true }).decode(bytes)));
      lastContract = new File([bytes], "ckodmk-local-test-contract.json", { type: "application/json" }); localContract = true; setInputFile("gate-contract", lastContract);
      global.document.getElementById("gate-contract-digest").value = await sha256(bytes);
      download.disabled = false; showDecision("READY", "Local test contract created. Its one percent accuracy allowance is required; decision equality and 0.25 maximum deviation are advisory. Review it before running.");
    } finally { button.disabled = false; }
  }

  async function createTestContractFromUi() { try { await createTestContract(); } catch (error) { lastContract = null; localContract = false; showDecision("BLOCK", `Contract creation stopped: ${error.message || "unknown contract error"}`); } }
  const DEMOS = Object.freeze({
    cnn: Object.freeze({ source: "demo/cnn/source.onnx", sourceName: "optdigits-cnn-source.onnx", candidate: "demo/cnn/candidate-int8.onnx", candidateName: "optdigits-cnn-ckodmk-int8.onnx", contract: "demo/cnn/browser-contract.json", contractName: "optdigits-cnn-browser-contract.json", contractDigest: "sha256:b1212573ebf8ca4f69f5f5ae6a7e15efeabd14d88d26b030e788ed3a45801a44" }),
    mlp16: Object.freeze({ source: "demo/source.onnx", sourceName: "mlp-w16-source.onnx", contract: "demo/browser-contract.json", contractName: "mlp-w16-browser-contract.json", contractDigest: "sha256:df48a3f723e5a8da61c3fe401172195f12c520a37e19a2bd4f5af03ee6f1c39e" }),
    mlp32: Object.freeze({ source: "demo/mlp-w32/source.onnx", sourceName: "mlp-w32-source.onnx", contract: "demo/mlp-w32/browser-contract.json", contractName: "mlp-w32-browser-contract.json", contractDigest: "sha256:78517c553ea43957f9efe8157318d8761ee64f52dfd215c868c6da3743cf2d32" }),
    mlp64: Object.freeze({ source: "demo/mlp-w64/source.onnx", sourceName: "mlp-w64-source.onnx", contract: "demo/mlp-w64/browser-contract.json", contractName: "mlp-w64-browser-contract.json", contractDigest: "sha256:bec1da451af633ed9e3d1941ab553313dab62c491b09f6c8b01991bc9eb9a25d" }),
    rbf40: Object.freeze({ source: "demo/rbf-c40/source.onnx", sourceName: "rbf-c40-source.onnx", contract: "demo/rbf-c40/browser-contract.json", contractName: "rbf-c40-browser-contract.json", contractDigest: "sha256:c44439d11bc5e7fa16de4123c3f50e3d9694ad93f799ba6b98230482b80e8577" }),
    rbf80: Object.freeze({ source: "demo/rbf/source.onnx", sourceName: "rbf-c80-source.onnx", contract: "demo/rbf/browser-contract.json", contractName: "rbf-c80-browser-contract.json", contractDigest: "sha256:46a9e1c548198e5401afaee38f2c95bb4a6853d654fffeb62738d94cd3433f0e" }),
    rbf160: Object.freeze({ source: "demo/rbf-c160/source.onnx", sourceName: "rbf-c160-source.onnx", contract: "demo/rbf-c160/browser-contract.json", contractName: "rbf-c160-browser-contract.json", contractDigest: "sha256:562334137aa920289fe2d3082be3be2fd3de35b9094bb60a2d2e8fd3f9b55420" }),
    mnist: Object.freeze({ source: "demo/mnist/source.onnx", sourceName: "mnist-12-source.onnx", candidate: "demo/mnist/candidate-int8.onnx", candidateName: "mnist-12-ckodmk-int8.onnx", dataset: "demo/mnist/test-1000.npz", datasetName: "mnist-test-1000.npz", contract: "demo/mnist/browser-contract.json", contractName: "mnist-browser-contract.json", contractDigest: "sha256:e142cb46083980dbd357fee5ab1db2118dcaba5d2c35dc41bcf31718fd9d3887" }),
    mobilenet: Object.freeze({
      sourceName: "mobilenet-v2-12-source.onnx", sourceBytes: 13964571, sourceDigest: "sha256:c0c3f76d93fa3fd6580652a45618618a220fced18babf65774ed169de0432ad5",
      sourceParts: Object.freeze([
        Object.freeze({ path: "demo/mobilenet/source.part-000", bytes: 4194304, sha256: "sha256:1e184ad57e8529d718ca395fad343df15150a1af221a9f38b2f4e65d95882cc5" }),
        Object.freeze({ path: "demo/mobilenet/source.part-001", bytes: 4194304, sha256: "sha256:4166f39185e04be8e56eb574223bb07ef1d1007871c07fa05ce9179f20c914bf" }),
        Object.freeze({ path: "demo/mobilenet/source.part-002", bytes: 4194304, sha256: "sha256:7e3452898f92b77b71053336c5e6930ff71e45aa0cbd558b304d0f8e59af4860" }),
        Object.freeze({ path: "demo/mobilenet/source.part-003", bytes: 1381659, sha256: "sha256:708e39299228680098e25d71ea145fe66a78328a573ff9b3a1986c76fdd2b8c6" })
      ]),
      candidateName: "mobilenet-v2-12-ckodmk-int8.onnx", candidateBytes: 3661816, candidateDigest: "sha256:3bc4ea55c7c39ccca1de7a504643f5a937b425b1a23bb1dc900760784a4c9adf",
      candidateParts: Object.freeze([
        Object.freeze({ path: "demo/mobilenet/candidate.part-000", bytes: 3661816, sha256: "sha256:3bc4ea55c7c39ccca1de7a504643f5a937b425b1a23bb1dc900760784a4c9adf" })
      ]),
      dataset: "demo/mobilenet/test-ten.npz", datasetName: "mobilenet-test-ten.npz", contract: "demo/mobilenet/browser-contract.json", contractName: "mobilenet-browser-contract.json", contractDigest: "sha256:172adb211d793e224a9f727822f069002d309820da96af943d7a1a595d7461f8"
    })
  });
  async function loadDemo(autoRun = false) { const button = global.document.getElementById("load-demo"), demoButton = global.document.getElementById("run-demo"), run = global.document.getElementById("run-gate"), selected = global.document.getElementById("demo-model")?.value || "mlp16", demo = DEMOS[selected]; if (!demo) { showDecision("BLOCK", "selected example is unsupported"); return; } button.disabled = true; if (demoButton) demoButton.disabled = true; run.disabled = true; showDecision("LOADING", "Loading the trained source, data set, and contract…"); try { const [source, dataset, contract, retainedCandidate] = await Promise.all([fetchDemoArtifact(demo, "source"), fetchFile(demo.dataset || "demo/optdigits-official-test.npz", demo.datasetName || "optdigits-official-test.npz", "application/octet-stream"), fetchFile(demo.contract, demo.contractName, "application/json"), (demo.candidate || demo.candidateParts) ? fetchDemoArtifact(demo, "candidate") : Promise.resolve(null)]); setInputFile("gate-source", source); setInputFile("gate-dataset", dataset); setInputFile("gate-contract", contract); global.document.getElementById("gate-contract-digest").value = demo.contractDigest; if (retainedCandidate) { lastCandidate = retainedCandidate; lastGeneration = { implementation: "mfenx-ckodmk-cli", version: "0.2.1", profile: "ort-static-qoperator-u8s8", source_bytes: source.size, candidate_bytes: retainedCandidate.size }; setInputFile("gate-candidate", retainedCandidate); setText("gate-size", `${source.size.toLocaleString()} to ${retainedCandidate.size.toLocaleString()}`); global.document.getElementById("download-candidate").disabled = false; } else { await buildCandidate(source); } showDecision("READY", autoRun ? "Candidate loaded. Starting local verification…" : "Example inputs and candidate loaded. Select RUN LOCAL GATE."); if (autoRun) global.document.getElementById("gate-form").requestSubmit(); } catch (error) { showDecision("BLOCK", error.message); } finally { button.disabled = false; if (demoButton) demoButton.disabled = false; if (!autoRun) run.disabled = false; } }
  function downloadCandidate() { if (!lastCandidate) return; const url = URL.createObjectURL(lastCandidate), anchor = global.document.createElement("a"); anchor.href = url; anchor.download = lastCandidate.name; anchor.click(); URL.revokeObjectURL(url); }
  function downloadContract() { if (!lastContract) return; const url = URL.createObjectURL(lastContract), anchor = global.document.createElement("a"); anchor.href = url; anchor.download = lastContract.name; anchor.click(); URL.revokeObjectURL(url); }
  function reportFile() { return new File([JSON.stringify(lastReport, null, 2) + "\n"], `ckodmk-browser-report-${Date.now()}.json`, { type: "application/json" }); }
  function downloadReport() { if (!lastReport) return; const file = reportFile(), url = URL.createObjectURL(file), anchor = global.document.createElement("a"); anchor.href = url; anchor.download = file.name; anchor.click(); URL.revokeObjectURL(url); }
  async function shareReport() { if (!lastReport || !global.navigator?.share) return; const file = reportFile(); if (!global.navigator.canShare?.({ files: [file] })) return; try { await global.navigator.share({ title: "CKODMK verification report", text: `CKODMK result: ${lastReport.decision}`, files: [file] }); } catch (error) { if (error?.name !== "AbortError") showDecision(lastReport.decision, "The report could not be shared. Download remains available."); } }
  function bindUi() { const labels = [["gate-source", "source-name"], ["gate-candidate", "candidate-name"], ["gate-dataset", "dataset-name"], ["gate-contract", "contract-name"]]; for (const [inputId, labelId] of labels) global.document.getElementById(inputId).addEventListener("change", (event) => setText(labelId, event.target.files[0]?.name || "No file selected")); global.document.getElementById("gate-source").addEventListener("change", () => { lastCandidate = null; lastGeneration = null; global.document.getElementById("download-candidate").disabled = true; }); global.document.getElementById("gate-candidate").addEventListener("change", (event) => { if (event.target.files[0] !== lastCandidate) { lastCandidate = null; lastGeneration = null; global.document.getElementById("download-candidate").disabled = true; } }); global.document.getElementById("gate-contract").addEventListener("change", (event) => { if (event.target.files[0] !== lastContract) { lastContract = null; localContract = false; global.document.getElementById("download-contract").disabled = true; } }); global.document.getElementById("gate-form").addEventListener("submit", runFromUi); global.document.getElementById("load-demo").addEventListener("click", () => loadDemo(false)); global.document.getElementById("run-demo").addEventListener("click", () => loadDemo(true)); global.document.getElementById("build-candidate").addEventListener("click", buildCandidateFromUi); global.document.getElementById("create-contract").addEventListener("click", createTestContractFromUi); global.document.getElementById("download-candidate").addEventListener("click", downloadCandidate); global.document.getElementById("download-contract").addEventListener("click", downloadContract); global.document.getElementById("cancel-gate").addEventListener("click", () => { cancelRequested = true; }); global.document.getElementById("download-report").addEventListener("click", downloadReport); global.document.getElementById("share-report").addEventListener("click", shareReport); }
  function loadInputs({ source, candidate, dataset, contract, contractDigest, generation }) { setInputFile("gate-source", source); setInputFile("gate-candidate", candidate); setInputFile("gate-dataset", dataset); setInputFile("gate-contract", contract); global.document.getElementById("gate-contract-digest").value = contractDigest; lastGeneration = generation || null; }

  const api = Object.freeze({ GateError, parseStrictJson, validateContract, parseStoredNpz, parseNpy, parseDataset, decimalFraction, doubleFraction, adjudicate, crc32, loadInputs }); global.CKODMKBrowserGate = api; if (typeof module !== "undefined" && module.exports) module.exports = api; if (global.document) bindUi();
})(typeof window !== "undefined" ? window : globalThis);
