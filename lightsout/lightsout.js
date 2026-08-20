"use strict";

const RELEASE_ROOT = "release/";
const CHECKPOINT_ROOT = "checkpoints/killed-and-resumed/";
const CHECKPOINT_PLAN_PATH = CHECKPOINT_ROOT + "plan.json";
const EXPECTED_MANIFEST_SHA256 = "3a08e8a61b6eb0a9cec94959fa5b666ff1441de6c310b07f388e9ce57a80296d";
const EXPECTED_MANIFEST_ENTRIES = 1298;
const RELEASE_CONTRACT = Object.freeze({
  acceptanceSchema: 2,
  imageSchema: 2,
  isaVersion: 6,
  resultSchema: 2,
  resourceCertificateSchema: 4,
  checkpointPlanSchema: 2,
  pieceReceiptSchema: 2,
  laneScheduleSchema: 1,
  laneSchedulePolicy: "deterministic_striped_v1",
  machineClass: "software_defined_local_supercomputer_v1",
  backend: "rarecomp_mfenx_local_cpu_lane_engine",
  opcode: "streamed_i32_gemm",
  verificationMethod: "independently_addressed_exact_replay"
});
const REQUIRED_ARTIFACTS = [
  "acceptance.json",
  "provenance/workload-contract.json",
  "provenance/memory-summary.json",
  "provenance/lane-overlap.json",
  "provenance/trace-summary.json",
  "provenance/post-kill-receipts.json",
  "provenance/post-resume-checkpoint.json",
  "provenance/containment.txt",
  "provenance/containment-probes.txt",
  "provenance/binary.sha256",
  "provenance/source-root.sha256",
  CHECKPOINT_PLAN_PATH,
  "artifacts/gemm.inspect.json",
  "artifacts/gemm.mfx.json",
  "artifacts/uninterrupted.result.json",
  "artifacts/uninterrupted.verify.json",
  "artifacts/resumed.result.json",
  "artifacts/resumed.verify.json"
];

const state = {
  manifest: null,
  acceptance: null,
  files: null,
  ready: false,
  binaryVerified: false
};

const byId = (id) => document.getElementById(id);

function updateClock() {
  byId("utc-clock").textContent = new Date().toISOString().slice(11, 19) + " UTC";
}

function humanBytes(value, decimals = 2) {
  const number = Number(value);
  if (!Number.isFinite(number) || number < 0) return "—";
  if (number >= 1024 ** 3) return (number / 1024 ** 3).toFixed(decimals) + " GiB";
  if (number >= 1024 ** 2) return (number / 1024 ** 2).toFixed(decimals) + " MiB";
  if (number >= 1024) return (number / 1024).toFixed(decimals) + " KiB";
  return number.toLocaleString() + " B";
}

function shortHash(value) {
  const text = String(value || "");
  return text.length > 23 ? text.slice(0, 14) + "…" + text.slice(-8) : text;
}

function equalArray(left, right) {
  return Array.isArray(left)
    && Array.isArray(right)
    && left.length === right.length
    && left.every((value, index) => value === right[index]);
}

function isRecord(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function equalJson(left, right) {
  if (Object.is(left, right)) return true;
  if (Array.isArray(left) || Array.isArray(right)) {
    return Array.isArray(left)
      && Array.isArray(right)
      && left.length === right.length
      && left.every((value, index) => equalJson(value, right[index]));
  }
  if (!isRecord(left) || !isRecord(right)) return false;
  const leftKeys = Object.keys(left).sort();
  const rightKeys = Object.keys(right).sort();
  return equalArray(leftKeys, rightKeys)
    && leftKeys.every((key) => equalJson(left[key], right[key]));
}

function assertSafeInteger(value, message, minimum = 0) {
  assert(Number.isSafeInteger(value) && value >= minimum, message);
}

function assertDigest(value, message) {
  assert(typeof value === "string" && /^[0-9a-f]{64}$/.test(value), message);
}

function pieceAssignments(pieceCount, lanes) {
  return Array.from({ length: pieceCount }, (_, pieceIndex) => ({
    piece_index: pieceIndex,
    lane_index: pieceIndex % lanes
  }));
}

function assignmentLabel(assignments) {
  return assignments.map((entry) => "p" + entry.piece_index + "→l" + entry.lane_index).join(", ");
}

function parseChecksumRecord(bytes, path, expectedTarget) {
  const text = decodeUtf8(bytes, path);
  const match = /^([0-9a-f]{64})  ([^\r\n]+)\n?$/.exec(text);
  assert(match, path + " is not one canonical SHA-256 record");
  const target = match[2].startsWith("./") ? match[2].slice(2) : match[2];
  assert(target === expectedTarget, path + " names an unexpected target");
  return match[1];
}

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

async function fetchBytes(path, maxBytes = 4 * 1024 * 1024) {
  const response = await fetch(RELEASE_ROOT + path, { cache: "no-store" });
  if (!response.ok) throw new Error(path + " returned HTTP " + response.status);
  assert(new URL(response.url).origin === location.origin, path + " redirected outside this site");
  const declaredLength = Number(response.headers.get("Content-Length"));
  if (Number.isFinite(declaredLength) && declaredLength > maxBytes) throw new Error(path + " exceeds its byte limit");
  const bytes = new Uint8Array(await response.arrayBuffer());
  assert(bytes.byteLength <= maxBytes, path + " exceeds its byte limit");
  return bytes;
}

async function sha256(bytes) {
  const digest = await crypto.subtle.digest("SHA-256", bytes);
  return Array.from(new Uint8Array(digest), (byte) => byte.toString(16).padStart(2, "0")).join("");
}

function decodeUtf8(bytes, path) {
  try {
    return new TextDecoder("utf-8", { fatal: true }).decode(bytes);
  } catch {
    throw new Error(path + " is not valid UTF-8");
  }
}

function parseJson(bytes, path) {
  try {
    return JSON.parse(decodeUtf8(bytes, path));
  } catch (error) {
    throw new Error(path + " is not valid JSON: " + error.message);
  }
}

function parseManifest(bytes) {
  const entries = new Map();
  const text = decodeUtf8(bytes, "SHA256SUMS");
  const lines = text.split("\n");
  if (lines.at(-1) === "") lines.pop();

  for (const line of lines) {
    const match = /^([0-9a-f]{64})  ([^\r\n]+)$/.exec(line);
    assert(match, "SHA256SUMS contains a malformed entry");
    const rawPath = match[2];
    assert(!rawPath.startsWith("../") && !rawPath.startsWith("/"), "SHA256SUMS contains an unsafe path");
    const path = rawPath.startsWith("./") ? rawPath.slice(2) : rawPath;
    const parts = path.split("/");
    assert(!path.startsWith("/") && !path.includes("\\") && parts.every((part) => part && part !== "." && part !== ".."), "SHA256SUMS contains an unsafe path");
    assert(!entries.has(path), "SHA256SUMS contains a duplicate path");
    entries.set(path, match[1]);
  }

  assert(entries.size === EXPECTED_MANIFEST_ENTRIES, "SHA256SUMS entry count changed");
  for (const path of REQUIRED_ARTIFACTS) assert(entries.has(path), "SHA256SUMS is missing " + path);
  assert(entries.has("bin/mfenx-local"), "SHA256SUMS is missing the release binary");
  return entries;
}

function setCheck(name, status, text) {
  const row = document.querySelector("[data-check='" + name + "']");
  if (!row) return;
  row.className = status;
  row.querySelector("b").textContent = text;
}

function releaseState(status, text) {
  const node = byId("release-state");
  node.className = "release-state " + status;
  node.querySelector("span").textContent = text;
}

function displayRelease(acceptance, memory, laneOverlap, postKill, uninterrupted, resumed) {
  const captured = new Date(acceptance.captured_at_utc);
  const capturedCopy = new Intl.DateTimeFormat("en-US", {
    month: "short", day: "2-digit", year: "numeric",
    hour: "2-digit", minute: "2-digit", second: "2-digit",
    hour12: false, timeZone: "UTC", timeZoneName: "short"
  }).format(captured);
  const ratio = acceptance.workload.logical_input_bytes / acceptance.workload.certified_managed_peak_bytes;
  const witnessedLanes = new Set(laneOverlap.witness.tasks.map((task) => task.lane_index)).size;

  byId("release-verdict").textContent = "PASS";
  byId("release-verdict").className = "pass";
  byId("capture-time").textContent = capturedCopy;
  byId("binary-hash").textContent = shortHash(acceptance.binary_sha256);
  byId("binary-hash").title = acceptance.binary_sha256;
  byId("source-hash").textContent = shortHash(acceptance.source_tree_sha256);
  byId("source-hash").title = acceptance.source_tree_sha256;
  byId("manifest-hash").textContent = shortHash(EXPECTED_MANIFEST_SHA256);
  byId("manifest-hash").title = EXPECTED_MANIFEST_SHA256;
  byId("logical-input").textContent = humanBytes(acceptance.workload.logical_input_bytes);
  byId("memory-ratio").textContent = ratio.toFixed(3) + "×";
  byId("managed-peak").textContent = humanBytes(acceptance.workload.certified_managed_peak_bytes);
  byId("lane-count").textContent = witnessedLanes + " / " + laneOverlap.configured_lanes;
  byId("peak-rss").textContent = humanBytes(memory.maxima_kib.VmHWM_kib * 1024);
  byId("swap-used").textContent = humanBytes(memory.maxima_kib.VmSwap_kib * 1024, 0);
  byId("gpu-count").textContent = String(acceptance.syscall_trace.product_gpu_device_paths);
  byId("network-count").textContent = String(acceptance.syscall_trace.product_network_syscalls);
  byId("kill-time").textContent = (memory.external_timings["killed-run"].wall_ns / 1e9).toFixed(3) + " s";
  byId("output-root").textContent = acceptance.uninterrupted_output_root;
  byId("uninterrupted-assignments").textContent = assignmentLabel(uninterrupted.metrics.executed_assignments);
  byId("retained-assignments").textContent = assignmentLabel(postKill.receipt_assignments);
  byId("reused-assignments").textContent = assignmentLabel(resumed.metrics.reused_assignments);
  byId("executed-assignments").textContent = assignmentLabel(resumed.metrics.executed_assignments);
  byId("capture-footer").textContent = "CAPTURED " + captured.toISOString().replace(".000Z", "Z");

  const timingNames = {
    input_chunk: "corrupt-input-chunk",
    checkpoint_piece: "corrupt-checkpoint-piece",
    checkpoint_receipt: "corrupt-checkpoint-receipt",
    image: "corrupt-image",
    result: "corrupt-result"
  };
  for (const [probe, timing] of Object.entries(timingNames)) {
    const card = document.querySelector("[data-probe='" + probe + "']");
    const passed = acceptance.corruption_rejection[probe] === "passed"
      && memory.external_timings[timing].exit_status === 1;
    card.className = passed ? "pass" : "fail";
    card.querySelector("b").textContent = passed ? "REJECTED · EXIT 1" : "NOT PROVEN";
  }
}

function validateTiming(memory, label, expectedExitStatus) {
  const timing = memory.external_timings[label];
  assert(isRecord(timing), "missing external timing for " + label);
  assert(timing.observer === "acceptance_harness_outside_product_process", label + " timing is not external");
  assertSafeInteger(timing.wall_ns, label + " has an invalid wall time");
  assert(timing.exit_status === expectedExitStatus, label + " has an unexpected exit status");
}

function validateRelease(files, rawFiles) {
  const acceptance = files["acceptance.json"];
  const workload = files["provenance/workload-contract.json"];
  const memory = files["provenance/memory-summary.json"];
  const laneOverlap = files["provenance/lane-overlap.json"];
  const trace = files["provenance/trace-summary.json"];
  const postKill = files["provenance/post-kill-receipts.json"];
  const postResume = files["provenance/post-resume-checkpoint.json"];
  const inspect = files["artifacts/gemm.inspect.json"];
  const image = files["artifacts/gemm.mfx.json"];
  const uninterrupted = files["artifacts/uninterrupted.result.json"];
  const uninterruptedVerify = files["artifacts/uninterrupted.verify.json"];
  const resumed = files["artifacts/resumed.result.json"];
  const resumedVerify = files["artifacts/resumed.verify.json"];
  const checkpointPlan = files[CHECKPOINT_PLAN_PATH];

  assert(acceptance.schema_version === RELEASE_CONTRACT.acceptanceSchema, "acceptance schema is not 2");
  assert(acceptance.status === "PASS" && acceptance.acceptance_level === "release" && acceptance.release_acceptance === true, "full release acceptance did not pass");
  assert(acceptance.product === "rarecomp-mfenx-local", "unexpected product");
  assert(Number.isFinite(Date.parse(acceptance.captured_at_utc)), "acceptance capture time is invalid");
  assertDigest(acceptance.binary_sha256, "accepted binary digest is invalid");
  assertDigest(acceptance.source_tree_sha256, "accepted source digest is invalid");
  assert(acceptance.build.package === "rarecomp-mfenx-local" && acceptance.build.binary === "mfenx-local", "build target changed");
  assert(acceptance.build.locked === true && acceptance.build.offline === true, "build was not locked and offline");
  assert(acceptance.build.forbidden_dependencies === 0 && acceptance.build.forbidden_dynamic_libraries === 0, "forbidden runtime dependency present");
  assertSafeInteger(acceptance.build.jobs, "build job count is invalid", 1);

  const machine = acceptance.machine;
  assert(machine.image_schema_version === RELEASE_CONTRACT.imageSchema, "machine image schema is not 2");
  assert(machine.isa_version === RELEASE_CONTRACT.isaVersion, "machine ISA is not MFENX 6");
  assert(machine.result_schema_version === RELEASE_CONTRACT.resultSchema, "result schema is not 2");
  assert(machine.resource_certificate_schema_version === RELEASE_CONTRACT.resourceCertificateSchema, "resource certificate schema is not 4");
  assert(machine.checkpoint_plan_schema_version === RELEASE_CONTRACT.checkpointPlanSchema, "checkpoint plan schema is not 2");
  assert(machine.piece_receipt_schema_version === RELEASE_CONTRACT.pieceReceiptSchema, "piece receipt schema is not 2");
  assert(machine.lane_schedule_schema_version === RELEASE_CONTRACT.laneScheduleSchema && machine.lane_schedule_instruction_index === 0, "lane schedule schema changed");
  assert(machine.lane_schedule_policy === RELEASE_CONTRACT.laneSchedulePolicy, "lane schedule policy changed");
  assert(machine.piece_to_lane_assignment === "lane_index = piece_index % lanes", "piece-to-lane assignment law changed");
  assert(machine.machine_class === RELEASE_CONTRACT.machineClass, "machine class is not the software-defined local supercomputer");
  assert(machine.backend === RELEASE_CONTRACT.backend, "backend is not the local CPU lane engine");
  assert(machine.opcode === RELEASE_CONTRACT.opcode, "machine opcode changed");
  assert(machine.verification_method === RELEASE_CONTRACT.verificationMethod, "verification method changed");
  assert(machine.gpu_devices_required === 0 && machine.network_transports_required === 0, "machine requires a GPU or network transport");
  assert(machine.runtime_telemetry_attestation === "self_reported" && machine.recovery_partition_attestation === "self_reported", "result attestation boundary changed");
  assert(machine.directory_metadata_sync === "available" && machine.process_crash_recovery === "supported", "durability contract changed");

  const containmentPair = acceptance.containment.mode + "/" + acceptance.containment.device_view;
  assert(new Set([
    "bubblewrap_network_namespace/bubblewrap_synthetic_dev",
    "unshare_network_namespace/unshare_synthetic_dev",
    "trace_only/host_traced"
  ]).has(containmentPair), "containment record is invalid");
  const containmentRecord = decodeUtf8(rawFiles["provenance/containment.txt"], "provenance/containment.txt");
  assert(containmentRecord === "containment=" + acceptance.containment.mode + "\ndevice_view=" + acceptance.containment.device_view + "\n", "containment record disagrees with acceptance");

  const binaryRecord = parseChecksumRecord(rawFiles["provenance/binary.sha256"], "provenance/binary.sha256", "bin/mfenx-local");
  const sourceRecord = parseChecksumRecord(rawFiles["provenance/source-root.sha256"], "provenance/source-root.sha256", "provenance/source-before.sha256");
  assert(binaryRecord === acceptance.binary_sha256, "binary provenance disagrees with acceptance");
  assert(sourceRecord === acceptance.source_tree_sha256, "source provenance disagrees with acceptance");

  for (const [key, value] of Object.entries(workload)) {
    assert(equalJson(acceptance.workload[key], value), "accepted workload disagrees on " + key);
  }
  const leftShape = acceptance.workload.left_shape;
  const rightShape = acceptance.workload.right_shape;
  assert(Array.isArray(leftShape) && leftShape.length === 2 && Array.isArray(rightShape) && rightShape.length === 2, "workload is not rank-two");
  for (const dimension of [...leftShape, ...rightShape]) assertSafeInteger(dimension, "workload has an invalid dimension", 1);
  assert(leftShape[1] === rightShape[0], "matrix inner dimensions disagree");
  const lanes = acceptance.workload.lanes;
  const pieces = workload.total_pieces;
  assertSafeInteger(lanes, "lane count is invalid", 2);
  assertSafeInteger(pieces, "piece count is invalid", lanes);
  const expectedAssignments = pieceAssignments(pieces, lanes);
  assert(equalJson(workload.expected_piece_lane_assignments, expectedAssignments), "workload assignments are not deterministic modulo stripes");
  assert(equalArray([...new Set(expectedAssignments.map((entry) => entry.lane_index))], Array.from({ length: lanes }, (_, index) => index)), "compiled schedule does not cover every lane");

  const expectedSchedule = {
    schema_version: RELEASE_CONTRACT.laneScheduleSchema,
    instruction_index: 0,
    policy: RELEASE_CONTRACT.laneSchedulePolicy,
    lanes,
    piece_count: pieces
  };
  assert(equalJson(workload.lane_schedule, expectedSchedule), "workload lane schedule changed");
  assert(workload.image_schema_version === RELEASE_CONTRACT.imageSchema && workload.isa_version === RELEASE_CONTRACT.isaVersion, "workload image identity changed");
  assert(workload.result_schema_version === RELEASE_CONTRACT.resultSchema && workload.resource_certificate_schema_version === RELEASE_CONTRACT.resourceCertificateSchema, "workload result or certificate schema changed");
  assert(workload.machine_class === RELEASE_CONTRACT.machineClass && workload.backend === RELEASE_CONTRACT.backend && workload.opcode === RELEASE_CONTRACT.opcode, "workload machine identity changed");
  assert(workload.gpu_required === 0 && workload.network_required === 0, "workload requires a GPU or network");

  assert(inspect.schema_version === RELEASE_CONTRACT.imageSchema && inspect.isa_version === RELEASE_CONTRACT.isaVersion, "inspect identity changed");
  assert(inspect.result_schema_version === RELEASE_CONTRACT.resultSchema && inspect.resource_certificate_schema_version === RELEASE_CONTRACT.resourceCertificateSchema, "inspect result or certificate schema changed");
  assert(inspect.lane_schedule_schema_version === RELEASE_CONTRACT.laneScheduleSchema && inspect.lane_schedule_instruction_index === 0, "inspect schedule schema changed");
  assert(inspect.lane_schedule_policy === RELEASE_CONTRACT.laneSchedulePolicy, "inspect schedule policy changed");
  assert(inspect.machine_class === RELEASE_CONTRACT.machineClass && inspect.backend === RELEASE_CONTRACT.backend, "inspect machine identity changed");
  assert(inspect.opcode === RELEASE_CONTRACT.opcode && inspect.verification === "exact_replay", "inspect execution contract changed");
  assert(inspect.gpu_required === false && inspect.network_required === false, "inspect requires a GPU or network");
  assert(inspect.lanes === lanes && inspect.piece_count === pieces, "inspect lane partition disagrees");
  assertDigest(inspect.image_digest, "inspect image digest is invalid");
  assertDigest(inspect.program_digest, "inspect program digest is invalid");

  assert(image.schema_version === RELEASE_CONTRACT.imageSchema && image.isa_version === RELEASE_CONTRACT.isaVersion, "machine image is not schema 2 / ISA 6");
  assert(image.machine_class === RELEASE_CONTRACT.machineClass && image.backend === RELEASE_CONTRACT.backend, "machine image class or backend changed");
  assert(image.program_digest === inspect.program_digest, "program digest disagrees with inspect");
  assert(equalJson(image.schedule, expectedSchedule), "machine image schedule is not the compiled deterministic schedule");
  assert(image.verification === "exact_replay", "machine image does not require exact replay");
  assert(Array.isArray(image.instructions) && image.instructions.length === 1 && image.instructions[0].opcode === RELEASE_CONTRACT.opcode, "machine image does not contain one streamed i32 GEMM instruction");
  assert(isRecord(image.inputs) && isRecord(image.inputs.left) && isRecord(image.inputs.right), "machine image input bindings are missing");
  assert(equalArray(image.inputs.left.ty.shape, leftShape) && equalArray(image.inputs.right.ty.shape, rightShape), "machine image input geometry changed");
  assert(equalJson(image.instructions[0].left, image.inputs.left) && equalJson(image.instructions[0].right, image.inputs.right), "instruction inputs are not the bound content-addressed tensors");
  const outputShape = [leftShape[0], rightShape[1]];
  assert(image.instructions[0].output_type.element === "i32" && equalArray(image.instructions[0].output_type.shape, outputShape), "instruction output geometry changed");
  assert(image.program.version === 1 && equalArray(image.program.outputs, [2]) && image.program.instructions.length === 3, "typed compiler program changed");
  assert(image.program.instructions[0].operation.op === "input" && image.program.instructions[0].operation.name === "left", "typed program lost its left input");
  assert(image.program.instructions[1].operation.op === "input" && image.program.instructions[1].operation.name === "right", "typed program lost its right input");
  assert(image.program.instructions[2].operation.op === "mat_mul" && image.program.instructions[2].operation.lhs === 0 && image.program.instructions[2].operation.rhs === 1, "typed program is not matrix multiplication");

  const leftBytes = leftShape[0] * leftShape[1] * 4;
  const rightBytes = rightShape[0] * rightShape[1] * 4;
  const logicalBytes = leftBytes + rightBytes;
  const outputBytes = outputShape[0] * outputShape[1] * 4;
  assertSafeInteger(logicalBytes, "logical input byte count is unsafe", 1);
  assertSafeInteger(outputBytes, "logical output byte count is unsafe", 1);
  assert(image.inputs.left.byte_length === leftBytes && image.inputs.right.byte_length === rightBytes, "input tensor byte lengths disagree with geometry");
  assert(workload.logical_input_bytes === logicalBytes, "logical input bytes are not independently reproducible");
  assert(workload.retained_output_storage_bytes === outputBytes, "logical output bytes are not independently reproducible");

  const certificate = image.resource_certificate;
  assert(certificate.schema_version === RELEASE_CONTRACT.resourceCertificateSchema, "resource certificate is not schema 4");
  assert(certificate.lanes === lanes && certificate.piece_count === pieces, "resource certificate lane partition disagrees");
  assertSafeInteger(certificate.rows_per_piece, "certificate rows per piece is invalid", 1);
  assert(Math.ceil(outputShape[0] / certificate.rows_per_piece) === pieces, "certificate piece count is not independently reproducible");
  assert(certificate.output_piece_bytes_per_lane === certificate.rows_per_piece * outputShape[1] * 4, "certificate output-piece buffer is not independently reproducible");
  assert(certificate.retained_input_storage_bytes === logicalBytes && certificate.retained_output_storage_bytes === outputBytes, "certificate retained tensor storage disagrees");
  assert(certificate.max_managed_bytes === acceptance.workload.managed_mib * 1024 ** 2 && certificate.max_io_bytes === acceptance.workload.io_mib * 1024 ** 2, "certificate admission limits disagree with the accepted command");
  const temporaryPieceStorage = lanes * certificate.output_piece_bytes_per_lane;
  const temporaryJsonStorage = 64 * 1024 + lanes * 1024;
  const checkpointStorage = outputBytes + 64 * 1024 + pieces * 1024 + temporaryPieceStorage + temporaryJsonStorage;
  const retainedStorage = logicalBytes + outputBytes + checkpointStorage;
  assert(certificate.checkpoint_temporary_piece_storage_upper_bound_bytes === temporaryPieceStorage, "certificate temporary piece storage is incomplete");
  assert(certificate.checkpoint_temporary_json_storage_upper_bound_bytes === temporaryJsonStorage, "certificate temporary JSON storage is incomplete");
  assert(certificate.retained_checkpoint_storage_upper_bound_bytes === checkpointStorage, "certificate checkpoint storage is incomplete");
  assert(workload.checkpoint_temporary_piece_storage_upper_bound_bytes === temporaryPieceStorage && workload.checkpoint_temporary_json_storage_upper_bound_bytes === temporaryJsonStorage, "workload temporary checkpoint bounds disagree");
  assert(workload.retained_checkpoint_storage_upper_bound_bytes === checkpointStorage && workload.retained_storage_upper_bound_bytes === retainedStorage, "workload retained storage bound disagrees");
  const positiveCertificateFields = [
    "coordinator_base_reserve_bytes", "piece_metadata_reserve_bytes", "lane_stack_bytes",
    "lane_control_reserve_bytes", "aggregate_execution_peak_bytes", "finalization_peak_bytes",
    "verification_peak_bytes"
  ];
  for (const field of positiveCertificateFields) assertSafeInteger(certificate[field], "certificate field " + field + " is invalid", 1);
  const certifiedManagedPeak = Math.max(certificate.aggregate_execution_peak_bytes, certificate.finalization_peak_bytes, certificate.verification_peak_bytes);
  assert(certificate.certified_managed_peak_bytes === certifiedManagedPeak && inspect.certified_managed_peak_bytes === certifiedManagedPeak && workload.certified_managed_peak_bytes === certifiedManagedPeak, "certified managed peak is not the maximum phase peak");
  assert(certifiedManagedPeak <= certificate.max_managed_bytes, "certified managed peak exceeds its admission ceiling");
  assert(logicalBytes > 4 * certifiedManagedPeak && workload.input_more_than_four_times_managed === true, "logical input is not greater than four certified managed peaks");
  assert(workload.input_to_managed_ratio_milli === Math.floor(logicalBytes * 1000 / certifiedManagedPeak), "workload memory ratio disagrees");
  for (const field of ["retained_input_storage_bytes", "retained_output_storage_bytes", "checkpoint_temporary_piece_storage_upper_bound_bytes", "checkpoint_temporary_json_storage_upper_bound_bytes", "retained_checkpoint_storage_upper_bound_bytes"]) {
    assert(inspect[field] === certificate[field], "inspect disagrees with the certificate on " + field);
  }

  assert(equalJson(acceptance.memory, memory), "accepted memory evidence differs from the selected memory summary");
  assert(memory.external_runtime_telemetry_attestation === "external_proc_and_monotonic_process_observer", "memory evidence is not externally attested");
  assert(memory.virtual_memory_within_external_limit === true, "external virtual-memory gate failed");
  const expectedLimit = acceptance.external_hard_rlimit_as_mib * 1024 ** 2;
  assertSafeInteger(expectedLimit, "external address-space limit is invalid", 1);
  assert(memory.external_hard_rlimit_as_bytes === expectedLimit, "external address-space limit disagrees");
  for (const field of ["VmRSS_kib", "VmHWM_kib", "VmSize_kib", "VmPeak_kib"]) assertSafeInteger(memory.maxima_kib[field], "external observer captured no " + field, 1);
  assert(memory.maxima_kib.VmSwap_kib === 0, "product used swap");
  assert(memory.maxima_kib.VmSize_kib * 1024 <= expectedLimit && memory.maxima_kib.VmPeak_kib * 1024 <= expectedLimit, "observed virtual memory exceeded the external limit");
  for (const label of ["uninterrupted-run", "killed-run", "resumed-run"]) assertSafeInteger(memory.sample_counts[label], "missing /proc samples for " + label, 1);
  validateTiming(memory, "uninterrupted-run", 0);
  validateTiming(memory, "uninterrupted-verify", 0);
  validateTiming(memory, "killed-run", 137);
  validateTiming(memory, "resumed-run", 0);
  validateTiming(memory, "resumed-verify", 0);

  assert(equalJson(memory.external_lane_concurrency_attestation, laneOverlap), "memory summary and lane-overlap evidence disagree");
  assert(laneOverlap.observer === "acceptance_harness_outside_product_process_via_proc_task", "lane concurrency was not externally observed");
  assert(laneOverlap.primary_execution === "uninterrupted-run" && laneOverlap.raw_evidence === "memory/uninterrupted-run.lane-tasks.tsv", "lane witness does not cover primary execution");
  assert(laneOverlap.configured_lanes === lanes && laneOverlap.minimum_distinct_overlapping_lane_tasks === 2, "lane witness configuration disagrees");
  assertSafeInteger(laneOverlap.observed_lane_task_rows, "lane observer captured no task rows", 2);
  assert(laneOverlap.overlap_proven === true && isRecord(laneOverlap.witness) && Array.isArray(laneOverlap.witness.tasks), "physical CPU-lane overlap was not proven");
  const witness = laneOverlap.witness;
  assertSafeInteger(witness.product_pid, "lane witness product PID is invalid", 1);
  assertSafeInteger(witness.anchor_tid_reobserved_after_sweep, "lane witness anchor TID is invalid", 1);
  assert(Number.isFinite(witness.epoch_ns) && witness.epoch_ns > 0, "lane witness epoch is invalid");
  assert(witness.tasks.length >= 2, "lane witness contains fewer than two tasks");
  const witnessTids = new Set();
  const witnessLanes = new Set();
  for (const task of witness.tasks) {
    assert(task.pid === witness.product_pid, "lane witness task belongs to another process");
    assertSafeInteger(task.tid, "lane witness TID is invalid", 1);
    assertSafeInteger(task.lane_index, "lane witness index is invalid");
    assert(task.lane_index < lanes && task.comm === "mfx-lane-" + String(task.lane_index).padStart(2, "0"), "lane witness task name and index disagree");
    assert(typeof task.state === "string" && !new Set(["X", "Z"]).has(task.state), "lane witness contains a dead task");
    assertSafeInteger(task.starttime_ticks, "lane witness task start time is invalid", 1);
    assert(task.anchor_tid === witness.anchor_tid_reobserved_after_sweep && task.anchor_reobserved_after_sweep === 1, "lane witness anchor was not re-observed");
    witnessTids.add(task.tid);
    witnessLanes.add(task.lane_index);
  }
  assert(witnessTids.has(witness.anchor_tid_reobserved_after_sweep) && witnessTids.size >= 2 && witnessLanes.size >= 2, "lane witness does not prove two simultaneous physical lane tasks");
  assert(acceptance.independent_attestation.runtime_telemetry === "external_proc_and_monotonic_process_observer", "accepted runtime evidence is not external");
  assert(acceptance.independent_attestation.physical_lane_concurrency === "external_proc_task_overlap_witness" && acceptance.independent_attestation.physical_lane_concurrency_evidence === "provenance/lane-overlap.json", "accepted physical lane concurrency is not externally witnessed");
  assert(acceptance.independent_attestation.recovery_partition === "external_post_kill_and_post_resume_checkpoint_snapshots", "accepted recovery partition is not external");

  assert(equalJson(acceptance.syscall_trace, trace), "accepted syscall evidence differs from the selected trace summary");
  assertSafeInteger(trace.product_trace_files, "no product syscall trace was captured", 1);
  assertSafeInteger(trace.host_trace_files, "no host build trace was captured", 1);
  assert(trace.product_network_syscalls === 0 && trace.product_gpu_device_paths === 0 && trace.offline_build_ip_network_attempts === 0, "network, GPU, or offline-build containment trace failed");

  assert(postKill.observer === "acceptance_harness_after_process_group_exit", "post-kill snapshot is not external");
  assert(postResume.observer === "acceptance_harness_after_resumed_process_exit", "post-resume snapshot is not external");
  assert(postKill.expected_total_pieces === pieces && postResume.expected_total_pieces === pieces, "checkpoint snapshots disagree with the piece count");
  assert(equalJson(postKill.schedule, expectedSchedule) && equalJson(postResume.schedule, expectedSchedule), "checkpoint plan did not retain the compiled schedule");
  assertDigest(postKill.plan_sha256, "post-kill checkpoint plan digest is invalid");
  assert(postKill.plan_sha256 === postResume.plan_sha256, "checkpoint plan changed across restart");
  assert(Array.isArray(postKill.entries) && postKill.entries.length >= 1 && postKill.entries.length < pieces, "SIGKILL did not leave a partial durable partition");
  assert(postKill.receipt_count === postKill.entries.length, "post-kill receipt count disagrees");
  const validateCheckpointEntries = (entries, label) => entries.map((entry, position) => {
    assertSafeInteger(entry.index, label + " checkpoint index is invalid");
    assert(entry.index === position || label === "partial", label + " checkpoint indices are not complete and ordered");
    assert(entry.lane_index === entry.index % lanes, label + " receipt lane is not piece modulo lanes");
    assertSafeInteger(entry.piece_bytes, label + " piece byte count is invalid", 1);
    assertSafeInteger(entry.receipt_bytes, label + " receipt byte count is invalid", 1);
    assertDigest(entry.piece_sha256, label + " piece digest is invalid");
    assertDigest(entry.receipt_sha256, label + " receipt digest is invalid");
    return entry.index;
  });
  const postKillIndices = validateCheckpointEntries(postKill.entries, "partial");
  assert(equalArray(postKillIndices, [...postKillIndices].sort((left, right) => left - right)) && new Set(postKillIndices).size === postKillIndices.length, "post-kill indices are not unique and ordered");
  assert(equalArray(postKill.receipt_indices, postKillIndices), "post-kill receipt indices disagree with entries");
  const retainedAssignments = postKill.entries.map((entry) => ({ piece_index: entry.index, lane_index: entry.lane_index }));
  assert(equalJson(postKill.receipt_assignments, retainedAssignments), "post-kill receipt assignments are not typed modulo assignments");
  assert(Array.isArray(postResume.entries) && postResume.entries.length === pieces, "resume did not complete every checkpoint piece");
  const postResumeIndices = validateCheckpointEntries(postResume.entries, "complete");
  assert(equalArray(postResumeIndices, Array.from({ length: pieces }, (_, index) => index)), "post-resume checkpoint is not complete");
  assert(equalArray(postResume.receipt_indices, postResumeIndices), "post-resume receipt indices disagree with entries");
  assert(equalJson(postResume.receipt_assignments, expectedAssignments), "post-resume receipts do not contain every deterministic lane assignment");
  const resumedByIndex = new Map(postResume.entries.map((entry) => [entry.index, entry]));
  for (const entry of postKill.entries) assert(equalJson(resumedByIndex.get(entry.index), entry), "durable piece or receipt changed across restart");
  const retainedIndexSet = new Set(postKillIndices);
  const missingAssignments = expectedAssignments.filter((entry) => !retainedIndexSet.has(entry.piece_index));

  assert(checkpointPlan.schema_version === RELEASE_CONTRACT.checkpointPlanSchema, "durable checkpoint plan is not schema 2");
  assert(checkpointPlan.image_digest === inspect.image_digest, "durable checkpoint plan image digest disagrees");
  assert(checkpointPlan.output_type.element === "i32" && equalArray(checkpointPlan.output_type.shape, outputShape), "durable checkpoint output type changed");
  assert(equalJson(checkpointPlan.schedule, expectedSchedule), "durable checkpoint plan schedule changed");
  assert(checkpointPlan.rows_per_piece === certificate.rows_per_piece && checkpointPlan.piece_count === pieces, "durable checkpoint partition disagrees");
  assert(state.manifest.get(CHECKPOINT_PLAN_PATH) === postResume.plan_sha256, "published checkpoint plan digest disagrees with the external snapshot");
  for (let index = 0; index < pieces; index += 1) {
    const path = CHECKPOINT_ROOT + "receipt-" + String(index).padStart(8, "0") + ".json";
    const receipt = files[path];
    const rowStart = index * certificate.rows_per_piece;
    const rowCount = Math.min(certificate.rows_per_piece, outputShape[0] - rowStart);
    const pieceBytes = rowCount * outputShape[1] * 4;
    assert(receipt.schema_version === RELEASE_CONTRACT.pieceReceiptSchema, "durable piece receipt is not schema 2");
    assert(receipt.image_digest === inspect.image_digest && receipt.index === index, "durable piece receipt identity changed");
    assert(receipt.lane_index === index % lanes, "durable piece receipt lane is not piece modulo lanes");
    assert(receipt.row_start === rowStart && receipt.row_count === rowCount && receipt.byte_length === pieceBytes, "durable piece receipt geometry changed");
    assertDigest(receipt.content_blake3, "durable piece content digest is invalid");
    assert(state.manifest.get(path) === postResume.entries[index].receipt_sha256, "published piece receipt digest disagrees with the external snapshot");
    assert(postResume.entries[index].piece_bytes === pieceBytes, "published checkpoint piece size disagrees with its receipt");
  }

  const validateResult = (result, verifyReport, label) => {
    assert(result.schema_version === RELEASE_CONTRACT.resultSchema, label + " result schema is not 2");
    assert(result.machine_class === RELEASE_CONTRACT.machineClass && result.backend === RELEASE_CONTRACT.backend, label + " result machine identity changed");
    assert(result.image_digest === inspect.image_digest, label + " result image digest disagrees");
    assertDigest(result.resource_certificate_digest, label + " certificate digest is invalid");
    assert(Array.isArray(result.outputs) && result.outputs.length === 1 && result.outputs[0].index === 0, label + " result output set changed");
    const tensor = result.outputs[0].tensor;
    assertDigest(tensor.manifest_digest, label + " output root is invalid");
    assert(tensor.ty.element === "i32" && equalArray(tensor.ty.shape, outputShape) && tensor.byte_length === outputBytes, label + " output tensor contract changed");
    const metrics = result.metrics;
    assert(metrics.lanes === lanes && metrics.total_pieces === pieces, label + " result schedule disagrees");
    assert(!Object.hasOwn(metrics, "reused_piece_indices") && !Object.hasOwn(metrics, "executed_piece_indices"), label + " result retained obsolete untyped assignment fields");
    assert(Array.isArray(metrics.reused_assignments) && Array.isArray(metrics.executed_assignments), label + " result lacks typed assignments");
    assert(metrics.reused_pieces === metrics.reused_assignments.length && metrics.executed_pieces === metrics.executed_assignments.length, label + " result counters disagree with assignments");
    assert(metrics.certified_managed_peak_bytes === certifiedManagedPeak && metrics.retained_storage_bytes === retainedStorage, label + " result resource bounds disagree");
    assert(metrics.gpu_devices_required === 0 && metrics.network_transports_required === 0, label + " result requires a GPU or network");
    assert(metrics.runtime_telemetry_attestation === "self_reported" && metrics.recovery_partition_attestation === "self_reported", label + " result attestation boundary changed");
    assert(metrics.directory_metadata_sync === "available" && metrics.process_crash_recovery === "supported", label + " result durability contract changed");
    assert(result.verification.verified === true && result.verification.method === RELEASE_CONTRACT.verificationMethod, label + " inline exact replay failed");
    assert(result.verification.pieces_checked === pieces && result.verification.bytes_checked === outputBytes, label + " inline verifier coverage changed");
    assert(verifyReport.verified === true && verifyReport.method === RELEASE_CONTRACT.verificationMethod, label + " standalone exact replay failed");
    assert(verifyReport.pieces_checked === pieces && verifyReport.bytes_checked === outputBytes, label + " standalone verifier coverage changed");
  };
  validateResult(uninterrupted, uninterruptedVerify, "uninterrupted");
  validateResult(resumed, resumedVerify, "resumed");
  assert(uninterrupted.resource_certificate_digest === resumed.resource_certificate_digest, "resource certificate changed across restart");
  assert(equalJson(uninterrupted.metrics.reused_assignments, []) && equalJson(uninterrupted.metrics.executed_assignments, expectedAssignments), "fresh run did not execute every deterministic lane assignment");
  assert(uninterrupted.metrics.reused_pieces === 0 && uninterrupted.metrics.executed_pieces === pieces, "fresh run piece counters changed");
  assert(equalJson(resumed.metrics.reused_assignments, retainedAssignments), "restart did not reuse exactly the durable assignments");
  assert(equalJson(resumed.metrics.executed_assignments, missingAssignments), "restart did not execute only the missing assignments");
  const uninterruptedRoot = uninterrupted.outputs[0].tensor.manifest_digest;
  const resumedRoot = resumed.outputs[0].tensor.manifest_digest;
  assert(uninterruptedRoot === resumedRoot && uninterruptedRoot === acceptance.uninterrupted_output_root && resumedRoot === acceptance.resumed_output_root, "uninterrupted and resumed output roots differ");
  assert(acceptance.recovery === "passed_exact_partial_reuse_with_deterministic_lane_assignments", "release recovery gate did not pass");

  const corruptionTimings = {
    input_chunk: "corrupt-input-chunk",
    checkpoint_piece: "corrupt-checkpoint-piece",
    checkpoint_receipt: "corrupt-checkpoint-receipt",
    image: "corrupt-image",
    result: "corrupt-result"
  };
  assert(equalArray(Object.keys(acceptance.corruption_rejection).sort(), Object.keys(corruptionTimings).sort()), "corruption suite changed");
  for (const [probe, timing] of Object.entries(corruptionTimings)) {
    assert(acceptance.corruption_rejection[probe] === "passed", probe + " corruption was not rejected");
    validateTiming(memory, timing, 1);
  }

  return { acceptance, memory, laneOverlap, postKill, uninterrupted, resumed };
}

async function loadRelease() {
  releaseState("pending", "hashing recorded release evidence");
  try {
    assertDigest(EXPECTED_MANIFEST_SHA256, "full acceptance manifest constants are pending");
    assertSafeInteger(EXPECTED_MANIFEST_ENTRIES, "full acceptance manifest entry count is pending", 1);
    const manifestBytes = await fetchBytes("SHA256SUMS", 256 * 1024);
    const manifestDigest = await sha256(manifestBytes);
    assert(manifestDigest === EXPECTED_MANIFEST_SHA256, "manifest digest changed");
    state.manifest = parseManifest(manifestBytes);
    setCheck("manifest", "pass", EXPECTED_MANIFEST_ENTRIES.toLocaleString() + " entries");

    const rawFiles = {};
    await Promise.all(REQUIRED_ARTIFACTS.map(async (path) => {
      const bytes = await fetchBytes(path);
      const digest = await sha256(bytes);
      assert(digest === state.manifest.get(path), path + " failed SHA-256");
      rawFiles[path] = bytes;
    }));

    const jsonPaths = REQUIRED_ARTIFACTS.filter((path) => path.endsWith(".json"));
    const parsed = {};
    for (const path of jsonPaths) parsed[path] = parseJson(rawFiles[path], path);
    const checkpointPieceCount = parsed[CHECKPOINT_PLAN_PATH].piece_count;
    assertSafeInteger(checkpointPieceCount, "checkpoint plan piece count is invalid", 1);
    assert(checkpointPieceCount <= 256, "checkpoint receipt selection exceeds the browser limit");
    const receiptPaths = Array.from({ length: checkpointPieceCount }, (_, index) => (
      CHECKPOINT_ROOT + "receipt-" + String(index).padStart(8, "0") + ".json"
    ));
    await Promise.all(receiptPaths.map(async (path) => {
      assert(state.manifest.has(path), "SHA256SUMS is missing " + path);
      const bytes = await fetchBytes(path, 64 * 1024);
      const digest = await sha256(bytes);
      assert(digest === state.manifest.get(path), path + " failed SHA-256");
      rawFiles[path] = bytes;
      parsed[path] = parseJson(bytes, path);
    }));
    setCheck("pack", "pass", (REQUIRED_ARTIFACTS.length + receiptPaths.length) + " artifacts");
    const validated = validateRelease(parsed, rawFiles);
    state.files = parsed;
    state.acceptance = validated.acceptance;
    state.ready = true;
    displayRelease(
      validated.acceptance,
      validated.memory,
      validated.laneOverlap,
      validated.postKill,
      validated.uninterrupted,
      validated.resumed
    );
    setCheck("contract", "pass", "recomputed / pass");
    releaseState("pass", "recorded release · selected artifacts hash-verified");
  } catch (error) {
    state.ready = false;
    releaseState("fail", "release evidence rejected");
    byId("release-verdict").textContent = "REJECTED";
    byId("release-verdict").className = "fail";
    setCheck("contract", "fail", "rejected");
    byId("verification-copy").textContent = error.message;
    throw error;
  }
}

async function verifyBinary() {
  const button = byId("verify-binary");
  if (!state.ready || state.binaryVerified) {
    if (!state.ready) byId("verification-copy").textContent = "The release evidence must validate before the executable is trusted.";
    return;
  }

  button.disabled = true;
  button.textContent = "hashing executable…";
  setCheck("binary", "", "hashing");
  try {
    const bytes = await fetchBytes("bin/mfenx-local");
    const digest = await sha256(bytes);
    assert(digest === state.manifest.get("bin/mfenx-local"), "binary does not match the manifest");
    assert(digest === state.acceptance.binary_sha256, "binary does not match accepted release");
    state.binaryVerified = true;
    setCheck("binary", "pass", "SHA-256 verified");
    button.textContent = "executable SHA-256 verified";
    byId("verification-copy").textContent = "The downloadable Linux x86_64 executable is the exact binary recorded by the accepted run. Checksums establish internal integrity; this release manifest is not publisher-signed.";
    releaseState("pass", "recorded release + executable hash-verified");
  } catch (error) {
    setCheck("binary", "fail", "rejected");
    button.textContent = "binary rejected";
    byId("verification-copy").textContent = error.message;
    releaseState("fail", "executable rejected");
  } finally {
    button.disabled = state.binaryVerified;
  }
}

async function copyCommands() {
  const button = byId("copy-commands");
  try {
    await navigator.clipboard.writeText(byId("run-commands").textContent);
    button.textContent = "copied";
  } catch {
    button.textContent = "select + copy";
  }
  window.setTimeout(() => { button.textContent = "copy commands"; }, 1800);
}

byId("verify-release").addEventListener("click", () => {
  byId("verify").scrollIntoView({ behavior: "smooth", block: "start" });
  verifyBinary();
});
byId("verify-binary").addEventListener("click", verifyBinary);
byId("copy-commands").addEventListener("click", copyCommands);

updateClock();
window.setInterval(updateClock, 1000);
loadRelease().catch(() => {});
