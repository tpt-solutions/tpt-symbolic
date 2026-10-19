//! Taylor series and limits.
//!
//! [`taylor`] is exact: it is built from the derivatives that [`crate::calculus::diff_n`]
//! already computes.
//!
//! [`limit`] and [`limit_at_infinity`] are conservative. They use direct substitution
//! when the result is defined, L'Hôpital's rule for `0/0` and `∞/∞` quotients, and
//! degree comparison for rational functions at infinity. Anything else is
//! [`Limit::Unevaluated`]. A limit is never reported unless one of those rules proves it.

use crate::calculus::{diff, diff_n};
use crate::error::Error;
use crate::eval::subs;
use crate::expr::{Expr, SymbolicFunc};
use crate::mathr::Number;
use crate::simplify::{expand, simplify};
use alloc::vec;
use alloc::vec::Vec;

/// Largest order accepted by [`taylor`].
pub const MAX_TAYLOR_ORDER: usize = 32;

/// How many times L'Hôpital's rule is applied before giving up.
const MAX_LHOPITAL_STEPS: usize = 4;

/// Result of a limit.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Limit {
    /// The limit is this finite value.
    Finite(Expr),
    /// The function grows without bound and positively.
    PositiveInfinity,
    /// The function grows without bound and negatively.
    NegativeInfinity,
    /// No rule proved the limit. Carries the original expression.
    Unevaluated(Expr),
}

/// The Taylor polynomial of `e` about `point`, of the given `order` (the highest power).
///
/// `sum_{k=0}^{order} e^(k)(point) / k! * (var - point)^k`, simplified. Requires that
/// the derivatives can be evaluated at `point` (the result may contain `undefined`
/// forms if they cannot).
pub fn taylor(e: &Expr, var: &str, point: &Expr, order: usize) -> Result<Expr, Error> {
    if order > MAX_TAYLOR_ORDER {
        return Err(Error::Unsupported("taylor order is too large"));
    }
    let mut terms = Vec::with_capacity(order + 1);
    let mut factorial = Number::one();
    for k in 0..=order {
        if k > 0 {
            factorial = factorial
                .checked_mul(&Number::from_int(k as i128))
                .ok_or(Error::Overflow)?;
        }
        let dk = diff_n(e, var, k)?;
        let coefficient = simplify(&subs(&dk, var, point));
        let inv_factorial = factorial.checked_recip().ok_or(Error::Overflow)?;
        let shifted = Expr::power(
            Expr::difference(Expr::var(var), point.clone()),
            Expr::int(k as i128),
        );
        terms.push(Expr::product(vec![
            coefficient,
            Expr::Const(inv_factorial),
            shifted,
        ]));
    }
    Ok(simplify(&expand(&Expr::sum(terms))))
}

/// The limit of `e` as `var` tends to `point`.
pub fn limit(e: &Expr, var: &str, point: &Expr) -> Limit {
    finite_limit(&simplify(e), var, point, 0)
        .map(Limit::Finite)
        .unwrap_or_else(|| Limit::Unevaluated(e.clone()))
}

/// The limit of `e` as `var` tends to `+∞` (`positive = true`) or `-∞`.
pub fn limit_at_infinity(e: &Expr, var: &str, positive: bool) -> Limit {
    let f = simplify(e);
    // A polynomial is a quotient over one. Otherwise the terms need one shared denominator.
    let (num, den) = if monomial_terms(&f, var).is_some() {
        (f.clone(), Expr::one())
    } else {
        match combined_quotient(&f) {
            Some(q) => q,
            None => return Limit::Unevaluated(e.clone()),
        }
    };
    let (Some(p), Some(q)) = (monomial_terms(&num, var), monomial_terms(&den, var)) else {
        return Limit::Unevaluated(e.clone());
    };
    let (Some(dp), Some(dq)) = (degree(&p), degree(&q)) else {
        return Limit::Unevaluated(e.clone());
    };
    let lead_p = leading(&p, dp);
    let lead_q = leading(&q, dq);
    // Leading coefficients must be numeric so that the sign of the limit is known.
    let (Some(cp), Some(cq)) = (lead_p.as_number(), lead_q.as_number()) else {
        return Limit::Unevaluated(e.clone());
    };
    if dp < dq {
        return Limit::Finite(Expr::zero());
    }
    if dp == dq {
        let ratio = cp.checked_div(&cq);
        return match ratio {
            Some(r) => Limit::Finite(Expr::Const(r)),
            None => Limit::Unevaluated(e.clone()),
        };
    }
    // Infinite. Its sign is the sign of the leading ratio, flipped at -∞ for odd differences.
    let mut negative = cp.is_negative() != cq.is_negative();
    if !positive && (dp - dq) % 2 == 1 {
        negative = !negative;
    }
    if negative {
        Limit::NegativeInfinity
    } else {
        Limit::PositiveInfinity
    }
}

fn finite_limit(f: &Expr, var: &str, point: &Expr, depth: usize) -> Option<Expr> {
    // Decide regularity on the tree before substitution: the smart constructors fold
    // `0 * 0^-1` to `0`, so the substituted value can hide a division by zero.
    if is_regular_at(f, var, point) {
        let value = simplify(&subs(f, var, point));
        if is_defined(&value) {
            return Some(value);
        }
    }
    if depth >= MAX_LHOPITAL_STEPS {
        return None;
    }
    // L'Hôpital: for num/den with both zero at `point`, the limit equals the limit of
    // num'/den'. Only quotients of that exact shape are handled.
    let (num, den) = combined_quotient(f)?;
    let num_at = simplify(&subs(&num, var, point));
    let den_at = simplify(&subs(&den, var, point));
    if !(is_zero(&num_at) && is_zero(&den_at)) {
        return None;
    }
    let quotient = Expr::quotient(diff(&num, var).ok()?, diff(&den, var).ok()?);
    finite_limit(&simplify(&quotient), var, point, depth + 1)
}

/// `true` when every division, logarithm and fractional power in `e` is safe at `point`.
///
/// Checks each denominator and log argument on its own, by substituting into that
/// subexpression only.
fn is_regular_at(e: &Expr, var: &str, point: &Expr) -> bool {
    match e {
        Expr::Pow(base, exp) => {
            if !is_regular_at(base, var, point) || !is_regular_at(exp, var, point) {
                return false;
            }
            let base_at = simplify(&subs(base, var, point));
            match exp.as_number() {
                Some(n) if n.is_negative() => !is_zero(&base_at) && is_defined(&base_at),
                Some(n) if !n.is_integer() => base_at.as_number().is_none_or(|b| !b.is_negative()),
                _ => true,
            }
        }
        Expr::Func(SymbolicFunc::Log, arg) => {
            let arg_at = simplify(&subs(arg, var, point));
            let positive = arg_at
                .as_number()
                .is_none_or(|a| !a.is_negative() && !a.is_zero());
            positive && is_regular_at(arg, var, point)
        }
        Expr::Func(_, arg) | Expr::Not(arg) => is_regular_at(arg, var, point),
        Expr::Add(v) | Expr::Mul(v) | Expr::And(v) | Expr::Or(v) => {
            v.iter().all(|t| is_regular_at(t, var, point))
        }
        Expr::Eq(a, b) => is_regular_at(a, var, point) && is_regular_at(b, var, point),
        _ => true,
    }
}

/// Writes a sum of quotients over one shared denominator as a single `(numerator, denominator)`.
/// Terms that are not quotients count as having denominator one, so they only combine when
/// the others share that denominator too. Returns `None` otherwise.
fn combined_quotient(f: &Expr) -> Option<(Expr, Expr)> {
    let Expr::Add(terms) = f else {
        return split_quotient(f);
    };
    let mut den: Option<Expr> = None;
    let mut nums = Vec::with_capacity(terms.len());
    for t in terms {
        let (n, d) = split_quotient(t).unwrap_or((t.clone(), Expr::one()));
        match &den {
            None => den = Some(d),
            Some(prev) if *prev == d => {}
            Some(_) => return None,
        }
        nums.push(n);
    }
    Some((Expr::sum(nums), den?))
}

fn is_zero(e: &Expr) -> bool {
    e.as_number().is_some_and(|n| n.is_zero())
}

/// `true` when the value `e` (after substitution) involves no division by zero, log of
/// zero or negative, or fractional power of a negative constant.
fn is_defined(e: &Expr) -> bool {
    match e {
        Expr::Var(_) => false,
        Expr::Const(_) | Expr::Pi | Expr::E | Expr::I | Expr::Bool(_) => true,
        Expr::Add(v) | Expr::Mul(v) | Expr::And(v) | Expr::Or(v) => v.iter().all(is_defined),
        Expr::Pow(base, exp) => {
            let base_zero_and_negative =
                is_zero(base) && exp.as_number().is_some_and(|n| n.is_negative());
            let fractional_of_negative = base.as_number().is_some_and(|b| b.is_negative())
                && exp.as_number().is_some_and(|n| !n.is_integer());
            !base_zero_and_negative
                && !fractional_of_negative
                && is_defined(base)
                && is_defined(exp)
        }
        Expr::Func(f, arg) => {
            let log_domain = matches!(f, SymbolicFunc::Log)
                && arg
                    .as_number()
                    .is_some_and(|a| !a.is_negative() && !a.is_zero());
            let log_ok = !matches!(f, SymbolicFunc::Log) || log_domain || arg.as_number().is_none();
            log_ok && is_defined(arg)
        }
        Expr::Eq(a, b) => is_defined(a) && is_defined(b),
        Expr::Not(a) => is_defined(a),
    }
}

/// Splits `f` into `(numerator, denominator)` when it is a product with a `D^-1` factor
/// (or is `D^-1` itself). Returns `None` otherwise.
fn split_quotient(f: &Expr) -> Option<(Expr, Expr)> {
    match f {
        Expr::Pow(base, exp) if exp.as_number() == Some(Number::neg_one()) => {
            Some((Expr::one(), (**base).clone()))
        }
        Expr::Mul(v) => {
            let idx = v.iter().position(
                |t| matches!(t, Expr::Pow(_, e) if e.as_number() == Some(Number::neg_one())),
            )?;
            let den = match &v[idx] {
                Expr::Pow(base, _) => (**base).clone(),
                _ => return None,
            };
            let rest: Vec<Expr> = v
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != idx)
                .map(|(_, t)| t.clone())
                .collect();
            Some((Expr::product(rest), den))
        }
        _ => None,
    }
}

/// The polynomial `e` in `var` as a list of `(degree, coefficient)` monomials, from its
/// expansion. `None` if `e` is not a polynomial in `var`.
pub(crate) fn monomial_terms(e: &Expr, var: &str) -> Option<Vec<(u32, Expr)>> {
    let expanded = expand(e);
    let terms: Vec<Expr> = match &expanded {
        Expr::Add(v) => v.clone(),
        other => vec![other.clone()],
    };
    terms.iter().map(|t| monomial(t, var)).collect()
}

/// One monomial `coefficient * var^degree`, when `t` has that form.
fn monomial(t: &Expr, var: &str) -> Option<(u32, Expr)> {
    if !t.contains_var(var) {
        return Some((0, t.clone()));
    }
    match t {
        Expr::Var(v) if &**v == var => Some((1, Expr::one())),
        Expr::Pow(base, exp) => match (&**base, exp.as_number()) {
            (Expr::Var(v), Some(n)) if &**v == var && n.is_integer() && !n.is_negative() => {
                Some((u32::try_from(n.as_i128()?).ok()?, Expr::one()))
            }
            _ => None,
        },
        Expr::Mul(v) => {
            let mut deg = 0u32;
            let mut coeff = Vec::new();
            for f in v {
                let (d, c) = monomial(f, var)?;
                deg = deg.checked_add(d)?;
                coeff.push(c);
            }
            Some((deg, Expr::product(coeff)))
        }
        _ => None,
    }
}

fn degree(terms: &[(u32, Expr)]) -> Option<u32> {
    terms.iter().map(|(d, _)| *d).max()
}

/// Sum of the coefficients of the monomials of degree `d`.
fn leading(terms: &[(u32, Expr)], d: u32) -> Expr {
    simplify(&Expr::sum(
        terms
            .iter()
            .filter(|(k, _)| *k == d)
            .map(|(_, c)| c.clone())
            .collect(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse;

    fn p(s: &str) -> Expr {
        parse(s).unwrap_or_else(|e| panic!("parse {s:?}: {e}"))
    }

    #[test]
    fn taylor_polynomial_of_exp_and_sin() {
        // exp(x) about 0 to order 3: 1 + x + x^2/2 + x^3/6.
        let t = taylor(&p("exp(x)"), "x", &Expr::zero(), 3).unwrap();
        assert_eq!(t, simplify(&p("1 + x + x^2/2 + x^3/6")));
        // sin(x) about 0 to order 3: x - x^3/6.
        let t = taylor(&p("sin(x)"), "x", &Expr::zero(), 3).unwrap();
        assert_eq!(t, simplify(&p("x - x^3/6")));
    }

    #[test]
    fn taylor_about_a_nonzero_point() {
        // x^2 about 1 to order 2 is itself: 1 + 2(x-1) + (x-1)^2.
        let t = taylor(&p("x^2"), "x", &Expr::one(), 2).unwrap();
        assert_eq!(t, simplify(&p("x^2")));
    }

    #[test]
    fn direct_substitution_when_defined() {
        assert_eq!(
            limit(&p("x^2 + 1"), "x", &Expr::int(2)),
            Limit::Finite(Expr::int(5))
        );
        assert_eq!(
            limit(&p("1/x"), "x", &Expr::int(2)),
            Limit::Finite(Expr::rational(1, 2).unwrap())
        );
    }

    #[test]
    fn lhopital_for_zero_over_zero() {
        // sin(x)/x -> 1 at 0; (x^2 - 1)/(x - 1) -> 2 at 1.
        assert_eq!(
            limit(&p("sin(x)/x"), "x", &Expr::zero()),
            Limit::Finite(Expr::one())
        );
        assert_eq!(
            limit(&p("(x^2-1)/(x-1)"), "x", &Expr::one()),
            Limit::Finite(Expr::int(2))
        );
    }

    #[test]
    fn division_by_zero_is_not_a_finite_limit() {
        // 1/x at 0 has no finite limit, and the rules must not invent one.
        assert!(matches!(
            limit(&p("1/x"), "x", &Expr::zero()),
            Limit::Unevaluated(_)
        ));
    }

    #[test]
    fn rational_functions_at_infinity() {
        assert_eq!(
            limit_at_infinity(&p("(2*x^2+1)/(x^2-3)"), "x", true),
            Limit::Finite(Expr::int(2))
        );
        assert_eq!(
            limit_at_infinity(&p("1/x"), "x", true),
            Limit::Finite(Expr::zero())
        );
        assert_eq!(
            limit_at_infinity(&p("x^3"), "x", true),
            Limit::PositiveInfinity
        );
        assert_eq!(
            limit_at_infinity(&p("x^3"), "x", false),
            Limit::NegativeInfinity
        );
        assert_eq!(
            limit_at_infinity(&p("x^2"), "x", false),
            Limit::PositiveInfinity
        );
    }

    #[test]
    fn unsupported_forms_stay_unevaluated() {
        assert!(matches!(
            limit_at_infinity(&p("sin(x)"), "x", true),
            Limit::Unevaluated(_)
        ));
    }
}
