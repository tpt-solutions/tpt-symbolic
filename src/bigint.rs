//! Arbitrary-precision signed integers backing the exact-number shim.
//!
//! Magnitudes are little-endian `u64` limbs with no trailing zero limb. Zero
//! has an empty magnitude and is never negative, so the representation is
//! canonical and the derived `PartialEq`/`Hash` are correct.
//!
//! Division is bitwise long division, which is quadratic in the operand size.
//! That is fine for the sizes this crate produces. `gcd` uses the binary
//! (Stein) algorithm, which avoids repeated long division.

use alloc::vec;
use alloc::vec::Vec;
use core::cmp::Ordering;
use core::fmt;

/// A signed arbitrary-precision integer.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct BigInt {
    neg: bool,
    mag: Vec<u64>,
}

fn trim(mut v: Vec<u64>) -> Vec<u64> {
    while v.last() == Some(&0) {
        v.pop();
    }
    v
}

fn mag_cmp(a: &[u64], b: &[u64]) -> Ordering {
    if a.len() != b.len() {
        return a.len().cmp(&b.len());
    }
    for (x, y) in a.iter().rev().zip(b.iter().rev()) {
        match x.cmp(y) {
            Ordering::Equal => {}
            other => return other,
        }
    }
    Ordering::Equal
}

fn mag_add(a: &[u64], b: &[u64]) -> Vec<u64> {
    let n = a.len().max(b.len());
    let mut out = Vec::with_capacity(n + 1);
    let mut carry = 0u128;
    for i in 0..n {
        let s = carry + *a.get(i).unwrap_or(&0) as u128 + *b.get(i).unwrap_or(&0) as u128;
        out.push(s as u64);
        carry = s >> 64;
    }
    if carry > 0 {
        out.push(carry as u64);
    }
    trim(out)
}

/// `a - b`, requiring `a >= b`.
fn mag_sub(a: &[u64], b: &[u64]) -> Vec<u64> {
    let mut out = Vec::with_capacity(a.len());
    let mut borrow = 0i128;
    for (i, &limb) in a.iter().enumerate() {
        let mut x = limb as i128 - *b.get(i).unwrap_or(&0) as i128 - borrow;
        if x < 0 {
            x += 1i128 << 64;
            borrow = 1;
        } else {
            borrow = 0;
        }
        out.push(x as u64);
    }
    trim(out)
}

fn mag_mul(a: &[u64], b: &[u64]) -> Vec<u64> {
    if a.is_empty() || b.is_empty() {
        return Vec::new();
    }
    let mut out = vec![0u64; a.len() + b.len()];
    for (i, &x) in a.iter().enumerate() {
        let mut carry = 0u128;
        for (j, &y) in b.iter().enumerate() {
            let t = out[i + j] as u128 + x as u128 * y as u128 + carry;
            out[i + j] = t as u64;
            carry = t >> 64;
        }
        let mut k = i + b.len();
        while carry > 0 {
            let t = out[k] as u128 + carry;
            out[k] = t as u64;
            carry = t >> 64;
            k += 1;
        }
    }
    trim(out)
}

fn mag_divrem_small(a: &[u64], d: u64) -> (Vec<u64>, u64) {
    let mut q = vec![0u64; a.len()];
    let mut rem: u128 = 0;
    for i in (0..a.len()).rev() {
        let cur = (rem << 64) | a[i] as u128;
        q[i] = (cur / d as u128) as u64;
        rem = cur % d as u128;
    }
    (trim(q), rem as u64)
}

fn bit_len(a: &[u64]) -> u64 {
    match a.last() {
        None => 0,
        Some(&top) => 64 * (a.len() as u64 - 1) + (64 - top.leading_zeros() as u64),
    }
}

fn bit(a: &[u64], i: u64) -> u64 {
    let limb = (i / 64) as usize;
    match a.get(limb) {
        Some(&x) => (x >> (i % 64)) & 1,
        None => 0,
    }
}

fn shl1_or(r: &mut Vec<u64>, low: u64) {
    let mut carry = low;
    for limb in r.iter_mut() {
        let next = *limb >> 63;
        *limb = (*limb << 1) | carry;
        carry = next;
    }
    if carry != 0 {
        r.push(carry);
    }
}

/// Quotient and remainder of magnitudes. `b` must be non-empty.
fn mag_divrem(a: &[u64], b: &[u64]) -> (Vec<u64>, Vec<u64>) {
    if mag_cmp(a, b) == Ordering::Less {
        return (Vec::new(), a.to_vec());
    }
    if b.len() == 1 {
        let (q, r) = mag_divrem_small(a, b[0]);
        let rem = if r == 0 { Vec::new() } else { vec![r] };
        return (q, rem);
    }
    let bits = bit_len(a);
    let mut q = vec![0u64; a.len()];
    let mut r: Vec<u64> = Vec::new();
    for i in (0..bits).rev() {
        shl1_or(&mut r, bit(a, i));
        if mag_cmp(&r, b) != Ordering::Less {
            r = mag_sub(&r, b);
            q[(i / 64) as usize] |= 1 << (i % 64);
        }
    }
    (trim(q), r)
}

fn trailing_zeros(a: &[u64]) -> u64 {
    for (i, &limb) in a.iter().enumerate() {
        if limb != 0 {
            return 64 * i as u64 + limb.trailing_zeros() as u64;
        }
    }
    0
}

fn mag_shr(a: &[u64], n: u64) -> Vec<u64> {
    let q = (n / 64) as usize;
    let s = (n % 64) as u32;
    if q >= a.len() {
        return Vec::new();
    }
    let src = &a[q..];
    let mut out = Vec::with_capacity(src.len());
    for i in 0..src.len() {
        let lo = src[i] >> s;
        let hi = if s > 0 && i + 1 < src.len() { src[i + 1] << (64 - s) } else { 0 };
        out.push(lo | hi);
    }
    trim(out)
}

fn mag_shl(a: &[u64], n: u64) -> Vec<u64> {
    if a.is_empty() {
        return Vec::new();
    }
    let q = (n / 64) as usize;
    let s = (n % 64) as u32;
    let mut out = vec![0u64; q];
    let mut carry = 0u64;
    for &x in a {
        if s == 0 {
            out.push(x);
        } else {
            out.push((x << s) | carry);
            carry = x >> (64 - s);
        }
    }
    if carry != 0 {
        out.push(carry);
    }
    trim(out)
}

/// Binary (Stein) gcd of two magnitudes.
fn mag_gcd(a: &[u64], b: &[u64]) -> Vec<u64> {
    if a.is_empty() {
        return b.to_vec();
    }
    if b.is_empty() {
        return a.to_vec();
    }
    let za = trailing_zeros(a);
    let zb = trailing_zeros(b);
    let k = za.min(zb);
    let mut x = mag_shr(a, za);
    let mut y = mag_shr(b, zb);
    loop {
        if mag_cmp(&x, &y) == Ordering::Greater {
            core::mem::swap(&mut x, &mut y);
        }
        y = mag_sub(&y, &x);
        if y.is_empty() {
            break;
        }
        let t = trailing_zeros(&y);
        y = mag_shr(&y, t);
    }
    mag_shl(&x, k)
}

impl BigInt {
    /// The integer zero.
    pub const fn zero() -> BigInt {
        BigInt { neg: false, mag: Vec::new() }
    }

    /// The integer one.
    pub fn one() -> BigInt {
        BigInt::from_i128(1)
    }

    /// Embeds an `i128`.
    pub fn from_i128(n: i128) -> BigInt {
        let m = n.unsigned_abs();
        let mut mag = Vec::new();
        if m != 0 {
            mag.push(m as u64);
            if (m >> 64) != 0 {
                mag.push((m >> 64) as u64);
            }
        }
        BigInt::make(n < 0, mag)
    }

    fn make(neg: bool, mag: Vec<u64>) -> BigInt {
        let neg = neg && !mag.is_empty();
        BigInt { neg, mag }
    }

    /// `true` for zero.
    pub fn is_zero(&self) -> bool {
        self.mag.is_empty()
    }

    /// `true` for one.
    pub fn is_one(&self) -> bool {
        !self.neg && self.mag.as_slice() == [1]
    }

    /// `true` for strictly negative values.
    pub fn is_negative(&self) -> bool {
        self.neg
    }

    /// Additive inverse.
    pub fn negate(&self) -> BigInt {
        BigInt::make(!self.neg, self.mag.clone())
    }

    /// Absolute value.
    pub fn abs(&self) -> BigInt {
        BigInt::make(false, self.mag.clone())
    }

    /// The value as an `i128`, or `None` if it does not fit.
    pub fn to_i128(&self) -> Option<i128> {
        if self.mag.len() > 2 {
            return None;
        }
        let m = self.mag.iter().rev().fold(0u128, |acc, &l| (acc << 64) | l as u128);
        if self.neg {
            if m <= i128::MAX as u128 + 1 {
                Some((m as i128).wrapping_neg())
            } else {
                None
            }
        } else if m <= i128::MAX as u128 {
            Some(m as i128)
        } else {
            None
        }
    }

    /// The value as a `u128`, or `None` if it is negative or too large.
    pub fn to_u128(&self) -> Option<u128> {
        if self.neg || self.mag.len() > 2 {
            return None;
        }
        Some(self.mag.iter().rev().fold(0u128, |acc, &l| (acc << 64) | l as u128))
    }

    /// Sum.
    pub fn add(&self, o: &BigInt) -> BigInt {
        if self.neg == o.neg {
            BigInt::make(self.neg, mag_add(&self.mag, &o.mag))
        } else if mag_cmp(&self.mag, &o.mag) == Ordering::Less {
            BigInt::make(o.neg, mag_sub(&o.mag, &self.mag))
        } else {
            BigInt::make(self.neg, mag_sub(&self.mag, &o.mag))
        }
    }

    /// Difference.
    pub fn sub(&self, o: &BigInt) -> BigInt {
        self.add(&o.negate())
    }

    /// Product.
    pub fn mul(&self, o: &BigInt) -> BigInt {
        BigInt::make(self.neg != o.neg, mag_mul(&self.mag, &o.mag))
    }

    /// Truncated quotient and remainder (the remainder takes the dividend's sign).
    /// `None` when dividing by zero.
    pub fn divrem(&self, o: &BigInt) -> Option<(BigInt, BigInt)> {
        if o.is_zero() {
            return None;
        }
        let (q, r) = mag_divrem(&self.mag, &o.mag);
        Some((BigInt::make(self.neg != o.neg, q), BigInt::make(self.neg, r)))
    }

    /// Greatest common divisor of the absolute values. Always non-negative.
    pub fn gcd(&self, o: &BigInt) -> BigInt {
        BigInt::make(false, mag_gcd(&self.mag, &o.mag))
    }
}

impl Ord for BigInt {
    fn cmp(&self, o: &BigInt) -> Ordering {
        match (self.neg, o.neg) {
            (false, true) => Ordering::Greater,
            (true, false) => Ordering::Less,
            (false, false) => mag_cmp(&self.mag, &o.mag),
            (true, true) => mag_cmp(&o.mag, &self.mag),
        }
    }
}

impl PartialOrd for BigInt {
    fn partial_cmp(&self, o: &BigInt) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

impl fmt::Display for BigInt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_zero() {
            return f.write_str("0");
        }
        // Peel off 19 decimal digits at a time, least significant first.
        const CHUNK: u64 = 10_000_000_000_000_000_000;
        let mut chunks = Vec::new();
        let mut m = self.mag.clone();
        while !m.is_empty() {
            let (q, r) = mag_divrem_small(&m, CHUNK);
            chunks.push(r);
            m = q;
        }
        if self.neg {
            f.write_str("-")?;
        }
        let mut iter = chunks.iter().rev();
        if let Some(first) = iter.next() {
            write!(f, "{first}")?;
        }
        for c in iter {
            write!(f, "{c:019}")?;
        }
        Ok(())
    }
}

impl fmt::Debug for BigInt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::format;
    use alloc::string::ToString;

    fn big(s: &str) -> BigInt {
        // Test helper: builds 10^k-scaled values by repeated multiply-add.
        let mut acc = BigInt::zero();
        let ten = BigInt::from_i128(10);
        let (neg, digits) = match s.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, s),
        };
        for ch in digits.chars() {
            let d = BigInt::from_i128(ch.to_digit(10).unwrap() as i128);
            acc = acc.mul(&ten).add(&d);
        }
        if neg {
            acc.negate()
        } else {
            acc
        }
    }

    #[test]
    fn arithmetic_matches_i128_in_range() {
        for (a, b) in [(7i128, -3i128), (-12345, 678), (0, -5), (i64::MAX as i128, i64::MIN as i128)] {
            let (x, y) = (BigInt::from_i128(a), BigInt::from_i128(b));
            assert_eq!(x.add(&y).to_i128(), Some(a + b));
            assert_eq!(x.sub(&y).to_i128(), Some(a - b));
            assert_eq!(x.mul(&y).to_i128(), a.checked_mul(b));
            if b != 0 {
                let (q, r) = x.divrem(&y).unwrap();
                assert_eq!(q.to_i128(), Some(a / b));
                assert_eq!(r.to_i128(), Some(a % b));
            }
        }
    }

    #[test]
    fn exceeds_i128_and_round_trips_through_text() {
        let mut p = BigInt::from_i128(2);
        for _ in 0..199 {
            p = p.mul(&BigInt::from_i128(2));
        }
        assert_eq!(format!("{p}"), "1606938044258990275541962092341162602522202993782792835301376");
        assert_eq!(p.to_i128(), None);
        assert_eq!(big("-123456789012345678901234567890").to_string(), "-123456789012345678901234567890");
    }

    #[test]
    fn division_is_exact_on_large_values() {
        let a = big("98765432109876543210987654321098765432109876543210");
        let b = big("1234567890123456789012345");
        let (q, r) = a.mul(&b).add(&BigInt::from_i128(17)).divrem(&b).unwrap();
        assert_eq!(q, a);
        assert_eq!(r, BigInt::from_i128(17));
    }

    #[test]
    fn gcd_on_large_values() {
        let g = big("1000000007000000000000000000049");
        let a = g.mul(&big("123456789123456789"));
        let b = g.mul(&big("987654321987654321"));
        // Cofactor gcd from an i128 Euclid, so the expectation is independent of BigInt.
        let (mut x, mut y) = (123456789123456789i128, 987654321987654321i128);
        while y != 0 {
            (x, y) = (y, x % y);
        }
        assert_eq!(a.gcd(&b), g.mul(&BigInt::from_i128(x)));
    }

    #[test]
    fn ordering_respects_sign_and_magnitude() {
        assert!(BigInt::from_i128(-5) < BigInt::from_i128(-4));
        assert!(BigInt::from_i128(-1) < BigInt::zero());
        assert!(big("1000000000000000000000000000000000000000000") > BigInt::from_i128(i128::MAX));
    }
}
