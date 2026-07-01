# Truth Boundary

Status: active security statement for Power House v0.3.22.

## What Power House Proves

Power House verifies `.pha` core fingerprints, proof payload bindings,
Rootprint graph validity, replay fingerprints, and deterministic rejection of
mutated core data.

## What Rootprint Remembers

Rootprint remembers deterministic artifact lineage: create, fork, merge,
navigation, replay, and graph equivalence over Power House core identities.

## What slbit Explains

`slbit` explains what a verified object means to a human or another tool. Its
packets may describe claims, transcript rounds, semantic DAG nodes, summaries,
visual hints, and audit context.

## What The Observatory Renders

The Observatory renders verified Rootprint state and non-core semantic packets.
Rendering is display, not proof. Text shown in the browser is escaped and must
not be treated as authoritative proof material.

## What Witnesses Attest

Witnesses attest that they observed specific capsule, core, and replay digests.
Witnesses do not make an invalid proof valid.

## What Mutation Tests Demonstrate

Mutation tests demonstrate that known tampering is rejected at a specific layer.
They are falsifiability tools. They do not prove every possible real-world
statement.

## What This Does Not Prove

Power House does not prove reality, guarantee AI correctness, replace all
audits, allocate an expanded Boolean hypercube, or turn semantic text into core
truth.

## How To Independently Reproduce

```bash
julian memory verify earth-001.phm
julian memory replay earth-001.phm
julian memory challenge earth-001.phm --all
```
