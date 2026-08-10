# CKODMK second transformation-toolchain results

This table is generated from the retained machine-readable lab. ONNXScript is an
external candidate producer and has no authority to issue a CKODMK decision.
The evaluation was executed by the CKODMK team and is not external reproduction.

- Lab manifest SHA-256: `e8ddce2d7a9a3a0b5a0dbe8afa31f6cac8eed914698ced1cca4b4ba7a257d978`
- Toolchain CycloneDX SBOM SHA-256: `b0c7e88dd77bdb2621d571722eb1180e6ce46926b171be0aaafd2cfa78270ec5`
- Producer: ONNXScript 0.7.1
- Built-in rule: `matmul_add_to_gemm_rule`
- Scope: six trained Optdigits models, two architecture families, complete 1,797-row test set

| Model | Family | Fusions | Bytes source/candidate | Size ratio | Decision changes | Max L∞ | Gate | Rust | NumPy profile | Polygraphy |
|---|---|---:|---:|---:|---:|---:|---|---|---|---|
| optdigits-mlp-w16-s1701 | multilayer-perceptron | 2 | 5,253 / 5,465 | 0.961208x | 0 / 1,797 | 0 | PASS | PASS | PASS | PASS |
| optdigits-mlp-w32-s2603 | multilayer-perceptron | 2 | 10,055 / 10,267 | 0.979351x | 0 / 1,797 | 0 | PASS | PASS | PASS | PASS |
| optdigits-mlp-w64-s3907 | multilayer-perceptron | 2 | 19,658 / 19,870 | 0.989331x | 0 / 1,797 | 0 | PASS | PASS | PASS | PASS |
| optdigits-rbf-c40-s1140 | radial-basis-function-network | 1 | 12,522 / 12,994 | 0.963676x | 0 / 1,797 | 0 | PASS | PASS | BLOCK (unsupported profile) | PASS |
| optdigits-rbf-c80-s2180 | radial-basis-function-network | 1 | 24,365 / 24,837 | 0.980996x | 0 / 1,797 | 0 | PASS | PASS | BLOCK (unsupported profile) | PASS |
| optdigits-rbf-c160-s3160 | radial-basis-function-network | 1 | 48,047 / 48,527 | 0.990109x | 0 / 1,797 | 0 | PASS | PASS | BLOCK (unsupported profile) | PASS |

## Result

All six ONNXScript candidates passed the complete finite-set CKODMK Gate,
the Rust evidence adjudicator, and the matched Polygraphy comparison. The
three MLP candidates also passed the separately implemented NumPy execution
profile. That NumPy profile rejects `Unsqueeze`, so all three RBF attempts are
retained as `BLOCK (unsupported profile)` rather than omitted or relabeled.

The rewrite increased serialized size on every model. This result establishes
second-toolchain compatibility for the tested scope; it does not establish a
size, latency, energy, or quality improvement.

## Reproduction

Install the hash-locked toolchain environment and verify the retained lab:

```bash
python -m pip install --require-hashes -r requirements-toolchains.lock
PYTHONPATH=src:. python scripts/verify_onnxscript_toolchain_lab.py \
  --lab artifacts/onnxscript-toolchain-lab-v1 \
  --manifest-sha256 sha256:e8ddce2d7a9a3a0b5a0dbe8afa31f6cac8eed914698ced1cca4b4ba7a257d978 \
  --adjudicator artifacts/real-model-lab-v1/tools/ckodmk-gate-adjudicate \
  --adjudicator-sha256 sha256:58c0cd14fc5d5e96f0a4f1e98ff3a225362de34e5c5a261b7c795bac3ec14447 \
  --device-profile-sha256 sha256:ec06ffd189da132045cbcafd160e692f9d4901f732ff95baf2d9ebf0547063f9
```
