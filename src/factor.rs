//! Factorisation of univariate polynomials with rational coefficients.
//!
//! [`factor`] peels off zero and rational roots, then splits a quartic into two
//! quadratics with integer coefficients when it can. A factor of degree 2 or 3 with no
//! rational root is irreducible over the rationals. A quartic with no rational root and
//! no integer quadratic split is irreducible too. A factor of degree 5 or more that is
//! left over is reported with [`Factorization::complete`] set to `false`, because the
//! search does not decide its irreducibility.

use crate::error::Error;
use crate::expr::Expr;
use crate::limits::monomial_terms;
use crate::mathr::Number;
use crate::solve::{divide_by_linear, divisors, eval_is_zero, find_rational_root};
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

/// One irreducible factor with its multiplicity.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Factor {
    /// The factor as an expression in the variable, monic.
    pub expr: Expr,
    /// How many times it occurs.
    pub multiplicity: u32,
}

/// The factorisation `leading * prod(expr^multiplicity)`.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Factorization {
    /// The leading coefficient of the original polynomial.
    pub leading: Number,
    /// Monic factors, each with its multiplicity.
    pub factors: Vec<Factor>,
    /// `true` when every factor is known to be irreducible over the rationals.
    pub complete: bool,
}

impl Factorization {
    /// The factors as text, such as `2 * (x - 1) * (x^2 + 1)^2`. Unlike [`Factorization::to_expr`],
    /// this keeps each factor separate.
    pub fn describe(&self) -> String {
        let mut parts = Vec::new();
        if !self.leading.is_one() {
            parts.push(alloc::format!("{}", self.leading));
        }
        for f in &self.factors {
            if f.multiplicity == 1 {
                parts.push(alloc::format!("({})", f.expr));
            } else {
                parts.push(alloc::format!("({})^{}", f.expr, f.multiplicity));
            }
        }
        if parts.is_empty() {
            return String::from("1");
        }
        parts.join(" * ")
    }

    /// The product as one expression. The smart constructors expand products of sums, so this
    /// is the expanded polynomial. Use [`Factorization::describe`] to see the factors.
    pub fn to_expr(&self) -> Expr {
        let mut terms = vec![Expr::Const(self.leading.clone())];
        for f in &self.factors {
            terms.push(Expr::power(
                f.expr.clone(),
                Expr::int(i128::from(f.multiplicity)),
            ));
        }
        crate::simplify::simplify(&Expr::product(terms))
    }
}

/// Dense coefficients of `e` in `var`, lowest degree first. `None` if `e` is not a
/// polynomial in `var` with rational coefficients.
pub(crate) fn dense_coefficients(e: &Expr, var: &str) -> Option<Vec<Number>> {
    let terms = monomial_terms(e, var)?;
    let degree = terms.iter().map(|(d, _)| *d as usize).max().unwrap_or(0);
    let mut out = vec![Number::zero(); degree + 1];
    for (d, coeff) in terms {
        let c = coeff.as_number()?;
        out[d as usize] = out[d as usize].checked_add(&c)?;
    }
    Some(out)
}

/// Builds the expression `sum coeffs[i] * var^i` from lowest-first coefficients.
pub(crate) fn poly_expr(coeffs: &[Number], var: &str) -> Expr {
    let terms = coeffs
        .iter()
        .enumerate()
        .filter(|(_, c)| !c.is_zero())
        .map(|(i, c)| {
            Expr::product(vec![
                Expr::Const(c.clone()),
                Expr::power(Expr::var(var), Expr::int(i as i128)),
            ])
        })
        .collect();
    Expr::sum(terms)
}

/// Factors the polynomial `e` in `var`.
///
/// Errors when `e` is not a polynomial in `var` with rational coefficients, or is zero.
/// # Example
/// ```
/// use tpt_symbolic::factor::factor;
/// use tpt_symbolic::parse::parse;
/// let f = factor(&parse("x^2 - 1").unwrap(), "x").unwrap();
/// assert!(f.complete);
/// assert_eq!(f.factors.len(), 2);
/// ```
pub fn factor(e: &Expr, var: &str) -> Result<Factorization, Error> {
    let coeffs = dense_coefficients(e, var).ok_or(Error::Unsupported(
        "factor needs a polynomial with rational coefficients",
    ))?;
    let mut low = coeffs;
    while low.last().is_some_and(Number::is_zero) {
        low.pop();
    }
    if low.is_empty() {
        return Err(Error::Unsupported("cannot factor the zero polynomial"));
    }
    let leading = low.last().cloned().ok_or(Error::Overflow)?;
    // Monic, highest degree first.
    let mut poly: Vec<Number> = low
        .iter()
        .rev()
        .map(|c| c.checked_div(&leading).ok_or(Error::DivisionByZero))
        .collect::<Result<_, _>>()?;

    let mut factors: Vec<(Vec<Number>, u32)> = Vec::new();
    let mut complete = true;
    loop {
        // Zero constant term: a factor of x.
        while poly.len() > 1 && poly.last().is_some_and(Number::is_zero) {
            poly.pop();
            add_factor(&mut factors, vec![Number::one(), Number::zero()]);
        }
        if poly.len() <= 2 {
            break;
        }
        let Some(r) = find_rational_root(&poly) else {
            break;
        };
        let mut multiplicity = 0u32;
        while poly.len() > 2 && eval_is_zero(&poly, &r) == Some(true) {
            poly = divide_by_linear(&poly, &r).ok_or(Error::Overflow)?;
            multiplicity += 1;
        }
        // (x - r)^multiplicity, monic.
        let linear = vec![Number::one(), r.checked_neg().ok_or(Error::Overflow)?];
        for _ in 0..multiplicity {
            add_factor(&mut factors, linear.clone());
        }
    }

    // What is left has no rational root. Degree 1 is the last linear factor.
    if poly.len() == 2 {
        // The remaining monic linear factor is already `x + c`, in the form used above.
        add_factor(&mut factors, poly.clone());
    } else if poly.len() > 1 {
        // With no rational root, degree 2 and 3 are irreducible, and so is a quartic with
        // no integer quadratic split. Each claim needs the divisor search to have been
        // exhaustive, which fails only for coefficients too large to enumerate.
        let exhaustive = search_is_exhaustive(&poly);
        if poly.len() == 5 {
            match split_quartic(&poly) {
                Some(Some((q1, q2))) => {
                    add_factor(&mut factors, q1);
                    add_factor(&mut factors, q2);
                }
                Some(None) => add_factor(&mut factors, poly),
                None => {
                    add_factor(&mut factors, poly);
                    complete = false;
                }
            }
        } else {
            // Degree 5 and up may factor, and the search does not decide that.
            complete = exhaustive && poly.len() <= 4;
            add_factor(&mut factors, poly);
        }
        if !exhaustive {
            complete = false;
        }
    }

    Ok(Factorization {
        leading,
        factors: factors
            .into_iter()
            .map(|(poly, multiplicity)| Factor {
                expr: monic_expr(&poly, var),
                multiplicity,
            })
            .collect(),
        complete,
    })
}

/// Adds one copy of `poly` (highest degree first, monic), merging equal factors.
fn add_factor(factors: &mut Vec<(Vec<Number>, u32)>, poly: Vec<Number>) {
    if let Some(entry) = factors.iter_mut().find(|(p, _)| *p == poly) {
        entry.1 += 1;
    } else {
        factors.push((poly, 1));
    }
}

/// `poly` (highest degree first) as an expression in `var`, with `x` for the variable.
fn monic_expr(poly: &[Number], var: &str) -> Expr {
    let low: Vec<Number> = poly.iter().rev().cloned().collect();
    poly_expr(&low, var)
}

/// `true` when the rational-root and quadratic-split searches can enumerate every
/// candidate for `poly`, so that their negative answers are proofs.
fn search_is_exhaustive(poly: &[Number]) -> bool {
    let Some(ints) = integer_coefficients(poly) else {
        return false;
    };
    let (Some(&lead), Some(&constant)) = (ints.first(), ints.last()) else {
        return false;
    };
    divisors(lead.unsigned_abs()).is_some() && divisors(constant.unsigned_abs()).is_some()
}

/// Splits a monic quartic (highest degree first) into two monic quadratics with rational
/// coefficients, if it has such a factorisation.
///
/// Returns `Some(Some(..))` for a split, `Some(None)` when the quartic is irreducible, and
/// `None` when the divisor search cannot enumerate the candidates.
///
/// Clears denominators to a primitive integer polynomial `L x^4 + ... + C`, then tries
/// every factorisation `(a x^2 + b x + c)(d x^2 + e x + f)` with `a d = L` and `c f = C`.
/// By Gauss's lemma this finds every rational split. The middle coefficients `b` and `e`
/// are solved from the `x^3` and `x` equations, and the `x^2` equation is then checked.
#[allow(clippy::type_complexity)]
fn split_quartic(poly: &[Number]) -> Option<Option<(Vec<Number>, Vec<Number>)>> {
    let ints = integer_coefficients(poly)?; // highest degree first
    let (g4, g3, g2, g1, g0) = (ints[0], ints[1], ints[2], ints[3], ints[4]);
    let lead = g4.checked_abs()?;
    let constant = g0.checked_abs()?;
    let lead_divisors = divisors(lead.unsigned_abs())?;
    let const_divisors = divisors(constant.unsigned_abs())?;
    for &a0 in &lead_divisors {
        let d0 = lead / a0 as i128;
        for &c0 in &const_divisors {
            let f0 = g0 / c0 as i128;
            for sa in [1i128, -1] {
                for sc in [1i128, -1] {
                    let (a, d) = (sa * a0 as i128, d0 * sa);
                    let (c, f) = (sc * c0 as i128, f0 * sc);
                    if a * d != g4 || c * f != g0 {
                        continue;
                    }
                    // Unknowns b, e from x^3 (d*b + a*e = g3) and x (f*b + c*e = g1).
                    let delta = d.checked_mul(c)?.checked_sub(a.checked_mul(f)?)?;
                    if delta == 0 {
                        continue;
                    }
                    let b_num = g3.checked_mul(c)?.checked_sub(a.checked_mul(g1)?)?;
                    let e_num = d.checked_mul(g1)?.checked_sub(f.checked_mul(g3)?)?;
                    if b_num % delta != 0 || e_num % delta != 0 {
                        continue;
                    }
                    let (b, e) = (b_num / delta, e_num / delta);
                    // x^2: b*e + a*f + c*d = g2.
                    let x2 = b
                        .checked_mul(e)?
                        .checked_add(a.checked_mul(f)?)?
                        .checked_add(c.checked_mul(d)?)?;
                    if x2 != g2 {
                        continue;
                    }
                    let q1 = monic_quadratic(a, b, c)?;
                    let q2 = monic_quadratic(d, e, f)?;
                    return Some(Some((q1, q2)));
                }
            }
        }
    }
    Some(None)
}

/// `a x^2 + b x + c` divided by `a`, as a monic polynomial (highest degree first).
fn monic_quadratic(a: i128, b: i128, c: i128) -> Option<Vec<Number>> {
    let a = Number::from_int(a);
    Some(vec![
        Number::one(),
        Number::from_int(b).checked_div(&a)?,
        Number::from_int(c).checked_div(&a)?,
    ])
}

/// A primitive integer polynomial with the same roots as `poly` (highest degree first).
/// `None` if the coefficients do not fit in `i128`.
fn integer_coefficients(poly: &[Number]) -> Option<Vec<i128>> {
    // Lcm of the denominators.
    let mut scale: i128 = 1;
    for c in poly {
        let d = c.denom().to_i128()?;
        let g = int_gcd(scale, d);
        scale = scale.checked_mul(d / g)?;
    }
    let mut ints: Vec<i128> = poly
        .iter()
        .map(|c| {
            let v = c.checked_mul(&Number::from_int(scale))?;
            v.as_i128()
        })
        .collect::<Option<_>>()?;
    let content = ints.iter().fold(0i128, |acc, &v| int_gcd(acc, v));
    if content > 1 {
        for v in &mut ints {
            *v /= content;
        }
    }
    Some(ints)
}

fn int_gcd(a: i128, b: i128) -> i128 {
    let (mut a, mut b) = (a.abs(), b.abs());
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse;
    use crate::simplify::simplify;

    fn p(s: &str) -> Expr {
        parse(s).unwrap_or_else(|e| panic!("parse {s:?}: {e}"))
    }

    fn check_product(src: &str) -> Factorization {
        let f = factor(&p(src), "x").unwrap();
        // The product of the factors is the original polynomial.
        assert_eq!(
            crate::simplify::expand(&f.to_expr()),
            crate::simplify::expand(&simplify(&p(src))),
            "{src}"
        );
        f
    }

    #[test]
    fn rational_roots_and_multiplicities() {
        // x (x - 1)^2 (x + 2): a zero root, a double root and a simple root.
        let f = check_product("x*(x-1)^2*(x+2)");
        assert!(f.complete);
        let mults: Vec<u32> = f.factors.iter().map(|x| x.multiplicity).collect();
        assert_eq!(mults.iter().sum::<u32>(), 4);
        assert!(f.factors.iter().any(|x| x.multiplicity == 2));
    }

    #[test]
    fn quadratic_and_cubic_without_roots_are_irreducible() {
        assert!(check_product("x^2 + 1").complete);
        assert!(check_product("x^3 + x + 1").complete);
    }

    #[test]
    fn quartic_splits_into_integer_quadratics() {
        // (x^2 + x + 1)(x^2 - x + 2) = x^4 + 2x^2 + x + 2.
        let f = check_product("(x^2 + x + 1)*(x^2 - x + 2)");
        assert!(f.complete);
        assert_eq!(f.factors.len(), 2);
    }

    #[test]
    fn irreducible_quartic_is_complete() {
        // x^4 + 1 is irreducible over Q.
        let f = check_product("x^4 + 1");
        assert!(f.complete);
        assert_eq!(f.factors.len(), 1);
    }

    #[test]
    fn rational_coefficients_are_handled() {
        // (x - 1/2)(x + 3) = x^2 + 5/2 x - 3/2.
        check_product("x^2 + 5/2*x - 3/2");
    }

    #[test]
    fn higher_degree_leftover_is_marked_incomplete() {
        // Degree 5 with no rational root: not decided by the search.
        let f = factor(&p("x^5 + x + 1"), "x").unwrap();
        assert!(!f.complete);
    }

    #[test]
    fn non_polynomials_are_rejected() {
        assert!(factor(&p("sin(x)"), "x").is_err());
        assert!(factor(&p("0"), "x").is_err());
    }
}

/// The greatest common divisor of two polynomials in `var`, monic, by the Euclidean
/// algorithm over the rationals. `gcd(0, 0)` is `0`.
/// # Example
/// ```
/// use tpt_symbolic::factor::{factor, gcd};
/// use tpt_symbolic::parse::parse;
/// let f = factor(&parse("x^2 - 1").unwrap(), "x").unwrap();
/// assert!(f.complete);
/// assert_eq!(f.factors.len(), 2);
/// let g = gcd(&parse("x^2 - 1").unwrap(), &parse("x + 1").unwrap(), "x").unwrap();
/// assert_eq!(g, tpt_symbolic::simplify::simplify(&parse("x + 1").unwrap()));
/// ```
pub fn gcd(a: &Expr, b: &Expr, var: &str) -> Result<Expr, Error> {
    let mut x = trim_low(dense_coefficients(a, var).ok_or(Error::Unsupported(
        "gcd needs polynomials with rational coefficients",
    ))?);
    let mut y = trim_low(dense_coefficients(b, var).ok_or(Error::Unsupported(
        "gcd needs polynomials with rational coefficients",
    ))?);
    while !(y.len() == 1 && y[0].is_zero()) {
        let (_, r) = crate::apart::poly_div(&x, &y)?;
        x = y;
        y = trim_low(r);
    }
    if x.len() == 1 && x[0].is_zero() {
        return Ok(Expr::zero());
    }
    let lead = x.last().cloned().ok_or(Error::Overflow)?;
    let monic: Vec<Number> = x
        .iter()
        .map(|c| c.checked_div(&lead).ok_or(Error::DivisionByZero))
        .collect::<Result<_, _>>()?;
    Ok(poly_expr(&monic, var))
}

/// Drops high-degree zero coefficients, keeping at least one entry.
fn trim_low(mut p: Vec<Number>) -> Vec<Number> {
    while p.len() > 1 && p.last().is_some_and(Number::is_zero) {
        p.pop();
    }
    if p.is_empty() {
        p.push(Number::zero());
    }
    p
}

#[cfg(test)]
mod gcd_tests {
    use super::*;
    use crate::parse::parse;

    fn p(s: &str) -> Expr {
        parse(s).unwrap_or_else(|e| panic!("parse {s:?}: {e}"))
    }

    #[test]
    fn gcd_of_polynomials() {
        // gcd(x^2 - 1, x^2 + 2x + 1) = x + 1.
        assert_eq!(
            gcd(&p("x^2-1"), &p("x^2+2*x+1"), "x").unwrap(),
            crate::simplify::simplify(&p("x+1"))
        );
        // gcd(x^3 - x, x^2 - 1) = x^2 - 1, monic.
        assert_eq!(
            gcd(&p("x^3-x"), &p("x^2-1"), "x").unwrap(),
            crate::simplify::simplify(&p("x^2-1"))
        );
        // Coprime polynomials give 1.
        assert_eq!(gcd(&p("x^2+1"), &p("x-2"), "x").unwrap(), Expr::one());
        // A zero argument gives the other one, monic.
        assert_eq!(
            gcd(&p("0"), &p("2*x+4"), "x").unwrap(),
            crate::simplify::simplify(&p("x+2"))
        );
        assert_eq!(gcd(&p("0"), &p("0"), "x").unwrap(), Expr::zero());
    }
}
