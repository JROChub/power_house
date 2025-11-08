# Release Notes


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
