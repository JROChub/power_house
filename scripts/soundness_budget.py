#!/usr/bin/env python3
"""Estimate the classical interactive soundness budget for sum-check."""

from __future__ import annotations

import argparse
import json
import math


def estimate(
    rounds: int,
    field_size: int,
    degree: int,
    repetitions: int,
    challenge_bits: int,
) -> dict[str, float | int | str]:
    if min(rounds, field_size, degree, repetitions, challenge_bits) <= 0:
        raise ValueError("all parameters must be positive")

    challenge_space = 2**challenge_bits
    max_preimages = (challenge_space + field_size - 1) // field_size
    max_challenge_probability = max_preimages / challenge_space
    single_error = min(1.0, rounds * degree * max_challenge_probability)
    repeated_error = single_error**repetitions
    bits = math.inf if repeated_error == 0 else -math.log2(repeated_error)
    return {
        "model": "classical interactive sum-check union bound",
        "rounds": rounds,
        "field_size": field_size,
        "individual_degree": degree,
        "repetitions": repetitions,
        "challenge_digest_bits": challenge_bits,
        "single_repetition_error_upper_bound": single_error,
        "combined_error_upper_bound": repeated_error,
        "soundness_bits_lower_bound": bits,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--rounds", type=int, default=1_000_000)
    parser.add_argument("--field", type=int, default=1_000_000_007)
    parser.add_argument("--degree", type=int, default=1)
    parser.add_argument("--repetitions", type=int, default=1)
    parser.add_argument("--challenge-bits", type=int, default=256)
    args = parser.parse_args()
    try:
        report = estimate(
            args.rounds,
            args.field,
            args.degree,
            args.repetitions,
            args.challenge_bits,
        )
    except ValueError as error:
        parser.error(str(error))
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
