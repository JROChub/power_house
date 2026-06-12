# Power House Archive (`.pha`) v1

## Status

This document defines the portable Power House Archive v1 JSON format. The
schema identifier is:

```text
power-house/pha/v1
```

A `.pha` artifact has one authoritative Power House core. Optional external
proof attachments may travel with that core, but they do not participate in
Power House identity, proof validity, branching, or equivalence.

## Top-level object

```json
{
  "schema": "power-house/pha/v1",
  "provenance": {},
  "embedded_proof": {
    "protocol": "power-house/sumcheck/v1",
    "public_inputs": {},
    "proof": {}
  },
  "phx_fingerprint": "sha256:<64 lowercase hex characters>"
}
```

| Field | Required | Meaning |
|---|---:|---|
| `schema` | yes | Must equal `power-house/pha/v1`. |
| `provenance` | yes | JSON provenance committed by the core fingerprint. |
| `embedded_proof` | yes | Power House protocol identifier, public inputs, proof, and optional attachments. |
| `phx_fingerprint` | yes | Domain-separated SHA-256 identity of core fields. |

## Embedded proof

`embedded_proof` contains:

| Field | Required | Core-bound |
|---|---:|---:|
| `protocol` | yes | yes |
| `public_inputs` | yes | yes |
| `proof` | yes | yes |
| `external_proof_attachments` | no | **no** |

The Rust field is declared with:

```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
pub external_proof_attachments: Option<Vec<ExternalProofAttachment>>
```

Artifacts that do not use EPA therefore serialize exactly without the field.

## External proof attachment

```json
{
  "id": "external-proof-1",
  "proof_system": "example/external-proof/v1",
  "payload": {},
  "payload_sha256": "sha256:<64 lowercase hex characters>",
  "verifier_hint": "optional verifier name or URI",
  "metadata": {}
}
```

`verifier_hint` and `metadata` are optional and omitted when absent.
`payload_sha256` is SHA-256 over the compact canonical JSON serialization of
`payload`. Built-in attachment verification checks structure and payload
integrity. Cryptographic verification of an external proof system is performed
only by an explicitly supplied external verifier.

Canonical JSON objects use lexicographically sorted keys, UTF-8, and no
insignificant whitespace. JSON numbers in fingerprinted core fields and EPA
payloads must be signed or unsigned integers. Floating-point values are
rejected so independent implementations cannot disagree about number
serialization.

## Core fingerprint

`phx_fingerprint` is:

```text
sha256(
  "power-house:pha:v1:phx-fingerprint\0" ||
  canonical_json({
    "embedded_proof": {
      "proof": embedded_proof.proof,
      "protocol": embedded_proof.protocol,
      "public_inputs": embedded_proof.public_inputs
    },
    "provenance": provenance,
    "schema": schema
  })
)
```

The following are excluded:

- `phx_fingerprint` itself
- `embedded_proof.external_proof_attachments`
- every EPA payload, digest, verifier hint, and metadata value

Adding, removing, reordering, or mutating EPA data cannot change the core
fingerprint. Mutating a core field must change it.

## Verification modes

`PhaArtifact::verify()` validates only the Power House core schema and
fingerprint. It does not read EPA data.

`PhaArtifact::verify_external_proof_attachments()` first validates the core,
then explicitly checks EPA structure and payload integrity.

`PhaArtifact::verify_external_proof_attachments_with(...)` additionally invokes
a caller-supplied verifier for external proof semantics.

No Power House workflow is permitted to require EPA verification for core
validity.
