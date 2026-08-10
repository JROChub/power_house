# CKODMK Gate v0.2 threat model

**Status:** implemented-control inventory plus residual risks  
**Evidence cutoff:** 2026-08-10

## Objective

CKODMK must prevent unsupported preservation or deployment claims from becoming
a release `PASS`. The primary asset is the truthfulness and boundary of the
decision—not merely the integrity of a JSON file.

SHA-256 answers whether named bytes match. It does not establish authorship,
authorization, freshness, dataset representativeness, program semantics, or
device behavior.

## System and trust boundaries

The v0.2 system has two paths:

1. An exact-rational path with an untrusted Python candidate producer and a
   separately implemented consumer-side Rust checker. Its authority ends at
   the restricted `ckodmk/q-relu-affine/v2` semantics.
2. An ONNX CPU path in which a Python Gate snapshots and executes artifacts in
   a resource-limited child process, followed by a separately implemented Rust
   adjudicator that recomputes bindings, metrics, claims, and the decision.

The Rust Gate adjudicator does **not** parse or execute ONNX and does not
independently observe inference, timing, device identity, or bitwise replay.
The output-bit vectors, integer-nanosecond timings, and replay flag are Python
runner assertions. Its value is independent derivation and tamper detection,
not independent execution.

The optional independent NumPy checker does parse and execute a strict ONNX
subset without CKODMK Gate, ONNX Runtime, or Polygraphy code. It checks only its
declared semantic profile. It does not observe the approved runtime, device,
latency, power, or thermal state, so those contract claims are `NOT_EVALUATED`
and the full-contract decision remains `INCONCLUSIVE`.

The current trusted computing base includes:

- approved contract, thresholds, dataset, labels, and caller-supplied expected
  digests;
- Python parser and Gate runner, CPython, NumPy, ONNX, ONNX Runtime, loaded
  native libraries, and their build/install process;
- Rust exact checker or Gate adjudicator, dependencies, compiler, linker, and
  executable selected by the consumer;
- Linux kernel, filesystem, process model, `/proc` profile observations,
  performance clock, CPU, microcode, and hardware;
- the authority that maps a profile ID to an approved physical system.

Untrusted or opaque by default:

- source and candidate models and transformation tools;
- model/training records, calibration data, datasets, and labels supplied by a
  producer;
- report transport when `authentication` is `none`;
- future TensorRT engines, plugins, generated kernels, JetPack, CUDA, drivers,
  firmware, preprocessing, postprocessing, application code, and devices;
- provenance or execution attestation that packages a semantically false claim.

Power House is outside the semantic trust core. It may bind or attest a CKODMK
execution, but cannot make the checker, importer, runtime, or claim correct.

## Assets and adversaries

Protected assets include artifact and policy identity, claim typing and scope,
raw observations and counterexamples, device binding, release decisions,
waiver authority, signing keys, and confidential customer models/data.

Failure sources include ordinary configuration mistakes, incorrect compiler or
runtime output, post-hoc threshold selection, malicious substitution or
resource exhaustion, compromised dependencies/host/build/signing keys,
timing/thermal/device drift, and defects in CKODMK itself.

## Implemented controls and residual risk

| Threat | Current control | Residual risk / required consequence |
|---|---|---|
| Source, candidate, dataset, or contract substitution | Caller-pinned contract digest; contract-pinned artifact digests; every input is rehashed; reports and adjudications bind the observed bytes. | An attacker controlling policy and all bytes can replace them together. Authentication and authorization are external. Mismatch must `BLOCK`. |
| Path swap, symlink, FIFO, or mutation during evaluation | Inputs are opened with `O_NOFOLLOW`, `O_NONBLOCK`, and `O_CLOEXEC`, required to be bounded regular files, copied once into a private directory, fsynced, then parsed and executed from those snapshots. Concurrent destination replacement is rejected in exact-bundle publication. | Filesystem, kernel, directory creation, and copy implementation remain trusted. No immutable-mount or hardware root of trust exists. |
| Ambiguous contract JSON | Size cap, UTF-8 parsing, duplicate-key and non-finite rejection, exact key sets, strict types, decimal strings, and bounded dimensions/work. | No signature canonicalization standard or cross-runtime signed envelope. Reject malformed input. |
| Ambiguous or malicious dataset container | Exactly two ordered stored ZIP members (`inputs.npy`, `labels.npy`), no compression, paths, comments, flags, extras, or alternate keys; canonical NumPy 2.3.5 round-trip; exact little-endian Float32 inputs and int64 labels; shape/count/finite checks and byte caps. Rust separately parses the canonical NPZ/NPY and labels. | Both implementations can share a specification error; ZIP/NumPy parsers remain attack surface. Dataset provenance and representativeness remain policy questions. |
| Malicious or incomplete ONNX | 64 MB artifact cap, regular-file snapshot, ONNX validation, single input/output profile, and external-tensor rejection. Parsing and inference occur in a child with no core dumps, 120 s CPU, 180 s wall, 3 GB address space, and 16 MB output/file limit. | No seccomp, namespace, cgroup, privilege drop, network isolation, or syscall allowlist. Native parser/runtime vulnerabilities can affect the host. Failures must `BLOCK`. |
| Dataset-dependent batching or dtype drift | Every row executes separately as batch one; input must be finite canonical Float32; output must be finite Float32 with fixed class count; `NC` layout and first-`argmax` are explicit. | Only one input/output classification profile is supported. Pre/postprocessing, dynamic state, multiple tensors, tokenizers, and application logic are outside scope. |
| Optimizer/quantizer semantic fault | Complete finite-set differential run records output bits, decisions, correctness, and maximum deviation. Required claim failure blocks. | No guarantee outside the 1,797 declared rows; no proof of compiler or kernel correctness. |
| Forged report metrics, labels, or decision | Rust adjudicator rehashes artifacts/report, parses dataset labels, and recomputes decisions, correctness, accuracy fractions, deviations, latency summaries, claims, and verdict from raw fields. Regression rejects self-consistent forged report labels. | It trusts the Python-produced raw output bits/timings and does not rerun ONNX. Two implementations are not a qualified external audit. |
| Float/decimal boundary rounding | Policy decimals are canonical strings; count ratios and nanosecond comparisons are evaluated exactly. Boundary regressions reject values that binary conversion could round into a pass. | Raw model outputs are Float32 runtime observations; ONNX kernels and their floating behavior remain opaque. |
| Device/runtime profile drift | Caller pins a static profile digest; lab compares actual host observations before work; Gate records before/after profile and returns `INCONCLUSIVE` for required runtime claims on drift. Profile binds vendor/product/board, CPU/model/count/microcode, kernel, governor, Python/NumPy/ONNX/ORT, and thread count. | Profile observations are operating-system assertions; no remote attestation. Package build identity, dynamic-library digests, clock frequency, temperature, power, and firmware are not fully bound. |
| Timing overclaim | Source and candidate use the same deterministic 500 indices; execution order alternates; raw integer nanoseconds retained; both medians and sample p95 values recomputed. Reports state finite-run scope. | Not a population-tail estimate, cold-start study, cross-device result, energy result, or performance superiority test. Host scheduling and clock are trusted. |
| Nondeterminism | CPU sequential execution, one thread, forced BLAS/thread environment, and a second bitwise run asserted by Python. | Rust does not independently demonstrate replay. Only two local executions under one runtime are covered. |
| Cherry-picked dataset or post-hoc policy | Contract, dataset digest, internal protocol, implementation sources, and profile are retained; protocol says it was chosen before candidate evaluation. | The protocol is team-controlled, not independently timestamped, preregistered, hidden, or blinded. External evaluation is required. |
| Report or waiver tampering/replay | Manifest digests and explicit `authentication: none`. External verifier uses caller-supplied manifest, adjudicator, and profile digests. | No signature, signer authorization, transparency log, expiry, revocation, nonce, or trusted time. Unsigned evidence must never be represented as authenticated. |
| Compromised dependency/build | Python requirements have hashes; Rust locks are present; final adjudicator binary is pinned in the lab. Source context and lock hashes are retained. | Dynamic loaded libraries are not fully attested; no qualified reproducible-build audit, signed release, independent vulnerability review, or externally verified SBOM is established. |
| Resource exhaustion in evidence verification | File/count/depth/aggregate limits, nonblocking regular-file reads, no symlinks, bounded JSON, subprocess limits, output caps, and timeouts. | Limits are platform-specific and do not prove denial-of-service resistance. Campaign/fuzz evidence is incomplete. |
| Customer IP or data leakage | CLI can run locally; no cloud or blockchain is required. | No complete access-control, encryption, isolation, telemetry, retention, secure-deletion, or incident-response policy. Do not process customer material without a separate approved control plane. |
| Training-history overclaim | Evidence labels recipe, history, and train accuracy as producer-reported. Gate recomputes source/candidate outcomes on the bound test file. | Training is not replayed; the record does not establish lineage from recipe to source weights. |

## Assurance invariants

1. Required failure implies `BLOCK`.
2. Required runtime evidence on a mismatched profile implies `INCONCLUSIVE`,
   never `PASS`.
3. Advisory claims cannot change the overall decision.
4. Artifact identity cannot become semantic evidence.
5. Finite-set evidence cannot become population or universal evidence.
6. Graph/runtime evidence cannot become installed-device evidence without a
   separately valid edge.
7. A waiver remains risk acceptance, not a successful technical test.
8. Trust, authentication state, opaque components, and unsupported boundaries
   remain explicit.
9. Evaluation errors fail closed; partial output cannot authorize release.
10. Consumer verification requires the exact expected checker/policy identity,
    not a self-described decision inside a bundle.

## Production blockers

The following prohibit a production or customer-facing assurance claim:

- any unresolved supported false acceptance or high-severity checker/evidence
  integrity defect;
- missing source, candidate, dataset, contract, runtime, or deployed-package
  binding;
- measured evidence presented as mathematical or universal equivalence;
- unsigned evidence presented as authenticated;
- absent preprocessing/postprocessing coverage for an application claim;
- absence of hardened native-code isolation for adversarial customer inputs;
- publication of Jetson, accelerator, power, thermal, or phone claims before
  actual protocol-complete execution and external reproduction;
- use of customer data without documented authorization and controls.

The V3 internal campaign is executed and unblinded. Its frozen 130-case corpus
produced zero false `PASS` decisions on 120 selected target faults, 9/9 valid
control passes, and no CKODMK abnormal result. The exact two-sided 95% upper
bound for 0/120 false passes is 3.027%; the 0/9 false-block upper bound is a weak
33.627%. This evidence is internal, agent-separated, selected rather than IID,
and far below the rubric's non-author-controlled 1,000-case gate. External
audit, external reproduction, and customer evidence remain absent.

## Target-device requirements

Before a Jetson Orin NX 16GB claim, retain the exact module/carrier/storage,
secure-boot state, JetPack/L4T, kernel, CUDA, cuDNN, TensorRT, drivers, firmware,
engine/plugins/calibration, build configuration, application package,
pre/postprocessing, power mode, clocks, cooling, ambient temperature, warmups,
input schedule, raw telemetry, operator fallback/partition data, and independent
replay. Product specifications are not benchmark evidence.

Relevant external frameworks include [NIST AI RMF Measure][nist], [SLSA
provenance][slsa], [RFC 8785][rfc8785], and [NVIDIA TensorRT/Polygraphy][trt].
They inform controls; CKODMK claims no certification or conformance.

[nist]: https://airc.nist.gov/airmf-resources/airmf/5-sec-core/
[slsa]: https://slsa.dev/spec/v1.2/provenance
[rfc8785]: https://datatracker.ietf.org/doc/html/rfc8785
[trt]: https://docs.nvidia.com/deeplearning/tensorrt/latest/inference-library/capabilities.html
