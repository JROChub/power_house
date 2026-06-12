# Benchmarks

`v0.3.0/report.json` is produced by:

```bash
python3 scripts/benchmark_v030.py
```

The report measures:

- closed-form verification over `2^70`;
- seeded-affine verification over `2^4096`;
- `.pha` fingerprinting and core verification;
- construction and full verification of a 2,049-branch Rootprint graph;
- repeated fingerprint reproducibility;
- published Rust/Python conformance and mutation coverage.

Timing values are machine-dependent measurements, not complexity guarantees.
The relevant algorithmic bounds and domain restrictions remain documented in
the protocol specifications.
