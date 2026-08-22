#!/usr/bin/env node

import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import vm from "node:vm";
import { fileURLToPath } from "node:url";
import { createHash, webcrypto } from "node:crypto";

const here = path.dirname(fileURLToPath(import.meta.url));
const lightsout = path.dirname(here);
const source = fs.readFileSync(path.join(lightsout, "lightsout.js"), "utf8");
const tail = source.indexOf('byId("verify-release").addEventListener');
assert(tail > 0, "test harness could not isolate browser startup");
const testSource = source.slice(0, tail) + `
window.__MFENX_TEST__ = Object.freeze({
  validationIndexedLocalUrl,
  validateFinalReleaseIndex,
  validatePrepublicationVerification,
  validateFinalStatus,
  validateScalingResults,
  validateFinalSelected,
  validateFinalRecord,
  validatePostPublicationRetrieval
});
`;

const context = {
  URL,
  TextDecoder,
  Uint8Array,
  ArrayBuffer,
  Map,
  Set,
  Number,
  Object,
  RegExp,
  JSON,
  Math,
  crypto: webcrypto,
  atob: (value) => Buffer.from(value, "base64").toString("binary"),
  location: { href: "https://mfenx.com/lightsout/", origin: "https://mfenx.com" },
  document: {},
  window: {},
  fetch: () => Promise.reject(new Error("network is disabled in semantic unit tests"))
};
vm.createContext(context);
vm.runInContext(testSource, context, { filename: "lightsout.js" });
const test = context.window.__MFENX_TEST__;

const readJson = (relative) => JSON.parse(fs.readFileSync(path.join(here, relative), "utf8"));
const index = readJson("release-index.json");
const prepublication = readJson("prepublication-input-retrieval-verification.json");
const indexBytes = fs.readFileSync(path.join(here, "release-index.json"));
const byRole = test.validateFinalReleaseIndex(index);
test.validatePrepublicationVerification(prepublication, index, indexBytes);
const status = readJson("validation-status.json");
test.validateFinalStatus(status);
const signedRecord = readJson("release/VALIDATION-RECORD.canonical.json");
test.validateFinalRecord(signedRecord, index, prepublication);
const retrieval = readJson("post-publication-retrieval-attestation.json");
test.validatePostPublicationRetrieval(retrieval, { index });
assert.equal(retrieval.check_count, 68);
assert.equal(retrieval.passed, 68);
assert.equal(retrieval.failed, 0);
assert.equal(new Set(retrieval.checks.map((row) => row.url)).size, retrieval.checks.length);
assert.equal(test.validationIndexedLocalUrl(index.web_summary.public_path).pathname, "/lightsout/validation-record/validation-status.json");

const retrievalDigest = createHash("sha256")
  .update(fs.readFileSync(path.join(here, "post-publication-retrieval-attestation.json")))
  .digest("hex");
const pinnedRetrieval = source.match(/retrievalAttestationSha256:\s*"([0-9a-f]{64})"/);
assert(pinnedRetrieval, "browser source omits a valid retrieval-attestation pin");
assert.equal(retrievalDigest, pinnedRetrieval[1], "browser retrieval-attestation pin differs from published bytes");

function publicPathFor(role) {
  const entry = byRole.get(role);
  assert(entry, `missing staged role ${role}`);
  assert(!entry.public_path.startsWith("../"), `selected test role unexpectedly uses an external staged path: ${role}`);
  return path.join(here, entry.public_path);
}

function selectedValue(role) {
  const raw = fs.readFileSync(publicPathFor(role), "utf8");
  return role === "final_replay_gated_execution_paper" ? raw : JSON.parse(raw);
}

const selected = Object.create(null);
for (const role of [
  "adversarial_a3_final_attestation",
  "commercial_evaluation_boundary",
  "commercial_evaluation_profile",
  "final_replay_gated_execution_paper",
  "final_uci_workload_summary",
  "hosted_reproduction_local_verification",
  "hosted_security_a2_local_verification",
  "procedurally_separate_code_security_review_report",
  "scaling_results",
  "scaling_v2_failure_attestation"
]) selected[role] = selectedValue(role);

const cells = test.validateFinalSelected(selected);
assert.equal(cells.size, 10);

function mustReject(label, callback) {
  assert.throws(callback, undefined, `${label} mutation was accepted`);
}

const mutatedStatus = structuredClone(status);
mutatedStatus.security.independent_human_review.status = "complete";
mustReject("human-review promotion", () => test.validateFinalStatus(mutatedStatus));

const mutatedSecurity = structuredClone(status);
mutatedSecurity.security.hosted_a2.unwaived_codeql_results = 0;
mustReject("security finding removal", () => test.validateFinalStatus(mutatedSecurity));

const mutatedHistory = structuredClone(status);
mutatedHistory.scaling.v2.failure = 0;
mustReject("failed scaling history removal", () => test.validateFinalStatus(mutatedHistory));

const mutatedBoundary = structuredClone(status);
mutatedBoundary.claim_boundary.supercomputer_hardware_claimed = true;
mustReject("hardware-claim promotion", () => test.validateFinalStatus(mutatedBoundary));

const mutatedIndex = structuredClone(index);
mutatedIndex.all_input_evidence_live_http_retrieval_claimed = true;
mustReject("prepublication HTTP promotion", () => test.validateFinalReleaseIndex(mutatedIndex));

const mutatedScaling = structuredClone(selected.scaling_results);
mutatedScaling.cells[0].external_executor_wall_ns.median_decimal = "1.0";
mustReject("scaling-result mutation", () => test.validateScalingResults(mutatedScaling));

const mutatedRetrieval = structuredClone(retrieval);
mutatedRetrieval.checks[0].observed_sha256 = "0".repeat(64);
mustReject("live-retrieval mutation", () => test.validatePostPublicationRetrieval(mutatedRetrieval, { index }));

const html = fs.readFileSync(path.join(lightsout, "index.html"), "utf8");
const ids = [...html.matchAll(/\sid="([^"]+)"/g)].map((match) => match[1]);
assert.equal(ids.length, new Set(ids).size, "HTML contains duplicate IDs");
for (const required of [
  "validation-record",
  "record-contract",
  "record-adversarial",
  "record-reproduction",
  "record-evaluation",
  "physical-scaling-result",
  "hosted-reproduction-result",
  "external-workload-result"
]) assert(ids.includes(required), `HTML omits required Validation Record ID ${required}`);

const localHrefs = [...html.matchAll(/\shref="([^"]+)"/g)]
  .map((match) => match[1])
  .filter((href) => !href.startsWith("https://") && !href.startsWith("#") && !href.startsWith("/"));
for (const href of localHrefs) {
  const pathname = href.split("?", 1)[0].split("#", 1)[0];
  assert(fs.existsSync(path.resolve(lightsout, pathname)), `HTML local link is absent: ${href}`);
}
for (const role of [...html.matchAll(/\sdata-final-role="([^"]+)"/g)].map((match) => match[1])) {
  assert(byRole.has(role), `HTML validation download role is absent from the release index: ${role}`);
}

const prohibited = new RegExp(
  "(?:\\b" + "st" + "ep\\s*[1-4]\\b|\\b" + "st" + "ep[1-4]\\b|\\b" + "S" + "34\\b|final" + "-roadmap|add" + "endum)",
  "i"
);
for (const relative of [
  "index.html",
  "lightsout.js",
  "lightsout.css",
  "validation-record/README.md",
  "validation-record/prepare-publication.py",
  "validation-record/audit-live-publication.py",
  "validation-record/test-validation-record.mjs",
  "validation-record/release-index.json",
  "validation-record/prepublication-input-retrieval-verification.json",
  "validation-record/validation-status.json"
]) {
  const publicText = fs.readFileSync(path.join(lightsout, relative), "utf8");
  assert(!prohibited.test(publicText), `public-bound neutral-label gate failed: ${relative}`);
}
for (const url of index.public_input_evidence_urls) {
  assert(!prohibited.test(url), `public evidence route failed neutral-label gate: ${url}`);
}

console.log(`PASS Validation Record semantics: ${index.artifact_count} inputs, ${cells.size} scaling cells, 7 negative controls, ${retrieval.check_count} live checks, ${localHrefs.length} local links`);
