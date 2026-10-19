//! Exact number type: a local stand-in for `tpt-mathr`.
//!
//! Only exact rationals over `i128` are provided for now. Every operation is
//! checked: an overflow yields `None` and is never rounded. Swapping in the
//! real `tpt-mathr` means replacing this module and keeping the method names.

use core::fmt;

/// An exact rational `num / den`, stored in lowest terms with `den > 0`.
///
/// The derived `Ord` is a structural order used for deterministic sorting of
/// canonical expressions. It is not the numeric order.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Number {
    num: i128,
    den: i128,
}

fn gcd(mut a: u128, mut b: u128) -> u128 {
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a
}

impl Number {
    /// The exact value `0`.
    pub const ZERO: Number = Number { num: 0, den: 1 };
    /// The exact value `1`.
    pub const ONE: Number = Number { num: 1, den: 1 };
    /// The exact value `-1`.
    pub const NEG_ONE: Number = Number { num: -1, den: 1 };

    /// Builds `num / den` in lowest terms. Returns `None` when `den == 0`.
    pub fn new(num: i128, den: i128) -> Option<Number> {
        if den == 0 {
            return None;
        }
        if num == 0 {
            return Some(Self::ZERO);
        }
        let g = gcd(num.unsigned_abs(), den.unsigned_abs());
        let n = num.unsigned_abs() / g;
        let d = den.unsigned_abs() / g;
        if n > i128::MAX as u128 || d > i128::MAX as u128 {
            return None;
        }
        let negative = (num < 0) != (den < 0);
        let n = if negative { -(n as i128) } else { n as i128 };
        Some(Number { num: n, den: d as i128 })
    }

    /// Builds the integer `n`.
    pub const fn from_int(n: i128) -> Number {
        Number { num: n, den: 1 }
    }

    /// Numerator in lowest terms (carries the sign).
    pub const fn numer(&self) -> i128 {
        self.num
    }

    /// Denominator in lowest terms (always positive).
    pub const fn denom(&self) -> i128 {
        self.den
    }

    /// `true` if the value is exactly zero.
    pub const fn is_zero(&self) -> bool {
        self.num == 0
    }

    /// `true` if the value is exactly one.
    pub const fn is_one(&self) -> bool {
        self.num == 1 && self.den == 1
    }

    /// `true` if the value is strictly negative.
    pub const fn is_negative(&self) -> bool {
        self.num < 0
    }

    /// `true` if the value is an integer.
    pub const fn is_integer(&self) -> bool {
        self.den == 1
    }

    /// Exact negation. `None` only on `i128::MIN` numerators.
    pub fn checked_neg(self) -> Option<Number> {
        self.num.checked_neg().map(|n| Number { num: n, den: self.den })
    }

    /// Exact sum.
    pub fn checked_add(self, o: Number) -> Option<Number> {
        let g = gcd(self.den as u128, o.den as u128) as i128;
        let bd = self.den / g;
        let num = self.num.checked_mul(o.den / g)?.checked_add(o.num.checked_mul(bd)?)?;
        let den = bd.checked_mul(o.den)?;
        Number::new(num, den)
    }

    /// Exact difference.
    pub fn checked_sub(self, o: Number) -> Option<Number> {
        self.checked_add(o.checked_neg()?)
    }

    /// Exact product, cross-reducing first to keep intermediate values small.
    pub fn checked_mul(self, o: Number) -> Option<Number> {
        let g1 = gcd(self.num.unsigned_abs(), o.den as u128) as i128;
        let g2 = gcd(o.num.unsigned_abs(), self.den as u128) as i128;
        let num = (self.num / g1).checked_mul(o.num / g2)?;
        let den = (self.den / g2).checked_mul(o.den / g1)?;
        Number::new(num, den)
    }

    /// Exact reciprocal. `None` for zero.
    pub fn checked_recip(self) -> Option<Number> {
        Number::new(self.den, self.num)
    }

    /// Exact quotient. `None` when dividing by zero.
    pub fn checked_div(self, o: Number) -> Option<Number> {
        self.checked_mul(o.checked_recip()?)
    }

    /// Exact integer power by squaring. `None` on overflow, or on a negative
    /// power of zero.
    pub fn checked_pow(self, exp: i64) -> Option<Number> {
        let mut base = if exp < 0 { self.checked_recip()? } else { self };
        let mut e = exp.unsigned_abs();
        let mut acc = Number::ONE;
        while e > 0 {
            if e & 1 == 1 {
                acc = acc.checked_mul(base)?;
            }
            e >>= 1;
            if e > 0 {
                base = base.checked_mul(base)?;
            }
        }
        Some(acc)
    }
}

impl fmt::Display for Number {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.den == 1 {
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

#[cfg(test)]
mod tests {
    use super::Number;

    fn q(n: i128, d: i128) -> Number {
        Number::new(n, d).unwrap()
    }

    #[test]
    fn normalizes_sign_and_common_factors() {
        assert_eq!(q(2, -4), q(-1, 2));
        assert_eq!(q(0, 7), Number::ZERO);
        assert_eq!(q(6, 3).denom(), 1);
        assert!(Number::new(1, 0).is_none());
    }

    #[test]
    fn third_plus_third_plus_third_is_exactly_one() {
        let third = q(1, 3);
        let sum = third.checked_add(third).unwrap().checked_add(third).unwrap();
        assert_eq!(sum, Number::ONE);
    }

    #[test]
    fn arithmetic_is_exact() {
        assert_eq!(q(1, 2).checked_mul(q(2, 3)).unwrap(), q(1, 3));
        assert_eq!(q(1, 2).checked_sub(q(1, 2)).unwrap(), Number::ZERO);
        assert_eq!(q(3, 4).checked_div(q(-3, 8)).unwrap(), q(-2, 1));
        assert_eq!(q(2, 3).checked_pow(-2).unwrap(), q(9, 4));
        assert!(Number::ZERO.checked_recip().is_none());
    }

    #[test]
    fn overflow_is_reported_not_rounded() {
        let big = Number::from_int(i128::MAX / 2 + 1);
        assert!(big.checked_mul(big).is_none());
        assert!(Number::from_int(2).checked_pow(200).is_none());
    }

    #[test]
    fn display_round_trips_shape() {
        extern crate alloc;
        use alloc::format;
        assert_eq!(format!("{}", q(-3, 6)), "-1/2");
        assert_eq!(format!("{}", q(4, 2)), "2");
    }
}

/// The arithmetic surface the crate needs from an exact number type.
///
/// `Number` implements it. The real `tpt-mathr` types should implement it
/// too, so that switching over is a change of type alias and not a rewrite.
pub trait ExactNumber: Sized + Copy + Eq + Ord + fmt::Display {
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
    /// Exact sum, or `None` on overflow.
    fn checked_add(self, o: Self) -> Option<Self>;
    /// Exact difference, or `None` on overflow.
    fn checked_sub(self, o: Self) -> Option<Self>;
    /// Exact product, or `None` on overflow.
    fn checked_mul(self, o: Self) -> Option<Self>;
    /// Exact quotient, or `None` on division by zero or overflow.
    fn checked_div(self, o: Self) -> Option<Self>;
    /// Exact negation, or `None` on overflow.
    fn checked_neg(self) -> Option<Self>;
    /// Exact reciprocal, or `None` for zero.
    fn checked_recip(self) -> Option<Self>;
    /// Exact integer power, or `None` on overflow or a negative power of zero.
    fn checked_pow(self, exp: i64) -> Option<Self>;
}

impl ExactNumber for Number {
    fn zero() -> Self {
        Number::ZERO
    }
    fn one() -> Self {
        Number::ONE
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
    fn checked_add(self, o: Self) -> Option<Self> {
        Number::checked_add(self, o)
    }
    fn checked_sub(self, o: Self) -> Option<Self> {
        Number::checked_sub(self, o)
    }
    fn checked_mul(self, o: Self) -> Option<Self> {
        Number::checked_mul(self, o)
    }
    fn checked_div(self, o: Self) -> Option<Self> {
        Number::checked_div(self, o)
    }
    fn checked_neg(self) -> Option<Self> {
        Number::checked_neg(self)
    }
    fn checked_recip(self) -> Option<Self> {
        Number::checked_recip(self)
    }
    fn checked_pow(self, exp: i64) -> Option<Self> {
        Number::checked_pow(self, exp)
    }
}

#[cfg(test)]
mod trait_tests {
    use super::{ExactNumber, Number};

    /// Generic code written against the trait only.
    fn third_times_three<N: ExactNumber>() -> Option<N> {
        let three = N::from_i128(3);
        N::one().checked_div(three)?.checked_mul(three)
    }

    #[test]
    fn trait_surface_is_exact() {
        assert_eq!(third_times_three::<Number>(), Some(Number::ONE));
    }
}
