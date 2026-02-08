# Power-House Operations Guide
Doc version: v0.1.54

This guide documents the production-grade deployment flow for the JULIAN network
nodes and blob service. It assumes explicit manual control (no hidden scripts),
so every step is auditable.

## 1. Requirements

- Rust toolchain (via `rustup`) and `cargo` on the build machine.
- `ssh`/`scp` access to every VPS (keys or password).
- systemd on the VPS (recommended).
- Python 3 on the VPS (healthcheck runner).

## 2. Build the binary

```bash
cargo build --release --features net --bin julian
```

## 3. Install/upgrade on each VPS

```bash
scp target/release/julian root@HOST:/usr/local/bin/julian.new
ssh root@HOST 'install -m 0755 /usr/local/bin/julian.new /usr/local/bin/julian && rm /usr/local/bin/julian.new'
```

## 4. Systemd units and environment

Use the unit files in `infra/systemd/` as the canonical service definitions.
Deploy them with:

```bash
scripts/deploy_units.sh root@BOOT1 root@BOOT2
```

Environment files (store in `/etc/jrocnet/`):

- `powerhouse-common.env` (shared settings)
- `powerhouse-boot1.env` (boot1 node)
- `powerhouse-boot2.env` (boot2 node)

Copy the examples from `infra/systemd/` and fill in real values.
The node wrappers read these files and start `/usr/local/bin/julian` using
`/usr/local/bin/powerhouse-boot.sh`.

Recommended production flags:

- `--blob-dir /var/lib/jrocnet/<node>/blobs`
- `--blob-listen 0.0.0.0:8181`
- `--max-blob-bytes <bytes>`
- `--blob-auth-token <token>` (enforces Authorization or x-api-key)
- `--blob-max-concurrency <n>` (defaults to 128)
- `--blob-request-timeout-ms <ms>` (defaults to 10000)
- `--attestation-quorum <n>`
- `--metrics :9100` (Prometheus metrics)

## 5. Health checks + alerts

Timers are included for continuous health checks:

```bash
systemctl enable --now powerhouse-healthcheck@boot1.timer
systemctl enable --now powerhouse-healthcheck@boot2.timer
```

The healthcheck verifies:
- systemd service state
- blob `/healthz`
- metrics `/metrics`
- finality progress (stall detection)

Alerts are delivered via `/usr/local/lib/powerhouse/alert.sh`.
If `PH_ALERT_EMAIL` is empty, alerts log to syslog only. To enable email,
set `PH_ALERT_EMAIL`/`PH_ALERT_FROM` and wire any local MTA (msmtp, postfix,
or equivalent). Keep SMTP credentials in a root‑only file.

## 6. Metrics + log export

Metrics endpoint:

```bash
curl http://<host>:9100/metrics
```

Hourly log exports (for shipping/archival):

```bash
systemctl enable --now powerhouse-log-export@boot1.timer
systemctl enable --now powerhouse-log-export@boot2.timer
```

Exports are written to `PH_LOG_EXPORT_DIR` (gzip). Optional shipping can be
enabled by setting `PH_LOG_SHIP_HOST` and related vars in the node env file.

## 7. Metrics snapshots (soak test)

Enable metrics snapshots (every 5 minutes):

```bash
systemctl enable --now powerhouse-metrics@boot1.timer
systemctl enable --now powerhouse-metrics@boot2.timer
```

Snapshots append to `PH_METRICS_OUT_DIR/metrics-<node>.jsonl`. Use this log to
baseline 24‑hour soak behavior (finality rate, rejects, invalid envelopes).

## 8. Backups + restore

Daily backups are enabled by:

```bash
systemctl enable --now powerhouse-backup@boot1.timer
systemctl enable --now powerhouse-backup@boot2.timer
```

Manual backup:

```bash
/usr/local/lib/powerhouse/backup.sh
```

Restore:

```bash
/usr/local/lib/powerhouse/restore.sh /var/backups/jrocnet/<archive>.tar.zst
```

## 9. Rollback

Use versioned releases under `/opt/jrocnet/releases`:

```bash
/usr/local/lib/powerhouse/deploy_release.sh /path/to/julian
/usr/local/lib/powerhouse/rollback.sh
```

## 10. Blob/DA endpoints

- Health: `curl http://<host>:8181/healthz`
- Submit: `curl -X POST http://<host>:8181/submit_blob -H 'X-Namespace: default' -H 'X-Fee: 10' --data-binary @file.bin`
- Commitment: `curl http://<host>:8181/commitment/default/<hash>`
- Sample: `curl "http://<host>:8181/sample/default/<hash>?count=2"`
- Prove storage: `curl http://<host>:8181/prove_storage/default/<hash>/0`
- Rollup settle: `curl -X POST http://<host>:8181/rollup_settle -H 'Content-Type: application/json' -d '{"namespace":"default","share_root":"…","payer_pk":"…","fee":1000,"mode":"optimistic"}'`

If `--blob-auth-token` is set, add:
- `Authorization: Bearer <token>` or `x-api-key: <token>`

## 11. Namespace policies and rate limits

`blob_policy.json` supports per-namespace guards:
- `max_bytes` (size cap)
- `min_fee` (fee floor)
- `max_per_min` (rate limit per namespace)

These are enforced at ingest time and are the first line of defense against
abuse on public endpoints.
