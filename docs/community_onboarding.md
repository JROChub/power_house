# Community Node Onboarding (Phase 3)

> Historical Phase 3 onboarding material. Bootstrap addresses, quorum values,
> policies, and incentives must be obtained from the current operator release.
> See [Documentation](README.md) and [Network Operations](ops.md).

This guide covers a minimal operator path for joining the Power-House network and maintaining 90%+ uptime.

## Prerequisites
- Linux host or Docker runtime
- 2 vCPU / 4 GB RAM minimum (4 vCPU preferred)
- Open TCP port for p2p (default 7000+), metrics (optional 9100)

## Quick Start (Docker)

1) Build the image:
```
docker build -t powerhouse-node .
```

2) Run a node (adjust ports + bootstraps):
```
docker run -it --rm \
  -p 7003:7003 -p 9103:9100 \
  -v $PWD/docker/node3:/data \
  powerhouse-node net start \
    --node-id node3 \
    --log-dir /data/logs \
    --listen /ip4/0.0.0.0/tcp/7003 \
    --bootstrap <BOOTSTRAP_MULTIADDR_1> \
    --bootstrap <BOOTSTRAP_MULTIADDR_2> \
    --broadcast-interval 1500 \
    --quorum 7 \
    --metrics :9100
```

Notes:
- Add `--key ed25519://<seed>` if you want a stable peer identity.
- If you expose metrics, restrict access with firewall rules.

## Native (Linux)

```
cargo build --release --features net --bin julian
sudo install -m 0755 target/release/julian /usr/local/bin/julian

julian net start \
  --node-id node3 \
  --log-dir ./logs/node3 \
  --listen /ip4/0.0.0.0/tcp/7003 \
  --bootstrap <BOOTSTRAP_MULTIADDR_1> \
  --bootstrap <BOOTSTRAP_MULTIADDR_2> \
  --broadcast-interval 1500 \
  --quorum 7 \
  --metrics :9100
```

## Uptime Expectations
- Target 90%+ availability.
- Keep storage stable; do not rotate `--key` seeds without coordination.
- Monitor `anchors_received_total`, `finality_events_total` and peer counts via Prometheus.

## Permissionless Mode
For the historical public-growth configuration, see the
[Permissionless Join Guide](permissionless_join.md).

## Tokenomics (Stake Registry)
- Fees and rewards are tracked in `stake_registry.json`.
- Submitters can pay fees by providing `x-fee`, `x-publisher`, and `x-publisher-sig` headers.
- Operators and attestors receive rewards according to `operator_reward_bps` (per namespace).

## Governance
- Membership is controlled by governance state (`stake` or `multisig` backends).
- Use the current CLI help and [Network Operations](ops.md) for governance
  procedures.
