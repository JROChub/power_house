# Memory CLI

Status: active CLI guide for Power House v0.3.15.

The `julian memory` commands operate offline by default.

```bash
julian memory create --pha main.pha --rootprint proof.rootprint.json --output capsule.phm
julian memory verify capsule.phm --policy strict --report verify.json
julian memory replay capsule.phm --report replay.json
julian memory challenge capsule.phm --all --report challenge.json
julian memory inspect capsule.phm --summary
julian memory explain-boundary capsule.phm
julian memory export capsule.phm --format directory --output capsule/
```

Verification output separates layers:

```text
CORE        VALID
ROOTPRINT   VALID
REPLAY      VALID
SIDECAR     VALID
SEMANTIC    VALID
```

Failure output reports where the falsehood died:

```text
rejection:
  layer: semantic
  code: PACKET_DIGEST_MISMATCH
  core_unchanged: true
```
