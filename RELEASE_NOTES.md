# Release Notes

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
- Fully renumbered each chapter (no ghost indices) and documented fallback typography for the Book of Power PDF.
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
- `docs/training_binder.md` cadet packet covering field drills, transcript printouts, challenge logs, and signature blocks for compliance.
- `docs/book_of_power.tex` + generated PDF export for sharing the manual without depending on local markdown renderers.

### Changed
- Wrapped the TeX build with `fvextra` to break long lines cleanly, keeping ledger commands/URLs readable in the PDF.
- Bumped crate version to `0.1.35` to mark this documentation-heavy release.
