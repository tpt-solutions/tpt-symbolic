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

use crate::error::Error;
use crate::bigint::BigInt;
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
    /// Solutions depend on parameters that were not fixed. The equation has
    /// no variable-free solution and no closed form was produced.
    Parametric,
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
    if !f.contains_var(var) {
        return Ok(match f.as_number() {
            Some(n) if n.is_zero() => SolveResult::Infinite,
            Some(_) => SolveResult::NoSolution,
            None => SolveResult::Parametric,
        });
    }
    let Some(coeffs) = poly_coeffs(&f, var) else {
        return Ok(numeric(f, var));
    };
    let mut coeffs: Vec<Expr> = coeffs.iter().map(simplify).collect();
    while coeffs.last().is_some_and(|c| c.as_number().is_some_and(|n| n.is_zero())) {
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
            Some(roots) => exact_roots(roots),
            None => numeric(f, var),
        },
    })
}

fn numeric(expr: Expr, var: &str) -> SolveResult {
    SolveResult::Numeric(SymbolicRoot { expr, var: Arc::from(var) })
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
    let disc = simplify(&Expr::difference(Expr::power(b.clone(), Expr::int(2)), four_ac));
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
    let minus = simplify(&Expr::quotient(Expr::sum(vec![neg_b, Expr::negate(root)]), denom));
    exact_roots(vec![plus, minus])
}

fn exact_roots(mut roots: Vec<Expr>) -> SolveResult {
    roots.sort();
    roots.dedup();
    SolveResult::Exact(roots)
}

/// Largest divisor the rational-root search will enumerate by trial division.
const MAX_DIVISOR_SEARCH: u128 = 1_000_000;

fn constant_coefficients(coeffs: &[Expr]) -> Option<Vec<Number>> {
    coeffs.iter().map(|c| c.as_number()).collect()
}

/// Exact roots of a polynomial with rational coefficients, lowest degree first.
///
/// Peels off rational roots (rational root theorem, then synthetic division)
/// until degree 2 or less, then applies the quadratic formula to the remainder.
/// Returns `None` when a remainder of degree 3 or more has no rational root
/// that the search can find.
fn rational_roots(low: &[Number]) -> Option<Vec<Expr>> {
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
        let r = find_rational_root(&poly)?;
        poly = divide_by_linear(&poly, &r)?;
        roots.push(Expr::Const(r));
    }
    match poly.as_slice() {
        [c] if !c.is_zero() => {}
        [a1, a0] => roots.push(Expr::Const(a0.checked_div(a1)?.checked_neg()?)),
        [a, b, c] => match quadratic(&Expr::Const(a.clone()), &Expr::Const(b.clone()), &Expr::Const(c.clone())) {
            SolveResult::Exact(v) => roots.extend(v),
            SolveResult::NoSolution => {}
            _ => return None,
        },
        _ => return None,
    }
    Some(roots)
}

/// A rational root of `poly` (highest degree first, non-zero constant term), if one is found.
fn find_rational_root(poly: &[Number]) -> Option<Number> {
    let ints = integer_scale(poly)?;
    let constant = ints.last()?.abs().to_u128()?;
    let leading = ints.first()?.abs().to_u128()?;
    for &p in &divisors(constant)? {
        for &q in &divisors(leading)? {
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
        let (q, _) = lcm.divrem(&lcm.gcd(d))?;
        lcm = q.mul(d);
    }
    poly.iter()
        .map(|c| {
            let (factor, _) = lcm.divrem(c.denom())?;
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
    if m.iter().any(|row| row[..n].iter().all(|x| x.is_zero()) && !row[n].is_zero()) {
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
        let (q, _) = lcm.divrem(&lcm.gcd(x.denom())).ok_or(Error::DivisionByZero)?;
        lcm = q.mul(x.denom());
    }
    let mut ints: Vec<BigInt> = Vec::with_capacity(v.len());
    for x in v {
        let (factor, _) = lcm.divrem(x.denom()).ok_or(Error::DivisionByZero)?;
        ints.push(x.numer().mul(&factor));
    }
    let g = ints.iter().fold(BigInt::zero(), |acc, x| acc.gcd(x));
    if !g.is_zero() {
        for x in ints.iter_mut() {
            let (q, _) = x.divrem(&g).ok_or(Error::DivisionByZero)?;
            *x = q;
        }
    }
    if ints.iter().find(|x| !x.is_zero()).is_some_and(|x| x.is_negative()) {
        for x in ints.iter_mut() {
            *x = x.negate();
        }
    }
    ints.iter().map(|x| x.to_i128().ok_or(Error::Overflow)).collect()
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
        assert_eq!(solve(&p("2*x + 6 = 0"), "x").unwrap(), SolveResult::Exact(vec![p("-3")]));
        assert_eq!(solve(&p("x = 1/3 + 1/3 + 1/3"), "x").unwrap(), SolveResult::Exact(vec![p("1")]));
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
        assert_eq!(r, SolveResult::Exact(vec![Expr::negate(root.clone()), root]));
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
        assert_eq!(solve(&p("x^2 + 1 = 0"), "x").unwrap(), SolveResult::NoSolution);
        assert_eq!(solve(&p("a = 0"), "x").unwrap(), SolveResult::Parametric);
    }

    #[test]
    fn transcendental_and_high_degree_hand_off_numerically() {
        match solve(&p("x = cos(x)"), "x").unwrap() {
            SolveResult::Numeric(root) => assert_eq!(&*root.var, "x"),
            other => panic!("expected numeric hand-off, got {other:?}"),
        }
        assert!(matches!(solve(&p("x^3 - 2 = 0"), "x").unwrap(), SolveResult::Numeric(_)));
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
        assert_eq!(solve_linear_system(&a, &[q(1, 1), q(3, 1)]).unwrap(), SolveResult::NoSolution);
        assert_eq!(solve_linear_system(&a, &[q(1, 1), q(2, 1)]).unwrap(), SolveResult::Parametric);
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
        assert_eq!(primitive_integer_vector(&basis[0]).unwrap(), vec![1, 5, 3, 4]);
    }
}
