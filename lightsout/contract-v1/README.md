# MFENX replay-gated execution contract v1

Status: frozen Step 0/Step 1 trust package for MFENX Local v2 release
`mfenx-local-v2-20260821-a1`.

This directory publishes the execution contract, claim ledger, canonical format rules,
threat model, conformance vectors, release signature, public verification material,
dependency SBOM, post-hoc build provenance, signed local reproduction record, and the
remaining validation roadmap.

The release manifest is the signed authority. The root `SHA256SUMS` in this directory is
an unsigned transport index for the public subset; it is not a second release signature.
Every release, contract, and conformance file copied here is byte-identical to the file
bound by the signed manifest.

## Verify the release signature

From this directory, with OpenSSH `ssh-keygen` available:

```sh
ssh-keygen -Y verify \
  -f release/allowed_signers \
  -I mfenx-release \
  -n mfenx-release \
  -s release/RELEASE-MANIFEST.canonical.json.sig \
  < release/RELEASE-MANIFEST.canonical.json
```

Expected signer identity: `mfenx-release`  
Expected namespace: `mfenx-release`  
Expected Ed25519 public-key fingerprint:
`SHA256:Uhj/Ci2+3KA2JN/H8+Sl6nhAiTeD76zvajqvxLOYTTc`

Pin that fingerprint through an independently trusted channel before relying on publisher
identity. A public key obtained from the same server as a signature proves consistency
with that key, not independently authenticated ownership of the key.

Verify the bytes in this public subset with:

```sh
sha256sum -c SHA256SUMS
```

The signed manifest names the signed release artifacts, frozen source inventory, and
complete sealed-evidence closure. This web subset intentionally omits the 277 MiB evidence
bundle and two repository-layout helper scripts. Selected accepted evidence and the
executable remain under `../release/`.

## Scope

This package freezes what Contract v1 means and which claims the current evidence
supports. It does not establish reproducible builds, unrelated-machine reproduction,
1-16 lane scaling, independent security review, exhaustive mutation/kill sweeps, an
external production workload, or commercial readiness. Those remain later roadmap
gates, and failures must be published alongside successes.
