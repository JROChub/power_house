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
/// arithmetic operations over the integers modulo `p`.  It does not
/// perform primality testing; it is the user's responsibility to
/// supply an odd prime.  If `p` is not prime, the multiplicative
/// inverse operation will panic when called with a non-unit element.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Field {
    p: u64,
}

impl Field {
    /// Creates a new finite field with the given modulus.
    ///
    /// # Panics
    ///
    /// Panics if the modulus is less than 3 or even.  Only odd primes are
    /// supported.
    pub fn new(p: u64) -> Self {
        assert!(p >= 3 && p % 2 == 1, "p must be an odd prime >= 3");
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
        let mut s = (a % self.p) + (b % self.p);
        if s >= self.p {
            s -= self.p;
        }
        s
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
