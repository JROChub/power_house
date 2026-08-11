"use strict";

const fs = require("fs");
const path = require("path");
const root = path.resolve(__dirname, "..");
require(path.join(root, "browser-gate.js"));
const gate = global.CKODMKBrowserGate;

const contractText = fs.readFileSync(path.join(root, "demo/browser-contract.json"), "utf8");
const contract = gate.validateContract(gate.parseStrictJson(contractText));
const datasetBuffer = fs.readFileSync(path.join(root, "demo/optdigits-official-test.npz"));
const arrayBuffer = datasetBuffer.buffer.slice(datasetBuffer.byteOffset, datasetBuffer.byteOffset + datasetBuffer.byteLength);
const dataset = gate.parseDataset(arrayBuffer, contract);
if (dataset.labels.length !== 1797 || dataset.inputs.length !== 115008) throw new Error("canonical demo shape mismatch");

for (const malformed of [
  '{"x":1,"x":2}',
  '{"x":1.0}',
  '{"x":true,"x":false}',
  '{"x":9007199254740992}',
  '{"x":1} trailing'
]) {
  let rejected = false;
  try { gate.parseStrictJson(malformed); } catch { rejected = true; }
  if (!rejected) throw new Error(`malformed JSON accepted: ${malformed}`);
}

const pass = gate.adjudicate(contract, {
  sourceDecisions: [0, 1], candidateDecisions: [0, 1], sourceCorrect: 2,
  candidateCorrect: 2, maxLinf: 0.2
});
if (pass.decision !== "PASS") throw new Error("honest claim set did not pass");
const block = gate.adjudicate(contract, {
  sourceDecisions: [0, 1], candidateDecisions: [1, 1], sourceCorrect: 2,
  candidateCorrect: 1, maxLinf: 0.3
});
if (block.decision !== "BLOCK") throw new Error("failed required claim did not block");

const crcCorrupt = new Uint8Array(arrayBuffer.slice(0));
const firstNpy = datasetBuffer.indexOf(Buffer.from([0x93, 0x4e, 0x55, 0x4d, 0x50, 0x59]));
if (firstNpy < 0) throw new Error("demo NPY member not found");
crcCorrupt[firstNpy + 1024] ^= 1;
let crcRejected = false;
try { gate.parseDataset(crcCorrupt.buffer, contract); } catch { crcRejected = true; }
if (!crcRejected) throw new Error("corrupted dataset was accepted");
console.log("CKODMK_BROWSER_UNIT_OK");
