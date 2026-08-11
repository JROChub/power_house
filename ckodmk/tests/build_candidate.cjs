"use strict";

const fs = require("node:fs");
const path = require("node:path");

if (process.argv.length !== 4) throw new Error("usage: node build_candidate.cjs SOURCE OUTPUT");
require(path.resolve(__dirname, "../browser-optimizer.js"));
const source = fs.readFileSync(process.argv[2]);
const candidate = global.CKODMKBrowserOptimizer.buildInt8Candidate(source);
fs.writeFileSync(process.argv[3], candidate, { flag: "wx" });
