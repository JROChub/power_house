# CKODMK deterministic generative campaign v1

## Purpose

This campaign exercises the restricted exact compiler and the modeled
assurance-composition rules at substantially greater breadth than the unit
suite. It is internal reproducible evidence. It is not a proof, an external
audit, or the non-author-controlled hostile campaign required by hard gate H5.

The default allocation is exactly 1,000,000 cases:

| Family | Cases | System under test | Oracle |
|---|---:|---|---|
| Restricted JSON parser | 250,000 | Python exact-v2 parser | Eight fixed valid/invalid grammars with a distinct nonce in every byte sequence |
| Normalization | 50,000 | Python exact-v2 compiler | Canonical second-pass idempotence and structural invariants |
| Rational equivalence | 400,000 | Compiled source/target pairs | Separate direct interpreter over serialized rational objects |
| Certificate mutation | 150,000 | Certificate reconstruction boundary | Canonical inequality for every mutation plus a bounded Rust-checker sample |
| Composition | 150,000 | Executable registered-rule predicate | Separately expressed truth-table predicate |

The allocation is fixed by the runner. A smaller `--total` value scales every
family proportionally and is a smoke test, not the retained result.

## Determinism

The seed is `0x434b4f444d4b0200`. Random values come from the SplitMix64 algorithm
implemented in the runner; they do not depend on CPython's `random` module.
The report omits wall-clock timing and local paths. It binds the selected
checker by filename, byte count, and SHA-256 only. Two runs using the same
runner, inputs, and checker bytes must produce identical report bytes.

Every parser byte sequence and every certificate mutation is distinct within
the retained default run. Generated model/point combinations are derived from
the specified stream. Coverage is reported by mutation family, statement
scope, reduction outcome, composition relation pair, and composition verdict.

## Oracle separation

The exact producer remains untrusted. The equivalence oracle in
`scripts/run_generative_campaign.py` directly interprets the serialized model
equations with `fractions.Fraction`; it does not import CKODMK validation or
evaluation. This is implementation diversity, not organizational independence,
and both paths still share CPython and its rational arithmetic.

Certificate mutations are not all executed in Rust. The default retained run
sends 128 genuine certificates and 128 hostile certificates to the separately
implemented Rust checker. The other certificate cases establish that a
versioned mutation changed canonical evidence bytes; they do not receive a
cross-language rejection claim. The report states these denominators
separately.

Composition cases cover exact transitivity, same-metric bound addition,
property-refinement transitivity, incompatible relations, artifact and semantic
discontinuity, empty or widened scopes, and missing deductive tags. They test an
executable restatement of the Coq core's rule shape. They do not prove that a
Python or Rust deployment implements the Coq definitions.

## Failure handling and minimization

The campaign fails at the first counterexample and reports its family, index,
and smallest generated witness available at that layer. The input grammars are
already single-mutation forms. Model failures are reproduced by their seed and
case index; reduction proceeds by removing hidden neurons, outputs, and input
dimensions while retaining the failure predicate before a regression is
accepted. Composition failures reduce bit-set scopes and then remove optional
continuity or evidence faults. A failure must become a minimal unit regression
before the retained report can be replaced by a passing result.

The current runner prints a failing witness to standard error and publishes no
PASS report. CI log retention is therefore part of the internal failure record;
an external evaluator must preserve its raw logs independently.

## Commands

Smoke profile:

```sh
PYTHONPATH=src python scripts/run_generative_campaign.py \
  --total 1000 \
  --output /tmp/ckodmk-generative-smoke.json
```

Retained profile with the Rust checker:

```sh
PYTHONPATH=src python scripts/run_generative_campaign.py \
  --total 1000000 \
  --checker checker-rs/target/release/ckodmk-check \
  --output reports/GENERATIVE_CAMPAIGN_V1.json
```

The runner refuses to replace an existing output. Remove or archive a previous
result only as an explicit evidence-management action.

## Claim boundary

A PASS means no mismatch was found in the recorded generated cases and the
recorded Rust sample agreed. It does not establish exhaustive correctness,
model minimality, floating-point behavior, ONNX equivalence, device behavior,
security outside the generated grammars, external independence, or a score
above 9/10.
