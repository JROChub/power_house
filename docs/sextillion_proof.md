# Sextillion-Scale Verification Certificate

Status: active proof profile for Power House v0.3.21.

Power-House can verify a sum-check claim over a domain larger than one
sextillion points without enumerating the domain.

The reproducible certificate uses a constant multilinear polynomial over
`2^70` Boolean points:

```text
2^70 = 1,180,591,620,717,411,303,424
```

That domain is larger than `10^21` points. The proof has one round per
variable, so the verifier replays `70` algebraic checks and one final
evaluation instead of reading every point.

## Reproduce

```bash
cargo run --example sextillion_verify
```

Expected invariants:

- `domain_exceeds_sextillion: true`
- `proof_rounds: 70`
- `verifier_replayed_rounds: 70`
- `final_evaluation: 173`

## What Is Proven

For the constant polynomial `f(x_1, ..., x_70) = 173` over
`F_1000000007`, the claimed sum is:

```text
sum_{x in {0,1}^70} f(x) = 173 * 2^70 mod 1,000,000,007
```

Each sum-check round sends a linear polynomial `g_i(z) = b_i`, where:

```text
b_i = 173 * 2^(remaining_variables - 1) mod 1,000,000,007
```

The verifier checks:

```text
g_i(0) + g_i(1) == running_claim
```

and then folds the running claim to `g_i(r_i)` using deterministic
Fiat-Shamir challenges. Because the polynomial is constant, each folded value
is independent of the challenge and the final evaluation is exactly `173`.

This certificate proves the scale claim honestly: the domain is sextillion
scale, while the proof and verification work are logarithmic in the number of
points for this polynomial family.

For a non-constant structured computation far beyond sextillion scale, see
[Hyperscale Seeded-Affine Proof](hyperscale_proof.md) and
`cargo run --example hyperscale_affine`.
