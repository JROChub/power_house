#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import json
import re
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
PAGE = ROOT / "publicpower" / "ckodmk"


def require(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


html = (PAGE / "index.html").read_text("utf-8")
css = (PAGE / "styles.css").read_text("utf-8")
javascript = (PAGE / "app.js").read_text("utf-8")
evidence = json.loads((PAGE / "evidence.json").read_text("utf-8"))
contract = json.loads((PAGE / "demo/browser-contract.json").read_text("utf-8"))
vendor = json.loads((PAGE / "vendor/ort/VENDOR.json").read_text("utf-8"))

require('<link rel="canonical" href="https://mfenx.com/ckodmk/">' in html, "canonical URL is missing")
require("github.com/JROChub/ckodmk" not in html, "private repository link leaked")
require("https://mfenx.com/ckodmk/downloads/mfenx_ckodmk-0.2.0-py3-none-any.whl" in html, "MFENX wheel install path is missing")
require("same intelligence" in html.lower(), "explicit same-intelligence nonclaim is missing")
require("highest performance" not in html.lower(), "unsupported performance superlative is present")
require("guaranteed" not in html.lower(), "unsupported guarantee is present")
require("0 / 120" in html and "3.027%" in html, "campaign boundary is missing")
require(evidence["campaign"]["false_pass"] == 0, "campaign false-pass count drifted")
require(evidence["campaign"]["target_faults"] == 120, "campaign denominator drifted")
require(evidence["demo"]["samples"] == 1797, "demo sample count drifted")
for expected in ("1.691243", "2.265660", "2.771074", "0.202297449", "0.218335152", "0.181334734"):
    require(expected in javascript, f"retained result missing from page data: {expected}")

for target in re.findall(r'(?:href|src)="([^"]+)"', html):
    if target.startswith(("https://", "#", "/")):
        continue
    local = (PAGE / target.split("?", 1)[0]).resolve()
    require(local.exists(), f"missing local page asset: {target}")

require("@media (max-width: 680px)" in css, "mobile layout is missing")
require("prefers-reduced-motion" in css, "reduced-motion path is missing")
for relative, expected in {
    "demo/source.onnx": evidence["demo"]["source_sha256"],
    "demo/candidate-int8.onnx": evidence["demo"]["candidate_sha256"],
    "demo/optdigits-official-test.npz": evidence["demo"]["dataset_sha256"],
    "demo/browser-contract.json": evidence["demo"]["contract_sha256"],
}.items():
    observed = "sha256:" + hashlib.sha256((PAGE / relative).read_bytes()).hexdigest()
    require(observed == expected, f"demo digest drift: {relative}")

require(contract["source"]["sha256"] == evidence["demo"]["source_sha256"], "source contract binding drifted")
require(contract["candidate"]["sha256"] == evidence["demo"]["candidate_sha256"], "candidate contract binding drifted")
require(contract["dataset"]["sha256"] == evidence["demo"]["dataset_sha256"], "dataset contract binding drifted")
for relative, expected in vendor["files"].items():
    observed = "sha256:" + hashlib.sha256((PAGE / "vendor/ort" / relative).read_bytes()).hexdigest()
    require(observed == expected, f"vendored runtime digest drift: {relative}")

wheel = PAGE / "downloads/mfenx_ckodmk-0.2.0-py3-none-any.whl"
wheel_checksum = (PAGE / f"downloads/{wheel.name}.sha256").read_text("ascii").split()[0]
require(hashlib.sha256(wheel.read_bytes()).hexdigest() == wheel_checksum, "published wheel checksum drift")

subprocess.run(["node", "--check", str(PAGE / "app.js")], check=True)
subprocess.run(["node", "--check", str(PAGE / "browser-gate.js")], check=True)
subprocess.run(["node", str(ROOT / "scripts/test_ckodmk_browser_unit.cjs")], check=True)
print("CKODMK_PAGE_OK")
