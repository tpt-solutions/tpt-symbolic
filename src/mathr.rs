//! Exact rational numbers, backed by the `tpt-mathr` arithmetic crate.
//!
//! Arithmetic never rounds and never overflows. `None` is returned only for
//! division by zero, and for powers whose exponent exceeds [`MAX_POW_EXPONENT`].
//! [`Number::numer`] and [`Number::denom`] return this crate's [`BigInt`], which
//! is converted from the `tpt-mathr` value on each call.

use crate::bigint::BigInt;
use alloc::format;
use alloc::string::ToString;
use core::cmp::Ordering;
use core::fmt;
use tpt_mathr_arith::{BigInt as RBigInt, BigRat, BigUint, Progress, Sign};

/// Largest exponent [`Number::checked_pow`] will expand.
pub const MAX_POW_EXPONENT: u64 = 1 << 16;

/// Unlimited progress handle for every `tpt-mathr` operation. With no limits and no
/// cancellation, the only failure left is division by zero, which callers already
/// map to `None`.
const PROGRESS: &Progress = &Progress::UNLIMITED;

/// An exact rational `num / den`, stored in lowest terms with `den > 0`. Zero is `0/1`.
///
/// The derived `Eq` and `Hash` agree with the value. [`Ord`] is a structural order,
/// numerator first, used for deterministic sorting of canonical expressions. It is
/// not the numeric order; use [`Number::cmp_value`] for that.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Number {
    rat: BigRat,
}

impl Ord for Number {
    fn cmp(&self, other: &Self) -> Ordering {
        self.rat
            .numerator()
            .cmp(other.rat.numerator())
            .then_with(|| self.rat.denominator().cmp(other.rat.denominator()))
    }
}

impl PartialOrd for Number {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Number {
    /// Exact zero.
    pub fn zero() -> Number {
        Number {
            rat: BigRat::zero(),
        }
    }

    /// Exact one.
    pub fn one() -> Number {
        Number { rat: BigRat::one() }
    }

    /// Exact negative one.
    pub fn neg_one() -> Number {
        Number::from_int(-1)
    }

    /// Embeds an integer.
    pub fn from_int(n: i128) -> Number {
        let rat = match i64::try_from(n) {
            Ok(v) => BigRat::from(v),
            Err(_) => BigRat::from_str_radix(&format!("{n}"), 10, PROGRESS)
                .expect("a decimal integer is valid"),
        };
        Number { rat }
    }

    /// Builds `num / den` in lowest terms. Returns `None` when `den == 0`.
    pub fn new(num: i128, den: i128) -> Option<Number> {
        if den == 0 {
            return None;
        }
        let num = RBigInt::from(num);
        let num = if den < 0 { num.neg() } else { num };
        let rat = BigRat::new(num, BigUint::from(den.unsigned_abs()), PROGRESS).ok()?;
        Some(Number { rat })
    }

    /// Builds `num / den` from arbitrary-precision parts, in lowest terms.
    /// Returns `None` when `den == 0`.
    pub fn from_parts(num: BigInt, den: BigInt) -> Option<Number> {
        if den.is_zero() {
            return None;
        }
        let (num, den) = if den.is_negative() {
            (num.negate(), den.abs())
        } else {
            (num, den)
        };
        let rat = BigRat::new(real_int(&num), real_int(&den).magnitude().clone(), PROGRESS).ok()?;
        Some(Number { rat })
    }

    /// Numerator in lowest terms (carries the sign).
    pub fn numer(&self) -> BigInt {
        local_real_int(self.rat.numerator())
    }

    /// Denominator in lowest terms (always positive).
    pub fn denom(&self) -> BigInt {
        match self.rat.denominator().to_u64() {
            Ok(v) => BigInt::from_i128(i128::from(v)),
            Err(_) => local_int(&self.rat.denominator().to_string()),
        }
    }

    /// The value as an `i128`, if it is an integer that fits.
    pub fn as_i128(&self) -> Option<i128> {
        if !self.is_integer() {
            return None;
        }
        match self.rat.numerator().to_i64() {
            Ok(v) => Some(i128::from(v)),
            Err(_) => self.numer().to_i128(),
        }
    }

    /// `true` if the value is exactly zero.
    pub fn is_zero(&self) -> bool {
        self.rat.is_zero()
    }

    /// `true` if the value is exactly one.
    pub fn is_one(&self) -> bool {
        // Positive with magnitude 1 and denominator 1. `bit_len() == 1` means the value is 1.
        matches!(self.rat.numerator().sign(), Sign::Positive)
            && self.rat.numerator().magnitude().bit_len() == 1
            && self.is_integer()
    }

    /// `true` if the value is strictly negative.
    pub fn is_negative(&self) -> bool {
        matches!(self.rat.numerator().sign(), Sign::Negative)
    }

    /// `true` if the value is an integer.
    pub fn is_integer(&self) -> bool {
        // The denominator is at least 1, so a bit length of 1 means it is exactly 1.
        self.rat.denominator().bit_len() == 1
    }

    /// Exact negation.
    pub fn checked_neg(&self) -> Option<Number> {
        Some(Number {
            rat: self.rat.neg(),
        })
    }

    /// Orders the values numerically (`1/3 < 1/2`).
    ///
    /// The derived `Ord` is structural and gives `1/2 < 1/3`, so use this when
    /// the order of the values matters.
    pub fn cmp_value(&self, other: &Number) -> Ordering {
        // Only fails on cancellation, which is never requested here.
        self.rat
            .cmp_rat(&other.rat, PROGRESS)
            .unwrap_or_else(|_| self.cmp(other))
    }

    /// Exact sum.
    pub fn checked_add(&self, o: &Number) -> Option<Number> {
        self.rat
            .add(&o.rat, PROGRESS)
            .ok()
            .map(|rat| Number { rat })
    }

    /// Exact difference.
    pub fn checked_sub(&self, o: &Number) -> Option<Number> {
        self.rat
            .checked_sub(&o.rat, PROGRESS)
            .ok()
            .map(|rat| Number { rat })
    }

    /// Exact product.
    pub fn checked_mul(&self, o: &Number) -> Option<Number> {
        self.rat
            .mul(&o.rat, PROGRESS)
            .ok()
            .map(|rat| Number { rat })
    }

    /// Exact reciprocal. `None` for zero.
    pub fn checked_recip(&self) -> Option<Number> {
        self.rat.inv().ok().map(|rat| Number { rat })
    }

    /// Exact quotient. `None` when dividing by zero.
    pub fn checked_div(&self, o: &Number) -> Option<Number> {
        self.rat
            .div(&o.rat, PROGRESS)
            .ok()
            .map(|rat| Number { rat })
    }

    /// Exact integer power by squaring. `None` for a negative power of zero,
    /// or when `|exp|` exceeds [`MAX_POW_EXPONENT`].
    pub fn checked_pow(&self, exp: i64) -> Option<Number> {
        let n = exp.unsigned_abs();
        if n > MAX_POW_EXPONENT {
            return None;
        }
        let mut base = if exp < 0 {
            self.checked_recip()?
        } else {
            self.clone()
        };
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

/// The `tpt-mathr` form of `n`. Values that fit an `i64` skip the string round-trip.
fn real_int(n: &BigInt) -> RBigInt {
    match n.to_i128() {
        Some(v) => RBigInt::from(v),
        None => BigRat::from_str_radix(&n.to_string(), 10, PROGRESS)
            .expect("a decimal integer is valid")
            .numerator()
            .clone(),
    }
}

/// The local form of a `tpt-mathr` integer. Values that fit an `i64` skip the string round-trip.
fn local_real_int(n: &RBigInt) -> BigInt {
    match n.to_i64() {
        Ok(v) => BigInt::from_i128(i128::from(v)),
        Err(_) => local_int(&n.to_string()),
    }
}

/// Converts a decimal string from `tpt-mathr` (optional leading `-`) to a [`BigInt`].
fn local_int(decimal: &str) -> BigInt {
    let (negative, digits) = match decimal.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, decimal),
    };
    let ten = BigInt::from_i128(10);
    let mut acc = BigInt::from_i128(0);
    for digit in digits.bytes() {
        acc = acc
            .mul(&ten)
            .add(&BigInt::from_i128(i128::from(digit - b'0')));
    }
    if negative {
        acc.negate()
    } else {
        acc
    }
}

impl fmt::Display for Number {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_integer() {
            write!(f, "{}", self.rat.numerator())
        } else {
            write!(f, "{}/{}", self.rat.numerator(), self.rat.denominator())
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
        let sum = third
            .checked_add(&third)
            .unwrap()
            .checked_add(&third)
            .unwrap();
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
        assert_eq!(
            format!("{}", Number::from_int(2).checked_pow(64).unwrap()),
            "18446744073709551616"
        );
    }

    #[test]
    fn oversized_exponents_are_refused() {
        assert!(Number::from_int(2).checked_pow(i64::MAX).is_none());
    }

    #[test]
    fn integer_accessors_cover_small_and_large_values() {
        use crate::bigint::BigInt;
        // Negative denominator moves the sign to the numerator.
        assert_eq!(q(3, -6).numer().to_i128(), Some(-1));
        assert_eq!(q(3, -6).denom().to_i128(), Some(2));
        assert!(q(3, -6).is_negative());
        assert!(!q(3, -6).is_integer());
        // Beyond i64: numer/denom take the string path and still agree with the value.
        let big = Number::from_int(2).checked_pow(100).unwrap();
        assert_eq!(big.numer(), BigInt::from_i128(1 << 100));
        assert_eq!(big.denom().to_i128(), Some(1));
        assert!(big.is_integer());
        // 2^100 fits i128, so it is still reported; 2^200 would not be.
        assert_eq!(big.as_i128(), Some(1 << 100));
        assert_eq!(
            Number::from_int(2).checked_pow(200).unwrap().as_i128(),
            None
        );
        assert!(Number::one().is_one());
        assert!(!Number::neg_one().is_one());
        assert!(!q(1, 2).is_one());
        let huge = Number::from_parts(BigInt::from_i128(1 << 100), BigInt::from_i128(3)).unwrap();
        assert_eq!(huge.numer(), BigInt::from_i128(1 << 100));
        assert_eq!(huge.denom().to_i128(), Some(3));
    }

    #[test]
    fn display_round_trips_shape() {
        assert_eq!(format!("{}", q(-3, 6)), "-1/2");
        assert_eq!(format!("{}", q(4, 2)), "2");
    }

    #[test]
    fn cmp_value_orders_numerically_not_structurally() {
        use core::cmp::Ordering::{Equal, Greater, Less};
        // The structural order puts 1/2 before 1/3; the value order does not.
        assert_eq!(q(1, 2).cmp_value(&q(1, 3)), Greater);
        assert_eq!(q(-1, 2).cmp_value(&q(1, 3)), Less);
        assert_eq!(q(2, 4).cmp_value(&q(1, 2)), Equal);
    }
}
