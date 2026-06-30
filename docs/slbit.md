# Power House + slbit Observatory

Status: active integration guide for Power House v0.3.19 and slbit v3.1.0.

Power House and `slbit` are independent technologies:

- Power House verifies proofs, `.pha` artifacts, identity bindings, and
  Rootprint lineage.
- `slbit` produces deterministic semantic transcripts and visualization
  packets.
- The MFENX Observatory verifies both layers and renders them together.

The semantic layer is deliberately non-core. It never participates in a
`phx_fingerprint`, Rootprint branch ID, replay fingerprint, equivalence result,
or proof-validity decision.

## Install

```bash
cargo add power_house
cargo add slbit
```

The `slbit` crate has zero dependencies. Power House does not depend on it at
runtime. This repository uses it only as a development dependency for examples,
conformance vectors, and cross-crate tests.

## Create a verified Rootprint

Power House canonical JSON accepts integers, not floating-point values. Encode
fractional application values with an explicit fixed-point unit.

```rust
use power_house::{prove_with_rootprint, provenance::PhaArtifact};
use serde_json::json;

let artifact = PhaArtifact::new(
    json!({"producer": "vision-pipeline", "frame": 7842}),
    "vision/v1",
    json!({
        "claim": "stop-sign-detected",
        "confidence_millionths": 987_000
    }),
    json!({"accepted": true}),
)?;

let graph = prove_with_rootprint!(
    label: "drone-perception-7842",
    artifact: artifact,
)?;
graph.verify()?;
let replay_fingerprint = graph.replay()?.state_fingerprint;
# let _ = replay_fingerprint;
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Create a semantic packet

```rust
use slbit::{
    BitInteractiveTranscript, LuminousClaim, SimpleLuminousSumcheck, VizHints,
};

let claim = LuminousClaim::new("drone-camera-frame-7842", 4096)
    .with_viz_hints(VizHints {
        color: Some([0, 200, 255]),
        icon: Some("camera".into()),
        layer_name: Some("perception-conv3".into()),
    });

let mut transcript = BitInteractiveTranscript::new(b"drone-seed-7842");
transcript.record_round_with_note(
    0,
    &[0xde, 0xad, 0xbe, 0xef],
    "sensor-processing",
    "Raw sensor frame converted into features",
);
transcript.record_round_with_note(
    1,
    &[0x42],
    "attention-head-7",
    "Stop-sign feature strongly activated",
);

let luminous = SimpleLuminousSumcheck { claim, transcript };
let packet = luminous.to_viz_packet()?;
packet.verify()?;
# Ok::<(), slbit::SlbitError>(())
```

`slbit` exports payload digests and annotations, not raw round payload bytes.
Its packet and transcript digests establish deterministic transport integrity;
they do not establish proof-system soundness. In v3.1, the standalone crate
also exposes Meaning Observatory inspection APIs such as authority counts,
dependency chains, shortest explanation paths, and deterministic ask reports.

## Bind the presentation layer

`ObservatorySidecar` binds opaque JSON packets to exact Rootprint branch IDs
and the canonical Rootprint replay fingerprint:

```rust,ignore
use power_house::ObservatorySidecar;
use std::collections::BTreeMap;

let packet_json = serde_json::from_str(&packet.to_json())?;
let sidecar = ObservatorySidecar::new(
    &graph,
    BTreeMap::from([(graph.root_branch.clone(), packet_json)]),
)?;
sidecar.verify(&graph)?;
```

The complete executable workflow is
[`examples/slbit_observatory.rs`](../examples/slbit_observatory.rs).

## CLI verification

```bash
julian rootprint verify proof.rootprint.json
julian observatory verify \
  proof.rootprint.json \
  proof.observatory.json
```

The second command first verifies the Power House graph, then checks the
optional sidecar schema, replay binding, branch references, and sidecar digest.
Power House treats packet bodies as opaque objects; `slbit` or another producer
must verify packet-specific semantics.

## Browser workflow

The public Power House Observatory downloads and verifies:

1. `artifacts/rootprint-valid.json`;
2. `artifacts/luminous-valid.json`;
3. every `.pha` core fingerprint and Rootprint branch ID;
4. the canonical Rootprint replay fingerprint;
5. the Observatory sidecar digest;
6. every published semantic-packet digest used by the sidecar.

After verification, each graph node exposes its claim, layer, icon, color, and
human-readable transcript rounds. Selecting a node changes presentation state
only.

The dedicated SLBIT page at `https://mfenx.com/slbit.html` is local-first. It
does not upload packets. The browser verifier currently validates
`slbit/viz-packet/v2` packet transport integrity and renders the SLBIT 3.1
Meaning Observatory inspection surface around that packet data:

- truth-boundary state;
- authority counts;
- deterministic ask answers;
- dependency path inspection;
- transcript playback;
- semantic graph rendering;
- Markdown and LLM-context export.

The page deliberately labels external proof validity as external. A rendered
SLBIT packet can explain a Rootprint-bound proof state, but it cannot change the
underlying Power House proof identity.

## Conformance

Generate the canonical integration vectors:

```bash
cargo run --example slbit_conformance_vectors
git diff --exit-code -- \
  conformance/slbit-v1 \
  publicpower/artifacts/luminous-valid.json
```

The manifest explicitly records
`"semantic_packets_affect_core_identity": false`. Mutation tests prove that a
semantic change rejects the sidecar while the underlying Rootprint remains
valid.

## Schemas

| Schema | Owner | Purpose |
| --- | --- | --- |
| `power-house/rootprint/v1` | Power House | Verified provenance DAG |
| `power-house/observatory-sidecar/v1` | Power House | Non-core branch-to-packet binding |
| `slbit/viz-packet/v1` | slbit | Semantic transcript and visualization data |
| `slbit/viz-packet/v2` | slbit | Extended semantic packets with anchors, summaries, redactions, and deterministic digests |
| `slbit/viz-packet/v3` | slbit | Meaning Observatory packets for bound-core inspection, deterministic ask reports, authority labels, and semantic DAG views |

The normative `slbit` packet specifications are published in the standalone
[`slbit` repository](https://github.com/JROChub/slbit/tree/main/docs).
