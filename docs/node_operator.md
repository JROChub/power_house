# MFENX Node Operator Guide

Release scope: Power House v0.3.15.

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
cargo install power_house --version 0.3.15 --features net --locked
julian --version
```

Or use the release container:

```bash
docker pull ghcr.io/jrochub/power_house:0.3.15
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

For a public observer to appear on mfenx.com without validator admission, use
the guided observer setup:

```bash
julian observer setup \
  --node-id external-observer-1 \
  --operator "Operator Name" \
  --region <region> \
  --public-host <host> \
  --p2p-port 7001 \
  --metrics-port 9102 \
  --output observer.registration.json
```

The public observer registry requires a matching live
`powerhouse_node_identity` metric before the observer is counted. See
[Public Observer Registry](observer_registry.md). Submit the signed registration
at `https://mfenx.com/register.html`, or use:

```bash
julian observer submit observer.registration.json
julian observer status <tracking-id>
```

Only the signed registration is uploaded. The private node key remains local.

After the observer is running, run the doctor:

```bash
julian observer doctor \
  --node-id external-observer-1 \
  --public-host <host> \
  --p2p-port 7001 \
  --metrics-port 9102
```

If the external probe fails, forward TCP `7001` and TCP `9102` from the router
or cloud firewall to the observer machine. Do not use a private LAN address as
the public host.

After admission is approved, create a signed monitoring registration:

```bash
julian validator-registry register \
  --node-id external-validator-1 \
  --operator "Operator Name" \
  --region <region> \
  --public-host <host> \
  --metrics-port 9100 \
  --system-metrics-port 9101 \
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
  --bootstrap /ip4/159.203.109.128/tcp/7002/p2p/12D3KooWMCyR9gXPXCGAMNCVJDKbisohRRq8oaTHNiR91HZ67cSR \
  --bootstrap /ip4/64.23.182.213/tcp/7002/p2p/12D3KooWGEHbPAQ9ZVB9Uqg1j8CnsNqKvS2xmAe5cmT4w3idUtmQ \
  --bootstrap /ip4/164.92.150.22/tcp/7002/p2p/12D3KooWFNv4sZfDKypMeWqRetghHxXzkhPTc4PvynDZKSETJqd8 \
  --key "$HOME/.powerhouse/node.key" \
  --metrics 0.0.0.0:9102
```

An observer verifies and relays network data but does not count toward
validator quorum.

Public observer bootnodes use TCP `7002`. Validator TCP `7001` remains a
restricted validator mesh port and should not be opened to arbitrary public
peers.

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
