# Release Notes

## v0.1.54 — 2026-02-08

### Changed
- Production ops hardening: shared node env files, new launcher script, and systemd security guards.
- Added healthcheck, backup, and log-export timers with alert hooks.
- Introduced release deployment/rollback helpers.
- Bumped crate/docs to `0.1.54` and synchronized README, ops notes, training binder, mainnet launch notes, and Protocol Manual version stamps.

## v0.1.49 — 2025-11-11

### Changed
- Bumped crate/docs to `0.1.49`; synchronized README, Protocol Manual, and schema samples. Generic `0.1.x` references remain as-is by design.

## v0.1.48 — 2025-11-09

### Changed
- Bumped crate/docs to `0.1.48` and updated the transcript ABNF (`numbers = number *(SP number)`), regenerating the Protocol Manual PDF to keep the canonical format aligned with the crate.

## v0.1.47 — 2025-11-08

### Changed
- Clarified transcript digest framing (binary layout, non-participating lines), documented the exact Merkle hash domain/markers, added the normative fold-digest framing, tightened ABNF to exactly 64 hex characters, and refreshed the Protocol Manual/PDF accordingly.

## v0.1.46 — 2025-11-08

### Changed
- Normalised transcript examples to include a space after every `statement:/transcript:/round_sums:/final:` colon, and bumped all version references/PDF output to `0.1.46`.

## v0.1.45 — 2025-11-08

### Changed
- Tightened the Protocol Manual transcript grammar (ABNF cardinality, historical `final_eval` note), clarified JSON/Merkle disclaimers, standardised “fold digest” terminology, and bumped all version references/PDF output to `0.1.45`.

## v0.1.44 — 2025-11-08

### Changed
- Corrected Protocol Manual inconsistencies (hash tables, transcript grammar, challenge/merkle descriptions, Chapter VI challenge wording), rebuilt the PDF, and bumped all version strings to `0.1.44`.

## v0.1.43 — 2025-11-08

### Changed
- Bumped crate/docs to `0.1.43` and removed stray nested `powerhouse/` + `publicpower/` directories so the workspace matches the published layout exactly.

## v0.1.42 — 2025-11-08

### Changed
- Ledger parsers now ignore non-`ledger_*.txt` files, accept `# challenge_mode`/`# fold_digest` comment lines, load `fold_digest.txt` hints, and include metadata in anchor files/JSON (`challenge_mode`, `fold_digest`, `crate_version`).
- Updated the hash-pipeline demo to anchor the actual proof transcript (not the genesis stub), regenerate the fold digest/field value, refresh bootstrap logs, and synchronize every README/book/training reference with the new constants.

## v0.1.41 — 2025-11-07

### Changed
- Title page and Chapter I now reference version `0.1.41` so the manual, crate metadata, and release stay aligned.

## v0.1.40 — 2025-11-07

### Changed
- Corrected the hypercube holo-map axes and vertex labels, added the explicit orientation legend, and regenerated the PDF to match.

## v0.1.39 — 2025-11-07

### Changed
- Realigned the hypercube holo-map ASCII diagram in `docs/book_of_power.md` and regenerated the PDF so inspectors receive the precise geometry reference.

## v0.1.38 — 2025-11-07

### Changed
- Fully renumbered each chapter (no ghost indices) and documented fallback typography for the Protocol Manual PDF.
- Added explicit version-lock blurb plus challenge-mode metadata in transcripts/anchors.
- Regenerated `docs/book_of_power.pdf` to match the finalized manual.

## v0.1.37 — 2025-11-07

### Added
- Transcript ABNF, hash framing pseudocode, challenge derivation notes, Merkle capsule spec, JSON schema sketch, metrics crib sheet, and glossary inside `docs/book_of_power.md`.
- Golden test vectors (ledgers, fold digest, anchor root) plus new Spec Compliance checklist in `docs/training_binder.md`.

### Changed
- Clarified fold digest persistence, field reduction endianness, temp-path overrides, key-handling cautions, and CI guardrails.
- Regenerated `docs/book_of_power.pdf` with the updated content.



## v0.1.36 — 2025-11-07

### Changed
- Fixed manual wording to remove disallowed terminology and regenerated the published PDF.

## v0.1.35 — 2025-11-07

### Added
- Ritual ASCII visualizations (hex sigil, hypercube map, anchor cross-section, challenge waterfall, compliance seal) directly into `docs/book_of_power.md` so auditors get instant visual cues.
- `docs/training_binder.md` operator packet covering field drills, transcript printouts, challenge logs, and signature blocks for compliance.
- `docs/book_of_power.tex` + generated PDF export for sharing the manual without depending on local markdown renderers.

### Changed
- Wrapped the TeX build with `fvextra` to break long lines cleanly, keeping ledger commands/URLs readable in the PDF.
- Bumped crate version to `0.1.35` to mark this documentation-heavy release.
