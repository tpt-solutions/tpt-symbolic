//! Real part, imaginary part and conjugate of expressions in the imaginary unit `I`.
//!
//! An expression built from `I`, sums, products and integer powers is split into
//! `a + b*I` with `a` and `b` free of `I`. Other forms that contain `I` (a function applied
//! to a complex argument, a non-integer power of one) have no such split here. For those,
//! [`split`] returns `None` and the functions here return `None`.
//!
//! Variables are treated as real. A variable `x` has imaginary part `0`.

use crate::expr::Expr;
use crate::simplify::simplify;
use alloc::vec;
use alloc::vec::Vec;

/// `(real, imaginary)` parts of `e`, both free of `I` and simplified.
pub fn split(e: &Expr) -> Option<(Expr, Expr)> {
    let (re, im) = split_raw(e)?;
    Some((simplify(&re), simplify(&im)))
}

/// Real part of `e`.
pub fn real_part(e: &Expr) -> Option<Expr> {
    split(e).map(|(re, _)| re)
}

/// Imaginary part of `e` (the coefficient of `I`).
pub fn imag_part(e: &Expr) -> Option<Expr> {
    split(e).map(|(_, im)| im)
}

/// Complex conjugate of `e`: `re - im * I`.
pub fn conjugate(e: &Expr) -> Option<Expr> {
    let (re, im) = split(e)?;
    Some(simplify(&Expr::difference(
        re,
        Expr::product(vec![im, Expr::I]),
    )))
}

/// Unsimplified `(real, imaginary)` parts.
fn split_raw(e: &Expr) -> Option<(Expr, Expr)> {
    if !contains_i(e) {
        return Some((e.clone(), Expr::zero()));
    }
    match e {
        Expr::I => Some((Expr::zero(), Expr::one())),
        Expr::Add(terms) => {
            let parts: Option<Vec<(Expr, Expr)>> = terms.iter().map(split_raw).collect();
            let parts = parts?;
            let re = Expr::sum(parts.iter().map(|(r, _)| r.clone()).collect());
            let im = Expr::sum(parts.iter().map(|(_, i)| i.clone()).collect());
            Some((re, im))
        }
        Expr::Mul(factors) => {
            let mut acc = (Expr::one(), Expr::zero());
            for f in factors {
                let (c, d) = split_raw(f)?;
                let (a, b) = acc;
                // (a + b i)(c + d i) = (ac - bd) + (ad + bc) i
                let re = Expr::difference(
                    Expr::product(vec![a.clone(), c.clone()]),
                    Expr::product(vec![b.clone(), d.clone()]),
                );
                let im = Expr::sum(vec![Expr::product(vec![a, d]), Expr::product(vec![b, c])]);
                acc = (re, im);
            }
            Some(acc)
        }
        Expr::Pow(base, exp) => {
            let n = exp.as_number()?;
            if !n.is_integer() {
                return None;
            }
            let k = n.as_i128()?;
            let (a, b) = split_raw(base)?;
            if k >= 0 {
                let mut acc = (Expr::one(), Expr::zero());
                for _ in 0..k {
                    let (c, d) = acc;
                    let re = Expr::difference(
                        Expr::product(vec![c.clone(), a.clone()]),
                        Expr::product(vec![d.clone(), b.clone()]),
                    );
                    let im = Expr::sum(vec![
                        Expr::product(vec![c, b.clone()]),
                        Expr::product(vec![d, a.clone()]),
                    ]);
                    acc = (re, im);
                }
                Some(acc)
            } else if k == -1 {
                // 1/(a + b i) = (a - b i) / (a^2 + b^2)
                let norm = Expr::sum(vec![
                    Expr::power(a.clone(), Expr::int(2)),
                    Expr::power(b.clone(), Expr::int(2)),
                ]);
                let re = Expr::quotient(a, norm.clone());
                let im = Expr::quotient(Expr::negate(b), norm);
                Some((re, im))
            } else {
                None
            }
        }
        _ => None,
    }
}

/// `true` if `I` occurs anywhere in `e`.
fn contains_i(e: &Expr) -> bool {
    match e {
        Expr::I => true,
        Expr::Add(v) | Expr::Mul(v) | Expr::And(v) | Expr::Or(v) => v.iter().any(contains_i),
        Expr::Pow(a, b) | Expr::Eq(a, b) => contains_i(a) || contains_i(b),
        Expr::Func(_, a) | Expr::Not(a) => contains_i(a),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse;

    fn p(s: &str) -> Expr {
        parse(s).unwrap_or_else(|e| panic!("parse {s:?}: {e}"))
    }

    #[test]
    fn parts_of_a_product() {
        // (1 + 2i)(3 - i) = 5 + 5i.
        assert_eq!(real_part(&p("(1+2*i)*(3-i)")), Some(Expr::int(5)));
        assert_eq!(imag_part(&p("(1+2*i)*(3-i)")), Some(Expr::int(5)));
    }

    #[test]
    fn parts_of_a_reciprocal_and_a_power() {
        // 1/(2 + i) = (2 - i)/5.
        assert_eq!(
            real_part(&p("1/(2+i)")),
            Some(Expr::rational(2, 5).unwrap())
        );
        assert_eq!(
            imag_part(&p("1/(2+i)")),
            Some(Expr::rational(-1, 5).unwrap())
        );
        // i^2 = -1 has no imaginary part.
        assert_eq!(imag_part(&p("i^2")), Some(Expr::zero()));
    }

    #[test]
    fn conjugate_negates_the_imaginary_part() {
        assert_eq!(conjugate(&p("3 + 4*i")), Some(simplify(&p("3 - 4*i"))));
        // Real expressions are their own conjugate.
        assert_eq!(conjugate(&p("x^2 + 1")), Some(simplify(&p("x^2 + 1"))));
    }

    #[test]
    fn functions_of_complex_values_are_refused() {
        assert_eq!(real_part(&p("sin(i)")), None);
        assert_eq!(real_part(&p("(1+i)^(1/2)")), None);
    }
}
