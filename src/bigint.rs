//! Arbitrary-precision signed integers backing the exact-number shim.
//!
//! Magnitudes are little-endian `u64` limbs with no trailing zero limb. Zero
//! has an empty magnitude and is never negative, so the representation is
//! canonical and the derived `PartialEq`/`Hash` are correct.
//!
//! Division is Knuth's Algorithm D over `u64` limbs, which is quadratic in the
//! operand size. That is fine for the sizes this crate produces. `gcd` uses the
//! binary (Stein) algorithm, which avoids repeated long division.

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

/// Quotient and remainder of magnitudes. `b` must be non-empty.
///
/// Knuth, TAOCP vol. 2, Algorithm D, in base 2^64.
fn mag_divrem(a: &[u64], b: &[u64]) -> (Vec<u64>, Vec<u64>) {
    if mag_cmp(a, b) == Ordering::Less {
        return (Vec::new(), a.to_vec());
    }
    if b.len() == 1 {
        let (q, r) = mag_divrem_small(a, b[0]);
        let rem = if r == 0 { Vec::new() } else { vec![r] };
        return (q, rem);
    }
    let base: u128 = 1 << 64;
    // Normalize so the divisor's top limb has its high bit set.
    let s = b[b.len() - 1].leading_zeros();
    let v = mag_shl(b, s as u64);
    let n = v.len();
    let mut u = mag_shl(a, s as u64);
    u.resize(a.len() + 1, 0);
    let m = a.len() - n;
    let mut q = vec![0u64; m + 1];
    let v_top = v[n - 1] as u128;
    let v_next = v[n - 2] as u128;
    for j in (0..=m).rev() {
        // Estimate the next quotient limb from the top two limbs.
        let num = ((u[j + n] as u128) << 64) | u[j + n - 1] as u128;
        let mut qhat = num / v_top;
        let mut rhat = num % v_top;
        while qhat >= base || qhat * v_next > ((rhat << 64) | u[j + n - 2] as u128) {
            qhat -= 1;
            rhat += v_top;
            if rhat >= base {
                break;
            }
        }
        // Multiply and subtract qhat * v from u[j..=j+n].
        let mut borrow: i128 = 0;
        let mut carry: u128 = 0;
        for i in 0..n {
            let p = qhat * v[i] as u128 + carry;
            carry = p >> 64;
            let t = u[i + j] as i128 - (p as u64) as i128 - borrow;
            u[i + j] = t as u64;
            borrow = i128::from(t < 0);
        }
        let t = u[j + n] as i128 - carry as i128 - borrow;
        u[j + n] = t as u64;
        if t < 0 {
            // The estimate was one too large: add v back.
            qhat -= 1;
            let mut c: u128 = 0;
            for i in 0..n {
                let sum = u[i + j] as u128 + v[i] as u128 + c;
                u[i + j] = sum as u64;
                c = sum >> 64;
            }
            u[j + n] = u[j + n].wrapping_add(c as u64);
        }
        q[j] = qhat as u64;
    }
    let rem = mag_shr(&trim(u[..n].to_vec()), s as u64);
    (trim(q), rem)
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
        let hi = if s > 0 && i + 1 < src.len() {
            src[i + 1] << (64 - s)
        } else {
            0
        };
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
        BigInt {
            neg: false,
            mag: Vec::new(),
        }
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
        let m = self
            .mag
            .iter()
            .rev()
            .fold(0u128, |acc, &l| (acc << 64) | l as u128);
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
        Some(
            self.mag
                .iter()
                .rev()
                .fold(0u128, |acc, &l| (acc << 64) | l as u128),
        )
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
        Some((
            BigInt::make(self.neg != o.neg, q),
            BigInt::make(self.neg, r),
        ))
    }

    /// Greatest common divisor of the absolute values. Always non-negative.
    pub fn gcd(&self, o: &BigInt) -> BigInt {
        BigInt::make(false, mag_gcd(&self.mag, &o.mag))
    }

    /// Floor of the square root. `None` for negative values.
    pub fn isqrt(&self) -> Option<BigInt> {
        if self.neg {
            return None;
        }
        if self.is_zero() {
            return Some(BigInt::zero());
        }
        // Newton iteration from above: starts at 2^ceil(bits/2) >= sqrt(self).
        let mut x = BigInt::make(false, mag_shl(&[1], bit_len(&self.mag).div_ceil(2)));
        loop {
            let (q, _) = self.divrem(&x)?;
            let y = BigInt::make(false, mag_shr(&x.add(&q).mag, 1));
            if y >= x {
                return Some(x);
            }
            x = y;
        }
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
        for (a, b) in [
            (7i128, -3i128),
            (-12345, 678),
            (0, -5),
            (i64::MAX as i128, i64::MIN as i128),
        ] {
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
        assert_eq!(
            format!("{p}"),
            "1606938044258990275541962092341162602522202993782792835301376"
        );
        assert_eq!(p.to_i128(), None);
        assert_eq!(
            big("-123456789012345678901234567890").to_string(),
            "-123456789012345678901234567890"
        );
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

    /// Deterministic pseudo-random integer with about `limbs` 64-bit limbs.
    fn pseudo_random(seed: &mut u64, limbs: usize) -> BigInt {
        let mut mag = Vec::with_capacity(limbs);
        for _ in 0..limbs {
            *seed ^= *seed << 13;
            *seed ^= *seed >> 7;
            *seed ^= *seed << 17;
            mag.push(*seed);
        }
        BigInt::make(false, trim(mag))
    }

    #[test]
    fn division_identity_holds_on_random_multi_limb_operands() {
        let mut seed = 0x9E37_79B9_7F4A_7C15u64;
        for round in 0..200 {
            let a = pseudo_random(&mut seed, 1 + round % 17);
            let b = pseudo_random(&mut seed, 1 + round % 9);
            if b.is_zero() {
                continue;
            }
            let (q, r) = a.divrem(&b).unwrap();
            assert!(r < b, "remainder not reduced in round {round}");
            assert_eq!(q.mul(&b).add(&r), a, "q*b + r != a in round {round}");
        }
    }

    /// Integer square root of 2 · 10^(2k) gives floor(sqrt(2) · 10^k).
    fn sqrt2_digits(k: u32) -> BigInt {
        let mut ten_pow = BigInt::one();
        let ten_19 = BigInt::from_i128(10_000_000_000_000_000_000);
        for _ in 0..(2 * k / 19) {
            ten_pow = ten_pow.mul(&ten_19);
        }
        for _ in 0..(2 * k % 19) {
            ten_pow = ten_pow.mul(&BigInt::from_i128(10));
        }
        BigInt::from_i128(2).mul(&ten_pow).isqrt().unwrap()
    }

    #[test]
    fn sqrt2_to_ten_thousand_decimal_places() {
        let k = 10_000;
        let root = sqrt2_digits(k);
        let digits = root.to_string();
        // floor(sqrt(2) * 10^k) has k + 1 digits.
        assert_eq!(digits.len(), k as usize + 1);
        // Known leading digits of sqrt(2).
        assert!(
            digits.starts_with("14142135623730950488016887242096980785696718753769"),
            "{}",
            &digits[..60]
        );
        // Exact bounds: root^2 <= 2*10^(2k) < (root+1)^2, checked in integers.
        let target = BigInt::from_i128(2).mul(&sqrt2_digits_square_base(k));
        let next = root.add(&BigInt::one());
        assert!(root.mul(&root) <= target);
        assert!(next.mul(&next) > target);
    }

    /// 10^(2k) as a BigInt, the scale for the bound checks.
    fn sqrt2_digits_square_base(k: u32) -> BigInt {
        let mut acc = BigInt::one();
        let ten = BigInt::from_i128(10);
        for _ in 0..(2 * k) {
            acc = acc.mul(&ten);
        }
        acc
    }
}

#[cfg(test)]
mod differential_tests {
    use super::*;
    use crate::testkit::Rng;

    /// A random i128 whose magnitude is spread across the full bit range, so small,
    /// single-limb and multi-limb operands all occur.
    fn sample(rng: &mut Rng) -> i128 {
        let raw = ((rng.next_u64() as u128) << 64) | rng.next_u64() as u128;
        let shift = rng.below(128) as u32;
        let v = (raw >> shift) as i128;
        // wrapping_neg: negating i128::MIN must not panic in a debug build.
        if rng.below(2) == 0 {
            v
        } else {
            v.wrapping_neg()
        }
    }

    /// `BigInt` agrees with `i128` wherever the `i128` result is defined.
    #[test]
    fn arithmetic_matches_i128_where_it_fits() {
        let mut rng = Rng::new(31337);
        for _ in 0..5000 {
            let (a, b) = (sample(&mut rng), sample(&mut rng));
            let (ba, bb) = (BigInt::from_i128(a), BigInt::from_i128(b));
            if let Some(s) = a.checked_add(b) {
                assert_eq!(ba.add(&bb).to_i128(), Some(s), "{a} + {b}");
            }
            if let Some(d) = a.checked_sub(b) {
                assert_eq!(ba.sub(&bb).to_i128(), Some(d), "{a} - {b}");
            }
            if let Some(p) = a.checked_mul(b) {
                assert_eq!(ba.mul(&bb).to_i128(), Some(p), "{a} * {b}");
            }
            if let (Some(q), Some(r)) = (a.checked_div(b), a.checked_rem(b)) {
                let (bq, br) = ba.divrem(&bb).expect("divisor is non-zero");
                assert_eq!(bq.to_i128(), Some(q), "{a} / {b}");
                assert_eq!(br.to_i128(), Some(r), "{a} % {b}");
            }
        }
    }

    /// Division identity on values too large for i128: a = q*b + r, 0 <= |r| < |b|.
    #[test]
    fn division_identity_holds_beyond_i128() {
        let mut rng = Rng::new(271828);
        for _ in 0..500 {
            let a = BigInt::from_i128(sample(&mut rng)).mul(&BigInt::from_i128(sample(&mut rng)));
            let b = BigInt::from_i128(sample(&mut rng)).mul(&BigInt::from_i128(sample(&mut rng)));
            if b.is_zero() {
                continue;
            }
            let (q, r) = a.divrem(&b).expect("divisor is non-zero");
            assert_eq!(q.mul(&b).add(&r), a, "a = q*b + r failed");
            assert!(r.abs() < b.abs(), "remainder not smaller than divisor");
        }
    }
}
