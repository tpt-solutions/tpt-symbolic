//! Equation and linear-system solvers.
//!
//! * [`solve`] handles one equation in one variable. Polynomials of degree 1
//!   and 2 give exact radical roots, possibly with symbolic coefficients. Any
//!   other equation becomes a [`SolveResult::Numeric`] hand-off for numeric
//!   root-finding in `tpt-math`.
//! * [`solve_linear_system`] and [`nullspace`] do exact Gaussian elimination over
//!   [`Number`], with no rounding anywhere.
//!
//! Real solutions only. A quadratic with a negative constant discriminant
//! gives [`SolveResult::NoSolution`].
//!
//! Not implemented yet: cubic and quartic radical formulas. Those degrees
//! currently return [`SolveResult::Numeric`].
//!
//! Symbolic leading coefficients are assumed non-zero, and symbolic
//! discriminants are assumed non-negative. Both assumptions are stated here
//! and are not checked.

use crate::bigint::BigInt;
use crate::error::Error;
use crate::expr::Expr;
use crate::mathr::Number;
use crate::simplify::simplify;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;

/// Highest polynomial degree the coefficient extractor accepts.
const MAX_POLY_DEGREE: usize = 8;

/// An equation `expr(var) = 0` that has no closed form here. A numeric
/// solver (Newton-Raphson in `tpt-math`) can take it from this point.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SymbolicRoot {
    /// Left-hand side after moving everything to one side.
    pub expr: Expr,
    /// The unknown.
    pub var: Arc<str>,
}

/// Outcome of solving an equation.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum SolveResult {
    /// Every solution, in exact form, sorted and without duplicates.
    Exact(Vec<Expr>),
    /// Solutions depend on free unknowns other than the one being solved for.
    /// Produced by [`solve_linear_system`].
    Parametric,
    /// The equation contains no unknown. It holds exactly when `Conditional(e)`
    /// is zero, where `e` is the expression returned here.
    Conditional(Expr),
    /// Some roots are exact. The remaining factor has no closed form here and is
    /// handed off to numeric root-finding.
    Partial {
        /// Exact roots, sorted and without duplicates.
        exact: Vec<Expr>,
        /// The remaining factor, to be solved numerically.
        rest: SymbolicRoot,
    },
    /// No closed form. Hand off to numeric root-finding.
    Numeric(SymbolicRoot),
    /// No real solution.
    NoSolution,
    /// Every value of the unknown is a solution.
    Infinite,
}

/// Solves `equation` for `var`. A bare expression `f` means `f = 0`.
pub fn solve(equation: &Expr, var: &str) -> Result<SolveResult, Error> {
    let f = match equation {
        Expr::Eq(l, r) => simplify(&Expr::difference((**l).clone(), (**r).clone())),
        other => simplify(other),
    };
    match f {
        Expr::Bool(true) => return Ok(SolveResult::Infinite),
        Expr::Bool(false) => return Ok(SolveResult::NoSolution),
        _ => {}
    }
    if !f.is_arithmetic() {
        return Err(Error::Unsupported("solve expects an arithmetic equation"));
    }
    if !f.contains_var(var) {
        return Ok(match f.as_number() {
            Some(n) if n.is_zero() => SolveResult::Infinite,
            Some(_) => SolveResult::NoSolution,
            None => SolveResult::Conditional(f.clone()),
        });
    }
    let Some(coeffs) = poly_coeffs(&f, var) else {
        return Ok(numeric(f, var));
    };
    let mut coeffs: Vec<Expr> = coeffs.iter().map(simplify).collect();
    while coeffs
        .last()
        .is_some_and(|c| c.as_number().is_some_and(|n| n.is_zero()))
    {
        coeffs.pop();
    }
    Ok(match coeffs.len() {
        0 | 1 => numeric(f, var),
        2 => SolveResult::Exact(vec![simplify(&Expr::quotient(
            Expr::negate(coeffs[0].clone()),
            coeffs[1].clone(),
        ))]),
        3 => quadratic(&coeffs[2], &coeffs[1], &coeffs[0]),
        _ => match constant_coefficients(&coeffs).and_then(|low| rational_roots(&low)) {
            Some(Split::Exact(roots)) => exact_roots(roots),
            Some(Split::Partial { roots, rest }) if !roots.is_empty() => partial(roots, &rest, var),
            _ => numeric(f, var),
        },
    })
}

fn numeric(expr: Expr, var: &str) -> SolveResult {
    SolveResult::Numeric(SymbolicRoot {
        expr,
        var: Arc::from(var),
    })
}

/// Roots of `a*y^2 + b*y + c = 0`.
fn quadratic(a: &Expr, b: &Expr, c: &Expr) -> SolveResult {
    // Pure quadratic: y = ±sqrt(-c/a).
    if b.as_number().is_some_and(|n| n.is_zero()) {
        let rhs = simplify(&Expr::quotient(Expr::negate(c.clone()), a.clone()));
        if rhs.as_number().is_some_and(|n| n.is_negative()) {
            return SolveResult::NoSolution;
        }
        let root = simplify(&Expr::power(rhs.clone(), Expr::Const(half())));
        return exact_roots(vec![root.clone(), Expr::negate(root)]);
    }

    let four_ac = Expr::product(vec![Expr::int(4), a.clone(), c.clone()]);
    let disc = simplify(&Expr::difference(
        Expr::power(b.clone(), Expr::int(2)),
        four_ac,
    ));
    if disc.as_number().is_some_and(|n| n.is_negative()) {
        return SolveResult::NoSolution;
    }
    let root = Expr::power(disc, Expr::Const(half()));
    let denom = Expr::product(vec![Expr::int(2), a.clone()]);
    let neg_b = Expr::negate(b.clone());
    let plus = simplify(&Expr::quotient(
        Expr::sum(vec![neg_b.clone(), root.clone()]),
        denom.clone(),
    ));
    let minus = simplify(&Expr::quotient(
        Expr::sum(vec![neg_b, Expr::negate(root)]),
        denom,
    ));
    exact_roots(vec![plus, minus])
}

fn exact_roots(mut roots: Vec<Expr>) -> SolveResult {
    if roots.is_empty() {
        return SolveResult::NoSolution;
    }
    roots.sort();
    roots.dedup();
    SolveResult::Exact(roots)
}

/// Largest divisor the rational-root search will enumerate by trial division.
const MAX_DIVISOR_SEARCH: u128 = 1_000_000;

fn constant_coefficients(coeffs: &[Expr]) -> Option<Vec<Number>> {
    coeffs.iter().map(|c| c.as_number()).collect()
}

/// Result of peeling roots off a polynomial.
enum Split {
    /// Every root is exact.
    Exact(Vec<Expr>),
    /// The exact `roots` were found; `rest` (highest degree first) has no closed
    /// form that the search can find.
    Partial { roots: Vec<Expr>, rest: Vec<Number> },
}

/// Exact roots of a polynomial with rational coefficients, lowest degree first.
///
/// Peels off rational roots (rational root theorem, then synthetic division)
/// until degree 2 or less, then applies the quadratic formula to the remainder.
/// Returns `None` only for shapes the search does not handle at all. A remainder
/// of degree 3 or more with no closed form is returned as [`Split::Partial`].
fn rational_roots(low: &[Number]) -> Option<Split> {
    let mut poly: Vec<Number> = low.iter().rev().cloned().collect(); // highest degree first
    let mut roots = Vec::new();
    loop {
        // A zero constant term means a root at zero.
        while poly.len() > 1 && poly.last()?.is_zero() {
            poly.pop();
            roots.push(Expr::zero());
        }
        if poly.len() <= 3 {
            break;
        }
        let Some(r) = find_rational_root(&poly) else {
            // No rational root. A pure power or biquadratic equation still has
            // exact real roots, so use those; anything else is a partial result.
            return Some(
                match pure_power_roots(&poly).or_else(|| biquadratic_roots(&poly)) {
                    Some(more) => {
                        roots.extend(more);
                        Split::Exact(roots)
                    }
                    None => Split::Partial { roots, rest: poly },
                },
            );
        };
        poly = divide_by_linear(&poly, &r)?;
        roots.push(Expr::Const(r));
    }
    match poly.as_slice() {
        [c] if !c.is_zero() => {}
        [a1, a0] => roots.push(Expr::Const(a0.checked_div(a1)?.checked_neg()?)),
        [a, b, c] => match quadratic(
            &Expr::Const(a.clone()),
            &Expr::Const(b.clone()),
            &Expr::Const(c.clone()),
        ) {
            SolveResult::Exact(v) => roots.extend(v),
            SolveResult::NoSolution => {}
            _ => return None,
        },
        _ => return None,
    }
    Some(Split::Exact(roots))
}

/// Builds [`SolveResult::Partial`] from exact `roots` and the unsolved factor `rest`
/// (highest degree first).
fn partial(mut roots: Vec<Expr>, rest: &[Number], var: &str) -> SolveResult {
    roots.sort();
    roots.dedup();
    // Scale to a monic factor. The zeros are unchanged, and the sign of the
    // leading coefficient depends on how the equation was arranged.
    let lead = &rest[0];
    let top = rest.len() - 1;
    let terms = rest.iter().enumerate().map(|(i, c)| {
        let c = c.checked_div(lead).unwrap_or_else(|| c.clone());
        Expr::product(vec![
            Expr::Const(c),
            Expr::power(Expr::var(var), Expr::int((top - i) as i128)),
        ])
    });
    let expr = simplify(&Expr::sum(terms.collect()));
    SolveResult::Partial {
        exact: roots,
        rest: SymbolicRoot {
            expr,
            var: Arc::from(var),
        },
    }
}

/// Real roots of `a*x^n + c = 0` (highest degree first, `n >= 3`), in exact form.
///
/// Returns `None` for any other shape. Even `n` with a negative `-c/a` has no real
/// roots, and gives an empty list.
fn pure_power_roots(poly: &[Number]) -> Option<Vec<Expr>> {
    let n = poly.len().checked_sub(1)?;
    if n < 3 || poly[1..n].iter().any(|c| !c.is_zero()) {
        return None;
    }
    let (lead, konst) = (poly.first()?, poly.last()?);
    if lead.is_zero() || konst.is_zero() {
        return None;
    }
    // x^n = r, with r = -c/a.
    let r = konst.checked_div(lead)?.checked_neg()?;
    let negative = r.is_negative();
    let magnitude = if negative { r.checked_neg()? } else { r };
    let root = simplify(&Expr::power(
        Expr::Const(magnitude),
        Expr::rational(1, n as i128)?,
    ));
    if n % 2 == 1 {
        return Some(vec![if negative { Expr::negate(root) } else { root }]);
    }
    if negative {
        return Some(Vec::new());
    }
    Some(vec![root.clone(), Expr::negate(root)])
}

/// Real roots of `a*x^4 + b*x^2 + c = 0` (highest degree first), in exact form.
///
/// Substitutes `u = x^2`, solves `a*u^2 + b*u + c = 0` exactly, then takes `x = ±sqrt(u)`
/// for each non-negative `u`. Returns `None` for any other shape.
fn biquadratic_roots(poly: &[Number]) -> Option<Vec<Expr>> {
    if poly.len() != 5 || !poly[1].is_zero() || !poly[3].is_zero() {
        return None;
    }
    let (a, b, c) = (&poly[0], &poly[2], &poly[4]);
    if a.is_zero() {
        return None;
    }
    let disc = b
        .checked_mul(b)?
        .checked_sub(&a.checked_mul(c)?.checked_mul(&Number::from_int(4))?)?;
    if disc.is_negative() {
        // Complex u gives no real x.
        return Some(Vec::new());
    }
    let neg_b = b.checked_neg()?;
    let two_a = a.checked_mul(&Number::from_int(2))?;
    let root = simplify(&Expr::power(Expr::Const(disc.clone()), Expr::Const(half())));
    let mut roots = Vec::new();
    for sigma in [1i128, -1] {
        // Sign of u = (-b + sigma*sqrt(disc)) / (2a), decided exactly.
        let sign = sign_of_sum_with_root(&neg_b, &disc, sigma)?;
        let u_sign = if a.is_negative() { -sign } else { sign };
        if u_sign < 0 {
            continue;
        }
        if u_sign == 0 {
            roots.push(Expr::zero());
            continue;
        }
        let u_num = Expr::sum(vec![
            Expr::Const(neg_b.clone()),
            Expr::product(vec![Expr::int(sigma), root.clone()]),
        ]);
        let u = simplify(&Expr::quotient(u_num, Expr::Const(two_a.clone())));
        let x = simplify(&Expr::power(u, Expr::Const(half())));
        roots.push(Expr::negate(x.clone()));
        roots.push(x);
    }
    Some(roots)
}

/// Sign (-1, 0 or 1) of `s + sigma*sqrt(d)` for rational `s`, `d >= 0` and `sigma` = ±1.
fn sign_of_sum_with_root(s: &Number, d: &Number, sigma: i128) -> Option<i32> {
    if d.is_zero() || s.is_zero() {
        // s + sigma*sqrt(d) = sigma*sqrt(d) when s = 0.
        return Some(if d.is_zero() { s_sign(s) } else { sigma as i32 });
    }
    let s_pos = !s.is_negative();
    if s_pos == (sigma > 0) {
        // Both terms share a sign.
        return Some(if s_pos { 1 } else { -1 });
    }
    // Opposite signs: compare |s| with sqrt(d) through squares.
    let s_sq = s.checked_mul(s)?;
    Some(match s_sq.cmp_value(d) {
        core::cmp::Ordering::Equal => 0,
        // |s| > sqrt(d): the sign of s wins.
        core::cmp::Ordering::Greater => {
            if s_pos {
                1
            } else {
                -1
            }
        }
        // |s| < sqrt(d): the sqrt term wins.
        core::cmp::Ordering::Less => sigma as i32,
    })
}

fn s_sign(s: &Number) -> i32 {
    if s.is_zero() {
        0
    } else if s.is_negative() {
        -1
    } else {
        1
    }
}

/// A rational root of `poly` (highest degree first, non-zero constant term), if one is found.
fn find_rational_root(poly: &[Number]) -> Option<Number> {
    let ints = integer_scale(poly)?;
    let constant = ints.last()?.abs().to_u128()?;
    let leading = ints.first()?.abs().to_u128()?;
    // Candidates are built as i128, so larger coefficients fall back to numeric.
    if constant > i128::MAX as u128 || leading > i128::MAX as u128 {
        return None;
    }
    let leading_divisors = divisors(leading)?;
    for &p in &divisors(constant)? {
        for &q in &leading_divisors {
            for sign in [1i128, -1] {
                let Some(r) = Number::from_parts(
                    BigInt::from_i128(sign * p as i128),
                    BigInt::from_i128(q as i128),
                ) else {
                    continue;
                };
                if eval_is_zero(poly, &r) == Some(true) {
                    return Some(r);
                }
            }
        }
    }
    None
}

/// Scales rational coefficients by the lcm of their denominators.
fn integer_scale(poly: &[Number]) -> Option<Vec<BigInt>> {
    let mut lcm = BigInt::one();
    for c in poly {
        let d = c.denom();
        let (q, _) = lcm.divrem(&lcm.gcd(&d))?;
        lcm = q.mul(&d);
    }
    poly.iter()
        .map(|c| {
            let (factor, _) = lcm.divrem(&c.denom())?;
            Some(c.numer().mul(&factor))
        })
        .collect()
}

fn divisors(n: u128) -> Option<Vec<u128>> {
    if n == 0 {
        return None;
    }
    let mut out = Vec::new();
    let mut d: u128 = 1;
    while d * d <= n {
        if d > MAX_DIVISOR_SEARCH {
            return None;
        }
        if n.is_multiple_of(d) {
            out.push(d);
            if d != n / d {
                out.push(n / d);
            }
        }
        d += 1;
    }
    Some(out)
}

/// Horner evaluation: whether `poly` (highest degree first) vanishes at `r`.
fn eval_is_zero(poly: &[Number], r: &Number) -> Option<bool> {
    let mut acc = Number::zero();
    for c in poly {
        acc = acc.checked_mul(r)?.checked_add(c)?;
    }
    Some(acc.is_zero())
}

/// Quotient of `poly` (highest degree first) by `x - r`. The remainder is dropped,
/// so `r` must be a root.
fn divide_by_linear(poly: &[Number], r: &Number) -> Option<Vec<Number>> {
    let mut out = Vec::with_capacity(poly.len() - 1);
    let mut acc = Number::zero();
    for (i, c) in poly.iter().enumerate() {
        acc = acc.checked_mul(r)?.checked_add(c)?;
        if i + 1 < poly.len() {
            out.push(acc.clone());
        }
    }
    Some(out)
}

fn half() -> Number {
    Number::new(1, 2).unwrap_or(Number::one())
}

/// Coefficients of `e` viewed as a polynomial in `var`, lowest degree first.
/// `None` if `e` is not a polynomial of degree at most [`MAX_POLY_DEGREE`].
fn poly_coeffs(e: &Expr, var: &str) -> Option<Vec<Expr>> {
    if !e.contains_var(var) {
        return Some(vec![e.clone()]);
    }
    match e {
        Expr::Var(_) => Some(vec![Expr::zero(), Expr::one()]),
        Expr::Add(v) => {
            let mut acc = vec![Expr::zero()];
            for t in v {
                let c = poly_coeffs(t, var)?;
                acc = add_polys(&acc, &c);
            }
            Some(acc)
        }
        Expr::Mul(v) => {
            let mut acc = vec![Expr::one()];
            for f in v {
                let c = poly_coeffs(f, var)?;
                acc = mul_polys(&acc, &c)?;
            }
            Some(acc)
        }
        Expr::Pow(b, x) => {
            let n = x.as_number()?;
            let k = n.as_i128()?;
            if k < 0 || k > MAX_POLY_DEGREE as i128 {
                return None;
            }
            let base = poly_coeffs(b, var)?;
            let mut acc = vec![Expr::one()];
            for _ in 0..k {
                acc = mul_polys(&acc, &base)?;
            }
            Some(acc)
        }
        _ => None,
    }
}

fn add_polys(a: &[Expr], b: &[Expr]) -> Vec<Expr> {
    let n = a.len().max(b.len());
    (0..n)
        .map(|i| {
            let x = a.get(i).cloned().unwrap_or_else(Expr::zero);
            let y = b.get(i).cloned().unwrap_or_else(Expr::zero);
            Expr::sum(vec![x, y])
        })
        .collect()
}

fn mul_polys(a: &[Expr], b: &[Expr]) -> Option<Vec<Expr>> {
    let len = a.len() + b.len() - 1;
    if len > MAX_POLY_DEGREE + 1 {
        return None;
    }
    let mut out = vec![Expr::zero(); len];
    for (i, x) in a.iter().enumerate() {
        for (j, y) in b.iter().enumerate() {
            let term = Expr::product(vec![x.clone(), y.clone()]);
            out[i + j] = Expr::sum(vec![out[i + j].clone(), term]);
        }
    }
    Some(out)
}

/// Exact row-reduced echelon form, in place, over the first `ncols` columns.
/// Returns the pivot column of each pivot row, in order.
fn rref(m: &mut [Vec<Number>], ncols: usize) -> Result<Vec<usize>, Error> {
    let rows = m.len();
    let mut pivots = Vec::new();
    let mut r = 0;
    for c in 0..ncols {
        if r >= rows {
            break;
        }
        let Some(p) = (r..rows).find(|&i| !m[i][c].is_zero()) else {
            continue;
        };
        m.swap(r, p);
        let inv = m[r][c].checked_recip().ok_or(Error::DivisionByZero)?;
        for x in m[r][c..].iter_mut() {
            *x = x.checked_mul(&inv).ok_or(Error::Overflow)?;
        }
        let pivot_row: Vec<Number> = m[r].clone();
        for (i, row) in m.iter_mut().enumerate() {
            if i == r || row[c].is_zero() {
                continue;
            }
            let factor = row[c].clone();
            for k in c..row.len() {
                let t = pivot_row[k].checked_mul(&factor).ok_or(Error::Overflow)?;
                row[k] = row[k].checked_sub(&t).ok_or(Error::Overflow)?;
            }
        }
        pivots.push(c);
        r += 1;
    }
    Ok(pivots)
}

/// Solves the exact linear system `a * x = b`.
///
/// Returns [`SolveResult::Exact`] with one constant per unknown when the
/// solution is unique, [`SolveResult::Parametric`] when unknowns are free, and
/// [`SolveResult::NoSolution`] when the system is inconsistent.
pub fn solve_linear_system(a: &[Vec<Number>], b: &[Number]) -> Result<SolveResult, Error> {
    if a.len() != b.len() || a.is_empty() {
        return Err(Error::Unsupported("linear system dimensions do not match"));
    }
    let n = a[0].len();
    if a.iter().any(|row| row.len() != n) {
        return Err(Error::Unsupported("ragged coefficient matrix"));
    }
    let mut m: Vec<Vec<Number>> = a
        .iter()
        .zip(b)
        .map(|(row, rhs)| {
            let mut r = row.clone();
            r.push(rhs.clone());
            r
        })
        .collect();
    let pivots = rref(&mut m, n)?;
    if m.iter()
        .any(|row| row[..n].iter().all(|x| x.is_zero()) && !row[n].is_zero())
    {
        return Ok(SolveResult::NoSolution);
    }
    if pivots.len() < n {
        return Ok(SolveResult::Parametric);
    }
    let solution = pivots
        .iter()
        .enumerate()
        .map(|(i, _)| Expr::Const(m[i][n].clone()))
        .collect();
    Ok(SolveResult::Exact(solution))
}

/// Basis of the null space of `a`: every `v` with `a * v = 0`, exact.
pub fn nullspace(a: &[Vec<Number>]) -> Result<Vec<Vec<Number>>, Error> {
    if a.is_empty() {
        return Ok(Vec::new());
    }
    let n = a[0].len();
    if a.iter().any(|row| row.len() != n) {
        return Err(Error::Unsupported("ragged coefficient matrix"));
    }
    let mut m: Vec<Vec<Number>> = a.to_vec();
    let pivots = rref(&mut m, n)?;
    let mut basis = Vec::new();
    for free in (0..n).filter(|c| !pivots.contains(c)) {
        let mut v = vec![Number::zero(); n];
        v[free] = Number::one();
        for (row, &pc) in pivots.iter().enumerate() {
            v[pc] = m[row][free].checked_neg().ok_or(Error::Overflow)?;
        }
        basis.push(v);
    }
    Ok(basis)
}

/// Scales a rational vector to the smallest integer vector pointing the same way.
/// The first non-zero entry is made positive. Used for stoichiometric coefficients.
pub fn primitive_integer_vector(v: &[Number]) -> Result<Vec<i128>, Error> {
    let mut lcm = BigInt::one();
    for x in v {
        let (q, _) = lcm
            .divrem(&lcm.gcd(&x.denom()))
            .ok_or(Error::DivisionByZero)?;
        lcm = q.mul(&x.denom());
    }
    let mut ints: Vec<BigInt> = Vec::with_capacity(v.len());
    for x in v {
        let (factor, _) = lcm.divrem(&x.denom()).ok_or(Error::DivisionByZero)?;
        ints.push(x.numer().mul(&factor));
    }
    let g = ints.iter().fold(BigInt::zero(), |acc, x| acc.gcd(x));
    if !g.is_zero() {
        for x in ints.iter_mut() {
            let (q, _) = x.divrem(&g).ok_or(Error::DivisionByZero)?;
            *x = q;
        }
    }
    if ints
        .iter()
        .find(|x| !x.is_zero())
        .is_some_and(|x| x.is_negative())
    {
        for x in ints.iter_mut() {
            *x = x.negate();
        }
    }
    ints.iter()
        .map(|x| x.to_i128().ok_or(Error::Overflow))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse;

    fn p(s: &str) -> Expr {
        parse(s).unwrap_or_else(|e| panic!("parse {s:?}: {e}"))
    }

    fn q(n: i128, d: i128) -> Number {
        Number::new(n, d).unwrap()
    }

    #[test]
    fn linear_equation() {
        assert_eq!(
            solve(&p("2*x + 6 = 0"), "x").unwrap(),
            SolveResult::Exact(vec![p("-3")])
        );
        assert_eq!(
            solve(&p("x = 1/3 + 1/3 + 1/3"), "x").unwrap(),
            SolveResult::Exact(vec![p("1")])
        );
    }

    #[test]
    fn quadratic_gives_exact_radicals() {
        // x^2 - 2 = 0 -> ±sqrt(2)
        let r = solve(&p("x^2 - 2 = 0"), "x").unwrap();
        assert_eq!(r, SolveResult::Exact(vec![p("-(2^(1/2))"), p("2^(1/2)")]));
        // x^2 - 5x + 6 = 0 -> 2, 3
        let r = solve(&p("x^2 - 5*x + 6 = 0"), "x").unwrap();
        assert_eq!(r, SolveResult::Exact(vec![p("2"), p("3")]));
        // Double root.
        let r = solve(&p("x^2 + 2*x + 1 = 0"), "x").unwrap();
        assert_eq!(r, SolveResult::Exact(vec![p("-1")]));
    }

    #[test]
    fn circle_solved_for_y_matches_spec() {
        // y^2 + x^2 = 25 -> y = ±sqrt(25 - x^2)
        let r = solve(&p("x^2 + y^2 = 25"), "y").unwrap();
        let root = p("(25 - x^2)^(1/2)");
        assert_eq!(
            r,
            SolveResult::Exact(vec![Expr::negate(root.clone()), root])
        );
    }

    #[test]
    fn cubic_and_quartic_with_rational_roots_are_exact() {
        assert_eq!(
            solve(&p("x^3 - 6*x^2 + 11*x - 6 = 0"), "x").unwrap(),
            SolveResult::Exact(vec![p("1"), p("2"), p("3")])
        );
        assert_eq!(
            solve(&p("x^4 - 5*x^2 + 4 = 0"), "x").unwrap(),
            SolveResult::Exact(vec![p("-2"), p("-1"), p("1"), p("2")])
        );
        // (x - 1)(x^2 - 2): one rational root and a pair of radicals.
        assert_eq!(
            solve(&p("x^3 - x^2 - 2*x + 2 = 0"), "x").unwrap(),
            SolveResult::Exact(vec![p("1"), Expr::negate(p("2^(1/2)")), p("2^(1/2)")])
        );
    }

    #[test]
    fn degenerate_and_impossible_cases() {
        assert_eq!(solve(&p("0 = 0"), "x").unwrap(), SolveResult::Infinite);
        assert_eq!(solve(&p("1 = 0"), "x").unwrap(), SolveResult::NoSolution);
        assert_eq!(
            solve(&p("x^2 + 1 = 0"), "x").unwrap(),
            SolveResult::NoSolution
        );
        // No unknown in the equation: it holds when `a` is zero.
        assert_eq!(
            solve(&p("a = 0"), "x").unwrap(),
            SolveResult::Conditional(simplify(&p("a")))
        );
    }

    #[test]
    fn transcendental_and_high_degree_hand_off_numerically() {
        match solve(&p("x = cos(x)"), "x").unwrap() {
            SolveResult::Numeric(root) => assert_eq!(&*root.var, "x"),
            other => panic!("expected numeric hand-off, got {other:?}"),
        }
        // A cubic with no rational root and no pure-power shape stays numeric.
        assert!(matches!(
            solve(&p("x^3 + x + 1 = 0"), "x").unwrap(),
            SolveResult::Numeric(_)
        ));
    }

    #[test]
    fn partial_roots_keep_the_exact_part() {
        // (x - 1)(x^3 + x + 1): x = 1 is exact; the cubic factor is handed off.
        match solve(&p("x^4 - x^3 + x^2 - 1 = 0"), "x").unwrap() {
            SolveResult::Partial { exact, rest } => {
                assert_eq!(exact, vec![Expr::int(1)]);
                assert_eq!(rest.var.as_ref(), "x");
                assert_eq!(rest.expr, simplify(&p("x^3 + x + 1")));
            }
            other => panic!("expected a partial result, got {other:?}"),
        }
        // A quartic with no rational root and no closed form stays numeric.
        assert!(matches!(
            solve(&p("x^5 + x + 1 = 0"), "x").unwrap(),
            SolveResult::Numeric(_)
        ));
    }

    #[test]
    fn biquadratic_equations_have_exact_real_roots() {
        // x^4 - 3x^2 + 1: u = (3 ± sqrt5)/2, both positive, so four real roots.
        match solve(&p("x^4 - 3*x^2 + 1 = 0"), "x").unwrap() {
            SolveResult::Exact(v) => assert_eq!(v.len(), 4),
            other => panic!("expected exact roots, got {other:?}"),
        }
        // x^4 + x^2 + 1: u is complex, so no real roots.
        assert_eq!(
            solve(&p("x^4 + x^2 + 1 = 0"), "x").unwrap(),
            SolveResult::NoSolution
        );
        // x^4 - x^2 - 2 = (x^2 - 2)(x^2 + 1): only ±sqrt(2) are real.
        match solve(&p("x^4 - x^2 - 2 = 0"), "x").unwrap() {
            SolveResult::Exact(v) => assert_eq!(v.len(), 2),
            other => panic!("expected exact roots, got {other:?}"),
        }
        // x^4 - 2x^2 = 0: u = 0 or 2, so x = 0 and ±sqrt(2).
        match solve(&p("x^4 - 2*x^2 = 0"), "x").unwrap() {
            SolveResult::Exact(v) => assert_eq!(v.len(), 3),
            other => panic!("expected exact roots, got {other:?}"),
        }
    }

    #[test]
    fn pure_power_equations_have_exact_real_roots() {
        // x^3 = 2 has one real root; x^4 = 2 has two.
        assert_eq!(
            solve(&p("x^3 - 2 = 0"), "x").unwrap(),
            SolveResult::Exact(vec![simplify(&Expr::power(
                Expr::int(2),
                Expr::rational(1, 3).unwrap(),
            ))])
        );
        match solve(&p("x^4 - 2 = 0"), "x").unwrap() {
            SolveResult::Exact(v) => assert_eq!(v.len(), 2),
            other => panic!("expected exact roots, got {other:?}"),
        }
        // x^4 = -2 has no real roots.
        assert_eq!(
            solve(&p("x^4 + 2 = 0"), "x").unwrap(),
            SolveResult::NoSolution
        );
        // x^3 = -8 has one real root, and it is rational.
        assert_eq!(
            solve(&p("x^3 + 8 = 0"), "x").unwrap(),
            SolveResult::Exact(vec![Expr::int(-2)])
        );
    }

    #[test]
    fn nullspace_rejects_ragged_input() {
        let a = vec![vec![q(1, 1), q(2, 1)], vec![q(3, 1)]];
        assert!(matches!(nullspace(&a), Err(Error::Unsupported(_))));
    }

    #[test]
    fn exact_linear_system() {
        // 2x + 3y = 8, x - y = -1  ->  x = 1, y = 2
        let a = vec![vec![q(2, 1), q(3, 1)], vec![q(1, 1), q(-1, 1)]];
        let b = vec![q(8, 1), q(-1, 1)];
        assert_eq!(
            solve_linear_system(&a, &b).unwrap(),
            SolveResult::Exact(vec![Expr::int(1), Expr::int(2)])
        );
        // Rational solution stays exact.
        let a = vec![vec![q(3, 1)]];
        let b = vec![q(1, 1)];
        assert_eq!(
            solve_linear_system(&a, &b).unwrap(),
            SolveResult::Exact(vec![Expr::rational(1, 3).unwrap()])
        );
    }

    #[test]
    fn inconsistent_and_underdetermined_systems() {
        let a = vec![vec![q(1, 1), q(1, 1)], vec![q(2, 1), q(2, 1)]];
        assert_eq!(
            solve_linear_system(&a, &[q(1, 1), q(3, 1)]).unwrap(),
            SolveResult::NoSolution
        );
        assert_eq!(
            solve_linear_system(&a, &[q(1, 1), q(2, 1)]).unwrap(),
            SolveResult::Parametric
        );
    }

    #[test]
    fn balances_propane_combustion() {
        // C3H8 + O2 -> CO2 + H2O. Columns: C3H8, O2, CO2, H2O. Rows: C, H, O.
        let a = vec![
            vec![q(3, 1), q(0, 1), q(-1, 1), q(0, 1)],
            vec![q(8, 1), q(0, 1), q(0, 1), q(-2, 1)],
            vec![q(0, 1), q(2, 1), q(-2, 1), q(-1, 1)],
        ];
        let basis = nullspace(&a).unwrap();
        assert_eq!(basis.len(), 1);
        assert_eq!(
            primitive_integer_vector(&basis[0]).unwrap(),
            vec![1, 5, 3, 4]
        );
    }

    #[test]
    fn logical_connectives_are_not_arithmetic_equations() {
        assert!(matches!(
            solve(&p("x = 1 and y = 2"), "x"),
            Err(Error::Unsupported(_))
        ));
        assert!(matches!(
            solve(&p("x = 1 or x = 2"), "x"),
            Err(Error::Unsupported(_))
        ));
        // A top-level constant still means "always" or "never".
        assert_eq!(
            solve(&Expr::Bool(true), "x").unwrap(),
            SolveResult::Infinite
        );
    }
}
