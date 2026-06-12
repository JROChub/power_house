# MFENX Orbital Observatory

Release surface: Power House v0.3.1.

`mfenx.com` presents Power House as an interactive planetary proof
observatory. It combines live celestial telemetry, world time, published proof
artifacts, and browser-side verification in one static deployment.

## World observatory

The Three.js globe uses NASA Blue Marble daytime imagery and NASA Earth at
Night data. Desktop clients receive 4K textures; mobile clients receive
smaller responsive textures for faster startup.

The astronomy model provides:

- solar declination and equation-of-time calculations,
- a geographically anchored day/night terminator,
- the live subsolar point,
- solar altitude, sunrise, and sunset for each indexed city,
- lunar phase and illumination,
- an adjustable 48-hour celestial timeline,
- 18 searchable IANA time-zone clocks.

City markers and proof-orbit beacons are selectable directly on the globe.
The globe supports pointer rotation, wheel zoom, touch input, keyboard
rotation, keyboard zoom, and one-command focus on the selected city.

## Verification modes

The five instrument modes are:

1. Browser-native `.pha` and Rootprint core verification, including
   deterministic fingerprint, branch ID, ordering, and reachability checks.
2. A 70-round constant-polynomial equation replay over field
   `1,000,000,007`.
3. A 4,096-round seeded-affine structural replay.
4. SHA-256 verification of the published `PHSPv1` million-round artifact.
5. SHA-256 verification of both the `PHSMv1` workload and `PHCPv1`
   certificate.

Proof progress is reflected in both the verification dock and the active
orbital beacon. Successful runs can be shared through the Web Share API or
copied to the clipboard. The local-file control accepts `.pha`, Rootprint JSON,
the published `PHSPv1` certificate, or a paired `PHSMv1`/`PHCPv1` workload and
certificate. Portable JSON files receive core verification; binary release
artifacts receive exact-length and SHA-256 verification.

The browser replays provide immediate interactive checks. The canonical Rust
and Python tooling provides full artifact parsing, transcript validation,
BLAKE2b workload commitment checks, and algebraic replay as documented in the
[Verification Guide](verification_guide.md).

## Performance

- Texture resolution is selected at startup from the client viewport.
- Device pixel ratio is capped separately for desktop and mobile GPUs.
- Rendering pauses while the page is hidden.
- Reduced-motion preferences disable automatic orbital motion.
- Artifact downloads stream against the canonical uncompressed release size.
  The growable buffer deliberately ignores compressed HTTP `Content-Length`
  values, preventing gzip expansion from corrupting browser verification.
- Exact artifact-length checks reject truncated or expanded release data
  before hashing.
- Date and time formatters are cached per IANA zone.

## Deployment

The static site is stored under `publicpower/` and deployed from the
`gh-pages` branch. `publicpower/CNAME` maps GitHub Pages to `mfenx.com`.

Release artifacts and the canonical Rootprint vector are bundled under
`publicpower/artifacts/` so the browser can verify them from the same origin.

## Visual sources

- NASA Earth at Night maps:
  https://science.nasa.gov/earth/earth-observatory/earth-at-night/maps/
- Three.js:
  https://threejs.org/
- Lucide:
  https://lucide.dev/
- The computational proof-lattice backdrop was generated for this project
  with OpenAI image generation and optimized locally as WebP.
