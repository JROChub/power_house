# Provenance Security Model

Status: active security statement for Power House v0.3.18.

This document defines the integrity boundary for Power House Archive (`.pha`)
v1, Rootprint v1, and optional external proof attachments.

## Protected Core

A `.pha` core fingerprint commits to:

- the schema identifier;
- provenance JSON;
- the embedded Power House protocol identifier;
- public inputs;
- the embedded Power House proof payload.

The stored fingerprint, `identity_root`, and `external_proof_attachments` are
excluded from the fingerprint projection.

Rootprint branch identity commits to:

- the branch label;
- sorted parent branch IDs;
- the carried artifact's `phx_fingerprint`.

Sequence values enforce parent-before-child ordering but do not participate in
branch identity.

## Integrity Properties

### Core mutation rejection

Changing any fingerprinted `.pha` field without refreshing the fingerprint is
rejected by `PhaArtifact::verify()`.

### Deterministic identity

Rust, Python, and browser implementations use domain-separated SHA-256 over
canonical JSON. Canonical objects use lexicographically sorted keys, UTF-8,
compact encoding, and integer-only JSON numbers.

Identity verification separately binds the artifact `identity_root`, immutable
identity envelope, and resolved Rootprint branch. This graph-context binding
avoids a circular dependency between an artifact fingerprint and the branch ID
derived from that fingerprint.

### EPA isolation

Adding, removing, reordering, or mutating EPA data does not alter:

- `phx_fingerprint`;
- Rootprint branch IDs;
- Rootprint equivalence;
- Rootprint core validity.

EPA payload integrity is checked only through an explicit attachment
verification operation.

### Observatory isolation

An Observatory sidecar commits to:

- a canonical Rootprint replay fingerprint;
- exact Rootprint branch IDs;
- opaque semantic packet objects;
- a domain-separated sidecar SHA-256 digest.

Sidecars and `slbit` packets are not stored in `.pha` core projections or
Rootprint branches. Adding, removing, or mutating semantic data cannot change
Power House verification outcomes. A mutation rejects sidecar integrity while
the unchanged Power House graph continues to verify.

### Graph mutation rejection

Rootprint verification rejects:

- a missing or malformed root;
- branch map keys that do not match branch IDs;
- invalid carried `.pha` cores;
- recalculated branch ID mismatches;
- unsorted, duplicate, missing, or excessive parents;
- parent sequence values that do not precede children;
- branches unreachable from the root.

## Cryptographic Assumptions

Identity security relies on SHA-256 collision and second-preimage resistance.
Sparse transcript and workload formats additionally use their documented
BLAKE2b-256 domains. Signature and quorum behavior in the optional network
feature uses the algorithms and policy assumptions documented in the JULIAN
protocol and operations guides.

## Verification Responsibility

`.pha` core verification establishes deterministic artifact integrity. The
meaning of an opaque protocol-specific proof remains the responsibility of the
protocol identified by `embedded_proof.protocol`.

EPA integrity verification establishes that an attachment payload matches its
declared digest. Cryptographic interpretation of an external proof system
requires a caller-supplied verifier through
`verify_external_proof_attachments_with`.

## Conformance And Mutation Tests

The Rust integration suite and Python SDK consume `conformance/pha-v1` and
`conformance/identity-v1`.
Coverage includes:

- valid core-only artifacts;
- valid artifacts carrying EPA;
- valid fork and merge graphs;
- core-field mutation rejection;
- EPA mutation isolation;
- matching Rust and Python fingerprints and branch IDs;
- matching Rust and Python identity replay state;
- identity-root mutation and resolution rejection;
- complete CLI navigation, fork, merge, and verification workflows.
- sidecar replay-binding, branch-reference, digest, and isolation behavior;
- deterministic `slbit` transcript and packet verification in the browser.

Run:

```bash
cargo test --test provenance_protocol --test rootprint_cli --test identity_cli
cargo test --test observatory_protocol --test observatory_cli
PYTHONPATH=sdk/python python3 -m unittest discover -s sdk/python/tests -v
```

The browser verifier at `mfenx.com` independently recalculates the published
Rootprint vector's core fingerprints, branch IDs, ordering, and reachability.
It separately verifies the published Observatory sidecar and `slbit` packet
digests before rendering semantic transcripts.

## Related Documents

- [Power House Archive v1](pha_spec.md)
- [Rootprint v1](rootprint.md)
- [Identity Layer](identity.md)
- [SDKs](sdk.md)
- [Verification Guide](verification_guide.md)
- [Power House + slbit Observatory](slbit.md)
- [Sparse Certificate Security Model](security_model.md)
