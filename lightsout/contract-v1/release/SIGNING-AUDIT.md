# Signing decision record

Audit date: 2026-08-21

Available local implementations were OpenSSH 10.3p1, GnuPG 2.4.9, and
OpenSSL 3.6.2. Minisign, Cosign, and Syft were not installed. One pre-existing
OpenPGP secret identity, one SSH-agent identity, and one private SSH key file
were detected. Their identities and key material were not recorded in the
project.

The pre-existing identities were not used to sign this release because no
project record established that they were authorized MFENX release identities.
Using one would also couple a personal authentication identity to project
release operations.

A dedicated Ed25519 key was therefore created for MFENX release signing. The
private key was placed outside the workspace with directory mode `0700` and
file mode `0600`. Only its public key and SHA-256 fingerprint are in the release
bundle. Signatures use the OpenSSH `mfenx-release` namespace, and the supplied
allowed-signers policy restricts this key to that namespace.

This is the strongest project-separated signature that could be completed
locally without assuming authority over an existing personal key or access to a
hardware signer, managed key service, transparency log, or independently
controlled publication channel.

## Residual blocker

Cryptographic signing is complete, but third-party publisher authentication is
not. The new public-key fingerprint has not yet been independently published or
certified, and the signing operation was not hardware-attested. A verifier who
learns the public key only from this same directory can check internal integrity
but cannot distinguish the real bundle from a fully replaced bundle.

Before treating this as a commercial release identity:

1. publish the fingerprint through at least two independently controlled
   channels;
2. move the private key to encrypted offline or hardware-backed custody;
3. define rotation and revocation records;
4. have a separately authorized release operator countersign the key and
   manifest; and
5. publish signatures to a tamper-evident transparency service.
