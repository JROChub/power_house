#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import json
import re
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
WEB = ROOT / "publicpower" / "ckodmk"


def require(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


html = (WEB / "index.html").read_text("utf-8")
css = (WEB / "styles.css").read_text("utf-8")
javascript = (WEB / "app.js").read_text("utf-8")
service_worker = (WEB / "sw.js").read_text("utf-8")
evidence = json.loads((WEB / "evidence.json").read_text("utf-8"))
contract = json.loads((WEB / "demo/browser-contract.json").read_text("utf-8"))
manifest = json.loads((WEB / "manifest.webmanifest").read_text("utf-8"))
vendor = json.loads((WEB / "vendor/ort/VENDOR.json").read_text("utf-8"))

require('<link rel="canonical" href="https://mfenx.com/ckodmk/">' in html, "canonical URL is missing")
require("github.com/JROChub/ckodmk" not in html, "private repository link leaked")
public_text = "\n".join(path.read_text("utf-8", errors="replace") for path in WEB.rglob("*") if path.is_file() and path.suffix.lower() in {".html", ".js", ".json", ".md", ".webmanifest"})
for private_marker in ("JROC", "iMac12", "Mac-942", "i5-2500", "kali-amd64", "schedutil", "6.19.14"):
    require(private_marker.lower() not in public_text.lower(), f"private device marker leaked: {private_marker}")
require("—" not in html and "–" not in html, "dash-style marketing punctuation returned")
require("https://mfenx.com/ckodmk/downloads/mfenx_ckodmk-0.2.0-py3-none-any.whl" in html, "MFENX wheel install path is missing")
require("highest performance" not in html.lower() and "guaranteed" not in html.lower(), "unsupported product claim")
require("0 / 120" in html and "3.027%" in html, "campaign limits are missing")
require(evidence["campaign"]["false_pass"] == 0 and evidence["campaign"]["target_faults"] == 120, "campaign evidence drift")
for expected in ("1.691243", "2.265660", "2.771074", "0.202297449", "0.218335152", "0.181334734"):
    require(expected in javascript, f"retained result missing: {expected}")
for target in re.findall(r'(?:href|src)="([^"]+)"', html):
    if target.startswith(("https://", "#", "/")):
        continue
    require((WEB / target.split("?", 1)[0]).exists(), f"missing local asset: {target}")
require("@media (max-width: 680px)" in css and "prefers-reduced-motion" in css, "responsive/accessibility CSS missing")
require('<link rel="manifest" href="manifest.webmanifest">' in html, "PWA manifest link missing")
require('rel="apple-touch-icon"' in html, "Apple touch icon missing")
require(manifest["display"] == "standalone" and manifest["scope"] == "./" and manifest["start_url"] == "./", "PWA manifest scope drift")
require({icon["sizes"] for icon in manifest["icons"]} == {"192x192", "512x512"}, "PWA icon sizes drift")
for icon in manifest["icons"]:
    require((WEB / icon["src"]).is_file(), f"missing PWA icon: {icon['src']}")
require("demo/source.onnx" in service_worker and "demo/optdigits-official-test.npz" in service_worker, "offline model assets missing")
for relative, expected in {
    "demo/source.onnx": evidence["demo"]["source_sha256"],
    "demo/candidate-int8.onnx": evidence["demo"]["candidate_sha256"],
    "demo/optdigits-official-test.npz": evidence["demo"]["dataset_sha256"],
    "demo/browser-contract.json": evidence["demo"]["contract_sha256"],
}.items():
    require("sha256:" + hashlib.sha256((WEB / relative).read_bytes()).hexdigest() == expected, f"demo digest drift: {relative}")
require(contract["source"]["sha256"] == evidence["demo"]["source_sha256"], "source binding drift")
require(contract["candidate"]["sha256"] == evidence["demo"]["candidate_sha256"], "candidate binding drift")
require(contract["dataset"]["sha256"] == evidence["demo"]["dataset_sha256"], "dataset binding drift")
for relative, expected in vendor["files"].items():
    require("sha256:" + hashlib.sha256((WEB / "vendor/ort" / relative).read_bytes()).hexdigest() == expected, f"runtime digest drift: {relative}")
wheel = WEB / "downloads/mfenx_ckodmk-0.2.0-py3-none-any.whl"
wheel_checksum = (WEB / f"downloads/{wheel.name}.sha256").read_text("ascii").split()[0]
require(hashlib.sha256(wheel.read_bytes()).hexdigest() == wheel_checksum, "published wheel checksum drift")
subprocess.run(["node", "--check", str(WEB / "app.js")], check=True)
subprocess.run(["node", "--check", str(WEB / "browser-gate.js")], check=True)
subprocess.run(["node", "--check", str(WEB / "sw.js")], check=True)
subprocess.run(["node", str(ROOT / "scripts/test_ckodmk_browser_unit.cjs")], check=True)
print("CKODMK_WEB_OK")
