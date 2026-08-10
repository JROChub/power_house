"use strict";

(function (global) {
  const MODEL_LIMIT = 64 * 1024 * 1024;
  const DATASET_LIMIT = 64 * 1024 * 1024;
  const CONTRACT_LIMIT = 1024 * 1024;
  const WARMUP_PAIRS = 12;
  const MEASURED_PAIRS = 60;
  const INNER_REPETITIONS = 128;
  const SHA = /^sha256:[0-9a-f]{64}$/;
  const RELATIONSHIPS = new Set(["independent-external", "mfenx-author", "other"]);
  let authorized = false;
  let running = false;
  let cancelRequested = false;
  let lastStudy = null;

  class PhoneStudyError extends Error {}

  function boundedDeclarationText(value, label) {
    if (typeof value !== "string") throw new PhoneStudyError(`${label} is invalid`);
    const normalized = value.normalize("NFC").trim();
    if (normalized.length < 2 || normalized.length > 96 || /[\u0000-\u001f\u007f]/u.test(normalized)) {
      throw new PhoneStudyError(`${label} must contain 2 to 96 printable characters`);
    }
    return normalized;
  }

  function validateTargetDeclarationFields(value) {
    if (!value || value.include !== true) return null;
    if (!RELATIONSHIPS.has(value.evaluator_relationship)) throw new PhoneStudyError("evaluator relationship is invalid");
    return {
      basis: "evaluator-entered; not automatically detected or attested",
      target_model: boundedDeclarationText(value.target_model, "target model"),
      operating_system: boundedDeclarationText(value.operating_system, "operating system"),
      browser: boundedDeclarationText(value.browser, "browser"),
      evaluator_relationship: value.evaluator_relationship,
      consent: "include in this local report only"
    };
  }

  function targetDeclaration() {
    return validateTargetDeclarationFields({
      include: Boolean(global.document.getElementById("include-target-declaration")?.checked),
      target_model: global.document.getElementById("target-model")?.value || "",
      operating_system: global.document.getElementById("target-os")?.value || "",
      browser: global.document.getElementById("target-browser")?.value || "",
      evaluator_relationship: global.document.getElementById("evaluator-relationship")?.value || ""
    });
  }

  function sessionNonce() {
    const bytes = new Uint8Array(32);
    global.crypto.getRandomValues(bytes);
    return [...bytes].map((value) => value.toString(16).padStart(2, "0")).join("");
  }

  function selected(id) {
    return global.document.getElementById(id)?.files?.[0] || null;
  }

  async function readFile(file, limit, label) {
    if (!(file instanceof Blob) || file.size < 1 || file.size > limit) {
      throw new PhoneStudyError(`${label} is missing or exceeds the supported size`);
    }
    return file.arrayBuffer();
  }

  async function sha256(buffer) {
    const digest = await global.crypto.subtle.digest("SHA-256", buffer);
    return `sha256:${[...new Uint8Array(digest)].map((value) => value.toString(16).padStart(2, "0")).join("")}`;
  }

  function percentile(values, numerator, denominator) {
    if (!Array.isArray(values) || values.length < 1 || !Number.isInteger(numerator) || !Number.isInteger(denominator) || numerator < 1 || numerator > denominator) {
      throw new PhoneStudyError("invalid timing percentile request");
    }
    const ordered = values.map((value) => {
      if (!Number.isSafeInteger(value) || value < 1) throw new PhoneStudyError("timing observation is invalid");
      return value;
    }).sort((left, right) => left - right);
    return ordered[Math.ceil(ordered.length * numerator / denominator) - 1];
  }

  function formatMilliseconds(nanoseconds) {
    return `${(nanoseconds / 1_000_000).toFixed(3)} ms`;
  }

  function setStatus(state, message) {
    const stateNode = global.document.getElementById("phone-study-state");
    const messageNode = global.document.getElementById("phone-study-status");
    if (stateNode) stateNode.textContent = state;
    if (messageNode) messageNode.textContent = message;
  }

  function setMetric(id, value) {
    const node = global.document.getElementById(id);
    if (node) node.textContent = value;
  }

  function studyFile() {
    return new File([JSON.stringify(lastStudy, null, 2) + "\n"], `ckodmk-phone-study-${Date.now()}.json`, { type: "application/json" });
  }

  function setActions() {
    const run = global.document.getElementById("run-phone-study");
    const download = global.document.getElementById("download-phone-study");
    const share = global.document.getElementById("share-phone-study");
    const cancel = global.document.getElementById("cancel-phone-study");
    if (run) run.disabled = running || !authorized;
    if (cancel) cancel.disabled = !running;
    if (download) download.disabled = running || !lastStudy;
    if (share) share.disabled = running || !lastStudy || !(global.navigator?.share && global.navigator?.canShare?.({ files: [studyFile()] }));
    for (const id of ["include-target-declaration", "target-model", "target-os", "target-browser", "evaluator-relationship"]) {
      const control = global.document.getElementById(id);
      if (control) control.disabled = running;
    }
  }

  function invalidate() {
    authorized = false;
    lastStudy = null;
    setMetric("phone-source-p50", "Not run");
    setMetric("phone-source-p95", "Not run");
    setMetric("phone-candidate-p50", "Not run");
    setMetric("phone-candidate-p95", "Not run");
    setStatus("LOCKED", "Complete the local gate before running the timing study.");
    setActions();
  }

  async function timeInference(session, inputName, outputName, input, shape, classCount) {
    const start = global.performance.now();
    let result;
    for (let repetition = 0; repetition < INNER_REPETITIONS; repetition += 1) {
      result = await session.run({ [inputName]: new global.ort.Tensor("float32", input, shape) });
    }
    const elapsed = global.performance.now() - start;
    const output = result[outputName];
    if (!output || output.type !== "float32" || output.data.length !== classCount) throw new PhoneStudyError("timing run returned an unsupported output");
    for (const value of output.data) if (!Number.isFinite(value)) throw new PhoneStudyError("timing run returned a nonfinite output");
    const nanoseconds = Math.round(elapsed * 1_000_000 / INNER_REPETITIONS);
    if (!Number.isSafeInteger(nanoseconds) || nanoseconds < 1 || nanoseconds > 60_000_000_000) throw new PhoneStudyError("timing observation is outside the supported range");
    return nanoseconds;
  }

  async function runStudy() {
    if (running || !authorized) return;
    if (!global.CKODMKBrowserGate || !global.ort) throw new PhoneStudyError("browser runtime is unavailable");
    const sourceFile = selected("gate-source"), candidateFile = selected("gate-candidate"), datasetFile = selected("gate-dataset"), contractFile = selected("gate-contract");
    if (!sourceFile || !candidateFile || !datasetFile || !contractFile) throw new PhoneStudyError("the verified files are no longer selected");
    running = true; cancelRequested = false; lastStudy = null; setActions(); setStatus("RUNNING", "Running paired source and candidate observations locally.");
    const studyStarted = global.performance.now();
    let wakeLock = null;
    let sourceSession = null;
    let candidateSession = null;
    try {
      const declaredTarget = targetDeclaration();
      const nonce = sessionNonce();
      if (global.navigator?.wakeLock?.request) { try { wakeLock = await global.navigator.wakeLock.request("screen"); } catch { wakeLock = null; } }
      const [sourceBuffer, candidateBuffer, datasetBuffer, contractBuffer] = await Promise.all([
        readFile(sourceFile, MODEL_LIMIT, "source"), readFile(candidateFile, MODEL_LIMIT, "candidate"),
        readFile(datasetFile, DATASET_LIMIT, "data set"), readFile(contractFile, CONTRACT_LIMIT, "contract")
      ]);
      const expectedContract = global.document.getElementById("gate-contract-digest")?.value?.trim();
      if (!SHA.test(expectedContract || "")) throw new PhoneStudyError("the expected contract digest is invalid");
      const [sourceDigest, candidateDigest, datasetDigest, contractDigest] = await Promise.all([
        sha256(sourceBuffer), sha256(candidateBuffer), sha256(datasetBuffer), sha256(contractBuffer)
      ]);
      if (contractDigest !== expectedContract) throw new PhoneStudyError("the contract digest changed after verification");
      const contractText = new TextDecoder("utf-8", { fatal: true }).decode(contractBuffer);
      const contract = global.CKODMKBrowserGate.validateContract(global.CKODMKBrowserGate.parseStrictJson(contractText));
      if (sourceDigest !== contract.source.sha256 || candidateDigest !== contract.candidate.sha256 || datasetDigest !== contract.dataset.sha256) {
        throw new PhoneStudyError("an artifact digest changed after verification");
      }
      const dataset = global.CKODMKBrowserGate.parseDataset(datasetBuffer, contract);
      global.ort.env.wasm.numThreads = 1;
      global.ort.env.wasm.proxy = false;
      global.ort.env.wasm.wasmPaths = new URL("vendor/ort/", global.location.href).href;
      const options = { executionProviders: ["wasm"], executionMode: "sequential", graphOptimizationLevel: "disabled" };
      sourceSession = await global.ort.InferenceSession.create(new Uint8Array(sourceBuffer), options);
      candidateSession = await global.ort.InferenceSession.create(new Uint8Array(candidateBuffer), options);
      if (sourceSession.inputNames.length !== 1 || sourceSession.outputNames.length !== 1 || candidateSession.inputNames.length !== 1 || candidateSession.outputNames.length !== 1) {
        throw new PhoneStudyError("models do not match the timing profile");
      }
      const sourceInput = sourceSession.inputNames[0], sourceOutput = sourceSession.outputNames[0];
      const candidateInput = candidateSession.inputNames[0], candidateOutput = candidateSession.outputNames[0];
      const shape = [1, ...contract.dataset.sample_shape];
      const inputFor = (index) => dataset.inputs.slice(index * dataset.sampleElements, (index + 1) * dataset.sampleElements);
      for (let round = 0; round < WARMUP_PAIRS; round += 1) {
        if (cancelRequested) throw new PhoneStudyError("timing study was cancelled");
        if (global.performance.now() - studyStarted > 300_000) throw new PhoneStudyError("timing study exceeded five minutes");
        const input = inputFor(round % contract.dataset.samples);
        if (round % 2 === 0) {
          await timeInference(sourceSession, sourceInput, sourceOutput, input, shape, contract.output.class_count);
          await timeInference(candidateSession, candidateInput, candidateOutput, input.slice(), shape, contract.output.class_count);
        } else {
          await timeInference(candidateSession, candidateInput, candidateOutput, input, shape, contract.output.class_count);
          await timeInference(sourceSession, sourceInput, sourceOutput, input.slice(), shape, contract.output.class_count);
        }
      }
      const pairs = [];
      for (let round = 0; round < MEASURED_PAIRS; round += 1) {
        if (cancelRequested) throw new PhoneStudyError("timing study was cancelled");
        if (global.performance.now() - studyStarted > 300_000) throw new PhoneStudyError("timing study exceeded five minutes");
        const sampleIndex = (round * 977) % contract.dataset.samples;
        const input = inputFor(sampleIndex);
        let sourceNs, candidateNs;
        const order = round % 2 === 0 ? "source-candidate" : "candidate-source";
        if (round % 2 === 0) {
          sourceNs = await timeInference(sourceSession, sourceInput, sourceOutput, input, shape, contract.output.class_count);
          candidateNs = await timeInference(candidateSession, candidateInput, candidateOutput, input.slice(), shape, contract.output.class_count);
        } else {
          candidateNs = await timeInference(candidateSession, candidateInput, candidateOutput, input, shape, contract.output.class_count);
          sourceNs = await timeInference(sourceSession, sourceInput, sourceOutput, input.slice(), shape, contract.output.class_count);
        }
        pairs.push({ candidate_ns: candidateNs, order, sample_index: sampleIndex, source_ns: sourceNs });
      }
      const sourceValues = pairs.map((pair) => pair.source_ns), candidateValues = pairs.map((pair) => pair.candidate_ns);
      const sourceP50 = percentile(sourceValues, 1, 2), sourceP95 = percentile(sourceValues, 95, 100);
      const candidateP50 = percentile(candidateValues, 1, 2), candidateP95 = percentile(candidateValues, 95, 100);
      lastStudy = {
        schema: "mfenx/ckodmk-browser-phone-study/v2",
        created_at: new Date().toISOString(),
        authentication: "none",
        session_nonce: nonce,
        privacy: { automatic_device_identifiers_collected: [], files_uploaded: false, network_result_submission: false },
        target_declaration: declaredTarget,
        execution: {
          profile: "onnxruntime-web-wasm-f32-batch1-paired-timing/v1", runtime: "onnxruntime-web", runtime_version: "1.27.0",
          backend: "wasm", threads: 1, graph_optimization: "disabled", warmup_pairs: WARMUP_PAIRS,
          measured_pairs: MEASURED_PAIRS, inner_repetitions: INNER_REPETITIONS, order: "alternating",
          clock: "performance.now group duration divided by inner repetitions and converted to integer nanoseconds"
        },
        artifacts: { source_sha256: sourceDigest, candidate_sha256: candidateDigest, dataset_sha256: datasetDigest, contract_sha256: contractDigest },
        contract: { id: contract.contract_id, samples: contract.dataset.samples, class_count: contract.output.class_count },
        observations: { pairs, source_p50_ns: sourceP50, source_p95_ns: sourceP95, candidate_p50_ns: candidateP50, candidate_p95_ns: candidateP95 },
        interpretation: "finite browser timing observations only"
      };
      setMetric("phone-source-p50", formatMilliseconds(sourceP50));
      setMetric("phone-source-p95", formatMilliseconds(sourceP95));
      setMetric("phone-candidate-p50", formatMilliseconds(candidateP50));
      setMetric("phone-candidate-p95", formatMilliseconds(candidateP95));
      setStatus("COMPLETE", "The local timing report is ready to download or share.");
    } finally {
      if (sourceSession) await sourceSession.release();
      if (candidateSession) await candidateSession.release();
      if (wakeLock) { try { await wakeLock.release(); } catch {} }
      running = false;
      setActions();
    }
  }

  function downloadStudy() {
    if (!lastStudy) return;
    const file = studyFile(), url = URL.createObjectURL(file), anchor = global.document.createElement("a");
    anchor.href = url; anchor.download = file.name; anchor.click(); URL.revokeObjectURL(url);
  }

  async function shareStudy() {
    if (!lastStudy || !global.navigator?.share) return;
    const file = studyFile();
    if (!global.navigator.canShare?.({ files: [file] })) return;
    try { await global.navigator.share({ title: "CKODMK phone timing report", files: [file] }); } catch (error) {
      if (error?.name !== "AbortError") setStatus("COMPLETE", "Sharing was unavailable. The report can still be downloaded.");
    }
  }

  function bind() {
    const run = global.document.getElementById("run-phone-study");
    if (!run) return;
    run.addEventListener("click", () => runStudy().catch((error) => {
      lastStudy = null; setStatus("STOPPED", `Timing study stopped: ${error.message || "unknown error"}`); setActions();
    }));
    global.document.getElementById("download-phone-study")?.addEventListener("click", downloadStudy);
    global.document.getElementById("share-phone-study")?.addEventListener("click", shareStudy);
    global.document.getElementById("cancel-phone-study")?.addEventListener("click", () => { cancelRequested = true; });
    for (const id of ["gate-source", "gate-candidate", "gate-dataset", "gate-contract", "gate-contract-digest"]) {
      global.document.getElementById(id)?.addEventListener(id === "gate-contract-digest" ? "input" : "change", invalidate);
    }
    for (const id of ["include-target-declaration", "target-model", "target-os", "target-browser", "evaluator-relationship"]) {
      global.document.getElementById(id)?.addEventListener("input", () => {
        lastStudy = null;
        if (authorized) setStatus("READY", "Verified files are ready. A new timing run will use the current declaration.");
        setActions();
      });
    }
    global.addEventListener("ckodmk:verification-complete", () => {
      authorized = true; lastStudy = null; setStatus("READY", "Verified files are ready for a local paired timing study."); setActions();
    });
    invalidate();
  }

  global.CKODMKPhoneStudy = Object.freeze({ PhoneStudyError, percentile, validateTargetDeclarationFields });
  if (typeof module !== "undefined" && module.exports) module.exports = global.CKODMKPhoneStudy;
  if (global.document) bind();
})(typeof window !== "undefined" ? window : globalThis);
