# Power-House RPC Operations

An RPC URL is ready for public wallets only when it serves canonical state from
the same finalized network observed by every public node. A process that invents
block numbers, stores balances only on one host, or acknowledges transfers
before network finality must not be advertised as the chain RPC.

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

## Consensus requirements

A production RPC adapter must read finalized blocks, account state, and
transaction results from Power-House consensus. Every write must enter the
network transaction path, survive validation and quorum finality, and return
the same receipt from multiple RPC replicas. Contract methods must either have
a real execution engine or return a standards-compliant unsupported-method
error; fabricated success responses are not acceptable.

## Incident response

If the probe fails, remove the endpoint from public discovery or return a clear
maintenance response. Preserve node and reverse-proxy logs, compare finalized
anchor IDs across replicas, and restore service only after DNS, TLS, chain ID,
latest block, and replica state agree.
