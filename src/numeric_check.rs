//! Test-only numeric evaluation, used to check exact results against their equations and
//! integrands. It works in `f64` and is compiled only for tests, so the core stays exact.
//!
//! Principal branches are used throughout: `z^x = exp(x log z)`, and `log` is the principal
//! logarithm. `atan`, `abs` and `log` of a negative real are real-only where stated.

use crate::expr::{Expr, SymbolicFunc};
use crate::mathr::Number;
use alloc::format;
use alloc::vec;

/// A complex number as `(re, im)`.
pub type C = (f64, f64);

fn mul(a: C, b: C) -> C {
    (a.0 * b.0 - a.1 * b.1, a.0 * b.1 + a.1 * b.0)
}

fn add(a: C, b: C) -> C {
    (a.0 + b.0, a.1 + b.1)
}

fn div(a: C, b: C) -> Option<C> {
    let d = b.0 * b.0 + b.1 * b.1;
    if d == 0.0 {
        return None;
    }
    Some(((a.0 * b.0 + a.1 * b.1) / d, (a.1 * b.0 - a.0 * b.1) / d))
}

fn exp(z: C) -> C {
    let m = z.0.exp();
    (m * z.1.cos(), m * z.1.sin())
}

fn log(z: C) -> Option<C> {
    let r = z.0.hypot(z.1);
    if r == 0.0 {
        return None;
    }
    Some((r.ln(), z.1.atan2(z.0)))
}

fn pow_real_exponent(z: C, x: f64) -> Option<C> {
    if z.0 == 0.0 && z.1 == 0.0 {
        return if x > 0.0 { Some((0.0, 0.0)) } else { None };
    }
    Some(exp((log(z)?.0 * x, log(z)?.1 * x)))
}

fn cos(z: C) -> C {
    (z.0.cos() * z.1.cosh(), -z.0.sin() * z.1.sinh())
}

fn sin(z: C) -> C {
    (z.0.sin() * z.1.cosh(), z.0.cos() * z.1.sinh())
}

fn to_f64(n: &Number) -> Option<f64> {
    let num: f64 = format!("{}", n.numer()).parse().ok()?;
    let den: f64 = format!("{}", n.denom()).parse().ok()?;
    Some(num / den)
}

/// Evaluates `e` at the value `z` for the variable `var`, with complex arithmetic.
/// `None` for symbols other than `var`, and for forms this evaluator does not cover.
pub fn eval(e: &Expr, var: &str, z: C) -> Option<C> {
    Some(match e {
        Expr::Const(n) => (to_f64(n)?, 0.0),
        Expr::I => (0.0, 1.0),
        Expr::Pi => (core::f64::consts::PI, 0.0),
        Expr::E => (core::f64::consts::E, 0.0),
        Expr::Var(v) if &**v == var => z,
        Expr::Add(v) => v
            .iter()
            .try_fold((0.0, 0.0), |acc, t| Some(add(acc, eval(t, var, z)?)))?,
        Expr::Mul(v) => v
            .iter()
            .try_fold((1.0, 0.0), |acc, t| Some(mul(acc, eval(t, var, z)?)))?,
        Expr::Pow(b, x) => {
            let base = eval(b, var, z)?;
            let n = x.as_number()?;
            if n.is_integer() {
                let k = n.as_i128()?;
                let mut acc = (1.0, 0.0);
                for _ in 0..k.unsigned_abs() {
                    acc = mul(acc, base);
                }
                if k < 0 {
                    div((1.0, 0.0), acc)?
                } else {
                    acc
                }
            } else {
                pow_real_exponent(base, to_f64(&n)?)?
            }
        }
        Expr::Func(f, a) => {
            let arg = eval(a, var, z)?;
            match f {
                SymbolicFunc::Exp => exp(arg),
                SymbolicFunc::Log => log(arg)?,
                SymbolicFunc::Sin => sin(arg),
                SymbolicFunc::Cos => cos(arg),
                SymbolicFunc::Tan => div(sin(arg), cos(arg))?,
                // With iz = i * arg: sinh(z) = -i sin(iz), cosh(z) = cos(iz).
                SymbolicFunc::Sinh => {
                    let s = sin((-arg.1, arg.0));
                    (s.1, -s.0)
                }
                SymbolicFunc::Cosh => cos((-arg.1, arg.0)),
                SymbolicFunc::Tanh => {
                    let s = sin((-arg.1, arg.0));
                    div((s.1, -s.0), cos((-arg.1, arg.0)))?
                }
                SymbolicFunc::Abs => {
                    if arg.1 != 0.0 {
                        return None;
                    }
                    (arg.0.abs(), 0.0)
                }
                SymbolicFunc::Atan => {
                    if arg.1 != 0.0 {
                        return None;
                    }
                    (arg.0.atan(), 0.0)
                }
                _ => return None,
            }
        }
        _ => return None,
    })
}

/// Evaluates a polynomial with real coefficients, highest degree first, at `z`.
pub fn poly_at(coeffs: &[f64], z: C) -> C {
    coeffs
        .iter()
        .fold((0.0, 0.0), |acc, c| add(mul(acc, z), (*c, 0.0)))
}

/// A small deterministic generator for reproducible random cases.
pub struct Lcg(u64);

impl Lcg {
    /// A generator with the given seed.
    pub fn new(seed: u64) -> Lcg {
        Lcg(seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407))
    }

    /// The next value in `lo..=hi`.
    pub fn range(&mut self, lo: i64, hi: i64) -> i64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let span = (hi - lo + 1) as u64;
        lo + ((self.0 >> 33) % span) as i64
    }
}

/// `true` if `v` is close to zero relative to the scale `s` of the terms that formed it.
pub fn near_zero(v: C, scale: f64) -> bool {
    let tol = 1e-7 * (1.0 + scale);
    v.0.abs() <= tol && v.1.abs() <= tol
}

/// The scale of a polynomial at `z`: the sum of `|c_i| |z|^i`.
pub fn poly_scale(coeffs: &[f64], z: C) -> f64 {
    let r = z.0.hypot(z.1);
    coeffs
        .iter()
        .rev()
        .enumerate()
        .map(|(i, c)| c.abs() * r.powi(i as i32))
        .sum()
}

#[cfg(test)]
mod self_tests {
    use super::*;
    use crate::parse::parse;

    #[test]
    fn evaluates_the_basics() {
        let e = parse("x^2 + 2*x*i").unwrap();
        let v = eval(&e, "x", (1.0, 0.0)).unwrap();
        assert!((v.0 - 1.0).abs() < 1e-12 && (v.1 - 2.0).abs() < 1e-12);
        let l = eval(&parse("log(x)").unwrap(), "x", (-1.0, 0.0)).unwrap();
        assert!((l.1 - core::f64::consts::PI).abs() < 1e-12);
    }

    #[test]
    fn lcg_stays_in_range() {
        let mut g = Lcg::new(7);
        for _ in 0..200 {
            let v = g.range(-3, 3);
            assert!((-3..=3).contains(&v));
        }
        let _ = vec![0u8];
    }
}
