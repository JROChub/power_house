# Power-House Operations Guide
Doc version: v0.1.53

This guide replaces the brittle bootstrap scripts with a single declarative
workflow that works on any VPS (including stock Ubuntu/Debian images) and
survives future Rust toolchain updates.

## 1. Requirements

- Rust toolchain (via `rustup`) and `cargo` on the build machine.
- Python ≥ 3.11 for the new control script (ships with most modern distros).
- `ssh`/`scp` access to every VPS. Passwords or keys both work.
- Systemd on the VPS *or* a custom restart command (see below).

## 2. Configure hosts

`scripts/netctl.py` reads host metadata from a TOML file. Copy the template and
adjust it to match your environment:

```bash
cp infra/ops_hosts.example.toml infra/ops_hosts.toml
$EDITOR infra/ops_hosts.toml
```

Each `[[node]]` entry needs:

| Key            | Description                                                                 |
| -------------- | --------------------------------------------------------------------------- |
| `name`         | Short identifier used by the CLI (ex: `boot1`).                             |
| `host`         | DNS name or IP reachable over SSH.                                          |
| `ssh_user`     | (optional) Remote user; defaults to `root`.                                  |
| `service`      | (optional) Systemd unit. Falls back to `powerhouse-{name}.service`.         |
| `binary_path`  | Destination of the `julian` binary (defaults to `/usr/local/bin/julian`).   |
| `config_dir`   | Directory that will receive `infra/*.json`.                                 |
| `log_path`     | File read by the `logs`/`follow` sub-commands.                               |
| `work_dir`     | Persistent ledger/log directory on the node.                                |
| `restart_command` | (optional) Command to run instead of `systemctl restart …`.              |

Override the hosts file path via `POWERHOUSE_HOSTS_FILE=/path/to/ops.toml` or
`python3 scripts/netctl.py --hosts-file …`.

## 3. Systemd template (optional)

Install the following on each VPS when using systemd. Replace placeholders with
values from your TOML entry, and prefer literal `/ip4/<peer-ip>/tcp/<port>/p2p/<peer-id>`
bootstrap multiaddrs when DNS is slow to update. Keep these unit files in your
infra repo or secrets manager—do not ship live IPs or seeds in the public tree.

```ini
[Unit]
Description=JULIAN bootstrap node (%i)
After=network-online.target
Wants=network-online.target

[Service]
User=root
WorkingDirectory=/var/lib/jrocnet/%i
ExecStart=/usr/local/bin/julian net start \
  --node-id %i \
  --log-dir /var/lib/jrocnet/%i \
  --listen /ip4/0.0.0.0/tcp/700%I \
  --bootstrap /ip4/<peer-ip>/tcp/<peer-port>/p2p/<peer-id> \
  --quorum 2 \
  --broadcast-interval 5000 \
  --policy /etc/jrocnet/governance.json \
  --key ed25519://boot%I-seed
Restart=always
RestartSec=2
Environment=RUST_LOG=info

[Install]
WantedBy=multi-user.target
```

Enable with:

```bash
systemctl enable --now powerhouse-boot1.service
```

If systemd is unavailable, set `restart_command = "supervisorctl restart julian"`
or similar in `infra/ops_hosts.toml`.

## 4. Common workflows

| Task                           | Command                                                                 |
| ------------------------------ | ----------------------------------------------------------------------- |
| Show configured hosts          | `python3 scripts/netctl.py list-hosts`                                  |
| Build + compress artifacts     | `python3 scripts/netctl.py package`                                     |
| Deploy to all hosts            | `python3 scripts/netctl.py deploy`                                      |
| Deploy to a subset             | `python3 scripts/netctl.py deploy --hosts boot1 boot2`                  |
| Deploy without restarting      | `python3 scripts/netctl.py deploy --no-restart`                         |
| Restart services               | `python3 scripts/netctl.py restart --hosts boot1`                       |
| Check systemd status           | `python3 scripts/netctl.py status`                                      |
| Tail logs                      | `python3 scripts/netctl.py logs --lines 200`                            |
| Follow logs continuously       | `python3 scripts/netctl.py follow --hosts boot1`                        |
| Open an interactive SSH shell  | `python3 scripts/netctl.py shell boot1`                                 |
| Run ad-hoc command             | `python3 scripts/netctl.py exec boot2 'df -h /var'`                     |

Use `--dry-run` with any command to preview the actions before executing them.

## 5. End-to-end checklist

1. `rustup update stable` (ensures compatibility with new compiler releases).
2. `cargo test --all` locally.
3. `python3 scripts/netctl.py package` (builds + archives).
4. `python3 scripts/netctl.py deploy --hosts boot1 boot2`.
5. `python3 scripts/netctl.py status` to verify systemd health.
6. `python3 scripts/netctl.py logs --lines 120` to ensure anchors broadcast.

Following the checklist keeps deployments deterministic and prevents the
"works-on-my-laptop" failures that plagued the previous bash wrappers.

## 6. Manual systemd rollout (without `netctl`)

When you only have one or two ingress VPS nodes, you can deploy manually:

1. **Build** – `cargo build --release --features net --bin julian`
2. **Upload** – `scp target/release/julian root@host:/root/julian.new`
3. **Install** – `ssh root@host 'install -m 0755 /root/julian.new /usr/local/bin/julian && rm /root/julian.new'`
4. **Install/refresh unit** – copy the template above with the correct `--node-id`, log paths, and `/ip4/<peer-ip>/tcp/<port>/p2p/<peer-id>` bootstraps.
5. **Reload & restart** – `systemctl daemon-reload && systemctl enable --now powerhouse-bootN.service`
6. **Tail logs** – `journalctl -u powerhouse-bootN.service -n 40 -f` should show exactly one “waiting for gossip peers…” followed by alternating `broadcasted local anchor` and `finality reached` lines.
7. **Connectivity check** – from each node run `nc -vz <other-ip> 700{1,2}`. If either direction fails, fix firewall/routing before blaming libp2p.

This mirrors what `scripts/netctl.py` automates, but the explicit steps are handy for quick maintenance windows or bare-metal installs.

## Evidence outbox and stake registry (DA + rollup)

- Availability faults and rollup verification/settlement failures append evidence lines to `evidence_outbox.jsonl`. The HTTP handler writes under the blob service base dir; the CLI defaults to an outbox next to the stake registry unless `--outbox` is provided. Forward or harvest this file for slashing/audit.
- Stake registry accounts track `{ balance, stake, slashed }`. Fees debit `balance`; rewards credit `balance`; bonding moves `balance -> stake`; slashing zeroes `stake` and sets `slashed = true`.
- Rollup settlement: HTTP `POST /rollup_settle` or `julian rollup settle|settle-file`. On failure, `RollupFaultEvidence { namespace, commitment, reason, payload? }` is written to the outbox; on success, fees can be split between operator and attesters.

## DA/rollup endpoint quick commands (ops)

- Submit: `curl -X POST http://<host>:8080/submit_blob -H 'X-Namespace: default' -H 'X-Fee: 10' --data-binary @file.bin`
- Commitment: `curl http://<host>:8080/commitment/default/<hash>`
- Sample: `curl "http://<host>:8080/sample/default/<hash>?count=2"`
- Prove storage: `curl http://<host>:8080/prove_storage/default/<hash>/0`
- Rollup settle: `curl -X POST http://<host>:8080/rollup_settle -H 'Content-Type: application/json' -d '{"namespace":"default","share_root":"…","payer_pk":"…","fee":1000,"mode":"optimistic"}'`
