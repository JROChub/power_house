#!/usr/bin/env python3
"""Independent standard-library verifier for Power-House sparse certificates."""

from __future__ import annotations

import argparse
import hashlib
import json
import struct
import sys
from dataclasses import dataclass
from pathlib import Path

MAGIC = b"PHSPv1\0\0"
POLYNOMIAL_DOMAIN = b"power_house:v1:seeded-sparse-polynomial"
TRANSCRIPT_DOMAIN = b"power_house:v1:sparse-sumcheck-transcript"
CHALLENGE_DOMAIN = b"power_house:v1:sparse-sumcheck-challenge"
RESPONSE_DOMAIN = b"power_house:v1:sparse-sumcheck-response"
PRNG_DOMAIN = b"JROC_PRNG"


class CertificateError(ValueError):
    pass


class Reader:
    def __init__(self, data: bytes) -> None:
        self.data = data
        self.offset = 0

    def take(self, count: int) -> bytes:
        end = self.offset + count
        if end > len(self.data):
            raise CertificateError("unexpected end of certificate")
        chunk = self.data[self.offset:end]
        self.offset = end
        return chunk

    def u64(self) -> int:
        return struct.unpack(">Q", self.take(8))[0]


class SimplePrng:
    def __init__(self, seed: bytes) -> None:
        if len(seed) != 32:
            raise ValueError("PRNG seed must be 32 bytes")
        self.seed = seed
        self.counter = 0
        self.buffer = b""
        self.offset = 32

    def refill(self) -> None:
        hasher = hashlib.blake2b(digest_size=32)
        hasher.update(PRNG_DOMAIN)
        hasher.update(self.seed)
        hasher.update(self.counter.to_bytes(8, "big"))
        self.buffer = hasher.digest()
        self.counter = (self.counter + 1) & ((1 << 64) - 1)
        self.offset = 0

    def next_u64(self) -> int:
        if self.offset >= 32:
            self.refill()
        value = int.from_bytes(self.buffer[self.offset : self.offset + 8], "big")
        self.offset += 8
        return value

    def gen_mod(self, modulus: int) -> int:
        return self.next_u64() % modulus


@dataclass
class Term:
    coefficient: int
    variables: list[int]


@dataclass
class TermState:
    contribution: int
    next_round: int


def absorb_bytes(hasher: hashlib._Hash, value: bytes) -> None:
    hasher.update(len(value).to_bytes(8, "big"))
    hasher.update(value)


def polynomial_seed(
    num_vars: int, num_terms: int, max_degree: int, seed: bytes
) -> bytes:
    hasher = hashlib.blake2b(digest_size=32)
    absorb_bytes(hasher, POLYNOMIAL_DOMAIN)
    hasher.update(num_vars.to_bytes(8, "big"))
    hasher.update(num_terms.to_bytes(8, "big"))
    hasher.update(max_degree.to_bytes(8, "big"))
    absorb_bytes(hasher, seed)
    return hasher.digest()


def derive_terms(
    p: int, num_vars: int, num_terms: int, max_degree: int, seed: bytes
) -> list[Term]:
    prng = SimplePrng(polynomial_seed(num_vars, num_terms, max_degree, seed))
    terms: list[Term] = []
    for _ in range(num_terms):
        degree = 1 if max_degree == 1 else 2 + prng.gen_mod(max_degree - 1)
        coefficient = 1 + prng.gen_mod(p - 1)
        variables: list[int] = []
        while len(variables) < degree:
            candidate = prng.gen_mod(num_vars)
            if candidate not in variables:
                variables.append(candidate)
        variables.sort()
        terms.append(Term(coefficient, variables))
    return terms


def digest_terms(
    num_vars: int,
    num_terms: int,
    max_degree: int,
    seed: bytes,
    terms: list[Term],
) -> bytes:
    hasher = hashlib.blake2b(digest_size=32)
    absorb_bytes(hasher, POLYNOMIAL_DOMAIN)
    hasher.update(num_vars.to_bytes(8, "big"))
    hasher.update(num_terms.to_bytes(8, "big"))
    hasher.update(max_degree.to_bytes(8, "big"))
    absorb_bytes(hasher, seed)
    for term in terms:
        hasher.update(term.coefficient.to_bytes(8, "big"))
        hasher.update(len(term.variables).to_bytes(8, "big"))
        for variable in term.variables:
            hasher.update(variable.to_bytes(8, "big"))
    return hasher.digest()


def initial_transcript(
    p: int,
    num_vars: int,
    num_terms: int,
    max_degree: int,
    claimed_sum: int,
    polynomial_digest: bytes,
) -> bytes:
    hasher = hashlib.blake2b(digest_size=32)
    absorb_bytes(hasher, TRANSCRIPT_DOMAIN)
    hasher.update(p.to_bytes(8, "big"))
    hasher.update(num_vars.to_bytes(8, "big"))
    hasher.update(num_terms.to_bytes(8, "big"))
    hasher.update(max_degree.to_bytes(8, "big"))
    hasher.update(claimed_sum.to_bytes(8, "big"))
    hasher.update(polynomial_digest)
    return hasher.digest()


def transcript_round(
    state: bytes, counter: int, a: int, b: int, p: int
) -> tuple[bytes, int]:
    challenge_hasher = hashlib.blake2b(digest_size=32)
    absorb_bytes(challenge_hasher, CHALLENGE_DOMAIN)
    challenge_hasher.update(state)
    challenge_hasher.update(a.to_bytes(8, "big"))
    challenge_hasher.update(b.to_bytes(8, "big"))
    challenge_hasher.update(counter.to_bytes(8, "big"))
    challenge_digest = challenge_hasher.digest()

    challenge = 0
    for byte in challenge_digest:
        challenge = (challenge * 256 + byte) % p

    response_hasher = hashlib.blake2b(digest_size=32)
    absorb_bytes(response_hasher, RESPONSE_DOMAIN)
    response_hasher.update(challenge_digest)
    response_hasher.update(challenge.to_bytes(8, "big"))
    return response_hasher.digest(), challenge


def verify(path: Path) -> dict[str, int | str]:
    reader = Reader(path.read_bytes())
    if reader.take(8) != MAGIC:
        raise CertificateError("bad certificate magic")
    p = reader.u64()
    num_vars = reader.u64()
    num_terms = reader.u64()
    max_degree = reader.u64()
    seed = reader.take(reader.u64())
    stored_claimed_sum = reader.u64()
    stored_polynomial_digest = reader.take(32)
    round_count = reader.u64()

    if (
        p < 3
        or p % 2 == 0
        or num_vars == 0
        or num_terms == 0
        or max_degree == 0
        or max_degree > num_vars
        or round_count != num_vars
    ):
        raise CertificateError("invalid certificate parameters")

    terms = derive_terms(p, num_vars, num_terms, max_degree, seed)
    polynomial_digest = digest_terms(
        num_vars, num_terms, max_degree, seed, terms
    )
    if polynomial_digest != stored_polynomial_digest:
        raise CertificateError("polynomial digest mismatch")

    events: list[tuple[int, int]] = []
    states: list[TermState] = []
    claimed_sum = 0
    for term_id, term in enumerate(terms):
        contribution = term.coefficient * pow(2, num_vars - len(term.variables), p) % p
        claimed_sum = (claimed_sum + contribution) % p
        states.append(TermState(contribution, 0))
        events.extend((variable, term_id) for variable in term.variables)
    events.sort()
    if claimed_sum != stored_claimed_sum:
        raise CertificateError("claimed sum mismatch")

    state = initial_transcript(
        p,
        num_vars,
        num_terms,
        max_degree,
        claimed_sum,
        polynomial_digest,
    )
    inv_two = pow(2, p - 2, p)
    inverse_two_powers = [1]
    running_claim = claimed_sum
    event_cursor = 0

    for round_idx in range(num_vars):
        active: list[tuple[int, int]] = []
        a = 0
        while event_cursor < len(events) and events[event_cursor][0] == round_idx:
            term_id = events[event_cursor][1]
            term_state = states[term_id]
            gap = round_idx - term_state.next_round
            while len(inverse_two_powers) <= gap:
                inverse_two_powers.append(inverse_two_powers[-1] * inv_two % p)
            current = term_state.contribution * inverse_two_powers[gap] % p
            a = (a + current) % p
            active.append((term_id, current))
            event_cursor += 1

        b = (running_claim - a) % p * inv_two % p
        stored_a = reader.u64()
        stored_b = reader.u64()
        if stored_a != a or stored_b != b:
            raise CertificateError(f"round {round_idx} mismatch")

        state, challenge = transcript_round(state, round_idx, a, b, p)
        for term_id, current in active:
            states[term_id] = TermState(current * challenge % p, round_idx + 1)
        running_claim = (b + a * challenge) % p

    final_evaluation = 0
    for term_state in states:
        tail = num_vars - term_state.next_round
        while len(inverse_two_powers) <= tail:
            inverse_two_powers.append(inverse_two_powers[-1] * inv_two % p)
        final_evaluation = (
            final_evaluation
            + term_state.contribution * inverse_two_powers[tail]
        ) % p

    stored_final_evaluation = reader.u64()
    stored_transcript_digest = reader.take(32)
    if reader.offset != len(reader.data):
        raise CertificateError("trailing certificate bytes")
    if final_evaluation != running_claim:
        raise CertificateError("internal final evaluation mismatch")
    if stored_final_evaluation != final_evaluation:
        raise CertificateError("stored final evaluation mismatch")
    if stored_transcript_digest != state:
        raise CertificateError("transcript digest mismatch")

    return {
        "status": "verified",
        "field_modulus": p,
        "domain_variables": num_vars,
        "sparse_terms": num_terms,
        "maximum_term_degree": max_degree,
        "term_incidences": len(events),
        "rounds_verified": round_count,
        "final_evaluation": final_evaluation,
        "polynomial_digest": polynomial_digest.hex(),
        "transcript_digest": state.hex(),
        "certificate_bytes": len(reader.data),
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("certificate", type=Path)
    args = parser.parse_args()
    try:
        print(json.dumps(verify(args.certificate), indent=2, sort_keys=True))
    except (CertificateError, OSError) as error:
        print(f"verification failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
