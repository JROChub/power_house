//! The design philosophy underlying `power_house` is pedagogical, yet mathematically rigorous.
//! Each module encapsulates a discrete concept in modern computational complexity theory,
//! illustrating how modest abstractions compose into a cohesive proof infrastructure.
//!
//! This crate aspires to bridge gaps between theoretical exposition and practical engineering,
//! serving both as a didactic resource and a foundation for future cryptographic research.
//! Finite field arithmetic.
//!
//! This module provides a simple implementation of arithmetic in a prime
//! field.  The [`Field`](struct.Field.html) type encapsulates a prime
//! modulus and exposes methods for addition, subtraction, multiplication,
//! exponentiation and inversion.  All operations reduce their results
//! modulo the field modulus.

/// A finite field defined by an odd prime modulus.
///
/// The `Field` type stores the modulus `p` and provides elementary
/// arithmetic operations over the integers modulo `p`. Construction performs
/// deterministic Miller-Rabin primality testing for the full `u64` range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Field {
    p: u64,
}

impl Field {
    /// Creates a new finite field with the given modulus.
    ///
    /// # Panics
    ///
    /// Panics if the modulus is not an odd prime.
    pub fn new(p: u64) -> Self {
        assert!(
            p >= 3 && p % 2 == 1 && is_prime_u64(p),
            "p must be an odd prime >= 3"
        );
        Field { p }
    }

    /// Returns the modulus of the field.
    #[inline]
    pub fn modulus(&self) -> u64 {
        self.p
    }

    /// Adds two field elements.
    #[inline]
    pub fn add(&self, a: u64, b: u64) -> u64 {
        (((a % self.p) as u128 + (b % self.p) as u128) % self.p as u128) as u64
    }

    /// Subtracts `b` from `a`.
    #[inline]
    pub fn sub(&self, a: u64, b: u64) -> u64 {
        let a = a % self.p;
        let b = b % self.p;
        if a >= b {
            a - b
        } else {
            self.p - (b - a)
        }
    }

    /// Multiplies two field elements.
    #[inline]
    pub fn mul(&self, a: u64, b: u64) -> u64 {
        let a = a % self.p;
        let b = b % self.p;
        ((a as u128 * b as u128) % self.p as u128) as u64
    }

    /// Computes the multiplicative inverse of `a`.
    ///
    /// # Panics
    ///
    /// Panics if `a` is zero modulo `p`.  In a prime field, every non-zero
    /// element has a unique inverse.
    #[inline]
    pub fn inv(&self, a: u64) -> u64 {
        let a = a % self.p;
        assert!(a != 0, "cannot invert zero");
        // Use Fermat's little theorem: a^(p-2) mod p
        self.pow(a, self.p - 2)
    }

    /// Divides `a` by `b`.
    #[inline]
    pub fn div(&self, a: u64, b: u64) -> u64 {
        self.mul(a, self.inv(b))
    }

    /// Exponentiates `a` by `e` modulo `p`.
    #[inline]
    pub fn pow(&self, mut a: u64, mut e: u64) -> u64 {
        a %= self.p;
        let mut result = 1u64;
        while e > 0 {
            if e & 1 == 1 {
                result = self.mul(result, a);
            }
            a = self.mul(a, a);
            e >>= 1;
        }
        result
    }
}

fn is_prime_u64(value: u64) -> bool {
    if value < 2 {
        return false;
    }
    for prime in [2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37] {
        if value == prime {
            return true;
        }
        if value.is_multiple_of(prime) {
            return false;
        }
    }

    let exponent = value - 1;
    let shifts = exponent.trailing_zeros();
    let odd_part = exponent >> shifts;
    const BASES: [u64; 7] = [2, 325, 9_375, 28_178, 450_775, 9_780_504, 1_795_265_022];

    BASES.into_iter().all(|base| {
        let base = base % value;
        if base == 0 {
            return true;
        }
        let mut witness = mod_pow(base, odd_part, value);
        if witness == 1 || witness == value - 1 {
            return true;
        }
        for _ in 1..shifts {
            witness = mod_mul(witness, witness, value);
            if witness == value - 1 {
                return true;
            }
        }
        false
    })
}

fn mod_mul(left: u64, right: u64, modulus: u64) -> u64 {
    ((left as u128 * right as u128) % modulus as u128) as u64
}

fn mod_pow(mut base: u64, mut exponent: u64, modulus: u64) -> u64 {
    let mut result = 1u64;
    while exponent > 0 {
        if exponent & 1 == 1 {
            result = mod_mul(result, base, modulus);
        }
        base = mod_mul(base, base, modulus);
        exponent >>= 1;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::Field;

    #[test]
    fn rejects_composites_and_carmichael_numbers() {
        for composite in [0, 1, 2, 4, 9, 15, 21, 341, 561, 1_105, 1_729, 3_215_031_751] {
            assert!(
                std::panic::catch_unwind(|| Field::new(composite)).is_err(),
                "{composite} must be rejected"
            );
        }
    }

    #[test]
    fn supports_arithmetic_near_the_u64_limit() {
        let field = Field::new(18_446_744_073_709_551_557);
        assert_eq!(
            field.add(field.modulus() - 1, field.modulus() - 1),
            field.modulus() - 2
        );
        assert_eq!(field.mul(field.modulus() - 1, field.modulus() - 1), 1);
        assert_eq!(field.mul(7, field.inv(7)), 1);
    }
}
