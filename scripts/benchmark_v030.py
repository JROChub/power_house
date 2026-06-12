#!/usr/bin/env python3
"""Run and publish the reproducible Power House v0.3.0 benchmark report."""

import json
import pathlib
import subprocess


ROOT = pathlib.Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "benchmarks" / "v0.3.0" / "report.json"


def main() -> None:
    completed = subprocess.run(
        ["cargo", "run", "--release", "--example", "rootprint_benchmark"],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    report = json.loads(completed.stdout)
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(OUTPUT.relative_to(ROOT))


if __name__ == "__main__":
    main()
