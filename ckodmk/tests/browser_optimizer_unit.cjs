"use strict";

const crypto = require("node:crypto");
const fs = require("node:fs");
const path = require("node:path");

const root = path.resolve(__dirname, "..");
require(path.join(root, "browser-optimizer.js"));
const optimizer = global.CKODMKBrowserOptimizer;
const mlpProfiles = Object.freeze([
  { name: "w16", directory: "", digest: "d3f98ad864f5aaa270834ec788dc33f5408c6d4164c56c249f1427376ab2e782", bytes: 2723 },
  { name: "w32", directory: "mlp-w32", digest: "cbb46d6b7934cc873967f4e0c1fec607940608c7496fab31f49c80b3fcf4ddb3", bytes: 4055 },
  { name: "w64", directory: "mlp-w64", digest: "d425a2bcc9917ba8d7054f1ce6c18b04d0a06f9ded268705933140da7057ab5a", bytes: 6711 }
]);
const mlpResults = {};
for (const profile of mlpProfiles) {
  const directory = profile.directory ? `demo/${profile.directory}` : "demo";
  const profileSource = fs.readFileSync(path.join(root, directory, "source.onnx"));
  const retained = fs.readFileSync(path.join(root, directory, "candidate-int8.onnx"));
  const generatedMlp = optimizer.buildCandidate(profileSource);
  const first = Buffer.from(generatedMlp.bytes);
  const second = Buffer.from(optimizer.buildInt8Candidate(profileSource));
  const digest = crypto.createHash("sha256").update(first).digest("hex");
  if (!first.equals(second)) throw new Error(`${profile.name} candidate generation is not deterministic`);
  if (generatedMlp.profile !== "two-layer-mlp-per-channel-dynamic-int8/v1") throw new Error(`${profile.name} profile identifier drift`);
  if (!first.equals(retained)) throw new Error(`${profile.name} candidate differs from the retained generated candidate`);
  if (digest !== profile.digest) throw new Error(`${profile.name} candidate digest drift: ${digest}`);
  if (first.length !== profile.bytes || first.length >= profileSource.length) throw new Error(`${profile.name} candidate size regression`);
  mlpResults[profile.name] = { bytes: first.length, sha256: digest };
}
const source = fs.readFileSync(path.join(root, "demo/source.onnx"));
const retained = fs.readFileSync(path.join(root, "demo/candidate-int8.onnx"));

function mustReject(bytes, label) {
  let rejected = false;
  try { optimizer.buildInt8Candidate(bytes); } catch { rejected = true; }
  if (!rejected) throw new Error(`optimizer accepted ${label}`);
}

mustReject(Buffer.alloc(0), "an empty model");
mustReject(Buffer.alloc(64 * 1024 * 1024 + 1), "an oversized model");
mustReject(retained, "an already quantized graph");
for (const length of [1, 8, 32, 128, source.length - 1]) mustReject(source.subarray(0, length), `a ${length} byte truncation`);
const changedOperation = Buffer.from(source);
const reluAt = changedOperation.indexOf(Buffer.from("Relu", "ascii"));
if (reluAt < 0) throw new Error("test source does not contain Relu");
changedOperation[reluAt] = "T".charCodeAt(0);
mustReject(changedOperation, "an unsupported graph operation");

const rbfProfiles = Object.freeze([
  { name: "c40", directory: "rbf-c40", digest: "e854f79facd3b6009954f2268a90537db6087183d17235c9a6ef01839e6904e6", bytes: 4778 },
  { name: "c80", directory: "rbf", digest: "df73e1cfa1b62d7fa025508e7738ef2caeb0a1de4ca3ebb9255d7ddc75eba952", bytes: 7738 },
  { name: "c160", directory: "rbf-c160", digest: "7ca3e627e4cdc90c77744019f3a7d338a881f373faffc2c30af3098de53e1563", bytes: 13660 }
]);
const rbfResults = {};
for (const profile of rbfProfiles) {
  const rbfSource = fs.readFileSync(path.join(root, `demo/${profile.directory}/source.onnx`));
  const rbfRetained = fs.readFileSync(path.join(root, `demo/${profile.directory}/candidate-int8.onnx`));
  const generatedRbf = optimizer.buildCandidate(rbfSource);
  const rbfFirst = Buffer.from(generatedRbf.bytes);
  const rbfSecond = Buffer.from(optimizer.buildInt8Candidate(rbfSource));
  const rbfDigest = crypto.createHash("sha256").update(rbfFirst).digest("hex");
  if (generatedRbf.profile !== "normalized-rbf-per-channel-dynamic-int8/v1") throw new Error(`${profile.name} profile identifier drift`);
  if (!rbfFirst.equals(rbfSecond) || !rbfFirst.equals(rbfRetained)) throw new Error(`${profile.name} generation is not deterministic or differs from the retained candidate`);
  if (rbfDigest !== profile.digest) throw new Error(`${profile.name} candidate digest drift: ${rbfDigest}`);
  if (rbfFirst.length !== profile.bytes || rbfFirst.length >= rbfSource.length) throw new Error(`${profile.name} candidate size regression`);
  mustReject(rbfRetained, `an already quantized ${profile.name} graph`);
  rbfResults[profile.name] = { bytes: rbfFirst.length, sha256: rbfDigest };
}
const rbfSource = fs.readFileSync(path.join(root, "demo/rbf/source.onnx"));
for (const length of [1, 16, 64, 256, rbfSource.length - 1]) mustReject(rbfSource.subarray(0, length), `an RBF ${length} byte truncation`);
const changedRbfOperation = Buffer.from(rbfSource);
const expAt = changedRbfOperation.indexOf(Buffer.from("Exp", "ascii"));
if (expAt < 0) throw new Error("test RBF source does not contain Exp");
changedRbfOperation[expAt] = "L".charCodeAt(0);
mustReject(changedRbfOperation, "an unsupported RBF operation");

console.log("CKODMK_BROWSER_OPTIMIZER_UNIT_OK", JSON.stringify({ mlp: mlpResults, rbf: rbfResults }));
