# Hyperscale Seeded-Affine Proof

Status: active proof profile for Power House v0.3.4.

`examples/hyperscale_affine.rs` demonstrates a non-constant sum-check
certificate over domains far larger than sextillion scale.

The default run proves and verifies a seeded affine multilinear polynomial over
`2^4096` Boolean points. That is roughly `1e1233` evaluations. The verifier
does not enumerate the domain; it replays 4096 linear sum-check rounds.

## Public Polynomial

The polynomial is:

```text
f(x_0, ..., x_{n-1}) = c + a_0*x_0 + ... + a_{n-1}*x_{n-1}
```

`c` and the coefficients `a_i` are derived deterministically from:

- the public seed,
- the field modulus,
- the number of variables,
- the domain tag `power_house:v1:seeded-affine`.

The seed is not a secret. It is the compact public description of the
structured computation being verified.

## Claimed Sum

For an affine multilinear polynomial over `{0,1}^n`:

```text
sum_x f(x) = 2^n*c + 2^(n-1)*sum_i a_i
```

All arithmetic is performed modulo the finite field.

## Verifier Work

The verifier checks:

1. the claimed sum matches the public seed-derived coefficients,
2. each round polynomial satisfies `g_i(0) + g_i(1) = running_claim`,
3. each round polynomial matches the affine closed form after previous
   Fiat-Shamir challenges are fixed,
4. the final folded value equals the affine evaluation at the verifier's
   challenge point.

This is an `O(n)` certificate for this structured family. It is not claiming to
verify an arbitrary `2^4096`-entry table without a commitment or polynomial
oracle. The point is precise: once the computation has a compact algebraic
description, `power_house` can verify claims over domains that are physically
impossible to enumerate.

## Run

```bash
cargo run --example hyperscale_affine
```

For a faster CI-sized run:

```bash
cargo run --example hyperscale_affine -- 1024
```
