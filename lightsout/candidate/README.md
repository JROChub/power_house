# MFENX Local v2 validation candidate

This directory publishes the authenticated `20260822-a1` validation-candidate
transport files. It is not a final, production, commercial, TOP500, or
performance-class release.

The canonical manifest is signed by the MFENX project-continuity key for
principal `mfenx-release` in the dedicated OpenSSH namespace
`mfenx-validation-candidate`. Verify that signature and the manifest's exact
artifact closure before executing the archive. The pinned public-key fingerprint
is:

```text
SHA256:Uhj/Ci2+3KA2JN/H8+Sl6nhAiTeD76zvajqvxLOYTTc
```

The manifest binds executor
`92e48bfe615ad5241202d2e49fac51d52e21d66f3d0c84c273af042d5852dac0`,
the standalone reference verifier
`f3714660b9deeef3bd8ecef716c40580c7596c0903f05e89d555ed0d32e9b9fa`,
and the separately retained A3 adversarial corpus result: 214 planned, 214
attempted exactly once, 214 independently passed, zero failed. The failed A1 and
A2 runs remain part of the public record and are not replaced by this candidate.

`SHA256SUMS` is an unsigned transport index. Authentication comes from the
detached manifest signature after the key fingerprint is obtained through an
independently trusted channel.
