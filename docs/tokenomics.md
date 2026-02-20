# Tokenomics (Phase 3)

This document defines fee flow and reward splitting for Power-House anchors.

## Roles
- **Submitter**: sends blobs to `/submit_blob`.
- **Operator**: runs the node that accepts the blob.
- **Attestors**: sign attestations that contribute to QC quorum.

## Fee Flow
1. Submitter sets headers on `/submit_blob`:
   - `x-fee`: fee amount (u64)
   - `x-publisher`: base64 ed25519 public key
   - `x-publisher-sig`: signature over `share_root`
2. Node debits the submitter’s balance in `stake_registry.json`.
3. Operator receives `operator_reward_bps` share.
4. Remaining share is split across attestors by stake weight.

## Configuration
- `blob_policy.json` controls per-namespace fee policy.
- Set `operator_reward_bps` per namespace.
- `stake_registry.json` must contain all operator + attestor keys with balances.

Example namespace policies:
```json
{
  "namespaces": {
    "default": {
      "min_fee": 0,
      "operator_reward_bps": 2000
    },
    "paid": {
      "min_fee": 1,
      "operator_reward_bps": 2000
    }
  }
}
```

## Registry Operations
```
julian stake show /path/to/stake_registry.json
julian stake fund /path/to/stake_registry.json <pubkey_b64> 1000
julian stake bond /path/to/stake_registry.json <pubkey_b64> 500
```

## Notes
- If `x-publisher` is omitted, the operator key is charged.
- Keep balances funded on the submitter key to avoid rejections.
