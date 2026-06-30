# Public Observer Registry

Release scope: Power House v0.3.18.

The public observer registry is the permissionless monitoring layer for nodes
that verify or relay public network data without joining validator quorum. It
lets mfenx.com show public observers independently from the signed validator
mesh.

An observer can increase the public observer count only after all of these
checks pass:

1. The observer submits a signed observer registration.
2. The registration signature verifies against the advertised public key.
3. The public key derives the advertised libp2p peer ID.
4. The p2p address ends in that peer ID.
5. The live metrics endpoint reports the same node ID, peer ID, public key, and
   chain ID.
6. The live metrics endpoint reports a non-negative integer
   `powerhouse_connected_peers` value.

Observer admission never changes validator membership, validator quorum,
finality, or local verification outcomes.

## Public Status Fields

`https://rpc.mfenx.com/network-status.json` separates consensus and public
observer telemetry:

- `peer_connections`: legacy validator mesh link observations.
- `validator_peer_links`: explicit validator mesh link observations.
- `public_peer_connections`: observer-reported connection observations.
- `observer_peers`: observer total, healthy count, connection count, and
  freshness.
- `observer_registry`: signed observer registry verification state.

The website and status page display validator links and public observers as
separate values.

## Fast Registration

Run this on the observer operator machine. Do not share the private key. The
guided command creates the default key if needed, writes the signed observer
registration, prints the node start command, and runs the doctor checks.

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

The command signs locally with `$HOME/.powerhouse/node.key` by default. The
private key is never written into the registration file. Use
`--key <path-or-key-spec>` when the node identity lives somewhere else.

Then start the observer with the command printed by setup. The common shape is:

```bash
julian net start \
  --node-id mynode \
  --log-dir ./logs/mynode-observer \
  --blob-dir ./data/mynode-observer \
  --listen /ip4/0.0.0.0/tcp/7001 \
  --bootstrap /ip4/159.203.109.128/tcp/7002/p2p/12D3KooWMCyR9gXPXCGAMNCVJDKbisohRRq8oaTHNiR91HZ67cSR \
  --bootstrap /ip4/64.23.182.213/tcp/7002/p2p/12D3KooWGEHbPAQ9ZVB9Uqg1j8CnsNqKvS2xmAe5cmT4w3idUtmQ \
  --bootstrap /ip4/164.92.150.22/tcp/7002/p2p/12D3KooWFNv4sZfDKypMeWqRetghHxXzkhPTc4PvynDZKSETJqd8 \
  --key "$HOME/.powerhouse/node.key" \
  --metrics 0.0.0.0:9102
```

The default public observer bootnodes listen on TCP `7002`. They are separate
from validator TCP `7001`; validator transport remains restricted to the
validator mesh and is not opened to arbitrary public peers.

After the observer is running, use the doctor. It checks the local key, local
ports, local metrics identity, and the production-side public reachability
probe.

```bash
julian observer doctor \
  --node-id mynode \
  --public-host <public-ip-or-dns> \
  --p2p-port 7001 \
  --metrics-port 9102
```

If the doctor reports a reachability failure, forward these ports from the
router or cloud firewall to the machine running the observer:

```text
TCP 7001 -> observer machine TCP 7001
TCP 9102 -> observer machine TCP 9102
```

`127.0.0.1`, `10.x.x.x`, `172.16-31.x.x`, and `192.168.x.x` addresses are
local/private addresses. They can work on the operator machine, but they cannot
be used for public registry admission.

The signed JSON can be submitted directly at `https://mfenx.com/register.html`.
The page sends only the signed public registration. It never asks for or reads
the private key.

The CLI performs the same direct submission and prints a tracking ID:

```bash
julian observer submit mynode.observer.registration.json
julian observer status <tracking-id>
```

Use `--no-upload` to validate and package the registration without submitting
it. Use `--intake-url` with either command when testing a non-production intake
service.

## Admission API

The production edge exposes:

```text
POST /observer-registrations
GET  /observer-registrations/<tracking-id>
POST /observer-registrations/<tracking-id>/retry
```

POST accepts either a signed `power-house-observer-registration-v1` object or a
package containing that object under `registration`. Successful intake returns
HTTP 202 and a random tracking ID. Exact duplicate submissions are idempotent:
they return the original tracking ID without creating another queue entry.

Admission states are:

- `queued`: persisted and waiting for verification.
- `verifying`: Rust signature, identity, timing, and endpoint checks are active.
- `approved`: all checks passed and registry promotion is pending.
- `promoted`: the verified canonical observer registry was atomically replaced.
- `rejected`: a permanent or retryable check failed.

The retry endpoint accepts only submissions marked retryable, such as temporary
metrics or p2p reachability failures. A registration at or below the last
admitted `issued_at_unix` for that peer is rejected as a replay. Node ID, peer
ID, public key, and metrics URL uniqueness are re-verified against the complete
candidate registry before promotion.

The intake service runs as the unprivileged `powerhouse-intake` account. Its
only writable production path is `/var/lib/powerhouse/observer-intake`. It has
no access to validator keys and never writes validator membership, genesis, or
quorum configuration.

## Manual Registration

The lower-level command remains available when p2p and metrics endpoints must
be specified directly:

```bash
julian key-info "$HOME/.powerhouse/node.key" --json

julian observer-registry create \
  --key "$HOME/.powerhouse/node.key" \
  --node-id external-observer-1 \
  --operator "Operator Name" \
  --region <region> \
  --p2p-address /dns4/<host>/tcp/7001/p2p/<peer-id> \
  --metrics-url http://<host>:9102/metrics \
  --output observer.registration.json
```

The metrics host must match the p2p host, and the metrics endpoint must expose:

```text
powerhouse_node_identity{node_id="external-observer-1",peer_id="<peer-id>",public_key_b64="<public-key>",chain_id="177155"} 1
powerhouse_connected_peers <integer>
```

## Manual Recovery And Verification

The automated intake is the primary workflow. Manual assembly remains a
recovery tool for maintainers:

```bash
julian observer-registry assemble \
  --registration observer-1.registration.json \
  --registration observer-2.registration.json \
  --output /etc/powerhouse/observer-registry.json

julian observer-registry verify \
  /etc/powerhouse/observer-registry.json \
  --json
```

For maintainers assembling a registry from an existing file and one refreshed
registration, the shortcut can write the assembled registry in one step:

```bash
julian observer-registry register \
  --node-id external-observer-1 \
  --public-host <host> \
  --registry /etc/powerhouse/observer-registry.json \
  --registry-output /etc/powerhouse/observer-registry.json
```

The primary monitoring node stores the canonical registry at
`/var/lib/powerhouse/observer-intake/observer-registry.json`. Secondary RPC
nodes fetch it over the tag-restricted internal intake port, verify it with
`julian observer-registry verify`, and atomically replace their local
`/etc/powerhouse/observer-registry.json`. The timer runs every 15 seconds. A
failed fetch or verification never replaces the last valid local registry.

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

Protect metrics with firewall rules or an authenticated reverse proxy when the
endpoint is public. The registry verifies identity, not operator intent or host
security.

## Operational Rule

Do not open production validator transport to arbitrary peers just to raise a
public count. Use the observer registry or a dedicated bootnode layer so public
growth remains separate from consensus membership.
