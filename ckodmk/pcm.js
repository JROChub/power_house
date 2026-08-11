"use strict";

(function (global) {
  const SCHEMA = "mfenx/ckodmk-pcm/v1";
  const STATEMENT_SCHEMA = "mfenx/ckodmk-pcm-statement/v1";
  const ALGORITHM = "ecdsa-p256-sha256-p1363";
  const EVIDENCE = "finite-replay/v1";
  const PROFILE = "ckodmk/browser-wasm-f32-batch1-classification/v1";
  const TRUSTED_KEY_ID = "sha256:bdb946d6261487728b61fd0b5ea7c9e9feadd2446778188302ea028bd9f9f6d0";
  const ARTIFACTS = Object.freeze(["candidate", "contract", "dataset", "source"]);
  const LIMITS = Object.freeze({ candidate: 67108864, contract: 1048576, dataset: 67108864, source: 67108864, pcm: 314572800 });
  const MEDIA = Object.freeze({ candidate: "application/onnx", contract: "application/json", dataset: "application/x-npz", source: "application/onnx" });
  const SHA = /^sha256:[0-9a-f]{64}$/;
  const ROOT = /^sha256:[0-9a-f]{64}$/;
  const P256_ORDER = BigInt("0xffffffff00000000ffffffffffffffffbce6faada7179e84f3b9cac2fc632551");
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
  async function domainHash(domain, value) { return sha256(join(ascii(domain), ascii(canonical(value)))); }
  async function powerHouseRootprint(statement) {
    const artifact = { embedded_proof: { proof: { evidence_types: [EVIDENCE] }, protocol: "mfenx/ckodmk-pcm-statement/v1", public_inputs: { statement_sha256: await sha256(ascii(canonical(statement))) } }, phx_fingerprint: "", provenance: { producer: "mfenx-ckodmk", serial: statement.serial }, schema: "power-house/pha/v1" };
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
    if (statement.admission.candidate_artifact !== "candidate" || JSON.stringify(statement.admission.required_evidence) !== JSON.stringify([EVIDENCE])) throw new PCMError("unsupported admission policy");
    if (!Array.isArray(statement.evidence) || statement.evidence.length !== 1) throw new PCMError("unsupported evidence set");
    const evidence = statement.evidence[0];
    exactKeys(evidence, ["candidate_artifact", "contract_artifact", "dataset_artifact", "runtime_profile", "source_artifact", "type"], "evidence");
    if (canonical(evidence) !== canonical({ candidate_artifact: "candidate", contract_artifact: "contract", dataset_artifact: "dataset", runtime_profile: PROFILE, source_artifact: "source", type: EVIDENCE })) throw new PCMError("unsupported evidence profile");
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
    return { bundle, decoded, issuer: issuer.name, rootId: bundle.statement_root_id, expiresAt: statement.expires_at };
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
    for (const id of ["pcm-root-id", "pcm-issuer", "pcm-expiry", "pcm-artifact"]) global.document.getElementById(id).textContent = "Not verified";
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
    state("REPLAYING", "Signature accepted. Replaying every required behavioral check before admission…");
    global.CKODMKBrowserGate.loadInputs({ source: files.source, candidate: files.candidate, dataset: files.dataset, contract: files.contract, contractDigest, generation: { implementation: "mfenx-ckodmk-pcm", version: "0.3.0", profile: PROFILE, statement_root_id: verified.rootId } });
    global.document.getElementById("gate-form").requestSubmit();
    for (let attempt = 0; attempt < 6000; attempt += 1) {
      await new Promise((resolve) => global.setTimeout(resolve, 100));
      const decision = global.document.getElementById("gate-decision").textContent;
      if (!["PASS", "BLOCK", "INCONCLUSIVE"].includes(decision)) continue;
      if (global.document.getElementById("gate-contract-digest").value !== contractDigest) throw new PCMError("active contract changed during replay");
      if (decision !== "PASS") { state("BLOCK", "Required behavioral checks failed. The model was not admitted."); throw new PCMError("behavioral replay blocked admission"); }
      admittedCandidate = files.candidate; download.disabled = false; check("pcm-check-replay", "pass", "Passed"); check("pcm-check-admission", "pass", "Unlocked"); state("ADMITTED", "Signature, identity, artifacts and required behavioral checks passed. The candidate is admitted in this session."); return verified;
    }
    throw new PCMError("behavioral replay timed out");
  }
  async function selected() { const file = global.document.getElementById("pcm-file").files[0], key = global.document.getElementById("pcm-trusted-key").files[0]; if (!file || file.size > LIMITS.pcm) throw new PCMError("select a bounded .pcm file"); if (!key || key.size < 1 || key.size > 65536) throw new PCMError("select the issuer's trusted SPKI DER public key"); return admit(new Uint8Array(await file.arrayBuffer()), new Uint8Array(await key.arrayBuffer())); }
  async function included() { const key = await fetchBytes("trust/pcm-release-public.der", 65536, "trusted release key"); return admit(await fetchBytes("pcm/optdigits-cnn-int8.pcm", LIMITS.pcm, "included PCM"), key, TRUSTED_KEY_ID); }
  function download() { if (!admittedCandidate) return; const url = URL.createObjectURL(admittedCandidate), anchor = global.document.createElement("a"); anchor.href = url; anchor.download = admittedCandidate.name; anchor.click(); global.setTimeout(() => URL.revokeObjectURL(url), 0); }
  function action(operation) { return async () => { const buttons = ["verify-pcm", "verify-included-pcm"].map((id) => global.document.getElementById(id)); resetAdmission("Preparing a new admission check…"); buttons.forEach((button) => { button.disabled = true; }); try { await operation(); } catch (error) { blockCurrentCheck(); state("BLOCK", `Fail closed: ${error.message || "PCM verification failed"}`); } finally { buttons.forEach((button) => { button.disabled = false; }); } }; }
  function bind() { global.document.getElementById("verify-pcm").addEventListener("click", action(selected)); global.document.getElementById("verify-included-pcm").addEventListener("click", action(included)); global.document.getElementById("download-admitted-model").addEventListener("click", download); for (const id of ["pcm-file", "pcm-trusted-key"]) global.document.getElementById(id).addEventListener("change", () => resetAdmission("Selected files changed. Verify again before admission.")); }

  const api = Object.freeze({ PCMError, canonical, verifyEnvelope }); global.CKODMKPCM = api; if (typeof module !== "undefined" && module.exports) module.exports = api; if (global.document) bind();
})(typeof window !== "undefined" ? window : globalThis);
