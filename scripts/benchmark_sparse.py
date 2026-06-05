#!/usr/bin/env python3
"""Run reproducible Rust and Python sparse-verifier benchmarks."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import statistics
import subprocess
import time
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def cpu_model() -> str:
    cpuinfo = Path("/proc/cpuinfo")
    if cpuinfo.exists():
        for line in cpuinfo.read_text(encoding="utf-8", errors="replace").splitlines():
            if line.startswith("model name"):
                return line.split(":", 1)[1].strip()
    return platform.processor() or "unknown"


def memory_bytes() -> int | None:
    try:
        return os.sysconf("SC_PAGE_SIZE") * os.sysconf("SC_PHYS_PAGES")
    except (ValueError, OSError, AttributeError):
        return None


def version(command: list[str]) -> str:
    return subprocess.check_output(command, cwd=ROOT, text=True).strip()


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def timed(command: list[str], repeats: int) -> dict[str, object]:
    durations = []
    last_stdout = ""
    for _ in range(repeats):
        start = time.perf_counter()
        process = subprocess.run(
            command,
            cwd=ROOT,
            check=True,
            capture_output=True,
            text=True,
        )
        durations.append(time.perf_counter() - start)
        last_stdout = process.stdout
    return {
        "command": command,
        "repeats": repeats,
        "seconds": durations,
        "median_seconds": statistics.median(durations),
        "minimum_seconds": min(durations),
        "last_stdout": last_stdout,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--polynomial",
        type=Path,
        default=ROOT / "target" / "external_interaction_model.phsm",
    )
    parser.add_argument(
        "--proof",
        type=Path,
        default=ROOT / "target" / "external_interaction_model.phcp",
    )
    parser.add_argument("--repeats", type=int, default=3)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    if args.repeats < 1:
        parser.error("--repeats must be positive")
    for path in (args.polynomial, args.proof):
        if not path.is_file():
            parser.error(f"missing artifact: {path}")

    subprocess.run(
        ["cargo", "build", "--release", "--example", "committed_workload"],
        cwd=ROOT,
        check=True,
    )
    rust_binary = ROOT / "target" / "release" / "examples" / "committed_workload"
    report = {
        "schema": "power-house-research-benchmark-v1",
        "generated_at_utc": datetime.now(timezone.utc).isoformat(),
        "git_commit": subprocess.check_output(
            ["git", "rev-parse", "HEAD"], cwd=ROOT, text=True
        ).strip(),
        "system": {
            "platform": platform.platform(),
            "machine": platform.machine(),
            "processor": cpu_model(),
            "python": platform.python_version(),
            "rustc": version(["rustc", "--version"]),
            "cargo": version(["cargo", "--version"]),
            "cpu_count": os.cpu_count(),
            "memory_bytes": memory_bytes(),
        },
        "artifacts": {
            "polynomial": {
                "path": str(args.polynomial),
                "bytes": args.polynomial.stat().st_size,
                "sha256": sha256(args.polynomial),
            },
            "proof": {
                "path": str(args.proof),
                "bytes": args.proof.stat().st_size,
                "sha256": sha256(args.proof),
            },
        },
        "rust": timed(
            [
                str(rust_binary),
                "verify",
                str(args.polynomial),
                str(args.proof),
            ],
            args.repeats,
        ),
        "python": timed(
            [
                "python3",
                "scripts/verify_sparse_certificate.py",
                str(args.proof),
                "--polynomial",
                str(args.polynomial),
            ],
            args.repeats,
        ),
    }

    encoded = json.dumps(report, indent=2, sort_keys=True) + "\n"
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(encoded, encoding="utf-8")
    print(encoded, end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
