#!/usr/bin/env python3
"""Verify that README API links resolve in the generated rustdoc tree."""

from __future__ import annotations

import argparse
from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parents[1]
DOCS_RS_PREFIX = "https://docs.rs/power_house/latest/power_house/"
EXPECTED_API_PAGES = (
    "provenance/pha/struct.PhaArtifact.html",
    "provenance/rootprint/struct.Rootprint.html",
    "macro.prove_with_rootprint.html",
    "sumcheck/struct.GeneralSumProof.html",
    "sparse_sumcheck/struct.SeededSparseProof.html",
    "sparse_sumcheck/struct.CommittedSparsePolynomial.html",
    "sparse_sumcheck/struct.CommittedSparseProof.html",
    "julian/struct.ProofLedger.html",
)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--doc-root",
        type=Path,
        default=ROOT / "target" / "doc" / "power_house",
        help="generated power_house rustdoc directory",
    )
    args = parser.parse_args()

    readme = (ROOT / "README.md").read_text(encoding="utf-8")
    errors: list[str] = []

    for page in EXPECTED_API_PAGES:
        url = f"{DOCS_RS_PREFIX}{page}"
        if url not in readme:
            errors.append(f"README is missing the canonical API link: {url}")

        generated = args.doc_root / page
        if not generated.is_file() or generated.stat().st_size == 0:
            errors.append(f"generated rustdoc page is missing or empty: {generated}")

    if errors:
        for error in errors:
            print(f"RUSTDOC LINK FAIL: {error}", file=sys.stderr)
        return 1

    print(f"RUSTDOC LINK PASS: {len(EXPECTED_API_PAGES)} README API pages")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
