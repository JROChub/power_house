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

`v0.3.2/rpc-report.json` records the public-RPC acceptance run for retired chain
177155: controlled backend failover, automatic node recovery, and bounded
read-only load profiles. It does not characterize the current chain 2026092601
or validator runtime 20261008.3. Internet latency, edge routing, throttling, and the test
origin affect those values, so they are measurements rather than capacity
guarantees.

`v0.4.1/verified-observatory.json` records repeated Observatory verification with
and without reuse of verified Rootprint state. The report includes the workload,
toolchain, timing, and reproduction command; its speed ratio applies to that
measured workload. `v0.4.1/dependency-audit-scope.json` records dependency advisory
applicability and the admission checks used for guest ELF inputs.
