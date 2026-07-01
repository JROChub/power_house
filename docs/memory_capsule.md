# Memory Capsule v1

Status: active format guide for Power House v0.3.20.

Memory Capsules are portable proof-memory objects. A `.phm` file packages a
Power House `.pha` artifact, Rootprint lineage, replay expectations, optional
Observatory sidecars, optional opaque semantic packets, witness receipts,
challenge vectors, and reproduction metadata.

The schema is:

```text
power-house/memory-capsule/v1
```

Recommended extension:

```text
.phm
```

Recommended MIME type:

```text
application/vnd.powerhouse.memory+json
```

## Verification Order

`julian memory verify` uses this order:

1. Parse strict UTF-8 JSON.
2. Reject duplicate keys and floating-point numbers.
3. Validate schema and critical extensions.
4. Recompute the capsule digest.
5. Verify the core `.pha` artifact.
6. Verify the core digest.
7. Verify Rootprint.
8. Replay Rootprint state.
9. Compare the replay fingerprint.
10. Verify the sidecar when present.
11. Verify semantic packet transport digests and branch bindings.
12. Verify witness receipts against observed digests.

Core verification always runs before semantic verification. Semantic data can
explain core truth, but it cannot alter `.pha` fingerprints, Rootprint branch
IDs, replay, or proof validity.

## CLI

Create:

```bash
julian memory create \
  --pha main.pha \
  --rootprint proof.rootprint.json \
  --sidecar proof.observatory.json \
  --output earth-001.phm
```

Verify:

```bash
julian memory verify earth-001.phm --report verify.json
```

Replay:

```bash
julian memory replay earth-001.phm --report replay.json
```

Challenge:

```bash
julian memory challenge earth-001.phm --all --report challenge.json
```

Export:

```bash
julian memory export earth-001.phm --format directory --output earth-001/
```

## Rejection Traces

Failures report the layer and stable code. Example:

```json
{
  "status": "rejected",
  "layer": "semantic",
  "code": "PACKET_DIGEST_MISMATCH",
  "core_valid_before_failure": true,
  "rootprint_valid_before_failure": true,
  "semantic_can_affect_core": false
}
```

This is intentional. A semantic mutation must be visible as a semantic failure,
not confused with a core proof failure.

## Current Challenge Suite

`ChallengeSuite::standard()` currently covers:

- capsule digest mutation,
- unsupported schema,
- core digest mutation,
- `.pha` core fingerprint mutation,
- Rootprint root mutation,
- replay fingerprint mutation,
- sidecar digest mutation,
- semantic packet digest mutation,
- semantic branch rebinding,
- semantic replay rebinding.

The suite mutates copies only. Source capsules are never modified in place.
