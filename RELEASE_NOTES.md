# Release Notes

## v0.3.7 - 2026-06-14

### Human-Observable Proofs
- Added the non-core `ObservatorySidecar` format for binding opaque semantic
  packets to exact Rootprint branch IDs and canonical replay state.
- Integrated the independent zero-dependency `slbit` crate through examples,
  tests, and conformance vectors without adding it to Power House runtime
  dependencies.
- Added deterministic semantic mutation tests proving that sidecar changes
  reject presentation integrity while Power House core verification remains
  unchanged.

### CLI And Observatory
- Added offline `julian observatory verify` for Rootprint-first sidecar
  verification.
- Added a browser-verified semantic DAG with node colors, icons, layer labels,
  claims, packet digests, and expandable transcript rounds.
- Added browser verification for Rootprint replay, sidecar, `slbit` transcript,
  and packet digests before semantic rendering.

### Conformance And Documentation
- Added reproducible `conformance/slbit-v1` vectors and the bundled
  `luminous-valid.json` public artifact.
- Added the complete Power House + `slbit` integration guide and rustdoc links.
- Extended CI and the public Observatory contract gate across the new artifact
  and rendering path.

## v0.3.6 - 2026-06-14

### Identity Layer
- Added immutable `Identity` envelopes over `.pha` artifacts and validated
  `RootprintId` values.
- Added deterministic create, fork, merge, verify, replay, and equivalence
  operations without requiring network connectivity.
- Added `julian identity` commands for the complete identity lifecycle.

### Rootprint And `.pha`
- Added canonical Rootprint replay state and domain-separated replay
  fingerprints.
- Added deterministic whole-graph union and graph equivalence.
- Canonicalized replay sequence from graph ancestry while retaining backward
  compatibility with larger parent-before-child sequence gaps.
- Added optional `.pha` `identity_root` bindings while preserving every legacy
  v1 core fingerprint and keeping external attachments outside core identity.

### SDK And Conformance
- Added matching Rust and Python identity operations.
- Added identity, graph, replay, CLI, merge reproducibility, mutation, and
  cross-SDK conformance tests and vectors.
- Added identity API examples and normative identity documentation.

### Licensing
- Standardized Power House v0.3.6 and later on `AGPL-3.0-only`.
- Added `LICENSE-CHANGE.md` to preserve the licensing history of releases
  through v0.3.5.

## v0.3.5 - 2026-06-13

### Monitoring Hotfix
- Corrected Prometheus file-discovery permissions from private health-state
  mode to world-readable public target metadata mode.
- Retained `0640` protection for detailed validator health state.
- Added regression assertions for validator discovery, system discovery, and
  private state-file permissions.
- Verified all three production validator and system targets are dynamically
  discovered and reporting `up`.

## v0.3.4 - 2026-06-13

### Signed Validator Registry
- Added versioned validator registrations signed by each node's existing
  Ed25519 identity.
- Bound every registration to its derived libp2p peer ID, chain ID, p2p
  address, operator, region, monitoring endpoints, and validity window.
- Required explicit admission by the active validator policy before a record
  can affect public validator totals.
- Added `julian validator-registry create`, `assemble`, and `verify`.

### Live Identity And Health
- Added `powerhouse_node_identity` metrics so the registry reconciler can
  compare the live node ID, peer ID, public key, and chain ID with the signed
  registration.
- Replaced hardcoded Prometheus validator targets with atomic file discovery.
- Added concurrent validator and system health probes, bounded responses,
  redirect rejection, stale-state detection, and last-known-good discovery
  preservation when registry verification fails.
- Updated the public status API and website to report dynamic registered and
  healthy validator totals independently from peer-link observations.

### Safety And Testing
- Kept monitoring enrollment separate from persisted consensus membership and
  quorum transitions.
- Added signature mutation, identity mismatch, expiration, duplicate,
  unadmitted-key, stale-state, dynamic-count, deployment-bundle, and
  reconciliation tests.
- Retained the existing three-validator finalized network while making
  monitoring discovery ready for controlled future validator admission.

## v0.3.3 - 2026-06-13

### Documentation
- Corrected every Primary Rust API link to the defining rustdoc module so the
  GitHub and crates.io README links resolve to populated docs.rs item pages.
- Added a direct rustdoc link for the `prove_with_rootprint!` macro.
- Added a generated-document validation gate that rejects missing, empty, or
  noncanonical README API targets before release.

### Orbital Observatory
- Rebuilt the public instrument around a brighter NASA day/night Earth,
  proof-reactive point shell, selected-city signal geometry, and animated
  routes between the three production validator regions.
- Added live public RPC state, block height, validator count, and peer count to
  the primary control surface.
- Added explicit focus, network, zoom, reset, sound, and motion controls plus
  direct URL state for proof mode, city, time offset, and open drawer.
- Expanded the Observatory with Amsterdam, a solar-position track, and
  selectable `sfo3`, `nyc3`, and `ams3` quorum controls.
- Added a CI contract test for all browser control bindings and immutable
  Rootprint, PHSPv1, PHSMv1, and PHCPv1 artifact sizes and hashes.

### Release Governance
- Decoupled the active software release from historical network benchmark
  identity, preserving measured v0.3.2 results without relabeling them.
- Synchronized Rust, Python, container, website, network, and operator-facing
  release labels for v0.3.3.

## v0.3.2 - 2026-06-12

### Public Network
- Added a health-checked rolling validator deployment with automatic rollback
  and post-restart binary/RPC version verification.
- Added Prometheus, Alertmanager, blackbox exporter, node exporter, and Grafana
  deployment automation for the three-region production topology.
- Added automatic node recovery with restart cooldowns and Slack/PagerDuty
  alert delivery support.
- Added a public status API and website status console for validator health,
  RPC reachability, block height, peer connections, and rolling uptime.
- Added DigitalOcean Terraform for validators, firewall rules, and the global
  RPC edge.

### Operations
- Standardized the public endpoint name as **LAX MFENX RPC**.
- Added node operator, incident response, load testing, network roadmap, and
  testnet-to-mainnet guides.
- Added release consistency enforcement across Rust, Python, documentation,
  website labels, chain metadata, Docker tags, and Git tags.
- Added a connected-peer Prometheus gauge for isolation detection.
- Renamed the live global edge to `lax-mfenx-rpc`, enabled HTTP-to-HTTPS
  redirection and strong TLS, and corrected ChainList metadata to the verified
  canonical endpoint.
- Verified controlled backend failover, automatic validator recovery, and a
  zero-error 69.114 requests/second single-origin read-only load profile.

## v0.3.1 - 2026-06-12

### Documentation
- Rebuilt the GitHub and crates.io README around Power House Archive,
  Rootprint, proof profiles, SDKs, and current network operations.
- Replaced the legacy rustdoc introduction with current `.pha`, Rootprint,
  sum-check, sparse proof, and feature documentation.
- Added an authoritative documentation index that separates current guides
  from retained historical material.
- Updated active protocol, SDK, verification, Observatory, RPC, and operations
  guides to the v0.3.1 release surface.
- Added docs.rs metadata to build all features and a CI gate that rejects
  rustdoc warnings.
- Fixed existing rustdoc markup and link warnings.

### Metadata
- Updated the crates.io description and keywords for portable provenance and
  Rootprint.
- Synchronized the bundled Python SDK and public verifier release labels.

## v0.3.0 - 2026-06-12

### Added
- Power House Archive (`.pha`) v1 with deterministic core fingerprints.
- Rootprint v1 navigation, forking, merging, equivalence, and graph verification.
- The `prove_with_rootprint!` macro and `julian rootprint` command family.
- Zero-dependency Python SDK with Rust-compatible conformance vectors.
- Browser-native Rootprint and `.pha` verification on mfenx.com.
- Reproducible v0.3.0 provenance, branching, scale, and verification benchmarks.

### External Proof Attachments
- Added EPA as optional transport data inside `embedded_proof`.
- Omitted the field when unused through `skip_serializing_if = "Option::is_none"`.
- Excluded EPA from core fingerprints, branch IDs, graph validity, and equivalence.
- Added explicit attachment integrity and caller-supplied semantic verification.
- Added mutation tests proving EPA changes preserve core validity while core
  mutations reject.

### Performance
- Changed Rootprint reachability verification to linear graph traversal.
- Published measured release results for `2^70`, `2^4096`, core provenance, and
  a 2,049-branch graph. Timings are machine-dependent measurements.

## v0.2.2 - 2026-06-05

### Added
- Canonical small `PHSPv1`/`PHSMv1`/`PHCPv1` conformance vectors and manifest.
- Property-based dense-equivalence tests and full single-byte mutation rejection.
- Reproducible soundness-budget and benchmark-report tools.
- Security model, falsifiable research protocol, and primary-source prior-art review.
- Orbital Observatory v2 with live solar geometry, lunar telemetry, searchable
  world clocks, a 48-hour celestial timeline, and direct globe interaction.

### Security
- Enforced deterministic primality validation for every Rust `Field`.
- Fixed near-`u64::MAX` field addition overflow.
- Added matching Python primality validation.
- Added decoder limits for variables, terms, degree, seeds, and total incidences.
- Rejected oversized polynomial degrees before allocation.

### Documentation
- Streamlined the crate README and documentation index.
- Removed the obsolete research-claim policy document.
- Updated the observatory documentation link and release label.
- Expanded the observatory operations guide and visual-source attribution.

### Website
- Connected all four public proof modes to interactive orbital telemetry and
  shareable browser verification results.
- Added responsive NASA Earth textures, optimized visual assets, mobile
  observatory controls, WebGL fallback, and reduced-memory artifact hashing.
- Rebuilt the public experience as a full-screen proof instrument with a
  generated computational lattice, proof-reactive WebGL geometry, a technical
  evaluation intake, and a responsive verification transcript.
- Added local `PHSPv1` and paired `PHSMv1`/`PHCPv1` release verification.
- Fixed gzip-expanded artifact streaming by validating canonical uncompressed
  sizes instead of allocating from the HTTP `Content-Length` header.

## v0.2.1 - 2026-06-05

### Added
- Closed-form constant sum-check over `2^70` Boolean points.
- Seeded-affine sum-check over configurable domains, demonstrated at `2^4096`.
- Stable `PHSPv1` million-round seeded sparse certificates.
- Stable `PHSMv1` external sparse workloads and commitment-bound `PHCPv1` proofs.
- Separately implemented standard-library Python verifier for both sparse formats.
- Unified verification guide and reproducible reference artifacts.

### Changed
- Combined the v0.2 network, migration, rollup, and operations line with the
  large-domain proof work.
- Normalized the source manifest and expanded packaged documentation.
- Tightened public claims to distinguish implicit-domain scale from arbitrary
  computation, succinct verification, and established novelty.

## v0.1.58 - 2026-02-18

### Added
- External DA publisher pipeline with receipts (HTTP relay + Ethereum anchoring support).
- Permissionless join guide, community onboarding, tokenomics, and bounty policy docs.
- Dockerfile + compose for community node operators.

### Changed
- Stake governance tuned for 5-of-7 signer threshold; membership expanded for multi-node scale.
- Shard gossip bridging + quorum/BFT flow tightened for stable finality.
- Ops references refreshed (mainnet launch guidance + governance policy notes).

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
