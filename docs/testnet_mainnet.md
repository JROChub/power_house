# MFENX Testnet To Mainnet Process

Release scope: Power House v0.3.6.

Chain metadata currently identifies chain `177155` as active. Future mainnet
launches or incompatible resets must use this process rather than silently
replacing state behind an existing chain ID.

## Entry Criteria

- three-region infrastructure passes a seven-day soak
- public RPC achieves at least 99.9% measured availability
- load and failover limits are published
- no unresolved critical security finding
- validator policy, genesis state, and binaries are reproducible
- at least two validators are independently administered
- rollback and incident drills have passed

## Launch Freeze

Freeze the candidate commit, Cargo version, container digest, chain metadata,
validator policy, genesis commitment, and operator runbook. The release gate
must pass before tags or images are published.

## Migration

Publish UTC cutover time, old and new endpoints, snapshot hash, account-state
reconciliation, validator activation order, and rollback deadline. A chain
reset requires a new chain ID unless preserving the old history is provably
equivalent.

## Exit Criteria

Mainnet status requires 72 hours of stable finality after cutover, matching
state across quorum, external RPC verification, public status reporting, and
operator sign-off. Marketing language does not substitute for these checks.
