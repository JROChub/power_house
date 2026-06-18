# LAX MFENX RPC Production Deployment

Release scope: Power House v0.3.7.

The retired VPS hosts should not be restored. Replace them with a reproducible
three-validator deployment whose membership, keys, genesis balances, service
configuration, and public edge are independently verifiable.

## Target architecture

- Three DigitalOcean Droplets in `nyc3`, `sfo3`, and `ams3`.
- Static validator membership with quorum `2`.
- Authenticated libp2p validator traffic on TCP `7001`, restricted by tag.
- Finalized RPC bound to `127.0.0.1:8545` on each validator.
- Nginx on port `80`, reachable only from the Global Load Balancer.
- Global ingress and managed TLS for a delegated `rpc.mfenx.com` DNS zone.
- Weekly Droplet backups plus application-level state backups.
- External probes comparing finalized height, block hash, and state root.
- Prometheus, Alertmanager, blackbox exporter, node exporter, and Grafana.
- Signed validator identity registry with dynamic Prometheus discovery.
- Optional signed public observer registry with separate public peer telemetry.
- A public status API at `/network-status.json`.
- A public observer reachability probe at `/observer-probe`.

Do not expose TCP `8545`, metrics, blob storage, or SSH directly to the
internet. Limit SSH to the operator's public CIDR and permit HTTP only from the
DigitalOcean Global Load Balancer.

## Authenticate DigitalOcean

Install `doctl`, revoke any token that has appeared in chat, logs, shell
history, or source control, and create a new write token. Store it only in a
local `doctl` context:

```bash
doctl auth init --context mfenx-production
```

Verify the new account, its three-Droplet limit, regions, size, SSH key, and
empty resource inventory:

```bash
scripts/digitalocean_rpc_preflight.sh
```

If the new account has no SSH key yet, register the existing operator key:

```bash
doctl --context mfenx-production compute ssh-key import mfenx-operator \
  --public-key-file ~/.ssh/id_ed25519.pub
```

Never pass the token as a command-line argument or commit the `doctl`
configuration.

## Provision the DigitalOcean edge

Register an SSH key in DigitalOcean, determine the operator's public IPv4 CIDR,
and review the non-billable plan:

```bash
scripts/provision_digitalocean_rpc.sh \
  --ssh-key <fingerprint-or-id> \
  --ssh-cidr <public-ip>/32
```

Create the DNS zone and resources only after reviewing the estimated charges:

```bash
scripts/provision_digitalocean_rpc.sh \
  --ssh-key <fingerprint-or-id> \
  --ssh-cidr <public-ip>/32 \
  --apply
```

On the first run, the provisioner creates the `rpc.mfenx.com` DNS zone and
exits with status `2` until the public delegation exists. In Namecheap, add
three `NS` records with host `rpc`:

```text
ns1.digitalocean.com
ns2.digitalocean.com
ns3.digitalocean.com
```

This delegates only `rpc.mfenx.com`; the apex website and mail records remain
at Namecheap. After DNS propagation, rerun the same `--apply` command. The
provisioner verifies all three authoritative nameservers before creating the
Global Load Balancer.

The provisioner creates or preserves:

- project `MFENX Power-House`
- tag `mfenx-rpc-validator`
- delegated DNS zone `rpc.mfenx.com`
- firewall `mfenx-rpc-firewall`
- `mfenx-validator-1` in `nyc3`
- `mfenx-validator-2` in `sfo3`
- `mfenx-validator-3` in `ams3`
- Global Load Balancer `lax-mfenx-rpc`, publicly named **LAX MFENX RPC**

It refuses an existing validator whose region or size disagrees with the
requested production topology. Infrastructure metadata is written to
`deployment/generated/digitalocean-infra.json`; it contains IDs and addresses,
not credentials.

## Generate the sealed cluster bundle

Build a network-enabled release binary:

```bash
cargo build --release --locked --features net --bin julian
```

Generate three identities and deterministic configuration. Use the three
Droplet public IPv4 addresses from the infrastructure manifest. Cloud Firewall
restricts TCP `7001` to the validator tag:

```bash
scripts/generate_rpc_cluster.py \
  --output deployment/generated/mfenx-production \
  --host <validator-1-ipv4> \
  --host <validator-2-ipv4> \
  --host <validator-3-ipv4> \
  --fund 0xYOUR_GENESIS_ADDRESS:1000000
```

The output directory contains consensus private keys plus signed public
validator registrations and an assembled `validator-registry.json`. It is
intentionally created with restrictive permissions and must be encrypted,
backed up, and kept outside source control.

## Install the cluster

After the hosts exist and SSH access works:

```bash
scripts/deploy_rpc_cluster.sh \
  deployment/generated/mfenx-production \
  root@<validator-1-ipv4> \
  root@<validator-2-ipv4> \
  root@<validator-3-ipv4>
```

The deployer installs the same binary and policy on all validators, copies only
the matching private key to each node, refuses to replace a conflicting genesis
registry, and preserves existing finalized chain state during upgrades. It then
restarts one validator at a time, waits for RPC health, verifies
`web3_clientVersion`, and rolls back that validator if validation fails.

## Deploy monitoring

Allow validator-tag traffic to TCP `9090`, `9100`, and `9101`, then run:

```bash
POWER_HOUSE_RELEASE=0.3.7 \
SSH_OPTS="-F /dev/null -o StrictHostKeyChecking=accept-new" \
scripts/deploy_monitoring_stack.sh \
  root@<validator-1-ipv4> \
  root@<validator-2-ipv4> \
  root@<validator-3-ipv4>
```

Grafana binds to `127.0.0.1:3000`; open it through an SSH tunnel. Alertmanager
always delivers to the local journal. Set `PH_SLACK_WEBHOOK_URL` or
`PH_PAGERDUTY_ROUTING_KEY` in the root-only monitoring alert environment to
add an external destination.

The deployment enables `powerhouse-validator-registry.timer` on each node.
Every 15 seconds it verifies signatures and policy admission, checks live
identity and health, then atomically updates the status state. Only the primary
monitor uses the generated Prometheus discovery files, but every RPC replica
can serve the same independently reconciled public status.

The deployment also enables `powerhouse-observer-registry.timer`. If
`/etc/powerhouse/observer-registry.json` is absent, the public API reports the
observer layer as staged without changing RPC health. When a signed observer
registry is present, the observer reconciler verifies signatures and live
identity metrics, then publishes `observer_peers`,
`public_peer_connections`, and observer Prometheus discovery independently from
validator quorum.

The status API also serves `/observer-probe`. The registration page and
`julian observer doctor` use this route to test an operator's public metrics
and p2p ports from the production edge. The probe refuses private or local
targets, follows no redirects, and never receives a private key.

The corresponding Terraform declaration is under
[`infra/terraform/digitalocean`](../infra/terraform/digitalocean/README.md).

## Publish DNS and verify

DigitalOcean then manages the Global Load Balancer's A/AAAA records and TLS
certificate without moving the website or mail records. Wait for delegation,
domain validation, and managed TLS to become active. Then run:

```bash
python3 scripts/check_rpc.py \
  https://rpc.mfenx.com \
  --expected-chain-id 177155 \
  --require-cors
```

Update ChainList only after this probe passes from an independent network.
The public display name is **LAX MFENX RPC**. Publish
`https://rpc.mfenx.com` in ChainList. Do not advertise an alternate hostname
until its DNS, managed TLS, health check, and publication probe all pass.

## Protocol limitation

The native chain currently uses a deterministic leader for each height and
does not yet implement a view-change protocol. If the expected leader is
offline, finalization pauses until that validator returns. Multi-zone
deployment, automated restart, and alerts reduce recovery time, but they do not
replace protocol-level leader failover. View change is the next required
protocol milestone before claiming continuous Byzantine liveness.
