"use strict";

(function (global) {
  const SCHEMA = "mfenx/ckodmk-pcm/v2";
  const STATEMENT_SCHEMA = "mfenx/ckodmk-pcm-statement/v2";
  const ALGORITHM = "ecdsa-p256-sha256-p1363";
  const EVIDENCE = "finite-replay/v1";
  const LINEAGE_EVIDENCE = "model-lineage/v1";
  const AUTHORIZATION_EVIDENCE = "release-authorization/v1";
  const SCALE_EVIDENCE = "model-scale/v1";
  const RELEASE_EVIDENCE = "release-evidence-set/v1";
  const PROFILE = "ckodmk/browser-wasm-f32-batch1-classification/v1";
  const TRUSTED_KEY_ID = "sha256:bdb946d6261487728b61fd0b5ea7c9e9feadd2446778188302ea028bd9f9f6d0";
  const ARTIFACTS = Object.freeze(["authorization", "candidate", "contract", "dataset", "lineage", "release_evidence", "scale", "source"]);
  const LIMITS = Object.freeze({ authorization: 1048576, candidate: 67108864, contract: 1048576, dataset: 67108864, lineage: 1048576, release_evidence: 8388608, scale: 1048576, source: 67108864, pcm: 314572800 });
  const MEDIA = Object.freeze({ authorization: "application/json", candidate: "application/onnx", contract: "application/json", dataset: "application/x-npz", lineage: "application/json", release_evidence: "application/json", scale: "application/json", source: "application/onnx" });
  const SHA = /^sha256:[0-9a-f]{64}$/;
  const ROOT = /^sha256:[0-9a-f]{64}$/;
  const P256_ORDER = BigInt("0xffffffff00000000ffffffffffffffffbce6faada7179e84f3b9cac2fc632551");
  const LIFECYCLE = Object.freeze(["SUBMITTED", "RUNNING", "OUTPUT_PUBLISHED", "ARTIFACT_VERIFIED", "RELOAD_VERIFIED", "CAPABILITY_MEASURED", "RELEASE_ELIGIBLE"]);
  const LINEAGE_CLASSES = new Set(["NATIVE_TRAINED", "DERIVATIVE_FINE_TUNE", "QUANTIZED_DERIVATIVE", "EXTERNAL_UNMODIFIED"]);
  let admittedCandidate = null;

  class PCMError extends Error {}

  function exactKeys(value, expected, label) {
    if (!value || typeof value !== "object" || Array.isArray(value)) throw new PCMError(`${label} must be an object`);
    if (Object.keys(value).sort().join("\0") !== [...expected].sort().join("\0")) throw new PCMError(`${label} fields are unsupported`);
  }

  function inspect(value, depth = 0) {
    if (depth > 32) throw new PCMError("PCM nesting exceeds limit");
    if (value === null || typeof value === "boolean" || Number.isSafeInteger(value)) return;
    if (typeof value === "string") { if (!/^[\x00-\x7f]*$/.test(value)) throw new PCMError("signed PCM strings must be ASCII"); return; }
    if (Array.isArray(value)) { if (value.length > 64) throw new PCMError("signed PCM array exceeds limit"); value.forEach((child) => inspect(child, depth + 1)); return; }
    if (value && typeof value === "object") { const keys = Object.keys(value); if (keys.length > 64) throw new PCMError("signed PCM object exceeds limit"); keys.forEach((key) => { if (!/^[\x00-\x7f]*$/.test(key)) throw new PCMError("signed PCM keys must be ASCII"); inspect(value[key], depth + 1); }); return; }
    throw new PCMError("unsupported signed PCM value");
  }

  function canonical(value) {
    inspect(value);
    if (Array.isArray(value)) return `[${value.map(canonical).join(",")}]`;
    if (value && typeof value === "object") return `{${Object.keys(value).sort().map((key) => `${JSON.stringify(key)}:${canonical(value[key])}`).join(",")}}`;
    return JSON.stringify(value);
  }

  function ascii(value) { return new TextEncoder().encode(value); }
  function join(first, second) { const result = new Uint8Array(first.length + second.length); result.set(first); result.set(second, first.length); return result; }
  async function sha256(bytes) { const result = await global.crypto.subtle.digest("SHA-256", bytes); return "sha256:" + [...new Uint8Array(result)].map((byte) => byte.toString(16).padStart(2, "0")).join(""); }
  function base64(text, maximum, label) {
    if (typeof text !== "string" || text.length > Math.floor(maximum * 4 / 3) + 8 || !/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(text)) throw new PCMError(`${label} is not canonical base64`);
    let binary; try { binary = global.atob(text); } catch { throw new PCMError(`${label} is invalid base64`); }
    if (binary.length < 1 || binary.length > maximum || global.btoa(binary) !== text) throw new PCMError(`${label} exceeds its byte limit or is noncanonical`);
    return Uint8Array.from(binary, (character) => character.charCodeAt(0));
  }
  function time(value, label) {
    if (typeof value !== "string" || !/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/.test(value)) throw new PCMError(`${label} is not canonical UTC`);
    const result = Date.parse(value); if (!Number.isFinite(result) || new Date(result).toISOString() !== value.replace("Z", ".000Z")) throw new PCMError(`${label} is invalid`); return result;
  }
  function identifier(value, label) { if (typeof value !== "string" || !/^[A-Za-z0-9][A-Za-z0-9._:/+\-]{0,255}$/.test(value)) throw new PCMError(`${label} is not a canonical identifier`); return value; }
  function digest(value, label) { if (typeof value !== "string" || !SHA.test(value)) throw new PCMError(`${label} is not a canonical SHA-256 digest`); return value; }
  function origin(value, label) { exactKeys(value, ["kind", "name", "reference", "sha256"], label); identifier(value.kind, `${label}.kind`); identifier(value.name, `${label}.name`); identifier(value.reference, `${label}.reference`); digest(value.sha256, `${label}.sha256`); return value; }

  function parseEvidenceJson(bytes) {
    let text;
    try { text = new TextDecoder("utf-8", { fatal: true }).decode(bytes); } catch { throw new PCMError("evidence record is not valid UTF-8"); }
    return global.CKODMKBrowserGate.parseStrictEvidenceJson(text);
  }

  function verifyTrainingRecord(record, known) {
    if (record.schema !== "mfenx/ckodmk-optdigits-cnn-training/v1") throw new PCMError("unsupported training record");
    if (!record.artifacts || !record.result || !record.training) throw new PCMError("training record sections are missing");
    for (const name of ["candidate_sha256", "contract_sha256", "dataset_sha256", "source_sha256"]) if (!known.has(record.artifacts[name])) throw new PCMError("training record references an unsigned artifact");
    const { samples, source_correct: sourceCorrect, candidate_correct: candidateCorrect } = record.result;
    if (![samples, sourceCorrect, candidateCorrect].every(Number.isSafeInteger) || samples < 1 || sourceCorrect < 1 || sourceCorrect > samples || candidateCorrect < 1 || candidateCorrect > samples) throw new PCMError("training result counters are invalid");
    if (!Array.isArray(record.training.history) || record.training.history.length < 1 || record.training.history.at(-1)?.epoch !== record.training.epochs) throw new PCMError("training did not reach its declared final epoch");
  }

  function verifyBehaviorReport(report, known) {
    if (report.schema !== "mfenx/ckodmk-web-evidence/v1" || !report.cnn_demo) throw new PCMError("unsupported behavior report");
    const result = report.cnn_demo;
    for (const name of ["candidate_sha256", "contract_sha256", "dataset_sha256", "source_sha256", "training_record_sha256"]) if (!known.has(result[name])) throw new PCMError("behavior report references an unsigned artifact");
    if (!Number.isSafeInteger(result.samples) || result.samples < 1) throw new PCMError("behavior report sample count is invalid");
    for (const name of ["source_correct", "candidate_correct", "decision_changes"]) if (!Number.isSafeInteger(result[name]) || result[name] < 0 || result[name] > result.samples) throw new PCMError("behavior report counter is invalid");
    return { capability_accuracy_ppm: Math.floor(result.candidate_correct * 1000000 / result.samples), comparative_delta_ppm: Math.floor((result.candidate_correct - result.source_correct) * 1000000 / result.samples), evaluation_samples: result.samples };
  }

  function verifyProductionEvaluation(document, known) {
    exactKeys(document, ["artifacts", "metrics", "schema"], "production evaluation");
    if (document.schema !== "mfenx/production-evaluation/v1" || !document.artifacts || Object.keys(document.artifacts).length < 1 || Object.values(document.artifacts).some((value) => !known.has(value))) throw new PCMError("production evaluation references an unsigned artifact");
    const metrics = document.metrics; exactKeys(metrics, ["candidate_correct", "evaluation_samples", "reference_correct", "reliability_successes", "reliability_trials", "safety_critical_failures"], "production evaluation metrics");
    if (!Object.values(metrics).every(Number.isSafeInteger) || metrics.evaluation_samples < 1 || metrics.reliability_trials < 1 || metrics.candidate_correct < 0 || metrics.candidate_correct > metrics.evaluation_samples || metrics.reference_correct < 0 || metrics.reference_correct > metrics.evaluation_samples || metrics.reliability_successes < 0 || metrics.reliability_successes > metrics.reliability_trials || metrics.safety_critical_failures < 0) throw new PCMError("production evaluation counters are invalid");
    return { capability_accuracy_ppm: Math.floor(metrics.candidate_correct * 1000000 / metrics.evaluation_samples), comparative_delta_ppm: Math.floor((metrics.candidate_correct - metrics.reference_correct) * 1000000 / metrics.evaluation_samples), evaluation_samples: metrics.evaluation_samples, reliability_success_ppm: Math.floor(metrics.reliability_successes * 1000000 / metrics.reliability_trials), safety_critical_failures: metrics.safety_critical_failures };
  }

  async function validateEvidenceSet(document, artifactDigests) {
    exactKeys(document, ["records", "schema"], "release evidence set");
    if (document.schema !== "mfenx/release-evidence-set/v1" || !Array.isArray(document.records) || document.records.length < 1 || document.records.length > 64) throw new PCMError("unsupported release evidence set");
    const all = new Set(artifactDigests), decoded = [];
    for (const record of document.records) {
      exactKeys(record, ["payload_base64", "sha256", "verification_profile"], "release evidence record");
      if (!["artifact-only/v1", "training-record/v1", "behavior-report/v1", "production-evaluation/v1"].includes(record.verification_profile)) throw new PCMError("unsupported evidence verification profile");
      const payload = base64(record.payload_base64, 8388608, "release evidence payload");
      const observed = await sha256(payload);
      if (record.sha256 !== observed || all.has(observed)) throw new PCMError("release evidence digest mismatch or duplicate");
      all.add(observed); decoded.push({ digest: observed, payload, profile: record.verification_profile });
    }
    const results = new Set(), qualifications = new Map();
    for (const record of decoded) {
      if (record.profile === "training-record/v1") { verifyTrainingRecord(parseEvidenceJson(record.payload), all); results.add(record.digest); }
      if (record.profile === "behavior-report/v1") { qualifications.set(record.digest, verifyBehaviorReport(parseEvidenceJson(record.payload), all)); results.add(record.digest); }
      if (record.profile === "production-evaluation/v1") { qualifications.set(record.digest, verifyProductionEvaluation(parseEvidenceJson(record.payload), all)); results.add(record.digest); }
    }
    return { all, qualifications, results };
  }

  async function validateLineage(lineage, candidateBytes, evidence) {
    exactKeys(lineage, ["accelerator_evidence", "architecture_origin", "classification", "final_model_sha256", "initial_weight_origin", "model_id", "parent_checkpoint_sha256", "schema", "tokenizer_origin", "total_independently_verified_tokens", "trainer_identity", "training_data_commitments", "training_implementation_sha256", "training_receipts"], "model lineage");
    if (lineage.schema !== "mfenx/model-lineage/v1" || !LINEAGE_CLASSES.has(lineage.classification)) throw new PCMError("unsupported model lineage profile");
    identifier(lineage.model_id, "model_id"); if (digest(lineage.final_model_sha256, "final_model_sha256") !== await sha256(candidateBytes)) throw new PCMError("lineage final model does not match the candidate");
    const architecture = origin(lineage.architecture_origin, "architecture_origin"), initial = origin(lineage.initial_weight_origin, "initial_weight_origin"), tokenizer = origin(lineage.tokenizer_origin, "tokenizer_origin"); if (architecture.kind === "NOT_APPLICABLE") throw new PCMError("architecture origin is missing");
    exactKeys(lineage.trainer_identity, ["identity_key_id", "name"], "trainer_identity"); digest(lineage.trainer_identity.identity_key_id, "trainer identity key"); identifier(lineage.trainer_identity.name, "trainer name"); digest(lineage.training_implementation_sha256, "training implementation");
    if (lineage.parent_checkpoint_sha256 !== null) digest(lineage.parent_checkpoint_sha256, "parent checkpoint");
    const required = new Set([architecture.sha256, initial.sha256, lineage.training_implementation_sha256]); if (lineage.parent_checkpoint_sha256 !== null) required.add(lineage.parent_checkpoint_sha256); if (tokenizer.kind !== "NOT_APPLICABLE") required.add(tokenizer.sha256); for (const item of required) if (!evidence.all.has(item)) throw new PCMError("lineage digest does not resolve to a signed artifact");
    if (!Array.isArray(lineage.training_data_commitments) || lineage.training_data_commitments.length < 1 || lineage.training_data_commitments.length > 64 || new Set(lineage.training_data_commitments).size !== lineage.training_data_commitments.length) throw new PCMError("training data commitments are invalid"); lineage.training_data_commitments.forEach((value) => { digest(value, "training data commitment"); if (!evidence.all.has(value)) throw new PCMError("training data commitment is not a signed artifact"); });
    if (!Array.isArray(lineage.training_receipts) || lineage.training_receipts.length < 1 || lineage.training_receipts.length > 32) throw new PCMError("training receipts are invalid"); lineage.training_receipts.forEach((receipt) => { exactKeys(receipt, ["evidence_sha256", "provider", "receipt_id", "status"], "training receipt"); identifier(receipt.provider, "receipt provider"); identifier(receipt.receipt_id, "receipt id"); digest(receipt.evidence_sha256, "receipt evidence"); if (receipt.status !== "VERIFIED" || !evidence.results.has(receipt.evidence_sha256)) throw new PCMError("training receipt result was not independently verified"); });
    if (!Array.isArray(lineage.accelerator_evidence) || lineage.accelerator_evidence.length < 1 || lineage.accelerator_evidence.length > 32) throw new PCMError("accelerator evidence is invalid"); lineage.accelerator_evidence.forEach((record) => { exactKeys(record, ["accelerator_class", "evidence_sha256", "provider", "status"], "accelerator evidence"); identifier(record.accelerator_class, "accelerator class"); identifier(record.provider, "accelerator provider"); digest(record.evidence_sha256, "accelerator evidence digest"); if (record.status !== "VERIFIED" || !evidence.results.has(record.evidence_sha256)) throw new PCMError("accelerator result was not independently verified"); });
    if (!Number.isSafeInteger(lineage.total_independently_verified_tokens) || lineage.total_independently_verified_tokens < 0) throw new PCMError("verified token count is invalid");
    if (lineage.classification === "NATIVE_TRAINED" && initial.kind !== "RANDOM_INITIALIZATION") throw new PCMError("native training lineage is contradictory");
    if (["DERIVATIVE_FINE_TUNE", "QUANTIZED_DERIVATIVE"].includes(lineage.classification) && (lineage.parent_checkpoint_sha256 === null || initial.kind !== "PRETRAINED_MODEL")) throw new PCMError("derivative lineage is contradictory");
    if (lineage.classification === "EXTERNAL_UNMODIFIED" && (lineage.parent_checkpoint_sha256 !== null || initial.kind !== "EXTERNAL_MODEL" || initial.sha256 !== lineage.final_model_sha256)) throw new PCMError("external model lineage is contradictory");
    return lineage;
  }
  function passResult(value, label) { exactKeys(value, ["decision", "evidence_sha256"], label); if (value.decision !== "PASS") throw new PCMError(`${label} did not pass`); digest(value.evidence_sha256, `${label} evidence`); return value; }
  async function validateAuthorization(authorization, lineage, lineageBytes, candidateBytes, contractBytes, evidence) {
    exactKeys(authorization, ["capability_result", "claims", "contract_sha256", "lifecycle", "lineage_sha256", "model_sha256", "qualification", "release_id", "reload_result", "runtime_replay_result", "schema"], "release authorization");
    if (authorization.schema !== "mfenx/release-authorization/v1") throw new PCMError("unsupported release authorization profile"); identifier(authorization.release_id, "release_id");
    if (authorization.model_sha256 !== await sha256(candidateBytes) || authorization.lineage_sha256 !== await sha256(lineageBytes) || authorization.contract_sha256 !== await sha256(contractBytes)) throw new PCMError("release authorization artifact binding failed");
    const qualification = authorization.qualification; exactKeys(qualification, ["capability_accuracy_ppm", "comparative_delta_ppm", "evaluation_samples", "profile", "reliability_success_ppm", "safety_critical_failures"], "release qualification");
    if (!["DEMONSTRATION", "PRODUCTION"].includes(qualification.profile)) throw new PCMError("unsupported release qualification profile");
    for (const field of ["capability_accuracy_ppm", "reliability_success_ppm"]) if (!Number.isSafeInteger(qualification[field]) || qualification[field] < 0 || qualification[field] > 1000000) throw new PCMError(`${field} is invalid`);
    if (!Number.isSafeInteger(qualification.comparative_delta_ppm) || qualification.comparative_delta_ppm < -1000000 || qualification.comparative_delta_ppm > 1000000 || !Number.isSafeInteger(qualification.evaluation_samples) || qualification.evaluation_samples < 1 || !Number.isSafeInteger(qualification.safety_critical_failures) || qualification.safety_critical_failures < 0) throw new PCMError("release qualification metrics are invalid");
    const productionQualified = qualification.evaluation_samples >= 1000 && qualification.capability_accuracy_ppm >= 950000 && qualification.safety_critical_failures === 0 && qualification.reliability_success_ppm >= 999000 && qualification.comparative_delta_ppm >= -10000;
    if (qualification.profile === "PRODUCTION" && !productionQualified) throw new PCMError("production qualification thresholds were not met");
    if (!Array.isArray(authorization.lifecycle) || authorization.lifecycle.length !== LIFECYCLE.length) throw new PCMError("training lifecycle is incomplete"); const life = Object.create(null);
    authorization.lifecycle.forEach((record, index) => { exactKeys(record, ["evidence_class", "evidence_sha256", "state", "status"], `lifecycle ${index}`); if (record.state !== LIFECYCLE[index] || record.status !== "VERIFIED") throw new PCMError("training lifecycle order or verification is invalid"); identifier(record.evidence_class, "lifecycle evidence class"); life[record.state] = digest(record.evidence_sha256, "lifecycle evidence"); if (!evidence.all.has(record.evidence_sha256)) throw new PCMError("lifecycle evidence is not a signed artifact"); });
    const reload = passResult(authorization.reload_result, "reload result"), capability = passResult(authorization.capability_result, "capability result"), replay = passResult(authorization.runtime_replay_result, "runtime replay result");
    for (const result of [reload, capability, replay]) if (!evidence.results.has(result.evidence_sha256)) throw new PCMError("release result was not independently verified");
    const derived = evidence.qualifications.get(capability.evidence_sha256); if (!derived || ["capability_accuracy_ppm", "comparative_delta_ppm", "evaluation_samples"].some((name) => qualification[name] !== derived[name])) throw new PCMError("release qualification disagrees with independently verified evidence");
    if (qualification.profile === "PRODUCTION" && ["reliability_success_ppm", "safety_critical_failures"].some((name) => qualification[name] !== derived[name])) throw new PCMError("production qualification lacks verified reliability or safety evidence");
    if (life.RELOAD_VERIFIED !== reload.evidence_sha256 || life.CAPABILITY_MEASURED !== capability.evidence_sha256 || life.RELEASE_ELIGIBLE !== replay.evidence_sha256) throw new PCMError("release lifecycle evidence is contradictory");
    if (!Array.isArray(authorization.claims) || authorization.claims.length < 1 || authorization.claims.length > 32) throw new PCMError("claims are invalid"); const seen = new Set();
    authorization.claims.forEach((claim) => { exactKeys(claim, ["claim", "evidence_class", "evidence_sha256", "subject"], "claim"); const name = identifier(claim.claim, "claim name"); identifier(claim.evidence_class, "claim evidence class"); identifier(claim.subject, "claim subject"); digest(claim.evidence_sha256, "claim evidence"); if (!evidence.all.has(claim.evidence_sha256)) throw new PCMError("claim evidence is not a signed artifact"); if (seen.has(name)) throw new PCMError("duplicate public claim"); seen.add(name);
      if (name === "TRAINED_BY" && (lineage.classification !== "NATIVE_TRAINED" || claim.subject !== lineage.trainer_identity.name || claim.evidence_class !== "native-lineage" || claim.evidence_sha256 !== authorization.lineage_sha256)) throw new PCMError("TRAINED_BY lacks native lineage evidence");
      else if (name === "TRAINING_COMPLETE" && (claim.evidence_class !== "verified-final-training-receipt" || claim.evidence_sha256 !== life.OUTPUT_PUBLISHED)) throw new PCMError("TRAINING_COMPLETE lacks final receipt evidence");
      else if (name === "FUNCTIONAL" && (claim.evidence_class !== "reload-and-inference" || claim.evidence_sha256 !== reload.evidence_sha256)) throw new PCMError("FUNCTIONAL lacks reload evidence");
      else if (name === "PRODUCTION_GRADE" && (qualification.profile !== "PRODUCTION" || !productionQualified || claim.evidence_class !== "production-qualification" || claim.evidence_sha256 !== capability.evidence_sha256)) throw new PCMError("PRODUCTION_GRADE lacks qualification evidence");
      else if (name === "OUTPERFORMS_REFERENCE" && (claim.evidence_class !== "controlled-comparative-evaluation" || claim.evidence_sha256 !== capability.evidence_sha256)) throw new PCMError("OUTPERFORMS_REFERENCE lacks comparative evidence");
      else if (!["TRAINED_BY", "TRAINING_COMPLETE", "FUNCTIONAL", "PRODUCTION_GRADE", "OUTPERFORMS_REFERENCE"].includes(name)) throw new PCMError(`unsupported public claim: ${name}`);
    });
    return { ...authorization, activation_decision: "NOT_AUTHORIZED_FOR_ACTIVATION", activation_prerequisite: "VERIFIED_RUNTIME_REPLAY_REQUIRED" };
  }
  function tensorManifest(value, label) {
    exactKeys(value, ["all_initializer_elements", "parameter_count", "parameter_value_bytes", "tensors"], label);
    for (const name of ["all_initializer_elements", "parameter_count", "parameter_value_bytes"]) if (!Number.isSafeInteger(value[name]) || value[name] < 0) throw new PCMError(`${label} count is invalid`);
    if (!Array.isArray(value.tensors) || value.tensors.length < 1) throw new PCMError(`${label} tensor list is empty`); const names = new Set(); let all = 0, parameters = 0, parameterBytes = 0;
    value.tensors.forEach((tensor) => { exactKeys(tensor, ["dtype", "elements", "is_parameter", "name", "shape", "value_bytes", "value_sha256"], "tensor record"); identifier(tensor.dtype, "tensor dtype"); identifier(tensor.name, "tensor name"); digest(tensor.value_sha256, "tensor value digest"); if (names.has(tensor.name)) throw new PCMError("duplicate tensor name"); names.add(tensor.name); if (!Array.isArray(tensor.shape) || tensor.shape.some((item) => !Number.isSafeInteger(item) || item < 1) || !Number.isSafeInteger(tensor.elements) || tensor.elements < 1 || tensor.elements !== tensor.shape.reduce((a, b) => a * b, 1) || !Number.isSafeInteger(tensor.value_bytes) || tensor.value_bytes < 1 || typeof tensor.is_parameter !== "boolean") throw new PCMError("tensor shape or size is invalid"); all += tensor.elements; if (tensor.is_parameter) { parameters += tensor.elements; parameterBytes += tensor.value_bytes; } });
    if (all !== value.all_initializer_elements || parameters !== value.parameter_count || parameterBytes !== value.parameter_value_bytes) throw new PCMError(`${label} aggregate is invalid`); return value;
  }
  async function validateScale(scale, lineage, lineageBytes, sourceBytes, candidateBytes) {
    exactKeys(scale, ["architecture_dimensions", "checkpoint_bytes", "compression", "deployment", "inference_measurement", "lineage_sha256", "native_model", "scale_decision", "schema", "training_scale"], "model scale");
    if (scale.schema !== "mfenx/model-scale/v1" || scale.scale_decision !== "SCALE_RECORDED" || scale.lineage_sha256 !== await sha256(lineageBytes)) throw new PCMError("model scale profile or lineage binding is invalid");
    exactKeys(scale.architecture_dimensions, ["attention_heads", "context_length", "hidden_width", "layers", "vocabulary_size"], "architecture dimensions"); Object.values(scale.architecture_dimensions).forEach((value) => { if (value !== null && (!Number.isSafeInteger(value) || value < 1)) throw new PCMError("architecture dimension is invalid"); });
    const native = scale.native_model, deployment = scale.deployment; exactKeys(native, ["model_sha256", "native_precision", "tensor_manifest", "theoretical_weight_bytes", "total_parameters", "trainable_parameters"], "native model scale"); exactKeys(deployment, ["artifact_bytes", "deployment_precision", "model_sha256", "original_parameter_count", "tensor_manifest"], "deployment scale");
    const nativeManifest = tensorManifest(native.tensor_manifest, "native tensor manifest"), deploymentManifest = tensorManifest(deployment.tensor_manifest, "deployment tensor manifest");
    if (native.model_sha256 !== await sha256(sourceBytes) || deployment.model_sha256 !== await sha256(candidateBytes) || deployment.model_sha256 !== lineage.final_model_sha256) throw new PCMError("model scale artifact identity is invalid");
    if (native.total_parameters !== nativeManifest.parameter_count || native.trainable_parameters !== nativeManifest.parameter_count || native.theoretical_weight_bytes !== nativeManifest.parameter_value_bytes || deployment.original_parameter_count !== native.total_parameters || deployment.artifact_bytes !== candidateBytes.length) throw new PCMError("model scale parameter or byte accounting is invalid");
    identifier(native.native_precision, "native precision"); identifier(deployment.deployment_precision, "deployment precision");
    exactKeys(scale.checkpoint_bytes, ["model", "optimizer", "optimizer_status"], "checkpoint bytes"); if (scale.checkpoint_bytes.model !== sourceBytes.length || scale.checkpoint_bytes.optimizer !== 0 || !["NOT_RETAINED", "NOT_APPLICABLE"].includes(scale.checkpoint_bytes.optimizer_status)) throw new PCMError("checkpoint byte accounting is invalid");
    exactKeys(scale.compression, ["method", "ratio_denominator", "ratio_numerator"], "compression"); identifier(scale.compression.method, "quantization method"); if (scale.compression.ratio_numerator !== sourceBytes.length || scale.compression.ratio_denominator !== candidateBytes.length) throw new PCMError("compression ratio is invalid");
    exactKeys(scale.training_scale, ["tokens_per_parameter", "verified_training_tokens"], "training scale"); exactKeys(scale.training_scale.tokens_per_parameter, ["denominator", "numerator"], "tokens per parameter"); if (scale.training_scale.verified_training_tokens !== lineage.total_independently_verified_tokens || scale.training_scale.tokens_per_parameter.numerator !== lineage.total_independently_verified_tokens || scale.training_scale.tokens_per_parameter.denominator !== native.total_parameters) throw new PCMError("training scale is invalid");
    exactKeys(scale.inference_measurement, ["peak_ram_bytes", "peak_vram_bytes", "samples_per_second", "status"], "inference measurement"); if (scale.inference_measurement.status !== "NOT_MEASURED" || [scale.inference_measurement.peak_ram_bytes, scale.inference_measurement.peak_vram_bytes, scale.inference_measurement.samples_per_second].some((value) => value !== null)) throw new PCMError("unsupported inference measurement record");
    return scale;
  }
  async function domainHash(domain, value) { return sha256(join(ascii(domain), ascii(canonical(value)))); }
  async function powerHouseRootprint(statement) {
    const artifact = { embedded_proof: { proof: { evidence_types: [AUTHORIZATION_EVIDENCE, EVIDENCE, LINEAGE_EVIDENCE, SCALE_EVIDENCE, RELEASE_EVIDENCE] }, protocol: STATEMENT_SCHEMA, public_inputs: { statement_sha256: await sha256(ascii(canonical(statement))) } }, phx_fingerprint: "", provenance: { producer: "mfenx-ckodmk", serial: statement.serial }, schema: "power-house/pha/v1" };
    artifact.phx_fingerprint = await domainHash("power-house:pha:v1:phx-fingerprint\0", { embedded_proof: artifact.embedded_proof, provenance: artifact.provenance, schema: artifact.schema });
    const label = "pcm-statement";
    const branchId = await domainHash("power-house:rootprint:v1:branch-id\0", { artifact_phx_fingerprint: artifact.phx_fingerprint, label, parents: [] });
    return { branches: { [branchId]: { artifact, id: branchId, label, parents: [], sequence: 0 } }, root_branch: branchId, schema: "power-house/rootprint/v1" };
  }
  function protectedFields(bundle) { return { issuer: bundle.issuer, rootprint: bundle.rootprint, schema: bundle.schema, statement: bundle.statement, statement_root_id: bundle.statement_root_id }; }

  async function verifyEnvelope(raw, trustedKey, now = Date.now()) {
    if (!(raw instanceof Uint8Array) || raw.length < 1 || raw.length > LIMITS.pcm) throw new PCMError("PCM is empty or exceeds its byte limit");
    let text; try { text = new TextDecoder("utf-8", { fatal: true }).decode(raw); } catch { throw new PCMError("PCM is not valid UTF-8"); }
    if (!global.CKODMKBrowserGate) throw new PCMError("browser gate is unavailable");
    const bundle = global.CKODMKBrowserGate.parseStrictJson(text);
    exactKeys(bundle, ["issuer", "payloads", "rootprint", "schema", "signature", "statement", "statement_root_id"], "PCM");
    if (bundle.schema !== SCHEMA) throw new PCMError("unsupported PCM schema");
    const statement = bundle.statement;
    exactKeys(statement, ["admission", "artifacts", "evidence", "expires_at", "issued_at", "schema", "serial"], "statement");
    if (statement.schema !== STATEMENT_SCHEMA || !/^[a-z0-9][a-z0-9._-]{0,127}$/.test(statement.serial)) throw new PCMError("unsupported PCM statement");
    const expectedRootprint = await powerHouseRootprint(statement);
    if (!ROOT.test(bundle.statement_root_id) || canonical(bundle.rootprint) !== canonical(expectedRootprint) || bundle.statement_root_id !== expectedRootprint.root_branch) throw new PCMError("PCM Power House Rootprint binding mismatch");
    const issued = time(statement.issued_at, "issued_at"), expires = time(statement.expires_at, "expires_at");
    if (issued > now || expires <= now || expires <= issued) throw new PCMError("PCM is not currently valid");
    exactKeys(statement.admission, ["candidate_artifact", "required_evidence"], "admission");
    if (statement.admission.candidate_artifact !== "candidate" || canonical(statement.admission.required_evidence) !== canonical([AUTHORIZATION_EVIDENCE, EVIDENCE, LINEAGE_EVIDENCE, SCALE_EVIDENCE, RELEASE_EVIDENCE])) throw new PCMError("unsupported admission policy");
    const expectedEvidence = [{ artifact: "authorization", type: AUTHORIZATION_EVIDENCE }, { candidate_artifact: "candidate", contract_artifact: "contract", dataset_artifact: "dataset", runtime_profile: PROFILE, source_artifact: "source", type: EVIDENCE }, { artifact: "lineage", type: LINEAGE_EVIDENCE }, { artifact: "scale", type: SCALE_EVIDENCE }, { artifact: "release_evidence", type: RELEASE_EVIDENCE }];
    if (canonical(statement.evidence) !== canonical(expectedEvidence)) throw new PCMError("unsupported evidence profile");
    const issuer = bundle.issuer;
    exactKeys(issuer, ["algorithm", "key_id", "name", "public_key_spki_der_base64"], "issuer");
    if (issuer.algorithm !== ALGORITHM || !SHA.test(issuer.key_id) || !/^[\x20-\x7e]{1,128}$/.test(issuer.name)) throw new PCMError("PCM issuer record is invalid");
    if (!(trustedKey instanceof Uint8Array) || await sha256(trustedKey) !== issuer.key_id) throw new PCMError("PCM issuer is not the selected trusted key");
    const embedded = base64(issuer.public_key_spki_der_base64, 65536, "issuer key");
    if (embedded.length !== trustedKey.length || embedded.some((byte, index) => byte !== trustedKey[index])) throw new PCMError("embedded key differs from trusted key");
    exactKeys(bundle.signature, ["algorithm", "value_base64"], "signature");
    if (bundle.signature.algorithm !== ALGORITHM) throw new PCMError("unsupported signature algorithm");
    const signature = base64(bundle.signature.value_base64, 64, "signature"); if (signature.length !== 64) throw new PCMError("signature must contain 64 bytes");
    const scalar = (bytes) => bytes.reduce((value, byte) => (value << 8n) | BigInt(byte), 0n), r = scalar(signature.slice(0, 32)), s = scalar(signature.slice(32));
    if (r < 1n || r >= P256_ORDER || s < 1n || s > P256_ORDER / 2n) throw new PCMError("signature is not canonical low-S P-256 form");
    let key; try { key = await global.crypto.subtle.importKey("spki", trustedKey, { name: "ECDSA", namedCurve: "P-256" }, false, ["verify"]); } catch { throw new PCMError("trusted key import failed"); }
    if (!await global.crypto.subtle.verify({ name: "ECDSA", hash: "SHA-256" }, key, signature, ascii(canonical(protectedFields(bundle))))) throw new PCMError("PCM signature verification failed");
    exactKeys(statement.artifacts, ARTIFACTS, "artifacts"); exactKeys(bundle.payloads, ARTIFACTS, "payloads");
    const decoded = Object.create(null);
    for (const name of ARTIFACTS) {
      const record = statement.artifacts[name]; exactKeys(record, ["bytes", "media_type", "name", "sha256"], `${name} artifact`);
      const bytes = base64(bundle.payloads[name], LIMITS[name], `${name} payload`);
      if (!Number.isSafeInteger(record.bytes) || record.bytes !== bytes.length || record.media_type !== MEDIA[name] || typeof record.name !== "string" || !/^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$/.test(record.name) || !SHA.test(record.sha256) || record.sha256 !== await sha256(bytes)) throw new PCMError(`${name} artifact binding failed`);
      decoded[name] = { bytes, record };
    }
    const artifactDigests = new Set(ARTIFACTS.filter((name) => name !== "release_evidence").map((name) => decoded[name].record.sha256));
    const evidenceSet = await validateEvidenceSet(global.CKODMKBrowserGate.parseStrictJson(new TextDecoder("utf-8", { fatal: true }).decode(decoded.release_evidence.bytes)), artifactDigests);
    const lineage = await validateLineage(global.CKODMKBrowserGate.parseStrictJson(new TextDecoder("utf-8", { fatal: true }).decode(decoded.lineage.bytes)), decoded.candidate.bytes, evidenceSet);
    const authorization = await validateAuthorization(global.CKODMKBrowserGate.parseStrictJson(new TextDecoder("utf-8", { fatal: true }).decode(decoded.authorization.bytes)), lineage, decoded.lineage.bytes, decoded.candidate.bytes, decoded.contract.bytes, evidenceSet);
    const scale = await validateScale(global.CKODMKBrowserGate.parseStrictJson(new TextDecoder("utf-8", { fatal: true }).decode(decoded.scale.bytes)), lineage, decoded.lineage.bytes, decoded.source.bytes, decoded.candidate.bytes);
    return { activationDecision: authorization.activation_decision, authorization, bundle, decoded, evidenceSet, issuer: issuer.name, lineage, rootId: bundle.statement_root_id, scale, expiresAt: statement.expires_at };
  }

  function state(decision, message) {
    const badge = global.document?.getElementById("pcm-decision"); if (badge) { badge.textContent = decision; badge.className = `decision-badge ${decision === "BLOCK" ? "block" : decision === "ADMITTED" ? "pass" : ""}`; }
    const status = global.document?.getElementById("pcm-status"); if (status) status.textContent = message;
  }
  function check(id, status, label) { const row = global.document?.getElementById(id); if (!row) return; row.dataset.state = status; row.querySelector("b").textContent = label; }
  function resetAdmission(message = "Select a PCM or run the included trained-model package.") {
    admittedCandidate = null;
    const download = global.document.getElementById("download-admitted-model");
    download.disabled = true;
    for (const id of ["pcm-root-id", "pcm-issuer", "pcm-expiry", "pcm-artifact", "pcm-lineage", "pcm-authorization", "pcm-scale", "pcm-parameters", "pcm-precision"]) global.document.getElementById(id).textContent = "Not verified";
    check("pcm-check-package", "waiting", "Waiting"); check("pcm-check-replay", "waiting", "Waiting"); check("pcm-check-admission", "waiting", "Locked");
    state("READY", message);
  }
  function blockCurrentCheck() {
    for (const id of ["pcm-check-package", "pcm-check-replay", "pcm-check-admission"]) { const row = global.document.getElementById(id); if (row?.dataset.state !== "pass") { check(id, "block", "Blocked"); return; } }
  }
  async function fetchBytes(path, maximum, label) { const response = await fetch(path, { cache: "no-store", credentials: "same-origin" }); if (!response.ok) throw new PCMError(`${label} could not be loaded`); const bytes = new Uint8Array(await response.arrayBuffer()); if (bytes.length < 1 || bytes.length > maximum) throw new PCMError(`${label} exceeds its byte limit`); return bytes; }
  async function admit(raw, trustedKey, requiredKeyId = null) {
    const download = global.document.getElementById("download-admitted-model");
    check("pcm-check-package", "checking", "Checking");
    state("VERIFYING", "Checking signature, RootID, trust key, validity and artifact bindings…");
    if (requiredKeyId && await sha256(trustedKey) !== requiredKeyId) throw new PCMError("built-in release key pin mismatch");
    const verified = await verifyEnvelope(raw, trustedKey);
    check("pcm-check-package", "pass", "Passed"); check("pcm-check-replay", "checking", "Running");
    const files = Object.fromEntries(ARTIFACTS.map((name) => [name, new File([verified.decoded[name].bytes], verified.decoded[name].record.name, { type: verified.decoded[name].record.media_type })]));
    const contractDigest = verified.bundle.statement.artifacts.contract.sha256;
    global.document.getElementById("pcm-root-id").textContent = verified.rootId;
    global.document.getElementById("pcm-issuer").textContent = verified.issuer;
    global.document.getElementById("pcm-expiry").textContent = verified.expiresAt;
    global.document.getElementById("pcm-artifact").textContent = verified.decoded.candidate.record.sha256;
    global.document.getElementById("pcm-lineage").textContent = verified.lineage.classification;
    global.document.getElementById("pcm-authorization").textContent = verified.activationDecision;
    global.document.getElementById("pcm-scale").textContent = verified.scale.scale_decision;
    global.document.getElementById("pcm-parameters").textContent = verified.scale.native_model.total_parameters.toLocaleString("en-US");
    global.document.getElementById("pcm-precision").textContent = `${verified.scale.native_model.native_precision} checkpoint; ${verified.scale.deployment.deployment_precision} deployment`;
    state("REPLAYING", "Signature accepted. Replaying every required behavioral check before admission…");
    global.CKODMKBrowserGate.loadInputs({ source: files.source, candidate: files.candidate, dataset: files.dataset, contract: files.contract, contractDigest, generation: { implementation: "mfenx-ckodmk-pcm", version: "0.5.1", profile: PROFILE, statement_root_id: verified.rootId } });
    global.document.getElementById("gate-form").requestSubmit();
    for (let attempt = 0; attempt < 6000; attempt += 1) {
      await new Promise((resolve) => global.setTimeout(resolve, 100));
      const decision = global.document.getElementById("gate-decision").textContent;
      if (!["PASS", "BLOCK", "INCONCLUSIVE"].includes(decision)) continue;
      if (global.document.getElementById("gate-contract-digest").value !== contractDigest) throw new PCMError("active contract changed during replay");
      if (decision !== "PASS") { state("BLOCK", "Required behavioral checks failed. The model was not admitted."); throw new PCMError("behavioral replay blocked admission"); }
      verified.activationDecision = "AUTHORIZED_FOR_ACTIVATION"; global.document.getElementById("pcm-authorization").textContent = verified.activationDecision; admittedCandidate = files.candidate; download.disabled = false; check("pcm-check-replay", "pass", "Passed"); check("pcm-check-admission", "pass", "Unlocked"); state("ADMITTED", "Signed lineage, closed evidence and required runtime replay passed. The candidate is admitted in this session."); return verified;
    }
    throw new PCMError("behavioral replay timed out");
  }
  async function selected() { const file = global.document.getElementById("pcm-file").files[0], key = global.document.getElementById("pcm-trusted-key").files[0]; if (!file || file.size > LIMITS.pcm) throw new PCMError("select a bounded .pcm file"); if (!key || key.size < 1 || key.size > 65536) throw new PCMError("select the issuer's trusted SPKI DER public key"); return admit(new Uint8Array(await file.arrayBuffer()), new Uint8Array(await key.arrayBuffer())); }
  async function included() { const key = await fetchBytes("trust/pcm-release-public.der", 65536, "trusted release key"); return admit(await fetchBytes("pcm/optdigits-cnn-int8.pcm", LIMITS.pcm, "included PCM"), key, TRUSTED_KEY_ID); }
  function download() { if (!admittedCandidate) return; const url = URL.createObjectURL(admittedCandidate), anchor = global.document.createElement("a"); anchor.href = url; anchor.download = admittedCandidate.name; anchor.click(); global.setTimeout(() => URL.revokeObjectURL(url), 0); }
  function action(operation) { return async () => { const buttons = ["verify-pcm", "verify-included-pcm"].map((id) => global.document.getElementById(id)); resetAdmission("Preparing a new admission check…"); buttons.forEach((button) => { button.disabled = true; }); try { await operation(); } catch (error) { blockCurrentCheck(); state("BLOCK", `Fail closed: ${error.message || "PCM verification failed"}`); } finally { buttons.forEach((button) => { button.disabled = false; }); } }; }
  function bind() { global.document.getElementById("verify-pcm").addEventListener("click", action(selected)); global.document.getElementById("verify-included-pcm").addEventListener("click", action(included)); global.document.getElementById("download-admitted-model").addEventListener("click", download); for (const id of ["pcm-file", "pcm-trusted-key"]) global.document.getElementById(id).addEventListener("change", () => resetAdmission("Selected files changed. Verify again before admission.")); }

  const api = Object.freeze({ PCMError, canonical, validateAuthorization, validateEvidenceSet, validateLineage, validateScale, verifyEnvelope }); global.CKODMKPCM = api; if (typeof module !== "undefined" && module.exports) module.exports = api; if (global.document) bind();
})(typeof window !== "undefined" ? window : globalThis);
