# MFENX Network Load Testing

Release scope: Power House v0.3.2.

Run read-only public RPC tests before transaction tests. Never direct an
unbounded load generator at production.

## RPC Baseline

```bash
python3 scripts/rpc_load_test.py \
  https://rpc.mfenx.com \
  --duration 45 \
  --concurrency 6 \
  --max-error-rate 0 \
  --max-p95-ms 500
```

The report includes requests per second, status/error counts, and p50/p95/p99
latency. HTTP `429` responses are intentional edge throttling and still count
against the error gate. Repeat from at least three external regions.

The June 12, 2026 single-origin production run sustained 69.114 requests per
second with zero errors and 106.354 ms p95 latency. A higher profile passed
the 0.5% error gate at 91.51 requests per second and 192.992 ms p95. Unpaced
overload reached the configured edge limit and returned HTTP `429`.
The complete measured report is in
`benchmarks/v0.3.2/rpc-report.json`; it is not an extrapolation to multi-region
or transaction throughput.

## Data Availability

```bash
URL=http://127.0.0.1:8181/submit_blob \
CONCURRENCY=16 \
DURATION=60 \
scripts/load_test.sh
```

## Local Scale And Fault Tests

```bash
scripts/scale_net.sh 10
scripts/smoke_net.sh
scripts/fault_test.sh
scripts/stop_scale_net.sh
```

Capture CPU, memory, disk, peer count, finality rate, RPC latency, rejected
requests, and recovery time. Increase one dimension at a time and publish the
first saturation point rather than extrapolating beyond measured capacity.
