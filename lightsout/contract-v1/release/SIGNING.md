# Release signing operations

MFENX release signatures use OpenSSH Ed25519 signatures with namespace
`mfenx-release` and signer identity `mfenx-release`.

Public fingerprint:

```text
SHA256:Uhj/Ci2+3KA2JN/H8+Sl6nhAiTeD76zvajqvxLOYTTc
```

Verify the manifest directly from this directory:

```bash
ssh-keygen -Y verify \
  -f allowed_signers \
  -I mfenx-release \
  -n mfenx-release \
  -s RELEASE-MANIFEST.canonical.json.sig \
  < RELEASE-MANIFEST.canonical.json
```

The namespace prevents a valid signature created for another SSH-signing use
from being accepted as an MFENX release signature. `allowed_signers` also
restricts the key to that namespace.

For a future release, regenerate all artifact digests and sizes, serialize the
manifest under the documented MFENX JSON profile, inspect the resulting diff,
then sign those exact bytes:

```bash
ssh-keygen -Y sign \
  -f /path/to/operator-controlled-mfenx-release-key \
  -n mfenx-release \
  RELEASE-MANIFEST.canonical.json
```

Never copy the private key into this repository or an evidence bundle. Before
commercial distribution, replace local filesystem custody with an encrypted
offline or hardware-backed key, certify the successor key through at least two
independent channels, publish a revocation procedure, and retain the old public
key for verification of historical releases.
