# MFENX Local Supercomputer V2 Validation Record

This directory stages the public evidence package for the signed MFENX
Validation Record. The publication tree contains claim-routed evidence inputs,
a compact browser summary, a canonical release index, and a local
prepublication byte-verification record. Large raw run trees remain in the
source evidence archive rather than being duplicated here.

`release-index.json` and
`prepublication-input-retrieval-verification.json` are prepublication inputs to
the signed Validation Record. They state that the record is not yet published,
that live retrieval is not claimed, and that a separate post-deployment HTTPS
retrieval attestation is required.

Run the staging generator against the exact publication-ready finalization
input:

```sh
python3 lightsout/validation-record/prepare-publication.py \
  --finalization-input ../release/mfenx-local-v2-validation-record-20260822-a1/FINALIZATION-INPUT.canonical.json \
  --prepared-at 2026-08-22T16:15:00Z
```

The generator refuses to overwrite a frozen release index, prepublication
verification record, or web summary. After the evidence, signed Validation
Record, and detached signature are live, run `audit-live-publication.py` with
their exact SHA-256 identities. That audit refuses redirects and changed bytes
and creates the first record allowed to state that live HTTPS retrieval
occurred.

The published retrieval audit completed at `2026-08-22T16:47:14Z`. Its 68
unique, no-redirect HTTPS checks all passed, and the canonical attestation is
bound by SHA-256
`8985eb09d197a8a7e2593d4bddd9c27af1b1c51a09d3505c0a86aba74f869dbe`.
