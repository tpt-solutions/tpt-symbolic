//! Differentiation and integration.
//!
//! [`diff`] is total on the expression language. [`integrate`] covers the
//! common cases: the power rule (including `1/x`), linear combinations,
//! elementary functions of a linear argument, and simple u-substitution.
//! Every antiderivative it returns is checked by differentiating it, so a
//! wrong answer is never returned. A failed check yields
//! [`Integral::Unevaluated`].

use crate::error::Error;
use crate::expr::{Expr, SymbolicFunc};
use crate::mathr::Number;
use crate::simplify::simplify;
use alloc::format;
use alloc::vec;
use alloc::vec::Vec;

/// Deepest nesting of u-substitutions tried before giving up.
const MAX_SUB_DEPTH: usize = 3;
/// Most candidate substitutions tried at each level.
const MAX_CANDIDATES: usize = 16;

/// Result of [`integrate`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Integral {
    /// A verified antiderivative, with the constant of integration omitted.
    Exact(Expr),
    /// No verified closed form was found. Carries the original integrand.
    Unevaluated(Expr),
}

fn rat(p: i128, q: i128) -> Expr {
    // Callers pass a non-zero denominator, so the fallback is never used.
    Expr::Const(Number::new(p, q).unwrap_or(Number::ONE))
}

/// Derivative of `e` with respect to `var`, simplified.
pub fn diff(e: &Expr, var: &str) -> Result<Expr, Error> {
    Ok(simplify(&diff_raw(e, var)?))
}

fn diff_raw(e: &Expr, var: &str) -> Result<Expr, Error> {
    Ok(match e {
        Expr::Var(n) => {
            if &**n == var {
                Expr::one()
            } else {
                Expr::zero()
            }
        }
        Expr::Const(_) | Expr::Bool(_) => Expr::zero(),
        Expr::Add(v) => {
            let terms = v.iter().map(|t| diff_raw(t, var)).collect::<Result<Vec<_>, _>>()?;
            Expr::sum(terms)
        }
        Expr::Mul(v) => {
            // Product rule: sum over factors of (d factor) * (the others).
            let mut terms = Vec::with_capacity(v.len());
            for i in 0..v.len() {
                let mut factors = v.clone();
                factors[i] = diff_raw(&v[i], var)?;
                terms.push(Expr::product(factors));
            }
            Expr::sum(terms)
        }
        Expr::Pow(b, x) => {
            let base = (**b).clone();
            let ex = (**x).clone();
            if !ex.contains_var(var) {
                // Power rule with chain factor: x * b^(x-1) * b'.
                let db = diff_raw(&base, var)?;
                Expr::product(vec![ex.clone(), Expr::power(base.clone(), Expr::difference(ex, Expr::one())), db])
            } else {
                // Logarithmic differentiation: b^x * (x' ln b + x b'/b).
                let dx = diff_raw(&ex, var)?;
                let db = diff_raw(&base, var)?;
                let ln_b = Expr::func(SymbolicFunc::Log, base.clone());
                let inner = Expr::sum(vec![
                    Expr::product(vec![dx, ln_b]),
                    Expr::product(vec![ex, db, Expr::power(base.clone(), Expr::int(-1))]),
                ]);
                Expr::product(vec![e.clone(), inner])
            }
        }
        Expr::Func(SymbolicFunc::Log, a) if matches!(&**a, Expr::Func(SymbolicFunc::Abs, _)) => {
            // d/dx log|t| = t'/t. Taking the chain rule through abs would leave |t|^-2 terms that do not simplify.
            let Expr::Func(_, t) = &**a else { unreachable!() };
            let dt = diff_raw(t, var)?;
            Expr::product(vec![dt, Expr::power((**t).clone(), Expr::int(-1))])
        }
        Expr::Func(f, a) => {
            let da = diff_raw(a, var)?;
            Expr::product(vec![outer_derivative(*f, a), da])
        }
        Expr::Eq(..) | Expr::And(_) | Expr::Or(_) | Expr::Not(_) => {
            return Err(Error::Unsupported("differentiation of a boolean expression"));
        }
    })
}

/// Derivative of `f` at `a`, before the chain-rule factor `a'` is applied.
fn outer_derivative(f: SymbolicFunc, a: &Expr) -> Expr {
    let a = a.clone();
    let sq = Expr::power(a.clone(), Expr::int(2));
    match f {
        SymbolicFunc::Sin => Expr::func(SymbolicFunc::Cos, a),
        SymbolicFunc::Cos => Expr::negate(Expr::func(SymbolicFunc::Sin, a)),
        SymbolicFunc::Tan => Expr::power(Expr::func(SymbolicFunc::Cos, a), Expr::int(-2)),
        SymbolicFunc::Asin => {
            let inner = Expr::sum(vec![Expr::one(), Expr::negate(sq)]);
            Expr::power(inner, rat(-1, 2))
        }
        SymbolicFunc::Acos => {
            let inner = Expr::sum(vec![Expr::one(), Expr::negate(sq)]);
            Expr::negate(Expr::power(inner, rat(-1, 2)))
        }
        SymbolicFunc::Atan => Expr::power(Expr::sum(vec![Expr::one(), sq]), Expr::int(-1)),
        SymbolicFunc::Sinh => Expr::func(SymbolicFunc::Cosh, a),
        SymbolicFunc::Cosh => Expr::func(SymbolicFunc::Sinh, a),
        SymbolicFunc::Tanh => Expr::power(Expr::func(SymbolicFunc::Cosh, a), Expr::int(-2)),
        SymbolicFunc::Exp => Expr::func(SymbolicFunc::Exp, a),
        SymbolicFunc::Log => Expr::power(a, Expr::int(-1)),
        SymbolicFunc::Abs => Expr::product(vec![a.clone(), Expr::power(Expr::func(SymbolicFunc::Abs, a), Expr::int(-1))]),
    }
}

/// Integrates `e` with respect to `var`. The constant of integration is omitted.
///
/// The antiderivative is verified by differentiating it. If the check fails,
/// or no rule applies, the result is [`Integral::Unevaluated`].
pub fn integrate(e: &Expr, var: &str) -> Integral {
    let integrand = simplify(e);
    if let Some(f) = antideriv(&integrand, var, 0) {
        let f = simplify(&f);
        if let Ok(d) = diff(&f, var) {
            if simplify(&d) == integrand {
                return Integral::Exact(f);
            }
        }
    }
    Integral::Unevaluated(e.clone())
}

/// Constant `k` when `e` is linear in `var` with slope `k` (that is, `e' = k`).
fn linear_slope(e: &Expr, var: &str) -> Option<Number> {
    let d = diff(e, var).ok()?;
    match d.as_number() {
        Some(k) if !k.is_zero() => Some(k),
        _ => None,
    }
}

fn antideriv(e: &Expr, var: &str, depth: usize) -> Option<Expr> {
    if !e.contains_var(var) {
        return Some(Expr::product(vec![e.clone(), Expr::var(var)]));
    }
    structural(e, var, depth).or_else(|| substitution(e, var, depth))
}

fn structural(e: &Expr, var: &str, depth: usize) -> Option<Expr> {
    match e {
        Expr::Var(_) => {
            // Only reached when e is the variable itself.
            Some(Expr::product(vec![rat(1, 2), Expr::power(e.clone(), Expr::int(2))]))
        }
        Expr::Add(v) => {
            let parts: Option<Vec<Expr>> = v.iter().map(|t| antideriv(t, var, depth)).collect();
            parts.map(Expr::sum)
        }
        Expr::Mul(v) => {
            let (consts, vars): (Vec<Expr>, Vec<Expr>) = v.iter().cloned().partition(|f| !f.contains_var(var));
            if vars.len() != 1 {
                return None;
            }
            let inner = antideriv(&vars[0], var, depth)?;
            let mut factors = consts;
            factors.push(inner);
            Some(Expr::product(factors))
        }
        Expr::Pow(b, x) => power_antiderivative(b, x, var),
        Expr::Func(f, a) => {
            let k = linear_slope(a, var)?;
            let base = basic_antiderivative(*f, (**a).clone())?;
            Some(Expr::product(vec![base, Expr::Const(k.checked_recip()?)]))
        }
        _ => None,
    }
}

/// `∫ b^n dx` for a linear base `b` and a constant exponent `n`.
fn power_antiderivative(b: &Expr, x: &Expr, var: &str) -> Option<Expr> {
    let n = x.as_number()?;
    let k = linear_slope(b, var)?;
    let inner = if n == Number::NEG_ONE {
        Expr::func(SymbolicFunc::Log, Expr::func(SymbolicFunc::Abs, b.clone()))
    } else {
        let np1 = n.checked_add(Number::ONE)?;
        let denom = np1.checked_mul(k)?;
        return Some(Expr::product(vec![
            Expr::power(b.clone(), Expr::Const(np1)),
            Expr::Const(denom.checked_recip()?),
        ]));
    };
    Some(Expr::product(vec![inner, Expr::Const(k.checked_recip()?)]))
}

/// Antiderivative `F(t)` of `f(t)` for the basic functions. `t` is a variable or an expression in it.
fn basic_antiderivative(f: SymbolicFunc, t: Expr) -> Option<Expr> {
    Some(match f {
        SymbolicFunc::Sin => Expr::negate(Expr::func(SymbolicFunc::Cos, t)),
        SymbolicFunc::Cos => Expr::func(SymbolicFunc::Sin, t),
        SymbolicFunc::Exp => Expr::func(SymbolicFunc::Exp, t),
        SymbolicFunc::Sinh => Expr::func(SymbolicFunc::Cosh, t),
        SymbolicFunc::Cosh => Expr::func(SymbolicFunc::Sinh, t),
        SymbolicFunc::Tan => Expr::negate(Expr::func(
            SymbolicFunc::Log,
            Expr::func(SymbolicFunc::Abs, Expr::func(SymbolicFunc::Cos, t)),
        )),
        // ∫ ln t dt = t ln t - t, valid for t > 0.
        SymbolicFunc::Log => Expr::sum(vec![
            Expr::product(vec![t.clone(), Expr::func(SymbolicFunc::Log, t.clone())]),
            Expr::negate(t),
        ]),
        _ => return None,
    })
}

/// Simple u-substitution: find `g` inside the integrand such that the integrand
/// divided by `g'` depends only on `g`. Then integrate in `u = g` and substitute back.
fn substitution(e: &Expr, var: &str, depth: usize) -> Option<Expr> {
    if depth >= MAX_SUB_DEPTH {
        return None;
    }
    let u_name = format!("_u{depth}");
    let u = Expr::var(&u_name);

    let mut candidates = Vec::new();
    collect_candidates(e, var, &mut candidates);

    for g in candidates.iter().take(MAX_CANDIDATES) {
        let Ok(dg) = diff(g, var) else { continue };
        if dg.as_number().is_some_and(|n| n.is_zero()) {
            continue;
        }
        let q = simplify(&Expr::quotient(e.clone(), dg));
        let q_u = simplify(&q.replace(g, &u));
        if q_u.contains_var(var) {
            continue;
        }
        if let Some(f_u) = antideriv(&q_u, &u_name, depth + 1) {
            return Some(simplify(&f_u.replace(&u, g)));
        }
    }
    None
}

/// Subexpressions worth substituting: arguments of functions and bases of powers
/// that depend on the variable, outermost first.
fn collect_candidates(e: &Expr, var: &str, out: &mut Vec<Expr>) {
    match e {
        Expr::Func(_, a) => {
            push_candidate(a, var, out);
            collect_candidates(a, var, out);
        }
        Expr::Pow(b, x) => {
            push_candidate(b, var, out);
            collect_candidates(b, var, out);
            collect_candidates(x, var, out);
        }
        Expr::Add(v) | Expr::Mul(v) => {
            for t in v {
                collect_candidates(t, var, out);
            }
        }
        _ => {}
    }
}

fn push_candidate(g: &Expr, var: &str, out: &mut Vec<Expr>) {
    if g.contains_var(var) && !matches!(g, Expr::Var(_)) && !out.contains(g) {
        out.push(g.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse;
    use crate::testkit::{random_expr, Rng};
    use alloc::string::ToString;

    fn p(s: &str) -> Expr {
        parse(s).unwrap_or_else(|e| panic!("parse {s:?}: {e}"))
    }

    fn d(s: &str) -> Expr {
        diff(&p(s), "x").unwrap()
    }

    fn int_exact(s: &str) -> Expr {
        match integrate(&p(s), "x") {
            Integral::Exact(f) => f,
            Integral::Unevaluated(e) => panic!("expected a closed form for {e}"),
        }
    }

    #[test]
    fn derivative_rules() {
        assert_eq!(d("x^3 + 2*x"), p("3*x^2 + 2"));
        assert_eq!(d("sin(x^2)"), p("2*x*cos(x^2)"));
        assert_eq!(d("x*sin(x)"), p("sin(x) + x*cos(x)"));
        assert_eq!(d("exp(sin(x))"), p("cos(x)*exp(sin(x))"));
        assert_eq!(d("2^x"), p("2^x*log(2)"));
        assert_eq!(d("log(x)"), p("x^(-1)"));
        assert_eq!(d("y*x^2"), p("2*x*y"));
    }

    #[test]
    fn power_rule_and_reciprocal() {
        assert_eq!(int_exact("x^2"), p("1/3*x^3"));
        assert_eq!(int_exact("x^(-1)").to_string(), p("log(abs(x))").to_string());
        assert_eq!(int_exact("3*x + sin(x)"), p("3/2*x^2 - cos(x)"));
    }

    #[test]
    fn linear_argument_and_substitution() {
        assert_eq!(int_exact("exp(2*x)"), p("1/2*exp(2*x)"));
        assert_eq!(int_exact("1/(2*x + 1)"), p("1/2*log(abs(2*x + 1))"));
        assert_eq!(int_exact("2*x*cos(x^2)"), p("sin(x^2)"));
        assert_eq!(int_exact("x*cos(x^2)"), p("1/2*sin(x^2)"));
        assert_eq!(int_exact("x*exp(x^2)"), p("1/2*exp(x^2)"));
        assert_eq!(int_exact("log(x)"), p("x*log(x) - x"));
        assert_eq!(int_exact("tan(x)"), p("-log(abs(cos(x)))"));
    }

    #[test]
    fn failures_are_unevaluated_not_wrong() {
        // sin^2 needs a double-angle identity this engine does not have.
        assert!(matches!(integrate(&p("sin(x)^2"), "x"), Integral::Unevaluated(_)));
        assert!(matches!(integrate(&p("exp(x^3)"), "x"), Integral::Unevaluated(_)));
    }

    #[test]
    fn integrate_then_differentiate_round_trips() {
        // Property: whenever integration succeeds, d/dx of the result is the integrand.
        let mut rng = Rng::new(7);
        let mut checked = 0;
        for _ in 0..200 {
            let f = random_expr(&mut rng, 3);
            if let Integral::Exact(big_f) = integrate(&f, "x") {
                let back = diff(&big_f, "x").unwrap();
                assert_eq!(back, simplify(&f), "round trip failed for {f}");
                checked += 1;
            }
        }
        assert!(checked > 0, "random sample produced no closed forms to check");
    }
}
