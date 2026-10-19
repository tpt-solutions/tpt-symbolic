//! Assumptions about variables, and simplification that uses them.
//!
//! The default [`crate::simplify::simplify`] makes no sign assumptions. It does combine
//! `x * x^-1` into `1`, which holds only for `x != 0`. That combination is the one place
//! the default simplifier assumes something, and it is documented in [`crate::simplify`].
//!
//! [`simplify_assuming`] adds rewrites that need a declared fact about a variable:
//!
//! * `abs(x)` becomes `x` when `x` is positive.
//! * `log(abs(x))` becomes `log(x)` when `x` is positive.
//! * `sqrt(x^n)` becomes `x^(n/2)` when `x` is positive.
//!
//! Facts are not checked. Declaring `x` positive when it is not gives a wrong answer.

use crate::expr::{Expr, SymbolicFunc};
use crate::mathr::Number;
use crate::simplify::simplify;
use alloc::string::String;
use alloc::vec::Vec;

/// Facts declared about variables, by name.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Assumptions {
    positive: Vec<String>,
}

impl Assumptions {
    /// No assumptions.
    pub fn new() -> Assumptions {
        Assumptions::default()
    }

    /// Declares `name` positive (greater than zero). The fact is not checked.
    #[must_use]
    pub fn positive(mut self, name: &str) -> Assumptions {
        if !self.is_positive(name) {
            self.positive.push(String::from(name));
        }
        self
    }

    /// `true` if `name` was declared positive.
    pub fn is_positive(&self, name: &str) -> bool {
        self.positive.iter().any(|p| p == name)
    }
}

/// Simplifies `e` with the rewrites that `assumptions` enable, repeated to a fixed point.
pub fn simplify_assuming(e: &Expr, assumptions: &Assumptions) -> Expr {
    let mut cur = simplify(e);
    for _ in 0..crate::simplify::MAX_PASSES {
        let next = simplify(&rewrite(&cur, assumptions));
        if next == cur {
            break;
        }
        cur = next;
    }
    cur
}

/// One bottom-up pass of the assumption rewrites.
fn rewrite(e: &Expr, a: &Assumptions) -> Expr {
    let rebuilt = match e {
        Expr::Add(v) => Expr::sum(v.iter().map(|t| rewrite(t, a)).collect()),
        Expr::Mul(v) => Expr::product(v.iter().map(|t| rewrite(t, a)).collect()),
        Expr::Pow(b, x) => Expr::power(rewrite(b, a), rewrite(x, a)),
        Expr::Func(f, arg) => Expr::func(*f, rewrite(arg, a)),
        Expr::Eq(l, r) => Expr::equation(rewrite(l, a), rewrite(r, a)),
        Expr::And(v) => Expr::and_all(v.iter().map(|t| rewrite(t, a)).collect()),
        Expr::Or(v) => Expr::or_all(v.iter().map(|t| rewrite(t, a)).collect()),
        Expr::Not(x) => Expr::logical_not(rewrite(x, a)),
        other => return other.clone(),
    };
    rule(&rebuilt, a).unwrap_or(rebuilt)
}

/// The assumption rules that apply at one node.
fn rule(e: &Expr, a: &Assumptions) -> Option<Expr> {
    match e {
        // abs(x) = x for positive x.
        Expr::Func(SymbolicFunc::Abs, arg) => match &**arg {
            Expr::Var(v) if a.is_positive(v) => Some((**arg).clone()),
            _ => None,
        },
        // log(abs(x)) = log(x) for positive x.
        Expr::Func(SymbolicFunc::Log, arg) => match &**arg {
            Expr::Func(SymbolicFunc::Abs, inner) => match &**inner {
                Expr::Var(v) if a.is_positive(v) => {
                    Some(Expr::func(SymbolicFunc::Log, (**inner).clone()))
                }
                _ => None,
            },
            _ => None,
        },
        // sqrt(x^n) = x^(n/2) for positive x.
        Expr::Pow(base, half) if half.as_number() == Some(half_number()) => match &**base {
            Expr::Pow(x, n) => match (&**x, n.as_number()) {
                (Expr::Var(v), Some(n)) if a.is_positive(v) => {
                    let new_exp = n.checked_mul(&half_number())?;
                    Some(Expr::power(Expr::Var(v.clone()), Expr::Const(new_exp)))
                }
                _ => None,
            },
            _ => None,
        },
        _ => None,
    }
}

fn half_number() -> Number {
    Number::new(1, 2).unwrap_or_else(Number::one)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse;

    fn p(s: &str) -> Expr {
        parse(s).unwrap_or_else(|e| panic!("parse {s:?}: {e}"))
    }

    #[test]
    fn abs_and_log_need_positivity() {
        let pos = Assumptions::new().positive("x");
        assert_eq!(simplify_assuming(&p("abs(x)"), &pos), simplify(&p("x")));
        assert_eq!(
            simplify_assuming(&p("log(abs(x))"), &pos),
            simplify(&p("log(x)"))
        );
        // Without the assumption, abs stays.
        assert_eq!(
            simplify_assuming(&p("abs(x)"), &Assumptions::new()),
            simplify(&p("abs(x)"))
        );
    }

    #[test]
    fn square_root_of_a_square_for_positive_variables() {
        let pos = Assumptions::new().positive("x");
        assert_eq!(simplify_assuming(&p("sqrt(x^2)"), &pos), simplify(&p("x")));
        // Not valid for negative x, so unchanged without the assumption.
        assert_ne!(
            simplify_assuming(&p("sqrt(x^2)"), &Assumptions::new()),
            simplify(&p("x"))
        );
    }

    #[test]
    fn other_variables_are_not_affected() {
        let pos = Assumptions::new().positive("y");
        assert_eq!(
            simplify_assuming(&p("abs(x)"), &pos),
            simplify(&p("abs(x)"))
        );
    }
}
