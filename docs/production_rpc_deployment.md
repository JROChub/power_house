# Production RPC Deployment

The retired VPS hosts should not be restored. Replace them with a reproducible
three-validator deployment whose membership, keys, genesis balances, service
configuration, and public edge are independently verifiable.

## Target architecture

- Three validators in separate failure zones.
- Static validator membership with quorum `2`.
- Private validator traffic on TCP `7001`.
- Finalized RPC on TCP `8545`, reachable only from the HTTPS load balancer.
- A global static IP and managed TLS certificate for `rpc.mfenx.com`.
- Scheduled disk snapshots plus the application-level state backups.
- External probes comparing finalized height, block hash, and state root.

Do not expose TCP `8545`, metrics, blob storage, or SSH directly to the
internet. Permit SSH through the provider's authenticated access path and
permit RPC only from load-balancer and health-check source ranges.

## Generate the sealed cluster bundle

Build a network-enabled release binary:

```bash
cargo build --release --locked --features net --bin julian
```

Generate three identities and deterministic configuration. The host values are
the validators' private addresses:

```bash
scripts/generate_rpc_cluster.py \
  --output deployment/generated/mfenx-production \
  --host 10.42.0.11 \
  --host 10.42.0.12 \
  --host 10.42.0.13 \
  --fund 0xYOUR_GENESIS_ADDRESS:1000000
```

The output directory contains consensus private keys. It is intentionally
created with restrictive permissions and must be encrypted, backed up, and
kept outside source control.

## GCP readiness gate

The repository defaults to project `mfenx-485623`:

```bash
scripts/gcp_rpc_preflight.sh
```

The gate refuses to continue if billing or required APIs are unavailable. It
does not create billable resources.

Provision a dedicated VPC, three VMs in separate zones, persistent disks,
restricted firewall rules, a health-checked external HTTPS load balancer,
managed certificate, global address, monitoring, and snapshot schedules.
Record the validators' private addresses in the cluster bundle.

## Install the cluster

After the hosts exist and SSH access works:

```bash
scripts/deploy_rpc_cluster.sh \
  deployment/generated/mfenx-production \
  validator-1-ssh \
  validator-2-ssh \
  validator-3-ssh
```

The deployer installs the same binary and policy on all validators, copies only
the matching private key to each node, refuses to replace a conflicting genesis
registry, and preserves existing finalized chain state during upgrades.

## Publish DNS and verify

Point the Namecheap `A` record for `rpc.mfenx.com` to the load balancer's global
IP. Wait for the managed certificate to become active, then run:

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
