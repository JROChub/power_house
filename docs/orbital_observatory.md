# MFENX Orbital Observatory

Release surface: Power House v0.3.8.

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
- 19 searchable IANA time-zone clocks,
- an altitude-position solar track,
- live LAX MFENX RPC block, validator, and peer telemetry,
- selectable `sfo3`, `nyc3`, and `ams3` regional quorum controls.

City markers and proof-orbit beacons are selectable directly on the globe.
The globe supports pointer rotation, wheel zoom, touch input, keyboard
rotation, keyboard zoom, explicit zoom controls, orbital reset, and
one-command focus on the selected city. The active city adds a surface halo
and radial signal beam. Production validator regions are connected by animated
great-circle routes.

The scene also renders a proof-reactive point shell and field rings. Their
color follows the selected proof mode, while their density and scale respond
to live verification progress. URL parameters can open a selected mode, city,
time offset, or drawer directly:

```text
/?mode=affine&city=SFO&time=6&panel=observatory
```

## Verification modes

The five instrument modes are:

1. Browser-native `.pha` and Rootprint core verification, including
   deterministic fingerprint, branch ID, ordering, reachability, and canonical
   replay checks. The same mode separately verifies and renders the published
   `slbit` semantic sidecar.
2. A 70-round constant-polynomial equation replay over field
   `1,000,000,007`.
3. A 4,096-round seeded-affine structural replay.
4. SHA-256 verification of the published `PHSPv1` million-round artifact.
5. SHA-256 verification of both the `PHSMv1` workload and `PHCPv1`
   certificate.

Proof progress is reflected in both the verification dock and the active
orbital beacon. Successful runs can be shared through the Web Share API or
copied to the clipboard. The local-file control accepts `.pha`, Rootprint JSON,
Observatory sidecar JSON, the published `PHSPv1` certificate, or a paired
`PHSMv1`/`PHCPv1` workload and certificate. Portable JSON files receive core
verification; binary release artifacts receive exact-length and SHA-256
verification.

When a Rootprint and matching Observatory sidecar are selected together, the
browser verifies both layers and renders a selectable semantic DAG. Node color,
icon, layer label, claim, and transcript notes come from `slbit`; graph identity
and validity continue to come exclusively from Power House.

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
- The public status feed refreshes every 15 seconds without blocking local
  proof verification.
- CI validates every control-to-DOM binding and recomputes the size and
  SHA-256 digest of all five bundled public artifacts.

## Deployment

The static site is stored under `publicpower/` and deployed from the
`gh-pages` branch. `publicpower/CNAME` maps GitHub Pages to `mfenx.com`.

Release artifacts, the canonical Rootprint vector, and its non-core semantic
sidecar are bundled under `publicpower/artifacts/` so the browser can verify
them from the same origin.

## Visual sources

- NASA Earth at Night maps:
  https://science.nasa.gov/earth/earth-observatory/earth-at-night/maps/
- Three.js:
  https://threejs.org/
- Lucide:
  https://lucide.dev/
- The computational proof-lattice backdrop was generated for this project
  with OpenAI image generation and optimized locally as WebP.
