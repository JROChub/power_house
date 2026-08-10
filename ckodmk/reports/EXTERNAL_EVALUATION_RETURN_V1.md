# CKODMK external-evaluation return profile v1

Schema identifier: `mfenx/ckodmk-external-evaluation-return/v1`

This profile carries a qualified evaluator's signed assertions about one frozen
CKODMK release. It does not embed the raw evidence package. Every referenced
record is bound by SHA-256 and must be retained separately.

## Transport and authentication

The report is strict UTF-8 JSON with no duplicate keys, non-finite constants,
or JSON floating-point tokens. Counts and scores are integers. Digests use
`sha256:` followed by 64 lowercase hexadecimal characters. Text is bounded
ASCII. Unknown fields are rejected.

The evaluator signs the exact report bytes with OpenSSH `sshsig`, namespace
`ckodmk-external-evaluation`. The consumer receives through separate trusted
channels:

- the expected report SHA-256;
- an OpenSSH `allowed_signers` file;
- the exact expected evaluator identity used in that file.

The CKODMK verifier freezes all three files as bounded regular files, validates
the report, then delegates signature and trusted-identity verification to
`ssh-keygen -Y verify`. A bundled key, self-declared key, unsigned report, or
matching hash alone never establishes evaluator identity.

Example signing command:

```bash
ssh-keygen -Y sign \
  -f /secure/evaluator_ed25519 \
  -n ckodmk-external-evaluation \
  external-evaluation.json
```

Example verification command:

```bash
python scripts/verify_external_evaluation.py \
  external-evaluation.json \
  --sha256 sha256:<digest-obtained-separately> \
  --signature external-evaluation.json.sig \
  --allowed-signers /secure/ckodmk-evaluators.allowed_signers \
  --evaluator evaluator@example.org
```

## Evidence fields

The exact field grammar is enforced by the reference consumer and illustrated
by `specs/golden/external-evaluation-pending.json`. The report binds:

- evaluator identity, qualifications, conflicts, and non-author control;
- source commit, release archive, CI run, environment, commands, and logs;
- checker-independence and specification-agreement audit results;
- the complete non-author hostile campaign denominator and raw-evidence hash;
- physical-target profiles and measurement packages;
- matched baselines and common-fault-set records;
- third-party reproduction results;
- consequential external release-use records;
- every critical/high finding and its status;
- the evaluator's score, hard-gate count, and qualification assertion.

An incomplete evaluation uses `NOT_EVALUATED` and empty arrays. It is valid data
but cannot become qualification evidence. The target denominator is exhausted
exactly by safe non-passes, false passes, inconclusive results, crashes, and
timeouts. A separate control denominator is exhausted exactly by passes, false
blocks, inconclusive results, crashes, and timeouts. Invalid mutations and
exclusions remain in the selected denominator rather than disappearing.

For a zero-false-pass campaign, the consumer recomputes the one-sided exact 95%
upper bound `1 - 0.05^(1/n)` and requires the evaluator's integer parts-per-
million ceiling to match. No bound is accepted when the effective denominator
is zero or false passes are nonzero.

## Consumer result boundary

Successful verification means:

1. the report bytes match the caller's digest;
2. the report matches this strict structural and arithmetic profile; and
3. OpenSSH validates the detached signature for the caller-trusted evaluator.

It authenticates an evaluator assertion. It does not rerun models, inspect the
referenced raw packages, validate evaluator qualifications, prove the device
measurements, establish customer authority, or independently assign CKODMK's
rubric score. The consumer therefore reports both the signed assertion and a
mechanical `minimum_profile_satisfied` value; neither may be presented as a
completed MFENX review until the referenced evidence is inspected.
