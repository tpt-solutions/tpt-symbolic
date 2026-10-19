//! Checkable certificates for results.
//!
//! A [`Certificate`] records a claim together with the data needed to check it. Calling
//! [`Certificate::check`] re-verifies the claim using only exact arithmetic, and does not
//! rely on the code that produced the result. A certificate that fails to check is not
//! evidence of a wrong result, only that the symbolic check could not confirm it.
//!
//! Radical roots are the main case. `check` simplifies `f(root)` and compares it with
//! zero. The simplifier does not reduce every radical identity, so a correct radical root
//! can fail to check. The check then returns `false`, which means "not confirmed".

use crate::calculus::diff;
use crate::eval::subs;
use crate::expr::Expr;
use crate::interval::{excludes_zero, Interval};
use crate::simplify::{expand, simplify};
use alloc::string::String;

/// A claim with the evidence needed to check it.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Certificate {
    /// `expr` vanishes when `var = value`.
    Root {
        /// The function whose root is claimed.
        expr: Expr,
        /// The variable.
        var: String,
        /// The claimed root.
        value: Expr,
    },
    /// `antiderivative` differentiates to `integrand`.
    Antiderivative {
        /// The integrand.
        integrand: Expr,
        /// The variable of integration.
        var: String,
        /// The claimed antiderivative, with the constant omitted.
        antiderivative: Expr,
    },
    /// `product` equals `original` after expansion.
    Factorization {
        /// The polynomial that was factored.
        original: Expr,
        /// The claimed product of its factors.
        product: Expr,
    },
    /// `expr` has no zero on `[lo, hi]`, because its enclosure excludes zero.
    NoRoot {
        /// The function.
        expr: Expr,
        /// The variable.
        var: String,
        /// The interval checked.
        interval: Interval,
    },
}

impl Certificate {
    /// Re-verifies the claim. `true` means confirmed. `false` means not confirmed, which
    /// includes claims the simplifier cannot reduce to a symbolic zero.
    pub fn check(&self) -> bool {
        match self {
            Certificate::Root { expr, var, value } => is_zero(&simplify(&subs(expr, var, value))),
            Certificate::Antiderivative {
                integrand,
                var,
                antiderivative,
            } => match diff(antiderivative, var) {
                Ok(d) => simplify(&d) == simplify(integrand),
                Err(_) => false,
            },
            Certificate::Factorization { original, product } => is_zero(&expand(&simplify(
                &Expr::difference(original.clone(), product.clone()),
            ))),
            Certificate::NoRoot {
                expr,
                var,
                interval,
            } => excludes_zero(expr, var, interval),
        }
    }
}

fn is_zero(e: &Expr) -> bool {
    e.as_number().is_some_and(|n| n.is_zero())
}

/// Certificate that `value` is a root of `expr` in `var`.
pub fn root_certificate(expr: &Expr, var: &str, value: &Expr) -> Certificate {
    Certificate::Root {
        expr: expr.clone(),
        var: String::from(var),
        value: value.clone(),
    }
}

/// Certificate that `antiderivative` is an antiderivative of `integrand`.
pub fn antiderivative_certificate(
    integrand: &Expr,
    var: &str,
    antiderivative: &Expr,
) -> Certificate {
    Certificate::Antiderivative {
        integrand: integrand.clone(),
        var: String::from(var),
        antiderivative: antiderivative.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calculus::{integrate, Integral};
    use crate::mathr::Number;
    use crate::parse::parse;

    fn p(s: &str) -> Expr {
        parse(s).unwrap_or_else(|e| panic!("parse {s:?}: {e}"))
    }

    #[test]
    fn rational_roots_are_confirmed() {
        let c = root_certificate(&p("x^2 - 5*x + 6"), "x", &Expr::int(2));
        assert!(c.check());
        let wrong = root_certificate(&p("x^2 - 5*x + 6"), "x", &Expr::int(4));
        assert!(!wrong.check());
    }

    #[test]
    fn antiderivatives_are_confirmed() {
        let integrand = p("x*cos(x^2)");
        let Integral::Exact(f) = integrate(&integrand, "x") else {
            panic!("expected an antiderivative");
        };
        assert!(antiderivative_certificate(&integrand, "x", &f).check());
        assert!(!antiderivative_certificate(&integrand, "x", &p("x")).check());
    }

    #[test]
    fn factorizations_are_confirmed() {
        let good = Certificate::Factorization {
            original: p("x^2 - 1"),
            product: p("(x - 1)*(x + 1)"),
        };
        assert!(good.check());
        let bad = Certificate::Factorization {
            original: p("x^2 - 1"),
            product: p("(x - 1)^2"),
        };
        assert!(!bad.check());
    }

    #[test]
    fn absence_of_roots_is_confirmed_by_intervals() {
        let c = Certificate::NoRoot {
            expr: p("x^2 + 1"),
            var: String::from("x"),
            interval: Interval::new(Number::from_int(-3), Number::from_int(3)).unwrap(),
        };
        assert!(c.check());
    }
}
