# MFENX Stable Public Network Roadmap

Release scope: Power House v0.3.2.

The target is a public network that fails predictably, recovers automatically,
and exposes enough evidence for operators and users to determine its state.

## Phase 1: Infrastructure Stabilization

| Deliverable | Implementation | Verification |
| --- | --- | --- |
| Three regional validators | `nyc3`, `sfo3`, `ams3`; static quorum `2` | Compare `/healthz`, block hash, and state root |
| Health-aware public edge | DigitalOcean global load balancer | External `scripts/check_rpc.py` |
| Rate limiting | Nginx request and connection zones | RPC load test and Nginx metrics/logs |
| Automatic recovery | systemd restart plus healthcheck restart cooldown | Stop or wedge a validator in a controlled fault test |
| Monitoring | Prometheus, Alertmanager, blackbox exporter, node exporter, Grafana | Prometheus targets and Alertmanager test alert |
| Deployment documentation | Sealed bundle, rolling deploy, Terraform, recovery runbooks | Rebuild a disposable environment from docs |

The public service name is **LAX MFENX RPC**. The canonical edge is
`https://rpc.mfenx.com`, which is also the submitted ChainList endpoint.

### Production Verification

The v0.3.2 production acceptance run on June 12, 2026 verified:

- three active validators in `nyc3`, `sfo3`, and `ams3`;
- six validator peer connections and identical finalized state;
- successful public RPC service while the Amsterdam HTTP backend was offline;
- automatic restart and state-preserving recovery after a validator `SIGKILL`;
- seven healthy Prometheus targets and an operational public status feed;
- strong managed TLS, dual-stack ingress, and HTTP-to-HTTPS redirection;
- a zero-error 45-second profile at 69.114 requests/second and 106.354 ms p95,
  plus a gated pass at 91.51 requests/second and 192.992 ms p95.

The measured load profile is a single-origin edge test, not a claim of total
network capacity. See `benchmarks/v0.3.2/rpc-report.json`.

## Phase 2: Operational Reliability

| Deliverable | Implementation |
| --- | --- |
| Infrastructure as code | `infra/terraform/digitalocean` |
| Reproducible consensus deployment | `generate_rpc_cluster.py` and `deploy_rpc_cluster.sh` |
| Version-safe rolling upgrades | Health, binary version, and RPC version checks with rollback |
| Incident process | `incident_response.md` |
| Public operator documentation | `node_operator.md` |
| Capacity testing | `rpc_load_test.py`, `load_test.sh`, and `scale_net.sh` |
| Release governance | `check_release_consistency.py` in CI and tag workflows |

## Phase 3: Decentralization And Growth

The software and documentation can be completed in-repository. Independent
ownership cannot be manufactured by the core operator, so it remains a
measured network outcome.

| Outcome | Acceptance criterion |
| --- | --- |
| External operators | At least two independently administered nodes |
| Validator diversity | At least five validators across three providers and three regions |
| Public status | Current RPC, validator, peer, block, and 24-hour probe state |
| Mainnet governance | Published launch, upgrade, rollback, and migration criteria |
| EVM integration | Wallet metadata, ChainList endpoint, examples, and publication probe |

No uptime, decentralization, or mainnet-readiness claim is complete solely
because a configuration file exists. The acceptance criteria require observed
production evidence.

## Release Gate

Every release must pass:

```bash
python3 scripts/check_release_consistency.py
cargo publish --dry-run --locked
python3 scripts/check_rpc.py https://rpc.mfenx.com \
  --expected-chain-id 177155 --require-cors
```

The release version must agree across Cargo, Cargo.lock, Python, active guides,
website labels, network metadata, Docker tags, release notes, and the Git tag.
