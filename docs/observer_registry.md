# Public Observer Registry

Release scope: Power House v0.3.7.

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

Run this on the observer operator machine. Do not share the private key.

```bash
julian observer-registry register \
  --node-id external-observer-1 \
  --operator "Operator Name" \
  --region <region> \
  --public-host <host> \
  --metrics-port 9100 \
  --output observer.registration.json
```

The command signs locally with `$HOME/.powerhouse/node.key` by default. The
private key is never written into the registration file. Use
`--key <path-or-key-spec>` when the node identity lives somewhere else.

For a local node using a custom metrics port:

```bash
julian observer-registry register \
  --node-id mynode \
  --public-host <public-ip-or-dns> \
  --metrics-port 9102 \
  --output mynode.observer.registration.json
```

The signed JSON can be checked on `https://mfenx.com/register.html` before it
is submitted for public observer registry assembly.

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
  --metrics-url http://<host>:9100/metrics \
  --output observer.registration.json
```

The metrics host must match the p2p host, and the metrics endpoint must expose:

```text
powerhouse_node_identity{node_id="external-observer-1",peer_id="<peer-id>",public_key_b64="<public-key>",chain_id="177155"} 1
powerhouse_connected_peers <integer>
```

## Assemble And Verify

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

The monitoring stack runs `powerhouse-observer-registry.timer` every 15
seconds. If `/etc/powerhouse/observer-registry.json` is absent, the public API
reports the observer layer as not configured without degrading RPC status.

## Start An Observer

```bash
julian net start \
  --node-id external-observer-1 \
  --log-dir "$HOME/.powerhouse/logs" \
  --blob-dir "$HOME/.powerhouse/data" \
  --listen /ip4/0.0.0.0/tcp/7001 \
  --bootstrap /dns4/<bootstrap-host>/tcp/7001/p2p/<peer-id> \
  --key "$HOME/.powerhouse/node.key" \
  --metrics 0.0.0.0:9100
```

Protect metrics with firewall rules or an authenticated reverse proxy when the
endpoint is public. The registry verifies identity, not operator intent or host
security.

## Operational Rule

Do not open production validator transport to arbitrary peers just to raise a
public count. Use the observer registry or a dedicated bootnode layer so public
growth remains separate from consensus membership.
