# Power-House v0.1.54 Design Plan

Objective
- Deliver a verifiable, adversarially hardened core that is closer to a coordination primitive than a product feature set.
- Keep the implementation auditable and testable: no hidden magic, no opaque dependencies.

Non-goals (for this release)
- Replacing upstream L1 consensus or building a new chain.
- Introducing external paid dependencies.

Core deliverables
1) Stateless verification artifacts
   - Define a canonical "verification bundle" format:
     - anchor.json
     - attestation_qc.json
     - stake_registry.json (snapshot)
     - evidence.jsonl
     - blob meta + proof files referenced by anchor
   - Add `julian verify <bundle>` to validate:
     - anchor signatures
     - DA commitments
     - QC quorum thresholds
     - stake-weighted attestations
     - evidence integrity root

2) Oracle-independent truth consensus (attestation layer)
   - Formalize multi-attestor requirements:
     - `attestation_quorum` and `attestation_policy` are explicit in config and bundle metadata.
     - Require at least N independent attestations for each commitment.
   - Implement `julian attest` subcommand:
     - sign commitments offline
     - output attestation payloads for submission

3) Collusion-resistant incentives (economic honesty)
   - Add slashing rules to stake registry:
     - invalid attestations
     - equivocation
     - evidence mismatch
   - Implement `julian dispute`:
     - submit proof of misbehavior
     - enforce slashing and audit log entries

4) Liveness guarantees (termination)
   - Add explicit timeouts to route/anchor flow:
     - `deadline_at` on route and settlement
     - deterministic fallback on timeout
   - Provide a liveness proof report:
     - `julian liveness --from <anchor> --to <anchor>`
     - verifies no deadlocks and proper timeout handling

5) Identity-light participation
   - Introduce reputation snapshots:
     - `reputation.json` generated from historical accuracy
     - reputation is additive but not a centralized identity
   - Allow policies to reference reputation thresholds

Verification and adversarial testing
- Fuzzing: `cargo fuzz` on anchor parsing, attestation aggregation, and evidence ingestion.
- Property tests: `proptest` for determinism invariants (same input => same output).
- Concurrency checks: `loom` for mempool/anchor state transitions.
- CI: run `cargo test` + smoke net + fuzz nightly.

Release plan
Phase A — Spec
- Define bundle schema and update docs.
- Declare invariants and acceptance criteria.

Phase B — Implementation
- Add `julian verify`, `julian attest`, `julian dispute`, `julian liveness`.
- Update net config to enforce attestation policy.

Phase C — Harden
- Add fuzz/proptest suites and integrate into CI.
- Extend smoke_net to cover DA quorum and QC persistence.

Acceptance criteria
- `cargo test --all --locked` passes.
- `scripts/smoke_net.sh` passes.
- `julian verify` returns success for real bundle.
- Attestation quorum rejects malformed or insufficient attestations.
- Liveness report shows termination under normal and timeout conditions.
