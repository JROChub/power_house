# CKODMK external evaluator handoff

This document defines the minimum independent procedure for evaluating CKODMK
0.2. It is an execution checklist, not a claim that an external evaluation has
already occurred. The repository and evaluator archive are private. Access is
granted to a named evaluator by the repository owner; access does not authorize
redistribution.

## Independence record

Before receiving implementation guidance, the evaluator records:

- their organization or stable pseudonymous identifier;
- their relevant compiler, model-validation, statistics, and security
  experience;
- any financial, employment, authorship, or family relationship with MFENX;
- the exact Git commit and CI run selected by the evaluator;
- the research-ZIP SHA-256 obtained through a channel separate from the ZIP;
- the machine, operating system, Python, Rust, browser, and target-device
  versions used for each execution.

MFENX must not choose mutations, discard failures, edit returned reports, or
operate the evaluator's machines. A result produced by an MFENX author remains
internal evidence even if this checklist was followed.

## Frozen package

Download the `ckodmk-0.2.0-<commit>` artifact from the selected private GitHub
Actions release-gate run. Independently record the run URL, commit, artifact
digest supplied by GitHub, and the contents of `SHA256SUMS`. Verify the ZIP
before extraction:

```bash
python scripts/verify_release_archive.py \
  /path/to/CKODMK-0.2.0-research-evidence.zip \
  --sha256 sha256:<digest-obtained-separately>
```

The archive verifier establishes byte consistency with the supplied digest. It
does not establish MFENX authorization because the v0.2 package is unsigned.

The archive is incomplete and the evaluation must stop if it lacks any of:

- both Python hash locks;
- both Rust lockfiles and checker sources;
- the real-model and ONNXScript retained labs;
- the internal blind-campaign corpus, oracle, raw results, and final report;
- all specifications, reports, SBOMs, tests, and browser assets;
- `MANIFEST.json` and `RELEASE.json` at the archive root.

## Clean execution

Use a clean Linux x86-64 environment with CPython 3.13.12 and Rust 1.93.1. Do
not reuse an MFENX virtual environment or build directory.

```bash
python3.13 -m venv .venv-audit
.venv-audit/bin/python -m pip install --require-hashes \
  -r requirements-toolchains.lock
.venv-audit/bin/python -m pip install --no-deps -e .

PYTHONPATH=src .venv-audit/bin/python -m unittest discover -s tests -v
PYTHONPATH=. .venv-audit/bin/python -m unittest \
  independent_onnx_checker.test_checker -v
PYTHONPATH=campaign .venv-audit/bin/python -m unittest \
  campaign.test_harness_v3_unit -v

cargo test --locked --manifest-path checker-rs/Cargo.toml
cargo test --locked --manifest-path gate-adjudicator-rs/Cargo.toml
cargo clippy --locked --all-targets --manifest-path checker-rs/Cargo.toml \
  -- -D warnings
cargo clippy --locked --all-targets \
  --manifest-path gate-adjudicator-rs/Cargo.toml -- -D warnings
```

Run every generated-report freshness check and the browser static test:

```bash
.venv-audit/bin/python scripts/render_real_model_report.py --check
.venv-audit/bin/python scripts/render_blind_campaign_report.py --check
.venv-audit/bin/python scripts/render_second_toolchain_report.py --check
.venv-audit/bin/python scripts/test_web.py
```

## Evidence re-adjudication

Use caller-pinned digests copied from the selected archive, not values sent in
a separate chat message. Re-adjudicate both retained labs with the supplied
Rust executable. Then independently rebuild the research ZIP twice and require
identical bytes.

```bash
PYTHONPATH=src:. .venv-audit/bin/python \
  scripts/verify_onnxscript_toolchain_lab.py \
  --lab artifacts/onnxscript-toolchain-lab-v1 \
  --manifest-sha256 sha256:<second-toolchain-manifest> \
  --adjudicator artifacts/real-model-lab-v1/tools/ckodmk-gate-adjudicate \
  --adjudicator-sha256 sha256:<adjudicator> \
  --device-profile-sha256 sha256:<retained-device-profile>

SOURCE_DATE_EPOCH=1786320000 .venv-audit/bin/python \
  scripts/build_release_archive.py --output /tmp/ckodmk-a.zip
SOURCE_DATE_EPOCH=1786320000 .venv-audit/bin/python \
  scripts/build_release_archive.py --output /tmp/ckodmk-b.zip
cmp /tmp/ckodmk-a.zip /tmp/ckodmk-b.zip
```

The retained real-model lab is bound to its original device profile and cannot
be truthfully regenerated on another machine. Integrity verification is not an
execution reproduction. A new physical-target run therefore needs a new,
evaluator-owned profile and a separately retained output package; it must not
replace the original lab.

A physical-device evaluator can run the browser gate and paired timing study at
`https://mfenx.com/ckodmk/`, explicitly enter a non-serial target description,
and download the v2 report. Before transferring it, the evaluator records its
SHA-256 through a separate channel. The consumer recomputes the report schedule
and summaries:

```bash
python scripts/verify_phone_study.py /path/to/ckodmk-phone-study.json \
  --sha256 sha256:<digest-obtained-separately>
```

`STRUCTURE_AND_ARITHMETIC_VERIFIED` does not authenticate the evaluator, clock,
or device. A qualification result additionally needs the signed evaluator
statement and repeat protocol required below.

## External hostile campaign

The published 130-case campaign is internal evidence. Qualification gate H5
requires an evaluator-controlled campaign of at least 1,000 preregistered
semantics-changing cases. The evaluator selects the seed, mutation generator,
case distribution, exclusions, and unblinding procedure. MFENX receives only
the preregistration commitment until execution is irreversibly complete.

Every crash, timeout, unsupported case, invalid mutation, exclusion, and false
block remains in the returned denominator. The evaluator reports exact counts,
confidence intervals, and raw per-case outcomes. A CKODMK `PASS` on an
effective contract violation is a false accept regardless of aggregate score.

## Required return package

Return one immutable package containing:

- the independence and qualification record;
- exact source commit, release archive digest, and CI run URL;
- full commands, environment inventory, stdout, stderr, and exit status;
- raw generated cases and preregistration commitment;
- all successful and failed reports without deletion;
- physical-target profiles and measurements, with any personal fields removed
  before publication;
- a signed evaluator statement listing PASS, FAIL, and NOT EVALUATED items;
- a license permitting MFENX to retain the package and publish an agreed
  redacted version.

MFENX records an external gate as complete only after checking this return
package against `ACCEPTANCE_RUBRIC.md`. Silence, access to the repository, a
green internal CI run, or an unsigned summary is not external reproduction.

The evaluator also completes the strict v1 return record described in
`specs/external-evaluation-return-v1.md`, signs its exact bytes with an
independently trusted OpenSSH key, and supplies the detached signature. MFENX
verifies it with a signer identity and report digest obtained separately:

```bash
python scripts/verify_external_evaluation.py \
  external-evaluation.json \
  --sha256 sha256:<digest-obtained-separately> \
  --signature external-evaluation.json.sig \
  --allowed-signers /secure/ckodmk-evaluators.allowed_signers \
  --evaluator evaluator@example.org
```

This authenticates the evaluator's assertion and recomputes the profile's
minimum arithmetic conditions. It does not rerun or approve the referenced raw
evidence; MFENX must still inspect that immutable return package.
