#!/usr/bin/env python3
"""Validate Observatory controls and immutable public proof artifacts."""

from __future__ import annotations

import hashlib
from html.parser import HTMLParser
import json
from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parents[1]
PUBLIC = ROOT / "publicpower"
ARTIFACTS = {
    "artifacts/rootprint-valid.json": (
        4_232,
        "eeb33450c6473c082675b8fcdaf70abfb0e6070fe739eeda5c839070d13750a3",
    ),
    "artifacts/luminous-valid.json": (
        5_065,
        "4809cd8e937ae975d6ecc34ce4398ea75e6d9404bca84822f6b6bb1f33faa265",
    ),
    "artifacts/power_house_sparse_record.phsp": (
        16_000_171,
        "2b219ba189c3a38f1073c7797629e9aaf44a36820abb64c7628129480eb43f3b",
    ),
    "artifacts/external_interaction_model.phsm": (
        591_464,
        "c8376831f47a50a7423be6412776382bc23618b037e9fdd163594d389d68864d",
    ),
    "artifacts/external_interaction_model.phcp": (
        16_000_128,
        "82045e6eb851991e08d9c4cd782abff3bb06cb8ec5f149e7c2d4287113e6a54a",
    ),
}


class IdParser(HTMLParser):
    def __init__(self) -> None:
        super().__init__()
        self.ids: list[str] = []

    def handle_starttag(
        self, tag: str, attrs: list[tuple[str, str | None]]
    ) -> None:
        del tag
        for name, value in attrs:
            if name == "id" and value:
                self.ids.append(value)


def fail(errors: list[str], message: str) -> None:
    errors.append(message)


def main() -> int:
    errors: list[str] = []
    html = (PUBLIC / "index.html").read_text(encoding="utf-8")
    javascript = (PUBLIC / "app.js").read_text(encoding="utf-8")

    deployed_script_match = re.search(
        r'<script\s+type="module"\s+src="([^"?]+)(?:\?[^"]*)?"', html
    )
    if deployed_script_match is None:
        fail(errors, "could not locate the deployed Observatory module")
    else:
        deployed_script = PUBLIC / deployed_script_match.group(1)
        if not deployed_script.is_file():
            fail(errors, f"deployed Observatory module is missing: {deployed_script.name}")
        elif deployed_script.read_text(encoding="utf-8") != javascript:
            fail(errors, "deployed Observatory module differs from app.js")

    if 'href="observatory.css?' not in html:
        fail(errors, "the sovereign field stylesheet is not loaded")

    parser = IdParser()
    parser.feed(html)
    duplicates = sorted({value for value in parser.ids if parser.ids.count(value) > 1})
    if duplicates:
        fail(errors, f"duplicate HTML IDs: {', '.join(duplicates)}")

    element_block = re.search(
        r"const el = Object\.fromEntries\(\s*\[(.*?)\]\.map",
        javascript,
        re.DOTALL,
    )
    if element_block is None:
        fail(errors, "could not locate the Observatory element registry")
    else:
        registered = re.findall(r'"([a-z0-9-]+)"', element_block.group(1))
        missing = sorted(set(registered) - set(parser.ids))
        if missing:
            fail(errors, f"registered controls missing from HTML: {', '.join(missing)}")

    for relative, (expected_size, expected_hash) in ARTIFACTS.items():
        path = PUBLIC / relative
        if not path.is_file():
            fail(errors, f"missing public artifact: {relative}")
            continue
        payload = path.read_bytes()
        if len(payload) != expected_size:
            fail(
                errors,
                f"{relative} size differs: expected {expected_size}, received {len(payload)}",
            )
        digest = hashlib.sha256(payload).hexdigest()
        if digest != expected_hash:
            fail(errors, f"{relative} SHA-256 differs")

    rootprint = json.loads((PUBLIC / "artifacts/rootprint-valid.json").read_text())
    if rootprint.get("schema") != "power-house/rootprint/v1":
        fail(errors, "public Rootprint schema differs")

    required_visual_contracts = (
        "createNetworkTopology",
        "createSelectedCityGeometry",
        "proofParticlesMaterial",
        "refreshNetworkStatus",
        "verifyReleaseArtifacts",
        "verifyObservatorySidecar",
        "renderLuminousGraph",
    )
    for contract in required_visual_contracts:
        if contract not in javascript:
            fail(errors, f"missing Observatory behavior: {contract}")

    if errors:
        for error in errors:
            print(f"OBSERVATORY CONTRACT FAIL: {error}", file=sys.stderr)
        return 1
    print(
        f"OBSERVATORY CONTRACT PASS: {len(parser.ids)} controls, "
        f"{len(ARTIFACTS)} immutable artifacts"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
