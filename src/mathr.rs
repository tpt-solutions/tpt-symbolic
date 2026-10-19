//! Exact number type: a local stand-in for `tpt-mathr`.
//!
//! Exact rationals over arbitrary-precision integers. Arithmetic never rounds
//! and never overflows. `None` is returned only for division by zero, and for
//! powers whose exponent exceeds [`MAX_POW_EXPONENT`]. Replacing this module
//! with the real `tpt-mathr` means implementing [`ExactNumber`] for its type.

use crate::bigint::BigInt;
use core::fmt;

/// Largest exponent [`Number::checked_pow`] will expand.
pub const MAX_POW_EXPONENT: u64 = 1 << 16;

/// An exact rational `num / den`, stored in lowest terms with `den > 0`. Zero is `0/1`.
///
/// The derived `Ord` is a structural order used for deterministic sorting of
/// canonical expressions. It is not the numeric order.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Number {
    num: BigInt,
    den: BigInt,
}

impl Number {
    /// Exact zero.
    pub fn zero() -> Number {
        Number { num: BigInt::zero(), den: BigInt::one() }
    }

    /// Exact one.
    pub fn one() -> Number {
        Number { num: BigInt::one(), den: BigInt::one() }
    }

    /// Exact negative one.
    pub fn neg_one() -> Number {
        Number::from_int(-1)
    }

    /// Embeds an integer.
    pub fn from_int(n: i128) -> Number {
        Number { num: BigInt::from_i128(n), den: BigInt::one() }
    }

    /// Builds `num / den` in lowest terms. Returns `None` when `den == 0`.
    pub fn new(num: i128, den: i128) -> Option<Number> {
        Number::from_parts(BigInt::from_i128(num), BigInt::from_i128(den))
    }

    /// Builds `num / den` from arbitrary-precision parts, in lowest terms.
    /// Returns `None` when `den == 0`.
    pub fn from_parts(num: BigInt, den: BigInt) -> Option<Number> {
        if den.is_zero() {
            return None;
        }
        let (num, den) = if den.is_negative() { (num.negate(), den.negate()) } else { (num, den) };
        if num.is_zero() {
            return Some(Number::zero());
        }
        let g = num.gcd(&den);
        let (num, _) = num.divrem(&g)?;
        let (den, _) = den.divrem(&g)?;
        Some(Number { num, den })
    }

    /// Numerator in lowest terms (carries the sign).
    pub fn numer(&self) -> &BigInt {
        &self.num
    }

    /// Denominator in lowest terms (always positive).
    pub fn denom(&self) -> &BigInt {
        &self.den
    }

    /// The value as an `i128`, if it is an integer that fits.
    pub fn as_i128(&self) -> Option<i128> {
        if self.den.is_one() {
            self.num.to_i128()
        } else {
            None
        }
    }

    /// `true` if the value is exactly zero.
    pub fn is_zero(&self) -> bool {
        self.num.is_zero()
    }

    /// `true` if the value is exactly one.
    pub fn is_one(&self) -> bool {
        self.num.is_one() && self.den.is_one()
    }

    /// `true` if the value is strictly negative.
    pub fn is_negative(&self) -> bool {
        self.num.is_negative()
    }

    /// `true` if the value is an integer.
    pub fn is_integer(&self) -> bool {
        self.den.is_one()
    }

    /// Exact negation.
    pub fn checked_neg(&self) -> Option<Number> {
        Some(Number { num: self.num.negate(), den: self.den.clone() })
    }

    /// Exact sum.
    pub fn checked_add(&self, o: &Number) -> Option<Number> {
        let num = self.num.mul(&o.den).add(&o.num.mul(&self.den));
        Number::from_parts(num, self.den.mul(&o.den))
    }

    /// Exact difference.
    pub fn checked_sub(&self, o: &Number) -> Option<Number> {
        let num = self.num.mul(&o.den).sub(&o.num.mul(&self.den));
        Number::from_parts(num, self.den.mul(&o.den))
    }

    /// Exact product.
    pub fn checked_mul(&self, o: &Number) -> Option<Number> {
        Number::from_parts(self.num.mul(&o.num), self.den.mul(&o.den))
    }

    /// Exact reciprocal. `None` for zero.
    pub fn checked_recip(&self) -> Option<Number> {
        Number::from_parts(self.den.clone(), self.num.clone())
    }

    /// Exact quotient. `None` when dividing by zero.
    pub fn checked_div(&self, o: &Number) -> Option<Number> {
        self.checked_mul(&o.checked_recip()?)
    }

    /// Exact integer power by squaring. `None` for a negative power of zero,
    /// or when `|exp|` exceeds [`MAX_POW_EXPONENT`].
    pub fn checked_pow(&self, exp: i64) -> Option<Number> {
        let n = exp.unsigned_abs();
        if n > MAX_POW_EXPONENT {
            return None;
        }
        let mut base = if exp < 0 { self.checked_recip()? } else { self.clone() };
        let mut e = n;
        let mut acc = Number::one();
        while e > 0 {
            if e & 1 == 1 {
                acc = acc.checked_mul(&base)?;
            }
            e >>= 1;
            if e > 0 {
                base = base.checked_mul(&base)?;
            }
        }
        Some(acc)
    }
}

impl fmt::Display for Number {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.den.is_one() {
            write!(f, "{}", self.num)
        } else {
            write!(f, "{}/{}", self.num, self.den)
        }
    }
}

impl fmt::Debug for Number {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

/// The arithmetic surface the crate needs from an exact number type.
///
/// [`Number`] implements it. The real `tpt-mathr` types should implement it
/// too, so that switching over is a change of type alias and not a rewrite.
pub trait ExactNumber: Sized + Clone + Eq + Ord + fmt::Display {
    /// Exact zero.
    fn zero() -> Self;
    /// Exact one.
    fn one() -> Self;
    /// Embeds an integer.
    fn from_i128(n: i128) -> Self;
    /// `true` if the value is exactly zero.
    fn is_zero(&self) -> bool;
    /// `true` if the value is strictly negative.
    fn is_negative(&self) -> bool;
    /// Exact sum.
    fn checked_add(&self, o: &Self) -> Option<Self>;
    /// Exact difference.
    fn checked_sub(&self, o: &Self) -> Option<Self>;
    /// Exact product.
    fn checked_mul(&self, o: &Self) -> Option<Self>;
    /// Exact quotient. `None` when dividing by zero.
    fn checked_div(&self, o: &Self) -> Option<Self>;
    /// Exact negation.
    fn checked_neg(&self) -> Option<Self>;
    /// Exact reciprocal. `None` for zero.
    fn checked_recip(&self) -> Option<Self>;
    /// Exact integer power. `None` for a negative power of zero.
    fn checked_pow(&self, exp: i64) -> Option<Self>;
}

impl ExactNumber for Number {
    fn zero() -> Self {
        Number::zero()
    }
    fn one() -> Self {
        Number::one()
    }
    fn from_i128(n: i128) -> Self {
        Number::from_int(n)
    }
    fn is_zero(&self) -> bool {
        Number::is_zero(self)
    }
    fn is_negative(&self) -> bool {
        Number::is_negative(self)
    }
    fn checked_add(&self, o: &Self) -> Option<Self> {
        Number::checked_add(self, o)
    }
    fn checked_sub(&self, o: &Self) -> Option<Self> {
        Number::checked_sub(self, o)
    }
    fn checked_mul(&self, o: &Self) -> Option<Self> {
        Number::checked_mul(self, o)
    }
    fn checked_div(&self, o: &Self) -> Option<Self> {
        Number::checked_div(self, o)
    }
    fn checked_neg(&self) -> Option<Self> {
        Number::checked_neg(self)
    }
    fn checked_recip(&self) -> Option<Self> {
        Number::checked_recip(self)
    }
    fn checked_pow(&self, exp: i64) -> Option<Self> {
        Number::checked_pow(self, exp)
    }
}

#[cfg(test)]
mod tests {
    use super::{ExactNumber, Number};
    use alloc::format;

    fn q(n: i128, d: i128) -> Number {
        Number::new(n, d).unwrap()
    }

    /// Generic code written against the trait only.
    fn third_times_three<N: ExactNumber>() -> Option<N> {
        let three = N::from_i128(3);
        N::one().checked_div(&three)?.checked_mul(&three)
    }

    #[test]
    fn normalizes_sign_and_common_factors() {
        assert_eq!(q(2, -4), q(-1, 2));
        assert_eq!(q(0, 7), Number::zero());
        assert_eq!(q(6, 3).denom().to_i128(), Some(1));
        assert!(Number::new(1, 0).is_none());
    }

    #[test]
    fn third_plus_third_plus_third_is_exactly_one() {
        let third = q(1, 3);
        let sum = third.checked_add(&third).unwrap().checked_add(&third).unwrap();
        assert_eq!(sum, Number::one());
        assert_eq!(third_times_three::<Number>(), Some(Number::one()));
    }

    #[test]
    fn arithmetic_is_exact() {
        assert_eq!(q(1, 2).checked_mul(&q(2, 3)).unwrap(), q(1, 3));
        assert_eq!(q(1, 2).checked_sub(&q(1, 2)).unwrap(), Number::zero());
        assert_eq!(q(3, 4).checked_div(&q(-3, 8)).unwrap(), q(-2, 1));
        assert_eq!(q(2, 3).checked_pow(-2).unwrap(), q(9, 4));
        assert!(Number::zero().checked_recip().is_none());
    }

    #[test]
    fn large_values_are_exact_instead_of_overflowing() {
        // (2^200)^2 / 2^200 == 2^200, all exact.
        let two_200 = Number::from_int(2).checked_pow(200).unwrap();
        let squared = two_200.checked_mul(&two_200).unwrap();
        assert_eq!(squared.checked_div(&two_200).unwrap(), two_200);
        assert_eq!(squared.as_i128(), None);
        assert_eq!(format!("{}", Number::from_int(2).checked_pow(64).unwrap()), "18446744073709551616");
    }

    #[test]
    fn oversized_exponents_are_refused() {
        assert!(Number::from_int(2).checked_pow(i64::MAX).is_none());
    }

    #[test]
    fn display_round_trips_shape() {
        assert_eq!(format!("{}", q(-3, 6)), "-1/2");
        assert_eq!(format!("{}", q(4, 2)), "2");
    }
}
