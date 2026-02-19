# Community Node Onboarding (Phase 3)

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
    --bootstrap /ip4/137.184.33.2/tcp/7001/p2p/12D3KooWLASw1JVBdDFNATYDJMbAn69CeWieTBLxAKaN9eLEkh3q \
    --bootstrap /ip4/146.190.126.101/tcp/7002/p2p/12D3KooWRLM7PJrtjRM6NZPX8vmdu4YGJa9D6aPoEnLcE1o6aKCd \
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
  --bootstrap /ip4/137.184.33.2/tcp/7001/p2p/12D3KooWLASw1JVBdDFNATYDJMbAn69CeWieTBLxAKaN9eLEkh3q \
  --bootstrap /ip4/146.190.126.101/tcp/7002/p2p/12D3KooWRLM7PJrtjRM6NZPX8vmdu4YGJa9D6aPoEnLcE1o6aKCd \
  --broadcast-interval 1500 \
  --quorum 7 \
  --metrics :9100
```

## Uptime Expectations
- Target 90%+ availability.
- Keep storage stable; do not rotate `--key` seeds without coordination.
- Monitor `anchors_received_total`, `finality_events_total` and peer counts via Prometheus.

## Permissionless Mode
For public growth, run the network without policy gating. See `docs/permissionless_join.md`.

## Tokenomics (Stake Registry)
- Fees and rewards are tracked in `stake_registry.json`.
- Submitters can pay fees by providing `x-fee`, `x-publisher`, and `x-publisher-sig` headers.
- Operators and attestors receive rewards according to `operator_reward_bps` (per namespace).

## Governance
- Membership is controlled by governance state (`stake` or `multisig` backends).
- Use the governance update flow in `README.md` to add/remove members.
