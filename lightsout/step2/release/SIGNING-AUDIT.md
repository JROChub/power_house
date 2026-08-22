# Step 2 addendum signing decision record

Decision recorded: 2026-08-22T01:12:57Z (observational UTC; not a trusted
timestamp).

This record authorizes a detached OpenSSH signature only after the canonical
manifest generator and the complete addendum verifier pass for the exact
artifacts below. It does not authorize changing the Step 1 subject or widening
any claim.

## Frozen inputs reviewed

- Step 1 manifest `bf854ef7144f11358725aaf8021a91f12fede517ac8110fc44bc08a50950b071`
  and signature `2b26d302e978566c95709ac3c4cf93cc10f29ac5d9a02e97470283460efbf1db`
  verify in the `mfenx-release` namespace.
- Accepted executor
  `a1043e568704163b9dedf536c5feb60b0b7fd23097a2a8f0504d55d7ddcb1e3c`
  remains unchanged.
- Standalone reference verifier
  `f3714660b9deeef3bd8ecef716c40580c7596c0903f05e89d555ed0d32e9b9fa`
  is bound to its two-build record, sealed audit, source inventory, and W0
  report.
- Revisioned archive
  `470dee9e3578576b8c2b658c09f7c5b87987383b483a354fbc677f4925d81ff3`
  and validation inventory
  `e907a3280e8b41e30dbf5855da071cfb38c97f2f200fcc0216f2599c626db132`
  passed an extracted validation and a separate read-only audit.
- Same-host reproducible-build inventory
  `96db43766874be110f74f85572216924dd9f17c0dd406c904cc1181f074c1564`
  preserves both differing raw controls and two byte-identical remapped builds.
- Step 2 SBOM
  `732864847d007449d06b713527f16fc75c18f4a03d4a5e5b9351e877fa8baf44`
  and post-hoc provenance
  `cca7f3747b5f707db967934e63d6001f07a9e25d7fa5cb8c4b02fa9da5125b9a`
  were generated twice with byte-equal normalized results and validated by the
  sealed supply-chain record.

## Claim review

- The one-run same-workload cross-release ratio is recorded as
  `49.2271104608x`, rounded to 10 decimal places; it is not a statistical,
  scaling, universal, or unrelated-machine result.
- The scaling source contains 100 planned attempts and zero executed attempts.
- The adversarial source contains 214 planned cases and zero executed cases.
- The verifier checks final image/result/store state and fresh arithmetic; it
  does not attest the historical kill event or full execution lifecycle.
- The verifier is not independently authored or governed, no independent
  security review is claimed, and cross-host reproducibility remains open.
- The provenance is post hoc and claims no SLSA build level. The SBOM is
  dependency metadata, not a binary-composition or operating-system scan.

## Signing decision

The release owner requested that the current release be signed and directed the
work to continue. That authorization is applied only after every signing gate
in `SIGNING.md` passes. The signature must use the existing project-continuity
key, signer identity `mfenx-release`, and the separately domain-separated
namespace `mfenx-step2-addendum`. The private key remains outside the workspace;
only its public key and fingerprint are routed here.

A valid signature authenticates the reviewed bytes to that project key after
independent fingerprint pinning. It is not third-party certification, a trusted
timestamp, hardware-backed custody, transparency-log inclusion, or a security
or performance warranty.
