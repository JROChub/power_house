# External hostile-campaign terminal result v1

Schema: `mfenx/ckodmk-external-campaign-result/v1`

This profile carries the terminal, per-case output of one campaign conforming to
`external-campaign-preregistration-v1.md`. It is bounded strict JSON. The
consumer rejects duplicate or unknown fields, floating-point JSON tokens,
noncanonical digests, missing cases, duplicate identifiers, reordered selected
indices, unregistered groups/subtypes, target shortfalls, attempt-ceiling
overruns, omitted baselines, and budget-profile drift.

Every selected attempt remains a case record, including invalid, unsupported,
and excluded mutations. Every effective fault is classified from the
evaluator-owned oracle. CKODMK and all preregistered baselines carry a decision,
positive elapsed nanoseconds, common budget-profile digest, and evidence digest.
The consumer recomputes effective-fault, false-pass, safe-non-pass, control, and
per-baseline decision counts from those records.

The seed commitment is:

```text
SHA256(ASCII "mfenx/ckodmk-external-campaign-seed/v1", NUL, 32 seed bytes)
```

The preregistration contains only that commitment. The terminal result reveals
the lowercase seed only after a separate terminal result manifest is pinned,
and binds that manifest digest. A rerun needs a new preregistration and seed.

Verification requires caller-pinned digests for both inputs:

```bash
python scripts/verify_external_campaign_result.py \
  terminal-result.json \
  --sha256 sha256:<terminal-result-digest> \
  --preregistration preregistration.json \
  --preregistration-sha256 sha256:<signed-preregistration-digest>
```

`campaign_pass: true` means the record reaches the profile's numeric fault and
control targets with zero retained false passes, crashes, or timeouts and all
controls passing. It does not authenticate the evaluator, validate the
scientific representativeness of the generator, open the referenced evidence
objects, or replace the separately signed external-evaluation return and MFENX
review.
