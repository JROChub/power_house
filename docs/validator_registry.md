# Signed Validator Registry

Release scope: Power House v0.3.11.

The validator registry replaces hardcoded monitoring totals with signed,
policy-admitted, live identity checks. It controls monitoring discovery and
public health reporting. It does not rewrite an existing chain's persisted
consensus set or quorum.

## Security Model

A validator registration contains:

- chain ID
- node ID, operator, and region
- Ed25519 public key
- libp2p peer ID and address
- validator and system metrics endpoints
- issue and expiration times
- an Ed25519 signature from the validator identity

Verification requires all of the following:

1. The registration schema and chain ID are correct.
2. The peer ID is derived from the registered Ed25519 public key.
3. The libp2p address ends in that peer ID.
4. The signature covers every registration field.
5. The registration has not expired.
6. The public key appears in the active validator policy.
7. Node ID, peer ID, public key, and metrics endpoint are unique.
8. The live metrics endpoint reports the same node ID, peer ID, public key,
   and chain ID.

An ordinary connected peer cannot increase the validator count. A signed but
unadmitted identity cannot increase it either.

## Fast Registration

Run this on a protected operator machine with the validator key:

```bash
julian validator-registry register \
  --node-id validator-4 \
  --operator "Example Operator" \
  --region fra1 \
  --public-host validator-4.example \
  --metrics-port 9100 \
  --system-metrics-port 9101 \
  --output validator-4.registration.json
```

The command signs locally with `$HOME/.powerhouse/node.key` by default. Use
`--key /etc/powerhouse/validator-4.key` when the validator identity is stored
elsewhere. The output contains no private key.

Maintainers can refresh an existing assembled registry in one command after the
validator identity has been admitted by policy:

```bash
julian validator-registry register \
  --key /etc/powerhouse/validator-4.key \
  --node-id validator-4 \
  --operator "Example Operator" \
  --region fra1 \
  --public-host validator-4.example \
  --metrics-port 9100 \
  --system-metrics-port 9101 \
  --policy /etc/powerhouse/native-validators.json \
  --registry /etc/powerhouse/validator-registry.json \
  --registry-output /etc/powerhouse/validator-registry.json
```

The write is refused unless the full registry verifies against the allowlist.

## Manual Registration

The lower-level command remains available when p2p and metrics endpoints must
be specified directly:

```bash
julian validator-registry create \
  --key /etc/powerhouse/validator-4.key \
  --node-id validator-4 \
  --operator "Example Operator" \
  --region fra1 \
  --p2p-address /dns4/validator-4.example/tcp/7001/p2p/<peer-id> \
  --metrics-url http://validator-4.example:9100/metrics \
  --system-metrics-url http://validator-4.example:9101/metrics \
  --output validator-4.registration.json
```

Keep metrics endpoints restricted to the monitoring network. If a private
monitoring address is required, use the same host in both the p2p address and
the metrics URL so endpoint identity binding remains deterministic.

## Assemble And Verify

```bash
julian validator-registry assemble \
  --registration validator-1.registration.json \
  --registration validator-2.registration.json \
  --registration validator-3.registration.json \
  --policy /etc/powerhouse/native-validators.json \
  --output /etc/powerhouse/validator-registry.json

julian validator-registry verify \
  /etc/powerhouse/validator-registry.json \
  --policy /etc/powerhouse/native-validators.json
```

The production cluster generator performs these steps automatically.

## Reconciliation

`powerhouse-validator-registry.timer` runs every 15 seconds. Its reconciler:

- verifies every signature and policy admission before changing discovery
- probes validators concurrently
- checks the live identity metric against the signed registration
- writes Prometheus file-discovery and public status state atomically
- preserves the last known-good discovery files if registry verification fails

The public API reports dynamic `validators_healthy`, `validators_total`,
`peer_connections`, `validator_peer_links`, and `validator_registry` fields.
`peer_connections` is retained as the legacy validator mesh link observation
count. It is not used as validator membership and does not include public
observers. Public observer growth is reported through the separate
`observer_peers`, `observer_registry`, and `public_peer_connections` fields.
See [Public Observer Registry](observer_registry.md).

## Adding A Validator

Validator admission is a controlled protocol operation:

1. Approve the validator identity through the network membership process.
2. Apply the compatible consensus-set transition to every validator.
3. Obtain and verify the new validator's signed registration.
4. Assemble and deploy the updated registry.
5. Confirm live identity and health before advertising the new total.

Do not add an identity only to the monitoring registry and call it a consensus
validator. Current finalized state must remain compatible with the admitted
validator set.
