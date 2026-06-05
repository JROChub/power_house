# MFENX Orbital Observatory

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

## Proof orbits

The four orbit tracks map to the public verification modes:

1. A 70-round constant-polynomial equation replay over field
   `1,000,000,007`.
2. A 4,096-round seeded-affine structural replay.
3. SHA-256 verification of the published `PHSPv1` million-round artifact.
4. SHA-256 verification of both the `PHSMv1` workload and `PHCPv1`
   certificate.

Proof progress is reflected in both the verification dock and the active
orbital beacon. Successful runs can be shared through the Web Share API or
copied to the clipboard.

The browser replays provide immediate interactive checks. The canonical Rust
and Python tooling provides full artifact parsing, transcript validation,
BLAKE2b workload commitment checks, and algebraic replay as documented in
`docs/verification_guide.md`.

## Performance

- Texture resolution is selected at startup from the client viewport.
- Device pixel ratio is capped separately for desktop and mobile GPUs.
- Rendering pauses while the page is hidden.
- Reduced-motion preferences disable automatic orbital motion.
- Artifact downloads stream into a preallocated buffer when content length is
  available, avoiding a second full-size in-memory copy.
- Date and time formatters are cached per IANA zone.

## Deployment

The static site is stored under `publicpower/` and deployed from the
`gh-pages` branch. `publicpower/CNAME` maps GitHub Pages to `mfenx.com`.

Release artifacts are bundled under `publicpower/artifacts/` so the browser
can stream and hash them from the same origin.

## Visual sources

- NASA Earth at Night maps:
  https://science.nasa.gov/earth/earth-observatory/earth-at-night/maps/
- Three.js:
  https://threejs.org/
- Lucide:
  https://lucide.dev/
