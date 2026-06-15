# MFENX Node Operator Guide

Release scope: Power House v0.3.7.

This guide starts a public observer. Validator admission additionally requires
a consensus identity approved by the active membership policy.

## Requirements

- Ubuntu 24.04 or another current Linux distribution
- 2 CPU cores, 4 GB RAM, 60 GB SSD minimum
- Stable public IPv4 or DNS
- TCP `7001` reachable for validator transport
- Rust stable when building from source

Keep RPC, metrics, private keys, and blob administration bound to localhost
unless a firewall and authenticated reverse proxy explicitly protect them.

## Install

```bash
cargo install power_house --version 0.3.7 --features net --locked
julian --version
```

Or use the release container:

```bash
docker pull ghcr.io/jrochub/power_house:0.3.7
```

## Create An Identity

```bash
install -d -m 0700 "$HOME/.powerhouse"
head -c 32 /dev/urandom >"$HOME/.powerhouse/node.key"
chmod 0600 "$HOME/.powerhouse/node.key"
julian key-info "$HOME/.powerhouse/node.key" --json
```

Never send the private key. Share only the peer ID and public key when
requesting validator admission.

After admission is approved, create a signed monitoring registration:

```bash
julian validator-registry create \
  --key "$HOME/.powerhouse/node.key" \
  --node-id external-validator-1 \
  --operator "Operator Name" \
  --region <region> \
  --p2p-address /dns4/<host>/tcp/7001/p2p/<peer-id> \
  --metrics-url http://<monitoring-address>:9100/metrics \
  --system-metrics-url http://<monitoring-address>:9101/metrics \
  --output validator.registration.json
```

The signed record proves control of the claimed identity. It is counted only
after policy admission and a matching live identity health check. See
[Signed Validator Registry](validator_registry.md).

## Start An Observer

```bash
julian net start \
  --node-id external-observer-1 \
  --log-dir "$HOME/.powerhouse/logs" \
  --blob-dir "$HOME/.powerhouse/data" \
  --listen /ip4/0.0.0.0/tcp/7001 \
  --bootstrap /dns4/<bootstrap-host>/tcp/7001/p2p/<peer-id> \
  --key "$HOME/.powerhouse/node.key" \
  --metrics 127.0.0.1:9100
```

An observer verifies and relays network data but does not count toward
validator quorum.

## Run Under systemd

Use `infra/systemd/powerhouse-node@.service` and the examples under
`infra/systemd/`. The unit enforces:

- restart on failure
- restricted filesystem access
- private temporary storage
- explicit state and log directories
- bounded file descriptors

Enable the health, metrics, backup, and log-export timers supplied with the
repository.

## Upgrade

Build or download the exact release, verify `julian --version`, back up state,
then use the rolling deployment helper. Never regenerate
`native_chain_state.json`, the membership policy, or a validator key during an
upgrade.

## Troubleshooting

1. Confirm `systemctl status powerhouse-node@<name>`.
2. Confirm local `/healthz` and `/metrics`.
3. Confirm at least one `powerhouse_connected_peers`.
4. Compare chain ID, block height, hash, and state root with a public RPC.
5. Preserve logs and state before resynchronizing.

See [Incident Response](incident_response.md) before deleting or replacing
state.
