# Power-House RPC Operations

Release scope: Power House v0.3.8.

Power-House exposes a native-transfer JSON-RPC lane whose blocks and account
state are finalized by the configured validator quorum. Chain ID `177155` is
the default production identity.

The public service name is **LAX MFENX RPC**. Its canonical health-checked edge
and submitted ChainList endpoint is `https://rpc.mfenx.com`.

For the three-validator production topology, sealed configuration generator,
generic deployment command, TLS edge, and cloud readiness gate, see
[`production_rpc_deployment.md`](production_rpc_deployment.md).

## Consensus deployment

Use the same static membership policy, quorum, chain ID, and initial registry
on every validator. The quorum must be a strict majority. The first start
commits these values and the initial balances into
`native_chain_state.json`.

```bash
julian net start \
  --node-id validator-1 \
  --log-dir /var/lib/powerhouse/validator-1/logs \
  --blob-dir /var/lib/powerhouse/validator-1 \
  --listen /ip4/0.0.0.0/tcp/7001 \
  --policy /etc/powerhouse/native-validators.json \
  --quorum 2 \
  --evm-chain-id 177155 \
  --evm-rpc-listen 127.0.0.1:8545 \
  --key /etc/powerhouse/validator-1.key
```

Omit `--evm-rpc-listen` on validators that do not serve HTTP. Keep
`--evm-chain-id 177155` so they subscribe, validate, vote, and persist the
same finalized chain.

Native transfers currently support EIP-1559 type `0x02`, direct addresses,
empty calldata, and whole-token values. Contract creation and contract calls
return an explicit unsupported-operation error. Native transfer execution is
currently fee-free, so RPC gas price and effective gas price are zero.
`eth_sendRawTransaction` confirms mempool acceptance;
`eth_getTransactionReceipt` remains `null` until the block has a valid quorum
certificate.

## Genesis and recovery

Fund `stake_registry.json` before the first native-chain start. After
`native_chain_state.json` exists, that file is authoritative for RPC balances.
Do not independently delete or regenerate it on one replica.

At startup each node:

- verifies the genesis commitment
- replays every signed transaction
- verifies proposer and quorum signatures
- recomputes every state root
- rejects validator, quorum, chain ID, sequence, or account-state mismatch

Back up `native_chain_state.json` with the node identity and policy. Restore the
same finalized file to a replacement replica before exposing its RPC.

## Replica test

The repository includes a three-process transaction test:

```bash
scripts/test_native_rpc_cluster.sh
```

It submits a signed transfer with two validators, starts a third replica after
finality, and requires live catch-up plus identical block hash, state root,
balances, and successful receipt.

## Publication gate

Run the repository probe from an independent machine:

```bash
python3 scripts/check_rpc.py \
  https://rpc.example.org \
  --expected-chain-id 177155 \
  --require-cors
```

The command fails on:

- DNS, TCP, TLS, HTTP, or JSON decoding errors
- JSON-RPC error responses or mismatched request IDs
- chain ID disagreement between `eth_chainId` and `net_version`
- malformed or inconsistent latest-block metadata
- missing browser CORS headers when `--require-cors` is set

Run the probe after every RPC deployment and continuously from external
monitoring. Do not update ChainList until it passes.

Place TLS and request controls in a reverse proxy in front of
`127.0.0.1:8545`. Monitor `/healthz`, finalized height, finalized hash, process
restarts, disk durability, and agreement between at least two RPC replicas.
Prometheus exports `native_transactions_accepted_total`,
`native_blocks_finalized_total`, and `native_sync_blocks_applied_total`.
It also exports `powerhouse_connected_peers`, which must remain above zero on
every production validator.

Each validator also exports `powerhouse_node_identity`, binding its node ID,
libp2p peer ID, Ed25519 public key, and chain ID to the live metrics endpoint.
The signed validator registry reconciler checks that metric every 15 seconds
and supplies dynamic Prometheus targets. Public validator totals come from
fresh, policy-admitted, identity-verified registry state rather than peer
connection counts. See [Signed Validator Registry](validator_registry.md).

## Incident response

If the probe fails, remove the endpoint from public discovery or return a clear
maintenance response. Preserve node and reverse-proxy logs, compare finalized
block hashes and state roots across replicas, and restore service only after
DNS, TLS, chain ID, latest block, and replica state agree.
