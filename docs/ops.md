# Power-House Operations Guide
Doc version: v0.1.53

This guide documents the production-grade deployment flow for the JULIAN network
nodes and blob service. It assumes explicit manual control (no hidden scripts),
so every step is auditable.

## 1. Requirements

- Rust toolchain (via `rustup`) and `cargo` on the build machine.
- `ssh`/`scp` access to every VPS (keys or password).
- systemd on the VPS (recommended).

## 2. Build the binary

```bash
cargo build --release --features net --bin julian
```

## 3. Install/upgrade on each VPS

```bash
scp target/release/julian root@HOST:/usr/local/bin/julian.new
ssh root@HOST 'install -m 0755 /usr/local/bin/julian.new /usr/local/bin/julian && rm /usr/local/bin/julian.new'
```

## 4. Systemd units

Use the unit files in `infra/systemd/` as the canonical service definitions.
Deploy them with:

```bash
scripts/deploy_units.sh root@BOOT1 root@BOOT2
```

If you need custom flags, update the shell wrappers on the hosts
(`/usr/local/bin/powerhouse-boot1.sh` and `/usr/local/bin/powerhouse-boot2.sh`).

Recommended production flags:

- `--blob-dir /var/lib/jrocnet/<node>/blobs`
- `--blob-listen 0.0.0.0:8080`
- `--max-blob-bytes <bytes>`
- `--blob-auth-token <token>` (enforces Authorization or x-api-key)
- `--blob-max-concurrency <n>` (defaults to 128)
- `--blob-request-timeout-ms <ms>` (defaults to 10000)
- `--attestation-quorum <n>`

## 5. Health checks

1. Service status: `systemctl is-active powerhouse-bootN.service`
2. Logs: `journalctl -u powerhouse-bootN.service -n 40 -f`
   - Expect `QSYS|mod=ANCHOR|evt=STANDBY` then alternating
     `QSYS|mod=ANCHOR|evt=BROADCAST` and `QSYS|mod=QUORUM|evt=FINALIZED`.
3. Blob service health: `curl http://<host>:8080/healthz`
4. Metrics (if enabled): `curl http://<host>:9100`

## 6. Blob/DA endpoints

- Health: `curl http://<host>:8080/healthz`
- Submit: `curl -X POST http://<host>:8080/submit_blob -H 'X-Namespace: default' -H 'X-Fee: 10' --data-binary @file.bin`
- Commitment: `curl http://<host>:8080/commitment/default/<hash>`
- Sample: `curl "http://<host>:8080/sample/default/<hash>?count=2"`
- Prove storage: `curl http://<host>:8080/prove_storage/default/<hash>/0`
- Rollup settle: `curl -X POST http://<host>:8080/rollup_settle -H 'Content-Type: application/json' -d '{"namespace":"default","share_root":"…","payer_pk":"…","fee":1000,"mode":"optimistic"}'`

If `--blob-auth-token` is set, add:
- `Authorization: Bearer <token>` or `x-api-key: <token>`

## 7. Namespace policies and rate limits

`blob_policy.json` supports per-namespace guards:
- `max_bytes` (size cap)
- `min_fee` (fee floor)
- `max_per_min` (rate limit per namespace)

These are enforced at ingest time and are the first line of defense against
abuse on public endpoints.
