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

SEEDED_MAGIC = b"PHSPv1\0\0"
POLYNOMIAL_MAGIC = b"PHSMv1\0\0"
COMMITTED_MAGIC = b"PHCPv1\0\0"
POLYNOMIAL_DOMAIN = b"power_house:v1:seeded-sparse-polynomial"
COMMITTED_POLYNOMIAL_DOMAIN = b"power_house:v1:committed-sparse-polynomial"
TRANSCRIPT_DOMAIN = b"power_house:v1:sparse-sumcheck-transcript"
CHALLENGE_DOMAIN = b"power_house:v1:sparse-sumcheck-challenge"
RESPONSE_DOMAIN = b"power_house:v1:sparse-sumcheck-response"
PRNG_DOMAIN = b"JROC_PRNG"
MAX_DECODED_VARIABLES = 16_000_000
MAX_DECODED_TERMS = 1_000_000
MAX_DECODED_DEGREE = 1_000_000
MAX_DECODED_SEED_BYTES = 1_048_576
MAX_DECODED_INCIDENCES = 64_000_000


class CertificateError(ValueError):
    pass


def is_prime_u64(value: int) -> bool:
    if value < 2:
        return False
    for prime in (2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37):
        if value == prime:
            return True
        if value % prime == 0:
            return False

    odd_part = value - 1
    shifts = 0
    while odd_part % 2 == 0:
        shifts += 1
        odd_part //= 2

    for base in (2, 325, 9_375, 28_178, 450_775, 9_780_504, 1_795_265_022):
        base %= value
        if base == 0:
            continue
        witness = pow(base, odd_part, value)
        if witness in (1, value - 1):
            continue
        for _ in range(1, shifts):
            witness = witness * witness % value
            if witness == value - 1:
                break
        else:
            return False
    return True


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


def committed_polynomial(path: Path) -> tuple[int, list[Term], bytes]:
    data = path.read_bytes()
    reader = Reader(data)
    if reader.take(8) != POLYNOMIAL_MAGIC:
        raise CertificateError("bad polynomial magic")
    num_vars = reader.u64()
    num_terms = reader.u64()
    if (
        num_vars == 0
        or num_vars > MAX_DECODED_VARIABLES
        or num_terms == 0
        or num_terms > MAX_DECODED_TERMS
    ):
        raise CertificateError("polynomial dimensions exceed decoder limits")
    if num_terms > (len(data) - reader.offset) // 24:
        raise CertificateError("polynomial term count exceeds input size")
    terms: list[Term] = []
    total_incidences = 0
    for _ in range(num_terms):
        coefficient = reader.u64()
        degree = reader.u64()
        if (
            degree == 0
            or degree > num_vars
            or degree > MAX_DECODED_DEGREE
            or degree > (len(data) - reader.offset) // 8
        ):
            raise CertificateError("polynomial degree exceeds input or domain size")
        total_incidences += degree
        if total_incidences > MAX_DECODED_INCIDENCES:
            raise CertificateError("polynomial incidence count exceeds decoder limit")
        variables = [reader.u64() for _ in range(degree)]
        if coefficient == 0:
            raise CertificateError("invalid polynomial term")
        if variables != sorted(set(variables)):
            raise CertificateError("polynomial variables are not canonical")
        if variables[-1] >= num_vars:
            raise CertificateError("polynomial variable outside domain")
        terms.append(Term(coefficient, variables))
    if reader.offset != len(data):
        raise CertificateError("trailing polynomial bytes")
    return num_vars, terms, data


def committed_digest(data: bytes) -> bytes:
    hasher = hashlib.blake2b(digest_size=32)
    absorb_bytes(hasher, COMMITTED_POLYNOMIAL_DOMAIN)
    absorb_bytes(hasher, data)
    return hasher.digest()


def replay(
    reader: Reader,
    *,
    p: int,
    num_vars: int,
    num_terms: int,
    max_degree: int,
    stored_claimed_sum: int,
    polynomial_digest: bytes,
    terms: list[Term],
    round_count: int,
) -> dict[str, int | str]:
    if (
        p < 3
        or p % 2 == 0
        or not is_prime_u64(p)
        or num_vars == 0
        or num_terms == 0
        or max_degree == 0
        or max_degree > num_vars
        or round_count != num_vars
        or len(terms) != num_terms
    ):
        raise CertificateError("invalid certificate parameters")

    events: list[tuple[int, int]] = []
    states: list[TermState] = []
    claimed_sum = 0
    for term_id, term in enumerate(terms):
        coefficient = term.coefficient % p
        if coefficient == 0:
            raise CertificateError("polynomial coefficient is zero in field")
        contribution = coefficient * pow(2, num_vars - len(term.variables), p) % p
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


def verify_seeded(data: bytes) -> dict[str, int | str]:
    reader = Reader(data)
    if reader.take(8) != SEEDED_MAGIC:
        raise CertificateError("bad seeded certificate magic")
    p = reader.u64()
    num_vars = reader.u64()
    num_terms = reader.u64()
    max_degree = reader.u64()
    seed_length = reader.u64()
    if seed_length > MAX_DECODED_SEED_BYTES:
        raise CertificateError("seed length exceeds decoder limit")
    seed = reader.take(seed_length)
    stored_claimed_sum = reader.u64()
    stored_polynomial_digest = reader.take(32)
    round_count = reader.u64()
    if (
        num_vars == 0
        or num_vars > MAX_DECODED_VARIABLES
        or num_terms == 0
        or num_terms > MAX_DECODED_TERMS
        or max_degree == 0
        or max_degree > num_vars
        or max_degree > MAX_DECODED_DEGREE
        or num_terms * max_degree > MAX_DECODED_INCIDENCES
        or round_count != num_vars
        or round_count > (len(data) - reader.offset - 40) // 16
    ):
        raise CertificateError("round count exceeds input size")

    terms = derive_terms(p, num_vars, num_terms, max_degree, seed)
    polynomial_digest = digest_terms(
        num_vars, num_terms, max_degree, seed, terms
    )
    if polynomial_digest != stored_polynomial_digest:
        raise CertificateError("polynomial digest mismatch")

    return replay(
        reader,
        p=p,
        num_vars=num_vars,
        num_terms=num_terms,
        max_degree=max_degree,
        stored_claimed_sum=stored_claimed_sum,
        polynomial_digest=polynomial_digest,
        terms=terms,
        round_count=round_count,
    )


def verify_committed(data: bytes, polynomial_path: Path) -> dict[str, int | str]:
    reader = Reader(data)
    if reader.take(8) != COMMITTED_MAGIC:
        raise CertificateError("bad committed certificate magic")
    p = reader.u64()
    num_vars = reader.u64()
    num_terms = reader.u64()
    max_degree = reader.u64()
    stored_commitment = reader.take(32)
    stored_claimed_sum = reader.u64()
    round_count = reader.u64()
    if (
        num_vars == 0
        or num_vars > MAX_DECODED_VARIABLES
        or num_terms == 0
        or num_terms > MAX_DECODED_TERMS
        or max_degree == 0
        or max_degree > num_vars
        or max_degree > MAX_DECODED_DEGREE
        or num_terms * max_degree > MAX_DECODED_INCIDENCES
        or round_count != num_vars
        or round_count > (len(data) - reader.offset - 40) // 16
    ):
        raise CertificateError("committed round count exceeds input size")

    polynomial_num_vars, terms, polynomial_bytes = committed_polynomial(
        polynomial_path
    )
    if polynomial_num_vars != num_vars or len(terms) != num_terms:
        raise CertificateError("committed polynomial metadata mismatch")
    actual_max_degree = max(len(term.variables) for term in terms)
    if actual_max_degree != max_degree:
        raise CertificateError("committed polynomial degree mismatch")
    commitment = committed_digest(polynomial_bytes)
    if commitment != stored_commitment:
        raise CertificateError("committed polynomial digest mismatch")

    return replay(
        reader,
        p=p,
        num_vars=num_vars,
        num_terms=num_terms,
        max_degree=max_degree,
        stored_claimed_sum=stored_claimed_sum,
        polynomial_digest=commitment,
        terms=terms,
        round_count=round_count,
    )


def verify(path: Path, polynomial_path: Path | None) -> dict[str, int | str]:
    data = path.read_bytes()
    magic = data[:8]
    if magic == SEEDED_MAGIC:
        if polynomial_path is not None:
            raise CertificateError("seeded certificate does not use --polynomial")
        return verify_seeded(data)
    if magic == COMMITTED_MAGIC:
        if polynomial_path is None:
            raise CertificateError("committed certificate requires --polynomial")
        return verify_committed(data, polynomial_path)
    raise CertificateError("unknown certificate magic")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("certificate", type=Path)
    parser.add_argument("--polynomial", type=Path)
    args = parser.parse_args()
    try:
        print(
            json.dumps(
                verify(args.certificate, args.polynomial),
                indent=2,
                sort_keys=True,
            )
        )
    except (CertificateError, OSError) as error:
        print(f"verification failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
