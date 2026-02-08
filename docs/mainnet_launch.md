# POWER_HOUSE MAINNET LAUNCH – OPERATIONS GUIDE
Doc version: v0.1.54

This guide promotes your current two‑boot topology (boot1, boot2) to an open, public mainnet without manual peer approvals. It preserves uptime, removes trust boundaries, and keeps the network observable and maintainable.

Contents
- Overview and principles
- Policy changes (open network)
- Systemd updates (boot1, boot2)
- Networking and discovery
- Monitoring and verification
- Recommended join command for users
- Rollout plan (zero‑downtime)
- Troubleshooting and rollback

## Overview and Principles
- Zero downtime: restart one boot node at a time.
- Preserve identities: keep each node’s `--key` seed so Peer IDs remain stable.
- Open by default: remove policy gating; connections and gossip do not require approval.
- Observability: expose Prometheus metrics on `:9100` (or bind to localhost only if preferred).

## Policy Changes (Open Network)
Remove governance gating flags from boot nodes so any peer can connect and gossip. Anchor acceptance becomes open; if you later need quorum gating, reintroduce `--policy` with an allowlist or stake descriptor.

Remove the following flags from both boot nodes:
- `--policy /etc/jrocnet/governance.json`
- `--policy-allowlist /etc/jrocnet/allow.json`

The rest of the runtime remains unchanged.

## Systemd Updates (Boot1 and Boot2)
Use the templates in `infra/systemd/` plus node env files:

- `/etc/jrocnet/powerhouse-common.env`
- `/etc/jrocnet/powerhouse-boot1.env`
- `/etc/jrocnet/powerhouse-boot2.env`

Those feed `/usr/local/bin/powerhouse-boot.sh` (the shared launcher).
Copy the example env files from `infra/systemd/` and set:
`PH_BOOTSTRAPS`, `PH_BLOB_AUTH_TOKEN`, `PH_METRICS_ADDR`, and per-node paths.

Apply safely on each host:
```
systemctl daemon-reload
systemctl enable --now powerhouse-boot1.service   # on boot1
systemctl enable --now powerhouse-boot2.service   # on boot2
```

Enable ops timers (health, backup, log export):
```
systemctl enable --now powerhouse-healthcheck@boot1.timer powerhouse-backup@boot1.timer powerhouse-log-export@boot1.timer
systemctl enable --now powerhouse-healthcheck@boot2.timer powerhouse-backup@boot2.timer powerhouse-log-export@boot2.timer
```

## Networking and Discovery
- Open inbound TCP 7001 (boot1) and 7002 (boot2) in cloud firewall and any host firewall (UFW/iptables).
- Keep DNS seeds resolving to the public IPs: `boot1.jrocnet.com`, `boot2.jrocnet.com`.
- Optional: add more geographically distributed boot nodes to improve initial connectivity.

## Monitoring and Verification
Key runtime metrics (Prometheus on each node):
```
curl -s 127.0.0.1:9100 | egrep "anchors_(received|verified)_total|invalid_envelopes_total|finality_events_total|gossipsub_rejects_total"
```
Connection count (rough, per port):
```
ss -antp | grep ":7001" | grep ESTAB | wc -l   # boot1
ss -antp | grep ":7002" | grep ESTAB | wc -l   # boot2
```
Service logs (finality and gossip):
```
journalctl -u powerhouse-boot1.service -n 200 --no-pager | grep -E "QSYS\\|mod=ANCHOR\\|QSYS\\|mod=QUORUM"
journalctl -u powerhouse-boot2.service -n 200 --no-pager | grep -E "QSYS\\|mod=ANCHOR\\|QSYS\\|mod=QUORUM"
```

Blob health (auth required if token set):
```
curl -H 'Authorization: Bearer <token>' http://<host>:8181/healthz
```

## Recommended Join Command (for users)
Users can join without your approval; connections form automatically. Provide both seeds:
```
julian net start \
  --node-id node \
  --log-dir ./logs/node \
  --listen /ip4/0.0.0.0/tcp/0 \
  --bootstrap /dns4/boot1.jrocnet.com/tcp/7001/p2p/12D3KooWLASw1JVBdDFNATYDJMbAn69CeWieTBLxAKaN9eLEkh3q \
  --bootstrap /dns4/boot2.jrocnet.com/tcp/7002/p2p/12D3KooWRLM7PJrtjRM6NZPX8vmdu4YGJa9D6aPoEnLcE1o6aKCd \
  --broadcast-interval 5000 \
  --quorum 2 \
  --metrics :9100
```
Notes:
- `--key ed25519://node-seed` is optional; include only if a stable Peer ID is desired.
- For non-public metrics, replace `--metrics :9100` with `--metrics 127.0.0.1:9100`.

## Rollout Plan (Zero‑Downtime)
1) Boot1: update unit, reload, restart; verify metrics and logs.
2) Boot2: update unit, reload, restart; verify.
3) Confirm both see each other and continue finality (`entries=…`).
4) Announce join command above to the community.

## Troubleshooting and Rollback
- If peers cannot connect:
  - Verify ports 7001/7002 are open (cloud + UFW).
  - Check DNS seed resolution.
- If anchor conflicts appear during restarts:
  - Do not copy `fold_digest.txt` or `checkpoints/` between nodes.
  - Clear stale metadata before start: remove `/logs/checkpoints` and `fold_digest.txt` only.
- Rollback:
  - Reapply previous unit files (with policy) and restart one node at a time.
  - Reintroduce `--policy` if you must gate quorum acceptance again.

## DA/rollup endpoints (operator quick ref)

- Blob ingest (HTTP): `POST /submit_blob` with headers `X-Namespace`, optional `X-Fee`; returns `hash`, `share_root`, `pedersen_root`.
- Commitment: `GET /commitment/<ns>/<hash>` returns blob metadata + attestations.
- Sampling: `GET /sample/<ns>/<hash>?count=N` returns shares + Merkle proofs.
- Storage proof: `GET /prove_storage/<ns>/<hash>/<idx>`; on missing share, evidence is written to `evidence_outbox.jsonl`.
- Rollup settle: `POST /rollup_settle` (optimistic or zk) with commitment roots and payer/operator/attesters; on failure, `RollupFaultEvidence` is written to the evidence outbox.

Evidence outbox is under the blob service base dir by default; collect it for slashing/audit.
