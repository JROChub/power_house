# MFENX Orbital Observatory

`mfenx.com` is a static operational view of the Power House proof artifacts.
It combines a live world clock with an interactive Three.js Earth and proof
verification controls.

## Orbital map

The globe uses NASA Blue Marble daytime imagery and NASA Earth at Night data.
A fragment shader blends the two textures around a live solar terminator.

The subsolar point is updated from UTC:

- longitude follows UTC solar time,
- latitude follows the approximate annual solar declination,
- city clocks use the browser's IANA time-zone database,
- selecting a city rotates the globe to its coordinates.

The orbit tracks represent the four public proof modes, not physical
satellites.

## Browser verification

The interface provides four actions:

1. A real 70-round constant-polynomial equation replay over field
   `1,000,000,007`.
2. A 4,096-round seeded-affine structural replay. This is a browser model; the
   canonical Rust proof uses BLAKE2b Fiat-Shamir challenges.
3. SHA-256 verification of the published `PHSPv1` million-round artifact.
4. SHA-256 verification of both the `PHSMv1` workload and `PHCPv1` certificate.

The artifact actions prove that the bytes served by the site match the
immutable release digests. They do not replace the algebraic Rust/Python
verification described in `docs/verification_guide.md`.

## Deployment

The site is stored under `publicpower/` and deployed from the `gh-pages`
branch. `publicpower/CNAME` maps the Pages deployment to `mfenx.com`.

The release artifacts are bundled under `publicpower/artifacts/` because
GitHub release downloads do not allow the cross-origin browser fetch required
for local SHA-256 verification.

## Visual sources

- NASA Earth at Night maps:
  https://science.nasa.gov/earth/earth-observatory/earth-at-night/maps/
- Three.js:
  https://threejs.org/
