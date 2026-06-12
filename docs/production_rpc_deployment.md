# Production RPC Deployment

Release scope: Power House v0.3.1.

The retired VPS hosts should not be restored. Replace them with a reproducible
three-validator deployment whose membership, keys, genesis balances, service
configuration, and public edge are independently verifiable.

## Target architecture

- Three DigitalOcean Droplets in separate regions.
- Static validator membership with quorum `2`.
- Authenticated libp2p validator traffic on TCP `7001`, restricted by tag.
- Finalized RPC bound to `127.0.0.1:8545` on each validator.
- Nginx on port `80`, reachable only from the Global Load Balancer.
- Global ingress and managed TLS for a delegated `rpc.mfenx.com` DNS zone.
- Weekly Droplet backups plus application-level state backups.
- External probes comparing finalized height, block hash, and state root.

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
- Global Load Balancer `mfenx-rpc-global`

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

The output directory contains consensus private keys. It is intentionally
created with restrictive permissions and must be encrypted, backed up, and
kept outside source control.

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
registry, and preserves existing finalized chain state during upgrades.

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

## Protocol limitation

The native chain currently uses a deterministic leader for each height and
does not yet implement a view-change protocol. If the expected leader is
offline, finalization pauses until that validator returns. Multi-zone
deployment, automated restart, and alerts reduce recovery time, but they do not
replace protocol-level leader failover. View change is the next required
protocol milestone before claiming continuous Byzantine liveness.
