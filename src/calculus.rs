//! Differentiation and integration.
//!
//! [`diff`] is total on the expression language. [`integrate`] covers the
//! common cases: the power rule (including `1/x`), linear combinations,
//! elementary functions of a linear argument, and simple u-substitution.
//! Every antiderivative it returns is checked by differentiating it, so a
//! wrong answer is never returned. A failed check yields
//! [`Integral::Unevaluated`].

use crate::error::Error;
use crate::eval::subs;
use crate::expr::{Expr, SymbolicFunc};
use crate::mathr::Number;
use crate::simplify::{expand, simplify};
use alloc::format;
use alloc::vec;
use alloc::vec::Vec;

/// Deepest nesting of u-substitutions tried before giving up.
const MAX_SUB_DEPTH: usize = 3;
/// Most candidate substitutions tried at each level.
const MAX_CANDIDATES: usize = 16;

/// Result of [`integrate`].
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Integral {
    /// A verified antiderivative, with the constant of integration omitted.
    Exact(Expr),
    /// No verified closed form was found. Carries the original integrand.
    Unevaluated(Expr),
}

fn rat(p: i128, q: i128) -> Expr {
    // Callers pass a non-zero denominator, so the fallback is never used.
    Expr::Const(Number::new(p, q).unwrap_or(Number::one()))
}

/// The `n`-th derivative of `e` with respect to `var`, simplified after each step.
pub fn diff_n(e: &Expr, var: &str, n: usize) -> Result<Expr, Error> {
    let mut cur = e.clone();
    for _ in 0..n {
        cur = diff(&cur, var)?;
    }
    Ok(cur)
}

/// The gradient of `e`: the partial derivative with respect to each of `vars`.
///
/// Each partial derivative treats the other names as constants.
pub fn gradient(e: &Expr, vars: &[&str]) -> Result<Vec<Expr>, Error> {
    vars.iter().map(|v| diff(e, v)).collect()
}

/// Derivative of `e` with respect to `var`, simplified.
/// # Example
/// ```
/// use tpt_symbolic::calculus::diff;
/// use tpt_symbolic::parse::parse;
/// assert_eq!(diff(&parse("x^3").unwrap(), "x").unwrap().to_string(), "3*x^2");
/// ```
pub fn diff(e: &Expr, var: &str) -> Result<Expr, Error> {
    Ok(simplify(&diff_raw(e, var)?))
}

fn diff_raw(e: &Expr, var: &str) -> Result<Expr, Error> {
    Ok(match e {
        Expr::Pi | Expr::E | Expr::I => Expr::zero(),
        Expr::Var(n) => {
            if &**n == var {
                Expr::one()
            } else {
                Expr::zero()
            }
        }
        Expr::Const(_) => Expr::zero(),
        Expr::Add(v) => {
            let terms = v
                .iter()
                .map(|t| diff_raw(t, var))
                .collect::<Result<Vec<_>, _>>()?;
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
                Expr::product(vec![
                    ex.clone(),
                    Expr::power(base.clone(), Expr::difference(ex, Expr::one())),
                    db,
                ])
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
        Expr::Func(f, a) => {
            if *f == SymbolicFunc::Log {
                if let Expr::Func(SymbolicFunc::Abs, t) = &**a {
                    // d/dx log|t| = t'/t. Taking the chain rule through abs would leave |t|^-2 terms that do not simplify.
                    let dt = diff_raw(t, var)?;
                    return Ok(Expr::product(vec![
                        dt,
                        Expr::power((**t).clone(), Expr::int(-1)),
                    ]));
                }
            }
            let da = diff_raw(a, var)?;
            Expr::product(vec![outer_derivative(*f, a), da])
        }
        Expr::Bool(_) | Expr::Eq(..) | Expr::And(_) | Expr::Or(_) | Expr::Not(_) => {
            return Err(Error::Unsupported(
                "differentiation of a boolean expression",
            ));
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
        SymbolicFunc::Abs => Expr::product(vec![
            a.clone(),
            Expr::power(Expr::func(SymbolicFunc::Abs, a), Expr::int(-1)),
        ]),
    }
}

/// Integrates `e` with respect to `var`. The constant of integration is omitted.
///
/// The antiderivative is verified by differentiating it. If the check fails,
/// or no rule applies, the result is [`Integral::Unevaluated`].
/// # Example
/// ```
/// use tpt_symbolic::calculus::{integrate, Integral};
/// use tpt_symbolic::parse::parse;
/// use tpt_symbolic::simplify::simplify;
/// let f = integrate(&parse("2*x").unwrap(), "x");
/// assert_eq!(f, Integral::Exact(simplify(&parse("x^2").unwrap())));
/// ```
pub fn integrate(e: &Expr, var: &str) -> Integral {
    let integrand = simplify(e);
    // Try the integrand as written, then its expansion, so that `(x^2+1)^2` integrates
    // term by term. Either form is accepted only when its derivative matches it.
    for k in 0..2 {
        let candidate = if k == 0 {
            integrand.clone()
        } else {
            simplify(&expand(&integrand))
        };
        if let Some(f) = antideriv(&candidate, var, 0) {
            let f = simplify(&f);
            if let Ok(d) = diff(&f, var) {
                if simplify(&d) == candidate {
                    return Integral::Exact(f);
                }
            }
        }
    }
    if let Some(f) = atan_by_parts(&integrand, var) {
        return Integral::Exact(simplify(&f));
    }
    // Rational integrands are integrated by partial fractions. The result is checked term
    // by term at exact points, since its symbolic derivative does not always simplify to
    // the integrand's form.
    if let Some(f) = rational_antiderivative(&integrand, var) {
        return Integral::Exact(simplify(&f));
    }
    Integral::Unevaluated(e.clone())
}

/// Definite integral of `e` over `[lower, upper]` as `F(upper) - F(lower)`.
///
/// Conservative. The result is [`Integral::Unevaluated`] unless the integrand is built
/// only from polynomials with non-negative integer powers and the functions `sin`,
/// `cos`, `exp`, `sinh`, `cosh`, `tanh` and `atan`. Those are continuous on the whole
/// real line, so no pole or branch point can lie between the bounds. Poles and branch
/// points are the one way a definite integral gives a wrong number, so anything else
/// (including `1/x`, `log`, `abs`, `tan`, `asin`, `acos` and fractional powers) is
/// left unevaluated.
/// # Example
/// ```
/// use tpt_symbolic::calculus::{definite_integral, Integral};
/// use tpt_symbolic::parse::parse;
/// use tpt_symbolic::Expr;
/// let r = definite_integral(&parse("x^2").unwrap(), "x", &Expr::zero(), &Expr::one());
/// assert_eq!(r, Integral::Exact(Expr::rational(1, 3).unwrap()));
/// ```
pub fn definite_integral(e: &Expr, var: &str, lower: &Expr, upper: &Expr) -> Integral {
    if !is_regular(e) {
        return Integral::Unevaluated(e.clone());
    }
    match integrate(e, var) {
        Integral::Exact(f) => {
            let upper_value = subs(&f, var, upper);
            let lower_value = subs(&f, var, lower);
            Integral::Exact(simplify(&Expr::difference(upper_value, lower_value)))
        }
        other @ Integral::Unevaluated(_) => other,
    }
}

/// `true` if `e` has no pole or branch point the definite-integral rule must avoid.
fn is_regular(e: &Expr) -> bool {
    match e {
        Expr::Var(_) | Expr::Const(_) | Expr::Pi | Expr::E | Expr::I => true,
        Expr::Add(v) | Expr::Mul(v) => v.iter().all(is_regular),
        Expr::Pow(base, exp) => {
            let non_negative_integer = exp
                .as_number()
                .is_some_and(|n| n.is_integer() && !n.is_negative());
            non_negative_integer && is_regular(base)
        }
        Expr::Func(f, arg) => {
            let regular_function = matches!(
                f,
                SymbolicFunc::Sin
                    | SymbolicFunc::Cos
                    | SymbolicFunc::Exp
                    | SymbolicFunc::Sinh
                    | SymbolicFunc::Cosh
                    | SymbolicFunc::Tanh
                    | SymbolicFunc::Atan
            );
            regular_function && is_regular(arg)
        }
        _ => false,
    }
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
            Some(Expr::product(vec![
                rat(1, 2),
                Expr::power(e.clone(), Expr::int(2)),
            ]))
        }
        Expr::Add(v) => {
            let parts: Option<Vec<Expr>> = v.iter().map(|t| antideriv(t, var, depth)).collect();
            parts.map(Expr::sum)
        }
        Expr::Mul(v) => {
            let (consts, vars): (Vec<Expr>, Vec<Expr>) =
                v.iter().cloned().partition(|f| !f.contains_var(var));
            if vars.len() == 2 {
                let body = exp_times_trig(&vars[0], &vars[1], var)
                    .or_else(|| by_parts(&vars[0], &vars[1], var, depth))
                    .or_else(|| by_parts(&vars[1], &vars[0], var, depth))
                    .or_else(|| by_parts_general(&vars[0], &vars[1], var, depth))
                    .or_else(|| by_parts_general(&vars[1], &vars[0], var, depth))?;
                let mut factors = consts;
                factors.push(body);
                return Some(Expr::product(factors));
            }
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

/// Integration by parts for `∫ p(x) g(x) dx` when `p = x^n` with a positive integer `n`
/// and `g` has a basic antiderivative `G`:
///
/// `∫ p g dx = p G - ∫ p' G dx`, where `p' = n x^(n-1)`.
///
/// The recursion lowers `n` each step, so it ends at `n = 0`, or at the depth limit.
fn by_parts(p: &Expr, g: &Expr, var: &str, depth: usize) -> Option<Expr> {
    if depth >= MAX_SUB_DEPTH {
        return None;
    }
    let n = match p {
        Expr::Var(v) if &**v == var => 1,
        Expr::Pow(b, x) if matches!(&**b, Expr::Var(v) if &**v == var) => {
            x.as_number()?.as_i128().filter(|&k| k >= 1)?
        }
        _ => return None,
    };
    let big_g = antideriv(g, var, depth)?;
    let rest = simplify(&Expr::product(vec![
        Expr::int(n),
        Expr::power(Expr::var(var), Expr::int(n - 1)),
        big_g.clone(),
    ]));
    let second = antideriv(&rest, var, depth + 1)?;
    Some(Expr::difference(
        Expr::product(vec![p.clone(), big_g]),
        second,
    ))
}

/// Integration by parts with `u` differentiated and `g` integrated, for any `u` whose
/// derivative is simpler than `u`. Gives `u V - ∫ u' V dx`, where `V` is an antiderivative
/// of `g`. Covers `log(x) * x^2` (differentiate the log), which the `x^n` form cannot.
fn by_parts_general(u: &Expr, g: &Expr, var: &str, depth: usize) -> Option<Expr> {
    // Only worth trying when u is a log or atan: those are the factors whose derivative is
    // simpler than u. Trying it for every product costs time without finding new results.
    if !matches!(u, Expr::Func(SymbolicFunc::Log | SymbolicFunc::Atan, _)) {
        return None;
    }
    if depth >= MAX_SUB_DEPTH {
        return None;
    }
    let v = antideriv(g, var, depth)?;
    let du = diff(u, var).ok()?;
    let rest = simplify(&Expr::product(vec![v.clone(), du]));
    let second = antideriv(&rest, var, depth + 1)?;
    Some(Expr::difference(Expr::product(vec![u.clone(), v]), second))
}

/// `∫ e^(a x + c) sin(b x + d) dx` and the cosine case, for linear arguments.
///
/// `e^(a x + c) (a sin θ - b cos θ) / (a^2 + b^2)` for sine, and
/// `e^(a x + c) (a cos θ + b sin θ) / (a^2 + b^2)` for cosine, where `θ = b x + d`.
/// Either order of the two factors is accepted.
fn exp_times_trig(p: &Expr, q: &Expr, var: &str) -> Option<Expr> {
    let (exp_arg, trig_arg, is_sin) = match (p, q) {
        (Expr::Func(SymbolicFunc::Exp, a), Expr::Func(SymbolicFunc::Sin, b))
        | (Expr::Func(SymbolicFunc::Sin, b), Expr::Func(SymbolicFunc::Exp, a)) => (a, b, true),
        (Expr::Func(SymbolicFunc::Exp, a), Expr::Func(SymbolicFunc::Cos, b))
        | (Expr::Func(SymbolicFunc::Cos, b), Expr::Func(SymbolicFunc::Exp, a)) => (a, b, false),
        _ => return None,
    };
    let a = linear_slope(exp_arg, var)?;
    let b = linear_slope(trig_arg, var)?;
    let denom = a.checked_mul(&a)?.checked_add(&b.checked_mul(&b)?)?;
    let scale = Expr::Const(denom.checked_recip()?);
    let exp_part = Expr::func(SymbolicFunc::Exp, (**exp_arg).clone());
    let sin_part = Expr::func(SymbolicFunc::Sin, (**trig_arg).clone());
    let cos_part = Expr::func(SymbolicFunc::Cos, (**trig_arg).clone());
    let bracket = if is_sin {
        // a sin - b cos
        Expr::sum(vec![
            Expr::product(vec![Expr::Const(a), sin_part]),
            Expr::product(vec![Expr::Const(b.checked_neg()?), cos_part]),
        ])
    } else {
        // a cos + b sin
        Expr::sum(vec![
            Expr::product(vec![Expr::Const(a), cos_part]),
            Expr::product(vec![Expr::Const(b), sin_part]),
        ])
    };
    Some(Expr::product(vec![exp_part, bracket, scale]))
}

/// `∫ b^n dx` for a linear base `b` and a constant exponent `n`.
fn power_antiderivative(b: &Expr, x: &Expr, var: &str) -> Option<Expr> {
    let n = x.as_number()?;
    let k = linear_slope(b, var)?;
    let inner = if n == Number::neg_one() {
        Expr::func(SymbolicFunc::Log, Expr::func(SymbolicFunc::Abs, b.clone()))
    } else {
        let np1 = n.checked_add(&Number::one())?;
        let denom = np1.checked_mul(&k)?;
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
        // ∫ tanh t dt = ln(cosh t). cosh is positive, so no absolute value is needed.
        SymbolicFunc::Tanh => Expr::func(SymbolicFunc::Log, Expr::func(SymbolicFunc::Cosh, t)),
        SymbolicFunc::Tan => Expr::negate(Expr::func(
            SymbolicFunc::Log,
            Expr::func(SymbolicFunc::Abs, Expr::func(SymbolicFunc::Cos, t)),
        )),
        // ∫ asin t dt = t asin t + sqrt(1 - t^2), and the same for acos and atan.
        SymbolicFunc::Asin => Expr::sum(vec![
            Expr::product(vec![t.clone(), Expr::func(SymbolicFunc::Asin, t.clone())]),
            one_minus_square_root(t),
        ]),
        SymbolicFunc::Acos => Expr::difference(
            Expr::product(vec![t.clone(), Expr::func(SymbolicFunc::Acos, t.clone())]),
            one_minus_square_root(t),
        ),
        // ∫ atan t dt = t atan t - ln(1 + t^2) / 2.
        SymbolicFunc::Atan => Expr::difference(
            Expr::product(vec![t.clone(), Expr::func(SymbolicFunc::Atan, t.clone())]),
            Expr::product(vec![
                Expr::rational(1, 2).unwrap_or_else(Expr::one),
                Expr::func(
                    SymbolicFunc::Log,
                    Expr::sum(vec![Expr::one(), Expr::power(t, Expr::int(2))]),
                ),
            ]),
        ),
        // ∫ ln t dt = t ln t - t, valid for t > 0.
        SymbolicFunc::Log => Expr::sum(vec![
            Expr::product(vec![t.clone(), Expr::func(SymbolicFunc::Log, t.clone())]),
            Expr::negate(t),
        ]),
        _ => return None,
    })
}

/// `sqrt(1 - t^2)`, the term that appears in the inverse-sine and inverse-cosine antiderivatives.
fn one_minus_square_root(t: Expr) -> Expr {
    let radicand = Expr::sum(vec![
        Expr::one(),
        Expr::negate(Expr::power(t, Expr::int(2))),
    ]);
    Expr::power(radicand, Expr::rational(1, 2).unwrap_or_else(Expr::one))
}

/// Simple u-substitution: find `g` inside the integrand such that the integrand
/// divided by `g'` depends only on `g`. Then integrate in `u = g` and substitute back.
fn substitution(e: &Expr, var: &str, depth: usize) -> Option<Expr> {
    if depth >= MAX_SUB_DEPTH {
        return None;
    }
    // The substitution symbol must not clash with the variable or with any name
    // already in the integrand, or the substituted form would be ambiguous.
    let mut u_name = format!("_u{depth}");
    while u_name == var || e.contains_var(&u_name) {
        u_name.push('_');
    }
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

/// `∫ x^n atan(x) dx` for a non-negative integer `n`, by parts with `P = x^(n+1)/(n+1)`:
/// `P atan(x) - ∫ P/(1 + x^2) dx`. The rational integral is found by partial fractions and
/// checked term by term, and the `atan` term is exact by construction, so the result is not
/// checked by a symbolic derivative (it would not simplify back to the integrand).
fn atan_by_parts(e: &Expr, var: &str) -> Option<Expr> {
    let Expr::Mul(v) = e else { return None };
    let [Expr::Const(c), rest @ ..] = v.as_slice() else {
        // Accept the power and atan in either order, with no constant factor.
        return atan_by_parts_factors(v, var);
    };
    let inner = atan_by_parts_factors(rest, var)?;
    Some(Expr::product(vec![Expr::Const(c.clone()), inner]))
}

fn atan_by_parts_factors(factors: &[Expr], var: &str) -> Option<Expr> {
    let (power, atan_arg) = match factors {
        [a, b] => match (a, b) {
            (p, Expr::Func(SymbolicFunc::Atan, arg)) | (Expr::Func(SymbolicFunc::Atan, arg), p) => {
                (p, arg)
            }
            _ => return None,
        },
        _ => return None,
    };
    if !matches!(&**atan_arg, Expr::Var(name) if &**name == var) {
        return None;
    }
    let n = match power {
        Expr::Var(name) if &**name == var => 1,
        Expr::Pow(base, exp) if matches!(&**base, Expr::Var(name) if &**name == var) => {
            let k = exp.as_number()?.as_i128()?;
            if k < 0 {
                return None;
            }
            k + 1
        }
        _ => return None,
    };
    // P = x^n / n with n = (power + 1), and the rational part -P / (1 + x^2).
    let x = Expr::var(var);
    let p = Expr::product(vec![
        Expr::rational(1, n)?,
        Expr::power(x.clone(), Expr::int(n)),
    ]);
    let den = Expr::sum(vec![Expr::one(), Expr::power(x, Expr::int(2))]);
    let rational_integrand = Expr::negate(Expr::quotient(p.clone(), den));
    let rational = rational_antiderivative(&simplify(&rational_integrand), var)?;
    let atan_term = Expr::product(vec![p, Expr::func(SymbolicFunc::Atan, Expr::var(var))]);
    Some(Expr::sum(vec![atan_term, rational]))
}

/// Antiderivative of a rational function of `var`, by partial fractions.
///
/// Polynomial parts are integrated term by term. A term `A/(x + c)` gives `A log|x + c|`,
/// and `A/(x + c)^j` for `j > 1` gives the power rule. A term `(Bx + C)/(x^2 + px + q)` with
/// `4q - p^2 > 0` gives `(B/2) log(x^2 + px + q) + (C - Bp/2) (2/s) atan((2x + p)/s)`,
/// where `s = sqrt(4q - p^2)`. Higher powers of an irreducible quadratic are not
/// integrated here, and neither are factors of degree three or more.
fn rational_antiderivative(e: &Expr, var: &str) -> Option<Expr> {
    let (num, den) = crate::limits::combined_quotient(e).unwrap_or((e.clone(), Expr::one()));
    let d = crate::apart::decompose(&num, &den, var).ok()?;
    let x = Expr::var(var);
    let mut parts = Vec::new();
    for (k, c) in d.polynomial.iter().enumerate() {
        if c.is_zero() {
            continue;
        }
        let k1 = i128::try_from(k + 1).ok()?;
        let coeff = c.checked_div(&Number::from_int(k1))?;
        parts.push(Expr::product(vec![
            Expr::Const(coeff),
            Expr::power(x.clone(), Expr::int(k1)),
        ]));
    }
    for term in &d.terms {
        parts.push(partial_term_antiderivative(term, var)?);
    }
    Some(Expr::sum(parts))
}

/// Antiderivative of one partial-fraction term, checked at exact points where it needs
/// checking.
fn partial_term_antiderivative(t: &crate::apart::PartialTerm, var: &str) -> Option<Expr> {
    let f_expr = crate::factor::poly_expr(&t.factor, var);
    match t.factor.len() {
        // Linear factor x + c, monic: factor = [c, 1], numerator = [a].
        2 => {
            let a = t.numerator.first()?.clone();
            if t.power == 1 {
                Some(Expr::product(vec![
                    Expr::Const(a),
                    Expr::func(SymbolicFunc::Log, Expr::func(SymbolicFunc::Abs, f_expr)),
                ]))
            } else {
                let exp = 1 - i128::try_from(t.power).ok()?;
                let coeff = a.checked_div(&Number::from_int(exp))?;
                Some(Expr::product(vec![
                    Expr::Const(coeff),
                    Expr::power(f_expr, Expr::int(exp)),
                ]))
            }
        }
        // Irreducible quadratic f = x^2 + p x + q, factor = [q, p, 1], numerator = [c, b].
        3 if t.power == 1 => {
            let (q, p) = (t.factor[0].clone(), t.factor[1].clone());
            let c = t.numerator[0].clone();
            let b = t.numerator[1].clone();
            if !quadratic_term_matches(&b, &c, &p, &q) {
                return None;
            }
            let disc = Number::from_int(4)
                .checked_mul(&q)?
                .checked_sub(&p.checked_mul(&p)?)?;
            if !disc.cmp_value(&Number::zero()).is_gt() {
                return None;
            }
            let half_b = b.checked_div(&Number::from_int(2))?;
            let k = c.checked_sub(&b.checked_mul(&p)?.checked_div(&Number::from_int(2))?)?;
            let two_k = k.checked_mul(&Number::from_int(2))?;
            let s_inv = Expr::power(Expr::Const(disc), Expr::Const(Number::new(-1, 2)?));
            let arg = Expr::product(vec![
                Expr::sum(vec![
                    Expr::product(vec![Expr::int(2), x_of(var)]),
                    Expr::Const(p),
                ]),
                s_inv.clone(),
            ]);
            Some(Expr::sum(vec![
                Expr::product(vec![
                    Expr::Const(half_b),
                    Expr::func(SymbolicFunc::Log, f_expr),
                ]),
                Expr::product(vec![
                    Expr::Const(two_k),
                    s_inv,
                    Expr::func(SymbolicFunc::Atan, arg),
                ]),
            ]))
        }
        _ => None,
    }
}

fn x_of(var: &str) -> Expr {
    Expr::var(var)
}

/// Checks `d/dx [(b/2) log f + (c - bp/2)(2/s) atan((2x+p)/s)] = (bx + c)/f` at seven exact
/// points, for `f = x^2 + p x + q` with `s^2 = 4q - p^2`. Both sides are rational
/// functions of degree at most two, so agreement at seven points proves identity.
fn quadratic_term_matches(b: &Number, c: &Number, p: &Number, q: &Number) -> bool {
    let Some(disc) = Number::from_int(4)
        .checked_mul(q)
        .and_then(|v| v.checked_sub(&p.checked_mul(p)?))
    else {
        return false;
    };
    let Some(k) = b
        .checked_mul(p)
        .and_then(|bp| bp.checked_div(&Number::from_int(2)))
        .and_then(|bp2| c.checked_sub(&bp2))
    else {
        return false;
    };
    for x0 in [-3i128, -2, -1, 0, 1, 2, 3] {
        let x0 = Number::from_int(x0);
        // f(x0) and f'(x0) = 2 x0 + p.
        let f0 = x0
            .checked_mul(&x0)
            .and_then(|v| v.checked_add(&p.checked_mul(&x0)?))
            .and_then(|v| v.checked_add(q));
        let f0 = match f0 {
            Some(v) if !v.is_zero() => v,
            _ => return false,
        };
        let fp0 = Number::from_int(2)
            .checked_mul(&x0)
            .and_then(|v| v.checked_add(p));
        let Some(fp0) = fp0 else { return false };
        // d/dx of (2k/s) atan(u), u = (2x + p)/s: (2k/s)(2/s)/(1 + u^2) = 4k / (disc (1 + u^2)).
        let Some(u_sq) = fp0.checked_mul(&fp0).and_then(|v| v.checked_div(&disc)) else {
            return false;
        };
        let atan_part = Number::from_int(4)
            .checked_mul(&k)
            .and_then(|v| v.checked_div(&disc))
            .and_then(|v| v.checked_div(&Number::one().checked_add(&u_sq)?));
        let log_part = b
            .checked_div(&Number::from_int(2))
            .and_then(|hb| hb.checked_mul(&fp0))
            .and_then(|v| v.checked_div(&f0));
        let (Some(atan_part), Some(log_part)) = (atan_part, log_part) else {
            return false;
        };
        let lhs = atan_part.checked_add(&log_part);
        let rhs = b
            .checked_mul(&x0)
            .and_then(|v| v.checked_add(c))
            .and_then(|v| v.checked_div(&f0));
        if lhs != rhs {
            return false;
        }
    }
    true
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
        assert_eq!(
            int_exact("x^(-1)").to_string(),
            p("log(abs(x))").to_string()
        );
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
        assert!(matches!(
            integrate(&p("sin(x)^2"), "x"),
            Integral::Unevaluated(_)
        ));
        assert!(matches!(
            integrate(&p("exp(x^3)"), "x"),
            Integral::Unevaluated(_)
        ));
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
        assert!(
            checked > 0,
            "random sample produced no closed forms to check"
        );
    }

    #[test]
    fn higher_derivatives_and_gradients() {
        assert_eq!(diff_n(&p("x^4"), "x", 2).unwrap(), p("12*x^2"));
        assert_eq!(diff_n(&p("x^4"), "x", 0).unwrap(), p("x^4"));
        assert_eq!(
            gradient(&p("x*y"), &["x", "y"]).unwrap(),
            vec![p("y"), p("x")]
        );
    }

    #[test]
    fn inverse_trig_antiderivatives_are_verified_exact() {
        for src in ["asin(x)", "acos(x)", "atan(x)"] {
            match integrate(&p(src), "x") {
                Integral::Exact(f) => {
                    assert_eq!(
                        simplify(&diff(&f, "x").unwrap()),
                        simplify(&p(src)),
                        "{src}"
                    );
                }
                other => panic!("{src}: expected an exact antiderivative, got {other:?}"),
            }
        }
    }

    #[test]
    fn x_power_times_atan_integrates_by_parts() {
        // ∫ x atan x = x atan x - (1/2) log(1 + x^2).
        assert_eq!(
            integrate(&p("x*atan(x)"), "x"),
            Integral::Exact(simplify(&p("x*atan(x) - 1/2*log(x^2 + 1)")))
        );
        // ∫ x^2 atan x = (x^3/3) atan x - x^2/6 + (1/6) log(1 + x^2).
        assert_eq!(
            integrate(&p("x^2*atan(x)"), "x"),
            Integral::Exact(simplify(&p("x^3/3*atan(x) - x^2/6 + 1/6*log(x^2 + 1)")))
        );
        // Only the bare argument x is handled.
        assert!(matches!(
            integrate(&p("x*atan(2*x)"), "x"),
            Integral::Unevaluated(_)
        ));
    }

    #[test]
    fn rational_functions_integrate_by_partial_fractions() {
        for src in [
            "1/(x^2+1)",
            "1/(x^2-1)",
            "x/(x^2+1)",
            "(2*x+3)/(x^2+2*x+5)",
            "x^3/(x^2+1)",
            "1/(x*(x+1)^2)",
        ] {
            assert!(
                matches!(integrate(&p(src), "x"), Integral::Exact(_)),
                "{src} should integrate"
            );
        }
        // atan appears for the irreducible quadratic.
        let Integral::Exact(f) = integrate(&p("1/(x^2+1)"), "x") else {
            panic!("expected an exact antiderivative");
        };
        assert_eq!(f, simplify(&p("atan(x)")));
        // An irreducible cubic in the denominator has no closed form here.
        assert!(matches!(
            integrate(&p("1/(x^3+x+1)"), "x"),
            Integral::Unevaluated(_)
        ));
    }

    #[test]
    fn exp_times_trig_and_log_times_power_are_verified() {
        for src in [
            "exp(x)*sin(x)",
            "exp(2*x)*cos(3*x)",
            "x^2*log(x)",
            "x*log(x)",
        ] {
            match integrate(&p(src), "x") {
                Integral::Exact(f) => {
                    assert_eq!(
                        simplify(&diff(&f, "x").unwrap()),
                        simplify(&p(src)),
                        "{src}"
                    );
                }
                other => panic!("{src}: expected an exact antiderivative, got {other:?}"),
            }
        }
    }

    #[test]
    fn integration_by_parts_for_polynomial_times_basic_function() {
        for src in [
            "x*sin(x)",
            "x*cos(x)",
            "x*exp(x)",
            "x^2*exp(x)",
            "x^3*sin(x)",
            "x*cos(2*x)",
        ] {
            match integrate(&p(src), "x") {
                Integral::Exact(f) => {
                    assert_eq!(
                        simplify(&diff(&f, "x").unwrap()),
                        simplify(&p(src)),
                        "{src}"
                    );
                }
                other => panic!("{src}: expected an exact antiderivative, got {other:?}"),
            }
        }
    }

    #[test]
    fn tanh_antiderivative_is_verified_exact() {
        // ∫ tanh x dx = ln(cosh x), checked by differentiating back.
        match integrate(&p("tanh(x)"), "x") {
            Integral::Exact(f) => {
                assert_eq!(simplify(&diff(&f, "x").unwrap()), simplify(&p("tanh(x)")));
            }
            other => panic!("expected an exact antiderivative, got {other:?}"),
        }
    }

    #[test]
    fn pi_and_e_are_constants_not_variables() {
        // d/dx (pi * x^2) = 2*pi*x, and pi is not treated as the unknown.
        assert_eq!(
            simplify(&diff(&p("pi*x^2"), "x").unwrap()),
            simplify(&p("2*pi*x"))
        );
        assert!(!p("pi + e").contains_var("pi"));
        assert_eq!(p("pi + e").to_string(), "pi + e");
        assert!(crate::eval::eval(&p("pi"), &[]).is_err());
    }

    #[test]
    fn integrate_expands_when_the_direct_form_fails() {
        match integrate(&p("(x^2+1)^2"), "x") {
            Integral::Exact(f) => {
                assert_eq!(
                    simplify(&diff(&f, "x").unwrap()),
                    simplify(&expand(&p("(x^2+1)^2")))
                );
            }
            other => panic!("expected an exact antiderivative, got {other:?}"),
        }
    }

    #[test]
    fn definite_integrals_of_regular_functions() {
        // ∫_0^1 x^2 dx = 1/3, ∫_0^pi sin x dx = 2.
        let third = Expr::rational(1, 3).unwrap();
        assert_eq!(
            definite_integral(&p("x^2"), "x", &Expr::zero(), &Expr::one()),
            Integral::Exact(third)
        );
        assert_eq!(
            definite_integral(&p("sin(x)"), "x", &Expr::zero(), &p("pi")),
            Integral::Exact(Expr::int(2))
        );
        // ∫_0^1 (x^2+1)^2 dx = 28/15.
        assert_eq!(
            definite_integral(&p("(x^2+1)^2"), "x", &Expr::zero(), &Expr::one()),
            Integral::Exact(Expr::rational(28, 15).unwrap())
        );
    }

    #[test]
    fn definite_integrals_with_possible_poles_are_not_evaluated() {
        // ∫_{-1}^{1} 1/x dx is a principal value at best; it must not be reported as 0.
        for src in ["1/x", "log(x)", "tan(x)", "sqrt(x)", "abs(x)"] {
            let result = definite_integral(&p(src), "x", &p("-1"), &p("1"));
            assert!(
                matches!(result, Integral::Unevaluated(_)),
                "{src}: got {result:?}"
            );
        }
    }

    #[test]
    fn logical_operands_are_rejected_not_treated_as_numbers() {
        assert!(matches!(
            diff(&Expr::Bool(true), "x"),
            Err(Error::Unsupported(_))
        ));
        assert!(matches!(diff(&p("x = 1"), "x"), Err(Error::Unsupported(_))));
        assert!(matches!(
            integrate(&Expr::Bool(true), "x"),
            Integral::Unevaluated(_)
        ));
    }
}
