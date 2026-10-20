//! Partial-fraction decomposition over the rationals.
//!
//! [`partial_fractions`] takes a numerator and a denominator, both polynomials in one
//! variable with rational coefficients. It writes `num / den` as a polynomial plus a sum
//! of terms `A / f^j`, where each `f` is an irreducible factor of the denominator. The
//! unknown numerators `A` are solved exactly from a linear system. The result is then
//! checked against the original rational function at sample points, and returned only if
//! it agrees.
//!
//! `decompose` (crate-internal) is the same computation with the terms returned as data, for callers
//! that integrate or differentiate them term by term.
//!
//! The denominator must factor completely (see [`crate::factor`]). Otherwise the
//! result is an error, not an incomplete answer.

use crate::error::Error;
use crate::eval::eval;
use crate::expr::Expr;
use crate::factor::{dense_coefficients, factor, poly_expr};
use crate::mathr::Number;
use crate::simplify::simplify;
use crate::solve::{solve_linear_system, SolveResult};
use alloc::vec;
use alloc::vec::Vec;

/// Points at which the decomposition is compared with the original function.
const CHECK_POINTS: [i128; 7] = [-3, -2, -1, 0, 1, 2, 3];

/// One term `numerator / factor^power` of a decomposition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PartialTerm {
    /// Numerator, lowest degree first, of degree below `factor`'s degree.
    pub numerator: Vec<Number>,
    /// The irreducible factor, monic, lowest degree first.
    pub factor: Vec<Number>,
    /// The power of the factor in the denominator (at least 1).
    pub power: usize,
}

/// `num / den = polynomial + sum of terms`, with polynomial and coefficients lowest first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Decomposition {
    /// The polynomial part.
    pub polynomial: Vec<Number>,
    /// The fractional terms.
    pub terms: Vec<PartialTerm>,
}

/// The decomposition `num / den` as an expression in `var`.
/// # Example
/// ```
/// use tpt_symbolic::apart::partial_fractions;
/// use tpt_symbolic::parse::parse;
/// let d = partial_fractions(&parse("1").unwrap(), &parse("x^2 - 1").unwrap(), "x").unwrap();
/// assert_eq!(d.to_string(), "-1/2*(x + 1)^(-1) + 1/2*(x - 1)^(-1)");
/// ```
pub fn partial_fractions(num: &Expr, den: &Expr, var: &str) -> Result<Expr, Error> {
    let decomposition = decompose_exprs(num, den, var)?;
    let mut terms = vec![poly_expr(&decomposition.polynomial, var)];
    for term in &decomposition.terms {
        let denom = Expr::power(poly_expr(&term.factor, var), Expr::int(term.power as i128));
        terms.push(Expr::quotient(poly_expr(&term.numerator, var), denom));
    }
    let result = simplify(&Expr::sum(terms));

    // Compare with the original at sample points where the denominator is non-zero.
    let original = Expr::quotient(num.clone(), den.clone());
    let mut checked = 0;
    for x in CHECK_POINTS {
        let point = [(var, Number::from_int(x))];
        let (Ok(want), Ok(got)) = (eval(&original, &point), eval(&result, &point)) else {
            continue;
        };
        if want != got {
            return Err(Error::Unsupported("partial fraction check failed"));
        }
        checked += 1;
    }
    if checked < 3 {
        return Err(Error::Unsupported(
            "too few sample points to check the decomposition",
        ));
    }
    Ok(result)
}

/// Decomposes `num / den` into data. Checked the same way as [`partial_fractions`].
pub(crate) fn decompose(num: &Expr, den: &Expr, var: &str) -> Result<Decomposition, Error> {
    let d = decompose_exprs(num, den, var)?;
    let original = Expr::quotient(num.clone(), den.clone());
    let rebuilt = {
        let mut terms = vec![poly_expr(&d.polynomial, var)];
        for t in &d.terms {
            let denom = Expr::power(poly_expr(&t.factor, var), Expr::int(t.power as i128));
            terms.push(Expr::quotient(poly_expr(&t.numerator, var), denom));
        }
        simplify(&Expr::sum(terms))
    };
    let mut checked = 0;
    for x in CHECK_POINTS {
        let point = [(var, Number::from_int(x))];
        let (Ok(want), Ok(got)) = (eval(&original, &point), eval(&rebuilt, &point)) else {
            continue;
        };
        if want != got {
            return Err(Error::Unsupported("partial fraction check failed"));
        }
        checked += 1;
    }
    if checked < 3 {
        return Err(Error::Unsupported(
            "too few sample points to check the decomposition",
        ));
    }
    Ok(d)
}

/// The linear-algebra core shared by [`partial_fractions`] and [`decompose`].
fn decompose_exprs(num: &Expr, den: &Expr, var: &str) -> Result<Decomposition, Error> {
    let p = dense_coefficients(num, var).ok_or(Error::Unsupported(
        "partial fractions need polynomial numerators",
    ))?;
    let q = dense_coefficients(den, var).ok_or(Error::Unsupported(
        "partial fractions need polynomial denominators",
    ))?;
    if q.iter().all(Number::is_zero) {
        return Err(Error::DivisionByZero);
    }
    let fac = factor(den, var)?;
    if !fac.complete {
        return Err(Error::Unsupported(
            "partial fractions need a denominator that factors completely",
        ));
    }

    // Polynomial part and remainder: p = s * q + r with deg r < deg q.
    let (s, r) = poly_div(&trim(p), &trim(q.clone()))?;

    // The pieces `f^j` that get numerators, with f monic and its low-first coefficients.
    let mut pieces: Vec<(Vec<Number>, usize)> = Vec::new();
    for factor_entry in &fac.factors {
        let f = dense_coefficients(&factor_entry.expr, var).ok_or(Error::Overflow)?;
        for j in 1..=factor_entry.multiplicity as usize {
            pieces.push((f.clone(), j));
        }
    }

    // Each piece `f^j` gets a numerator of degree below deg f. Column k is one unknown.
    let q_full = trim(q);
    let mut columns: Vec<(usize, Vec<Number>, usize)> = Vec::new(); // (piece index, basis B, power t)
    for (idx, (f, j)) in pieces.iter().enumerate() {
        let f_pow_j = poly_pow(f, *j)?;
        let (basis, rem) = poly_div(&q_full, &f_pow_j)?;
        if !rem.iter().all(Number::is_zero) {
            return Err(Error::Unsupported("factor does not divide the denominator"));
        }
        for t in 0..f.len() - 1 {
            columns.push((idx, basis.clone(), t));
        }
    }

    let rows = q_full.len() - 1;
    let cols = columns.len();
    if cols == 0 {
        // The denominator is a constant: the decomposition is just the polynomial part.
        return Ok(Decomposition {
            polynomial: s,
            terms: Vec::new(),
        });
    }
    // Row d (coefficient of x^d): sum over columns of [x^(d-t)] B.
    let mut matrix = vec![vec![Number::zero(); cols]; rows];
    for (k, (_, basis, t)) in columns.iter().enumerate() {
        for (d, row) in matrix.iter_mut().enumerate() {
            if d >= *t {
                if let Some(c) = basis.get(d - t) {
                    row[k] = c.clone();
                }
            }
        }
    }
    let rhs: Vec<Number> = (0..rows)
        .map(|d| r.get(d).cloned().unwrap_or_else(Number::zero))
        .collect();
    let values = match solve_linear_system(&matrix, &rhs)? {
        SolveResult::Exact(v) => v,
        _ => {
            return Err(Error::Unsupported(
                "partial fraction system is not uniquely solvable",
            ))
        }
    };
    let values: Vec<Number> = values
        .into_iter()
        .map(|e| e.as_number().ok_or(Error::Overflow))
        .collect::<Result<_, _>>()?;

    let terms = pieces
        .iter()
        .enumerate()
        .map(|(idx, (f, j))| {
            let numerator: Vec<Number> = columns
                .iter()
                .enumerate()
                .filter(|(_, (piece, _, _))| *piece == idx)
                .map(|(k, (_, _, t))| (*t, values[k].clone()))
                .fold(vec![Number::zero(); f.len() - 1], |mut acc, (t, v)| {
                    acc[t] = v;
                    acc
                });
            PartialTerm {
                numerator,
                factor: f.clone(),
                power: *j,
            }
        })
        .collect();
    Ok(Decomposition {
        polynomial: s,
        terms,
    })
}

/// Drops high-degree zero coefficients.
fn trim(mut p: Vec<Number>) -> Vec<Number> {
    while p.len() > 1 && p.last().is_some_and(Number::is_zero) {
        p.pop();
    }
    p
}

/// Polynomial product, low degree first.
fn poly_mul(a: &[Number], b: &[Number]) -> Result<Vec<Number>, Error> {
    let mut out = vec![Number::zero(); a.len() + b.len() - 1];
    for (i, x) in a.iter().enumerate() {
        for (j, y) in b.iter().enumerate() {
            let prod = x.checked_mul(y).ok_or(Error::Overflow)?;
            out[i + j] = out[i + j].checked_add(&prod).ok_or(Error::Overflow)?;
        }
    }
    Ok(out)
}

fn poly_pow(f: &[Number], n: usize) -> Result<Vec<Number>, Error> {
    let mut acc = vec![Number::one()];
    for _ in 0..n {
        acc = poly_mul(&acc, f)?;
    }
    Ok(acc)
}

/// Quotient and remainder of `p` divided by `q`, low degree first.
pub(crate) fn poly_div(p: &[Number], q: &[Number]) -> Result<(Vec<Number>, Vec<Number>), Error> {
    let q = trim(q.to_vec());
    let lead = q.last().cloned().ok_or(Error::DivisionByZero)?;
    if lead.is_zero() {
        return Err(Error::DivisionByZero);
    }
    let mut rem = trim(p.to_vec());
    if rem.len() < q.len() {
        return Ok((vec![Number::zero()], rem));
    }
    let mut quot = vec![Number::zero(); rem.len() - q.len() + 1];
    while rem.len() >= q.len() && !(rem.len() == 1 && rem[0].is_zero()) {
        let shift = rem.len() - q.len();
        let factor = rem
            .last()
            .and_then(|c| c.checked_div(&lead))
            .ok_or(Error::Overflow)?;
        quot[shift] = factor.clone();
        for (i, c) in q.iter().enumerate() {
            let delta = factor.checked_mul(c).ok_or(Error::Overflow)?;
            rem[shift + i] = rem[shift + i].checked_sub(&delta).ok_or(Error::Overflow)?;
        }
        rem.pop();
        rem = trim(rem);
        if rem.is_empty() {
            rem = vec![Number::zero()];
        }
    }
    Ok((quot, trim(rem)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse;
    use crate::simplify::expand;

    fn p(s: &str) -> Expr {
        parse(s).unwrap_or_else(|e| panic!("parse {s:?}: {e}"))
    }

    /// `decomposition` must equal the original function at every sample point.
    fn assert_same_function(num: &str, den: &str, decomposition: &Expr) {
        let original = Expr::quotient(p(num), p(den));
        for x in [-5, -4, -2, 3, 4, 7] {
            let point = [("x", Number::from_int(x))];
            assert_eq!(
                eval(&original, &point).unwrap(),
                eval(decomposition, &point).unwrap(),
                "at x = {x}"
            );
        }
    }

    #[test]
    fn distinct_linear_factors() {
        // 1 / (x^2 - 1) = (1/2)/(x - 1) - (1/2)/(x + 1).
        let d = partial_fractions(&p("1"), &p("x^2 - 1"), "x").unwrap();
        assert_same_function("1", "x^2 - 1", &d);
    }

    #[test]
    fn repeated_linear_factor() {
        // (x + 3) / (x (x - 1)^2).
        let d = partial_fractions(&p("x + 3"), &p("x*(x-1)^2"), "x").unwrap();
        assert_same_function("x + 3", "x*(x-1)^2", &d);
    }

    #[test]
    fn improper_fraction_has_a_polynomial_part() {
        // (x^3 + 1) / (x^2 - 1) = x + (x + 1) / (x^2 - 1) = x + 1/(x - 1).
        let d = partial_fractions(&p("x^3 + 1"), &p("x^2 - 1"), "x").unwrap();
        assert_same_function("x^3 + 1", "x^2 - 1", &d);
    }

    #[test]
    fn irreducible_quadratic_factor() {
        // 1 / (x (x^2 + 1)).
        let d = partial_fractions(&p("1"), &p("x*(x^2+1)"), "x").unwrap();
        assert_same_function("1", "x*(x^2+1)", &d);
    }

    #[test]
    fn incomplete_denominators_are_refused() {
        // x^5 + x + 1 is not factored completely by the search.
        assert!(partial_fractions(&p("1"), &p("x^5 + x + 1"), "x").is_err());
    }

    #[test]
    fn division_helpers_agree_with_expansion() {
        // (x - 1)(x + 2) = x^2 + x - 2.
        let q = dense_coefficients(&expand(&p("(x-1)*(x+2)")), "x").unwrap();
        assert_eq!(q.len(), 3);
        assert_eq!(q[0], Number::from_int(-2));
    }

    #[test]
    fn decompose_returns_terms_with_their_powers() {
        let d = decompose(&p("1"), &p("x*(x-1)^2"), "x").unwrap();
        let powers: Vec<usize> = d.terms.iter().map(|t| t.power).collect();
        assert_eq!(powers.iter().sum::<usize>(), 1 + 1 + 2);
    }
}
