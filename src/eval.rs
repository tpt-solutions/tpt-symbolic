//! Exact evaluation at rational points, and exact decimal output.
//!
//! Everything here is rational arithmetic. An expression whose value is
//! irrational (`sqrt(2)`, `sin(1)`) is reported as [`Error::Unsupported`] rather
//! than rounded. Numeric approximation belongs to the `tpt-math` hand-off.

use crate::bigint::BigInt;
use crate::error::Error;
use crate::expr::Expr;
use crate::mathr::Number;
use alloc::string::{String, ToString};

/// Evaluates `e` exactly, taking each variable's value from `env`.
///
/// Powers need an integer exponent, because a non-integer power is usually
/// irrational. Functions and logical nodes have no exact rational value.
///
/// # Examples
///
/// ```
/// use tpt_symbolic::eval::eval;
/// use tpt_symbolic::parse::parse;
/// use tpt_symbolic::Number;
///
/// let e = parse("x^2 + 1/2").unwrap();
/// let v = eval(&e, &[("x", Number::new(3, 2).unwrap())]).unwrap();
/// assert_eq!(v, Number::new(11, 4).unwrap());
/// ```
pub fn eval(e: &Expr, env: &[(&str, Number)]) -> Result<Number, Error> {
    match e {
        Expr::Const(n) => Ok(n.clone()),
        Expr::Var(name) => env
            .iter()
            .find(|(n, _)| *n == &**name)
            .map(|(_, v)| v.clone())
            .ok_or(Error::Unsupported(
                "evaluation needs a value for every variable",
            )),
        Expr::Add(terms) => {
            let mut acc = Number::zero();
            for t in terms {
                let v = eval(t, env)?;
                acc = acc.checked_add(&v).ok_or(Error::Overflow)?;
            }
            Ok(acc)
        }
        Expr::Mul(factors) => {
            let mut acc = Number::one();
            for f in factors {
                let v = eval(f, env)?;
                acc = acc.checked_mul(&v).ok_or(Error::Overflow)?;
            }
            Ok(acc)
        }
        Expr::Pow(base, exp) => {
            let b = eval(base, env)?;
            let x = eval(exp, env)?;
            if !x.is_integer() {
                return Err(Error::Unsupported(
                    "non-integer power has no exact rational value",
                ));
            }
            let k = x
                .as_i128()
                .and_then(|v| i64::try_from(v).ok())
                .ok_or(Error::Overflow)?;
            b.checked_pow(k).ok_or(if b.is_zero() && k < 0 {
                Error::DivisionByZero
            } else {
                Error::Overflow
            })
        }
        Expr::Func(..) => Err(Error::Unsupported("functions have no exact rational value")),
        Expr::Bool(_) | Expr::Eq(..) | Expr::And(_) | Expr::Or(_) | Expr::Not(_) => Err(
            Error::Unsupported("only arithmetic expressions can be evaluated"),
        ),
    }
}

/// Replaces every occurrence of `var` with `value`, rebuilding canonically.
///
/// # Examples
///
/// ```
/// use tpt_symbolic::eval::subs;
/// use tpt_symbolic::parse::parse;
/// use tpt_symbolic::Expr;
///
/// let e = parse("x^2 + x").unwrap();
/// assert_eq!(subs(&e, "x", &Expr::int(3)), Expr::int(12));
/// ```
pub fn subs(e: &Expr, var: &str, value: &Expr) -> Expr {
    e.replace(&Expr::var(var), value)
}

/// Writes `n` in decimal with exactly `digits` places after the point.
///
/// The digits are truncated, not rounded: `to_decimal(2/3, 3)` is `"0.666"`.
/// A rational with a finite decimal expansion, such as `1/8`, is exact once
/// `digits` covers the expansion; otherwise the output is a truncation of the
/// infinite expansion.
///
/// # Examples
///
/// ```
/// use tpt_symbolic::eval::to_decimal;
/// use tpt_symbolic::Number;
///
/// assert_eq!(to_decimal(&Number::new(-1, 8).unwrap(), 4), "-0.1250");
/// assert_eq!(to_decimal(&Number::new(2, 3).unwrap(), 3), "0.666");
/// ```
pub fn to_decimal(n: &Number, digits: usize) -> String {
    let ten = BigInt::from_i128(10);
    let den = n.denom();
    let (int_part, mut rem) = n
        .numer()
        .abs()
        .divrem(den)
        .unwrap_or((BigInt::zero(), BigInt::zero()));
    let mut out = String::new();
    if n.is_negative() {
        out.push('-');
    }
    out.push_str(&int_part.to_string());
    if digits > 0 {
        out.push('.');
        for _ in 0..digits {
            let (digit, next) = rem
                .mul(&ten)
                .divrem(den)
                .unwrap_or((BigInt::zero(), BigInt::zero()));
            let d = digit.to_i128().unwrap_or(0);
            out.push(char::from(b'0' + d as u8));
            rem = next;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse;

    fn q(n: i128, d: i128) -> Number {
        Number::new(n, d).unwrap()
    }

    #[test]
    fn evaluates_polynomials_and_rational_powers_exactly() {
        let e = parse("x^3 - 2*x*y + 1/3").unwrap();
        let v = eval(&e, &[("x", q(2, 1)), ("y", q(1, 2))]).unwrap();
        // 2^3 - 2*2*(1/2) + 1/3 = 8 - 2 + 1/3
        assert_eq!(v, q(19, 3));
    }

    #[test]
    fn negative_powers_of_zero_are_division_errors() {
        let e = parse("x^-1").unwrap();
        assert_eq!(eval(&e, &[("x", q(0, 1))]), Err(Error::DivisionByZero));
    }

    #[test]
    fn irrational_or_unbound_values_are_unsupported_not_rounded() {
        let root2 = parse("sqrt(2)").unwrap();
        assert!(matches!(eval(&root2, &[]), Err(Error::Unsupported(_))));
        let open = parse("x + 1").unwrap();
        assert!(matches!(eval(&open, &[]), Err(Error::Unsupported(_))));
    }

    #[test]
    fn decimal_output_truncates_and_keeps_sign() {
        assert_eq!(to_decimal(&q(1, 3), 5), "0.33333");
        assert_eq!(to_decimal(&q(-7, 4), 2), "-1.75");
        assert_eq!(to_decimal(&q(5, 1), 0), "5");
        assert_eq!(to_decimal(&q(-1, 3), 2), "-0.33");
        // 1/2^100 = 7.88...e-31: thirty zeros after the point, then the first
        // digit. The denominator is past the 64-bit limb boundary.
        let tiny = Number::from_parts(BigInt::one(), BigInt::from_i128(1 << 100)).unwrap();
        let expected = alloc::format!("0.{}788", "0".repeat(30));
        assert!(to_decimal(&tiny, 33).starts_with(&expected));
    }
}
