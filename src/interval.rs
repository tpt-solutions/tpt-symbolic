//! Rigorous interval enclosures for polynomial and rational expressions.
//!
//! Endpoints are exact rationals, so every enclosure is a proof and no rounding can break
//! it. [`enclose`] returns an interval that contains every value the expression takes on
//! the input interval. When that interval does not contain zero, the expression has no root
//! there, and [`isolate_roots`] uses this to discard regions.
//!
//! Only polynomials and rational functions are supported. Transcendental functions and
//! constants such as `pi` have no exact rational endpoints, so [`enclose`] returns `None`
//! for them.

use crate::error::Error;
use crate::expr::Expr;
use crate::mathr::Number;
use alloc::vec::Vec;

/// Most intervals [`isolate_roots`] will return before giving up.
pub const MAX_ISOLATED: usize = 4096;

/// A closed interval `[lo, hi]` with exact rational endpoints, `lo <= hi`.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Interval {
    /// Left endpoint.
    pub lo: Number,
    /// Right endpoint.
    pub hi: Number,
}

impl Interval {
    /// The interval `[lo, hi]`. `None` when `lo > hi`.
    pub fn new(lo: Number, hi: Number) -> Option<Interval> {
        if lo.cmp_value(&hi) == core::cmp::Ordering::Greater {
            None
        } else {
            Some(Interval { lo, hi })
        }
    }

    /// `true` if `x` lies in the interval.
    pub fn contains(&self, x: &Number) -> bool {
        self.lo.cmp_value(x) != core::cmp::Ordering::Greater
            && x.cmp_value(&self.hi) != core::cmp::Ordering::Greater
    }

    /// `true` if zero lies in the interval.
    pub fn contains_zero(&self) -> bool {
        self.contains(&Number::zero())
    }

    /// Sum of two intervals.
    pub fn add(&self, o: &Interval) -> Option<Interval> {
        Interval::new(self.lo.checked_add(&o.lo)?, self.hi.checked_add(&o.hi)?)
    }

    /// Product of two intervals: the range of the four endpoint products.
    pub fn mul(&self, o: &Interval) -> Option<Interval> {
        let products = [
            self.lo.checked_mul(&o.lo)?,
            self.lo.checked_mul(&o.hi)?,
            self.hi.checked_mul(&o.lo)?,
            self.hi.checked_mul(&o.hi)?,
        ];
        let lo = min_of(&products)?;
        let hi = max_of(&products)?;
        Interval::new(lo, hi)
    }

    /// `1 / self`. `None` when the interval contains zero.
    pub fn recip(&self) -> Option<Interval> {
        if self.contains_zero() {
            return None;
        }
        Interval::new(self.hi.checked_recip()?, self.lo.checked_recip()?)
    }

    /// `self^n` for an integer `n`. `None` when `n` is negative and the interval contains zero.
    pub fn powi(&self, n: i64) -> Option<Interval> {
        if n < 0 {
            return self.powi(n.checked_neg()?)?.recip();
        }
        if n == 0 {
            return Interval::new(Number::one(), Number::one());
        }
        // Powers are monotone on each side of zero, so the endpoints give the range. Using
        // repeated products instead would lose the fact that even powers are non-negative.
        let at_lo = self.lo.checked_pow(n)?;
        let at_hi = self.hi.checked_pow(n)?;
        if n % 2 == 1 {
            return Interval::new(at_lo, at_hi);
        }
        if !self.lo.is_negative() {
            Interval::new(at_lo, at_hi)
        } else if self.hi.is_negative() || self.hi.is_zero() {
            Interval::new(at_hi, at_lo)
        } else {
            Interval::new(Number::zero(), max_of(&[at_lo, at_hi])?)
        }
    }
}

fn min_of(xs: &[Number]) -> Option<Number> {
    xs.iter().cloned().reduce(|a, b| {
        if a.cmp_value(&b) == core::cmp::Ordering::Greater {
            b
        } else {
            a
        }
    })
}

fn max_of(xs: &[Number]) -> Option<Number> {
    xs.iter().cloned().reduce(|a, b| {
        if a.cmp_value(&b) == core::cmp::Ordering::Less {
            b
        } else {
            a
        }
    })
}

/// An interval containing every value of `e` as `var` ranges over `iv`.
///
/// `None` when `e` has a symbol other than `var`, a non-rational constant or function,
/// a division by an interval that contains zero, or a boolean or equation node.
/// # Example
/// ```
/// use tpt_symbolic::interval::{enclose, Interval};
/// use tpt_symbolic::parse::parse;
/// use tpt_symbolic::Number;
/// let iv = Interval::new(Number::from_int(1), Number::from_int(2)).unwrap();
/// let r = enclose(&parse("x^2 - 2").unwrap(), "x", &iv).unwrap();
/// assert_eq!(r.lo, Number::from_int(-1));
/// assert_eq!(r.hi, Number::from_int(2));
/// ```
pub fn enclose(e: &Expr, var: &str, iv: &Interval) -> Option<Interval> {
    match e {
        Expr::Var(v) if &**v == var => Some(iv.clone()),
        Expr::Const(c) => Interval::new(c.clone(), c.clone()),
        Expr::Add(v) => v
            .iter()
            .try_fold(Interval::new(Number::zero(), Number::zero())?, |acc, t| {
                acc.add(&enclose(t, var, iv)?)
            }),
        Expr::Mul(v) => v
            .iter()
            .try_fold(Interval::new(Number::one(), Number::one())?, |acc, t| {
                acc.mul(&enclose(t, var, iv)?)
            }),
        Expr::Pow(base, exp) => {
            let n = exp.as_number()?;
            if !n.is_integer() {
                return None;
            }
            let n = i64::try_from(n.as_i128()?).ok()?;
            enclose(base, var, iv)?.powi(n)
        }
        _ => None,
    }
}

/// `true` if the expression has no root in `iv`: its enclosure excludes zero.
///
/// A `false` result means only that the enclosure contains zero, not that a root exists.
pub fn excludes_zero(e: &Expr, var: &str, iv: &Interval) -> bool {
    enclose(e, var, iv).is_some_and(|r| !r.contains_zero())
}

/// Intervals that together contain every root of `e` in `[lo, hi]`.
///
/// Bisects until each piece is either proved root-free or is narrower than `2^-depth` of
/// the original width. Every root lies in some returned interval, but a returned interval
/// need not contain a root. Errors when `e` cannot be enclosed, or when more than
/// [`MAX_ISOLATED`] pieces would be needed.
pub fn isolate_roots(
    e: &Expr,
    var: &str,
    lo: &Number,
    hi: &Number,
    depth: u32,
) -> Result<Vec<Interval>, Error> {
    let whole = Interval::new(lo.clone(), hi.clone())
        .ok_or(Error::Unsupported("interval bounds are out of order"))?;
    let min_width = {
        let mut w = whole.hi.checked_sub(&whole.lo).ok_or(Error::Overflow)?;
        for _ in 0..depth {
            w = w.checked_div(&Number::from_int(2)).ok_or(Error::Overflow)?;
        }
        w
    };
    let mut out = Vec::new();
    let mut stack = alloc::vec![whole];
    while let Some(iv) = stack.pop() {
        if enclose(e, var, &iv).is_none() {
            return Err(Error::Unsupported(
                "expression cannot be enclosed by intervals",
            ));
        }
        if excludes_zero(e, var, &iv) {
            continue;
        }
        let width = iv.hi.checked_sub(&iv.lo).ok_or(Error::Overflow)?;
        if width.cmp_value(&min_width) != core::cmp::Ordering::Greater {
            out.push(iv);
        } else {
            let mid = iv
                .lo
                .checked_add(&iv.hi)
                .and_then(|s| s.checked_div(&Number::from_int(2)))
                .ok_or(Error::Overflow)?;
            stack.push(Interval::new(iv.lo.clone(), mid.clone()).ok_or(Error::Overflow)?);
            stack.push(Interval::new(mid, iv.hi.clone()).ok_or(Error::Overflow)?);
        }
        if out.len() + stack.len() > MAX_ISOLATED {
            return Err(Error::Unsupported(
                "too many intervals to isolate the roots",
            ));
        }
    }
    out.sort_by(|a, b| a.lo.cmp_value(&b.lo));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse;

    fn p(s: &str) -> Expr {
        parse(s).unwrap_or_else(|e| panic!("parse {s:?}: {e}"))
    }

    fn iv(lo: i128, hi: i128) -> Interval {
        Interval::new(Number::from_int(lo), Number::from_int(hi)).unwrap()
    }

    #[test]
    fn polynomial_enclosure_contains_all_values() {
        // x^2 - 2 on [1, 2] lies in [-1, 2].
        let r = enclose(&p("x^2 - 2"), "x", &iv(1, 2)).unwrap();
        assert_eq!(r, iv(-1, 2));
    }

    #[test]
    fn sign_proofs_exclude_roots() {
        // x^2 + 1 has no root anywhere on [-10, 10].
        assert!(excludes_zero(&p("x^2 + 1"), "x", &iv(-10, 10)));
        // x^2 - 4 has a root at 2, so it is not excluded on [1, 3].
        assert!(!excludes_zero(&p("x^2 - 4"), "x", &iv(1, 3)));
        assert!(excludes_zero(&p("x^2 - 4"), "x", &iv(3, 5)));
    }

    #[test]
    fn rational_functions_need_nonzero_denominators() {
        // 1/x on [1, 2] is enclosed; on [-1, 1] it is not, since x can be 0.
        assert!(enclose(&p("1/x"), "x", &iv(1, 2)).is_some());
        assert!(enclose(&p("1/x"), "x", &iv(-1, 1)).is_none());
    }

    #[test]
    fn transcendental_and_symbolic_forms_are_not_enclosed() {
        assert!(enclose(&p("sin(x)"), "x", &iv(0, 1)).is_none());
        assert!(enclose(&p("pi*x"), "x", &iv(0, 1)).is_none());
        assert!(enclose(&p("y*x"), "x", &iv(0, 1)).is_none());
    }

    #[test]
    fn isolates_the_positive_root_of_x2_minus_2() {
        let roots = isolate_roots(
            &p("x^2 - 2"),
            "x",
            &Number::zero(),
            &Number::from_int(2),
            30,
        )
        .unwrap();
        // The root sqrt(2) lies in exactly one returned interval, which is narrow.
        assert_eq!(roots.len(), 1);
        let r = &roots[0];
        let two = Number::from_int(2);
        // lo^2 <= 2 <= hi^2, so the interval brackets sqrt(2).
        assert!(r.lo.checked_mul(&r.lo).unwrap().cmp_value(&two) != core::cmp::Ordering::Greater);
        assert!(r.hi.checked_mul(&r.hi).unwrap().cmp_value(&two) != core::cmp::Ordering::Less);
    }

    #[test]
    fn isolates_nothing_when_there_is_no_root() {
        let roots = isolate_roots(
            &p("x^2 + 1"),
            "x",
            &Number::from_int(-5),
            &Number::from_int(5),
            10,
        )
        .unwrap();
        assert!(roots.is_empty());
    }
}
