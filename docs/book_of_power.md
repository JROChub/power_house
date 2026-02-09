Power-House Protocol Manual
===========================
Version: v0.1.54
Crate: power_house v0.1.54
Scope: deterministic proof ledger, DA commitments, and MFENX Power-House Network operations.

Table of Contents
-----------------
1. Canonical digests and reference commands
2. Transcript hashing specification
3. Encoding rules
4. Domain tags
5. Ledger logs and anchors
6. Deterministic randomness
7. Data availability commitments and evidence
8. Network operations (MFENX Power-House Network)
9. Troubleshooting
10. Glossary

1. Canonical digests and reference commands
-------------------------------------------
Reference digests (v0.1.54):
- Genesis digest: 139f1985df5b36dae23fa509fb53a006ba58e28e6dbb41d6d71cc1e91a82d84a
- Dense polynomial digest: ded75c45b3b7eedd37041aae79713d7382e000eb4d83fab5f6aca6ca4d276e8c
- Hash anchor proof digest: c72413466b2f76f1471f2e7160dadcbf912a4f8bc80ef1f2ffdb54ecb2bb2114
- Fold digest (hash_pipeline): c87282dddb8d85a8b09a9669a1b2d97b30251c05b80eae2671271c432698aabe

Reference commands:
- `cargo run --example hash_pipeline` must emit the fold digest above and a reduced field value of 219.
- Example logs are staged under `/tmp/power_house_anchor_a` and `/tmp/power_house_anchor_b`.
  On hosts without `/tmp`, set `POWER_HOUSE_TMP=/path/to/workdir`.
- `julian node anchor /tmp/power_house_anchor_a` should print `MFENX Power-House Network` lines including the genesis digest.

Keep the fold digest with exported anchors (comment or `anchor_meta.json`).

2. Transcript hashing specification
-----------------------------------
Binary framing specification (BLAKE2b-256, domain-separated):

```
transcript_bytes = concat(u64::to_be_bytes(challenge_i) for each entry in transcript)
round_sum_bytes  = concat(u64::to_be_bytes(sum_i) for each entry in round_sums)
hasher = BLAKE2b-256()
hasher.update(b"MFENX_TRANSCRIPT")
hasher.update(len(transcript_bytes) as u64_be)
hasher.update(transcript_bytes)
hasher.update(len(round_sum_bytes) as u64_be)
hasher.update(round_sum_bytes)
hasher.update(final_value.to_be_bytes())
digest = hasher.finalize()
```

- `statement:` text, comments, and the `hash:` line do not participate in the digest.
- Transcript numbers are encoded as u64 big-endian before hashing.
- The canonical digest is 32 raw bytes rendered as 64 lowercase hex characters.

3. Encoding rules
-----------------
- Transcript text is decimal ASCII tokens (e.g., `round_sums: 209 235`).
- Hash inputs are serialized as u64 big-endian bytes.
- Hex digests are 64 lowercase `[0-9a-f]` chars with no spaces.
- Line endings are LF only.

4. Domain tags
--------------
- `MFENX_TRANSCRIPT` — transcript hashing
- `MFENX_ANCHOR` — ledger fold digest
- `MFENX_CHALLENGE` — Fiat–Shamir challenge derivation
- `MFENX_MERKLE` — Merkle root hashing

5. Ledger logs and anchors
--------------------------
- Ledger logs are `ledger_0000.txt`, `ledger_0001.txt`, etc.
- Each entry contains a lowercase `hash:` line (32 bytes rendered as hex).
- `LedgerAnchor::anchor()` prepends `JULIAN::GENESIS` automatically.
- When exporting anchors, store the fold digest next to the anchor JSON.

Schema references:
- Anchor schema: `mfenx.powerhouse.anchor.v1` (see README)
- Envelope schema: `mfenx.powerhouse.envelope.v1` (see README)

6. Deterministic randomness
---------------------------
- Challenge derivation uses BLAKE2b-256 with `MFENX_CHALLENGE` domain tag.
- `simple_prng` is deprecated; `prng.rs` implements the deterministic stream.
- Current derivation uses `next_u64() % p`. For small primes, use the documented rejection sampler.

7. Data availability commitments and evidence
----------------------------------------------
- Every blob carries both `share_root` and `pedersen_root`.
- `/submit_blob`, `/commitment/<ns>/<hash>`, `/sample/<ns>/<hash>` expose `pedersen_root` and proofs.
- Anchors are gated on DA commitments and stake-weighted QC.
- Evidence is appended to `evidence_outbox.jsonl` and used for slashing/reconciliation.
- Rollup settlement failures emit `RollupFaultEvidence` and are stored in the same outbox.

8. Network operations (MFENX Power-House Network)
--------------------------------
- Local smoke test: `./scripts/smoke_net.sh` (confirms broadcast + finality under DA gating).
- Metrics (if enabled): `curl http://<host>:9100`
- Blob health (requires auth token if enabled):
  `curl -H 'Authorization: Bearer <token>' http://<host>:8181/healthz`
- Log format (journal):
  - `QSYS|mod=ANCHOR|evt=STANDBY`
  - `QSYS|mod=ANCHOR|evt=BROADCAST`
  - `QSYS|mod=QUORUM|evt=FINALIZED`

9. Troubleshooting
------------------
- Missing logs: verify `--log-dir` exists and is writable.
- DA quorum failures: confirm blobs exist and QC files are written under the blob dir.
- Rate-limit errors: check `/etc/powerhouse/blob_policy.json` (`max_per_min`).
- Auth failures: verify `--blob-auth-token` and request headers.
- Anchor divergence: verify the same log files and deterministic keys on each node.

10. Glossary
------------
- Anchor: ordered list of statements plus transcript digests, optionally with fold metadata.
- Transcript: ASCII record of statement/challenges/round sums/final value/hash for a single proof.
- Fold digest: BLAKE2b-256 hash across transcript digests, used as quorum hinge.
- Quorum: minimum count of matching anchors needed for finality.
- Evidence outbox: durable stream of signed fault evidence for audit and slashing.
