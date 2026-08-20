"use strict";

const RELEASE_ROOT = "release/";
const EXPECTED_MANIFEST_SHA256 = "5a5687de77d9cc203e4a3073b9b849219a5c6e558293e9658677d5cff5490b22";
const EXPECTED_MANIFEST_ENTRIES = 1186;
const REQUIRED_ARTIFACTS = [
  "acceptance.json",
  "provenance/workload-contract.json",
  "provenance/memory-summary.json",
  "provenance/trace-summary.json",
  "provenance/post-kill-receipts.json",
  "provenance/post-resume-checkpoint.json",
  "provenance/containment.txt",
  "provenance/containment-probes.txt",
  "provenance/binary.sha256",
  "provenance/source-root.sha256",
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

function displayRelease(acceptance, memory) {
  const captured = new Date(acceptance.captured_at_utc);
  const capturedCopy = new Intl.DateTimeFormat("en-US", {
    month: "short", day: "2-digit", year: "numeric",
    hour: "2-digit", minute: "2-digit", second: "2-digit",
    hour12: false, timeZone: "UTC", timeZoneName: "short"
  }).format(captured);
  const ratio = acceptance.workload.logical_input_bytes / acceptance.workload.certified_managed_peak_bytes;

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
  byId("peak-rss").textContent = humanBytes(memory.maxima_kib.VmHWM_kib * 1024);
  byId("swap-used").textContent = humanBytes(memory.maxima_kib.VmSwap_kib * 1024, 0);
  byId("gpu-count").textContent = String(acceptance.syscall_trace.product_gpu_device_paths);
  byId("network-count").textContent = String(acceptance.syscall_trace.product_network_syscalls);
  byId("kill-time").textContent = (memory.external_timings["killed-run"].wall_ns / 1e9).toFixed(3) + " s";
  byId("output-root").textContent = acceptance.uninterrupted_output_root;
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

function validateRelease(files) {
  const acceptance = files["acceptance.json"];
  const workload = files["provenance/workload-contract.json"];
  const memory = files["provenance/memory-summary.json"];
  const trace = files["provenance/trace-summary.json"];
  const postKill = files["provenance/post-kill-receipts.json"];
  const postResume = files["provenance/post-resume-checkpoint.json"];
  const inspect = files["artifacts/gemm.inspect.json"];
  const image = files["artifacts/gemm.mfx.json"];
  const uninterrupted = files["artifacts/uninterrupted.result.json"];
  const uninterruptedVerify = files["artifacts/uninterrupted.verify.json"];
  const resumed = files["artifacts/resumed.result.json"];
  const resumedVerify = files["artifacts/resumed.verify.json"];

  assert(acceptance.schema_version === 1, "unexpected acceptance schema");
  assert(acceptance.status === "PASS" && acceptance.acceptance_level === "release" && acceptance.release_acceptance === true, "release acceptance did not pass");
  assert(acceptance.product === "rarecomp-mfenx-local", "unexpected product");
  assert(acceptance.build.binary === "mfenx-local" && acceptance.build.locked === true && acceptance.build.offline === true, "build boundary changed");
  assert(acceptance.build.forbidden_dependencies === 0 && acceptance.build.forbidden_dynamic_libraries === 0, "forbidden runtime dependency present");
  assert(acceptance.machine.backend === "rarecomp_mfenx_local_virtual_gpu", "backend changed");
  assert(acceptance.machine.isa_version === 5 && acceptance.machine.resource_certificate_schema_version === 2, "machine schema changed");
  assert(acceptance.machine.opcode === "streamed_i32_gemm", "machine opcode changed");
  assert(acceptance.machine.verification_method === "independently_addressed_exact_replay", "verification method changed");
  assert(acceptance.machine.gpu_devices_required === 0 && acceptance.machine.network_transports_required === 0, "external device requirement changed");
  assert(acceptance.machine.directory_metadata_sync === "available" && acceptance.machine.process_crash_recovery === "supported", "durability contract changed");
  assert(acceptance.containment.mode === "bubblewrap_network_namespace" && acceptance.containment.device_view === "bubblewrap_synthetic_dev", "containment changed");

  assert(acceptance.workload.logical_input_bytes > 4 * acceptance.workload.certified_managed_peak_bytes, "input no longer exceeds four managed peaks");
  assert(acceptance.workload.input_at_least_four_times_managed === true, "workload ratio gate changed");
  assert(acceptance.workload.logical_input_bytes === workload.logical_input_bytes, "workload byte count disagrees");
  assert(acceptance.workload.certified_managed_peak_bytes === workload.certified_managed_peak_bytes, "managed peak disagrees");
  assert(workload.isa_version === 5 && workload.resource_certificate_schema_version === 2 && workload.total_pieces === 2, "workload contract changed");

  assert(memory.external_hard_rlimit_as_bytes === 201326592, "external address-space limit changed");
  assert(memory.maxima_kib.VmHWM_kib === 24636 && memory.maxima_kib.VmSwap_kib === 0, "external memory evidence changed");
  assert(memory.external_timings["killed-run"].exit_status === 137, "SIGKILL status changed");
  assert(trace.product_network_syscalls === 0 && trace.product_gpu_device_paths === 0 && trace.offline_build_ip_network_attempts === 0, "containment trace failed");
  assert(acceptance.syscall_trace.product_network_syscalls === trace.product_network_syscalls, "network trace summary disagrees");
  assert(acceptance.syscall_trace.product_gpu_device_paths === trace.product_gpu_device_paths, "GPU trace summary disagrees");

  assert(postKill.expected_total_pieces === 2 && postKill.receipt_count === 1 && equalArray(postKill.receipt_indices, [0]), "post-kill partition changed");
  assert(equalArray(postResume.receipt_indices, [0, 1]), "post-resume partition changed");
  assert(postKill.plan_sha256 === postResume.plan_sha256, "recovery plan changed");
  assert(postKill.entries[0].piece_sha256 === postResume.entries[0].piece_sha256, "reused piece changed");
  assert(postKill.entries[0].receipt_sha256 === postResume.entries[0].receipt_sha256, "reused receipt changed");

  assert(inspect.backend === acceptance.machine.backend && inspect.isa_version === 5 && inspect.resource_certificate_schema_version === 2, "inspected image identity changed");
  assert(inspect.opcode === "streamed_i32_gemm" && inspect.piece_count === 2, "inspected machine changed");
  assert(image.isa_version === 5 && image.instructions.length === 1 && image.instructions[0].opcode === "streamed_i32_gemm", "machine image changed");
  assert(image.resource_certificate.certified_managed_peak_bytes === acceptance.workload.certified_managed_peak_bytes, "machine certificate disagrees");
  assert(equalArray(image.inputs.left.ty.shape, [4, 32]) && equalArray(image.inputs.right.ty.shape, [32, 1048576]), "input geometry changed");

  assert(uninterrupted.backend === acceptance.machine.backend && resumed.backend === acceptance.machine.backend, "result backend changed");
  assert(equalArray(uninterrupted.metrics.executed_piece_indices, [0, 1]) && equalArray(uninterrupted.metrics.reused_piece_indices, []), "uninterrupted partition changed");
  assert(equalArray(resumed.metrics.executed_piece_indices, [1]) && equalArray(resumed.metrics.reused_piece_indices, [0]), "resumed partition changed");
  assert(uninterrupted.outputs[0].tensor.manifest_digest === resumed.outputs[0].tensor.manifest_digest, "result roots differ");
  assert(uninterrupted.outputs[0].tensor.manifest_digest === acceptance.uninterrupted_output_root, "uninterrupted root disagrees");
  assert(resumed.outputs[0].tensor.manifest_digest === acceptance.resumed_output_root, "resumed root disagrees");
  assert(acceptance.uninterrupted_output_root === acceptance.resumed_output_root, "accepted roots differ");
  assert(uninterrupted.verification.verified === true && resumed.verification.verified === true, "inline exact replay failed");
  assert(uninterruptedVerify.verified === true && resumedVerify.verified === true, "standalone exact replay failed");
  assert(uninterruptedVerify.method === "independently_addressed_exact_replay" && resumedVerify.method === "independently_addressed_exact_replay", "standalone verifier changed");

  const probes = Object.values(acceptance.corruption_rejection);
  assert(probes.length === 5 && probes.every((value) => value === "passed"), "corruption rejection suite failed");
  for (const name of ["corrupt-input-chunk", "corrupt-checkpoint-piece", "corrupt-checkpoint-receipt", "corrupt-image", "corrupt-result"]) {
    assert(memory.external_timings[name].exit_status === 1, name + " did not reject");
  }

  return { acceptance, memory };
}

async function loadRelease() {
  releaseState("pending", "hashing recorded release evidence");
  try {
    const manifestBytes = await fetchBytes("SHA256SUMS", 256 * 1024);
    const manifestDigest = await sha256(manifestBytes);
    assert(manifestDigest === EXPECTED_MANIFEST_SHA256, "sealed manifest digest changed");
    state.manifest = parseManifest(manifestBytes);
    setCheck("manifest", "pass", EXPECTED_MANIFEST_ENTRIES.toLocaleString() + " entries");

    const rawFiles = {};
    await Promise.all(REQUIRED_ARTIFACTS.map(async (path) => {
      const bytes = await fetchBytes(path);
      const digest = await sha256(bytes);
      assert(digest === state.manifest.get(path), path + " failed SHA-256");
      rawFiles[path] = bytes;
    }));
    setCheck("pack", "pass", REQUIRED_ARTIFACTS.length + " artifacts");

    const jsonPaths = REQUIRED_ARTIFACTS.filter((path) => path.endsWith(".json"));
    const parsed = {};
    for (const path of jsonPaths) parsed[path] = parseJson(rawFiles[path], path);
    const validated = validateRelease(parsed);
    state.files = parsed;
    state.acceptance = validated.acceptance;
    state.ready = true;
    displayRelease(validated.acceptance, validated.memory);
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
  button.textContent = "hashing 1.9 MB executable…";
  setCheck("binary", "", "hashing");
  try {
    const bytes = await fetchBytes("bin/mfenx-local");
    const digest = await sha256(bytes);
    assert(digest === state.manifest.get("bin/mfenx-local"), "binary does not match sealed manifest");
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
