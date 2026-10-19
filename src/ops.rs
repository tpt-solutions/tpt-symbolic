//! Operator and conversion impls for [`Expr`], so expressions can be built with
//! ordinary Rust syntax: `x.clone() * x + Expr::int(1)`.
//!
//! Each operator calls the canonical constructor, so the result is in the same
//! form as parsing the equivalent text.

use crate::expr::Expr;
use crate::mathr::Number;
use alloc::vec;
use core::ops::{Add, Div, Mul, Neg, Sub};

impl Add for Expr {
    type Output = Expr;

    fn add(self, rhs: Expr) -> Expr {
        Expr::sum(vec![self, rhs])
    }
}

impl Sub for Expr {
    type Output = Expr;

    fn sub(self, rhs: Expr) -> Expr {
        Expr::difference(self, rhs)
    }
}

impl Mul for Expr {
    type Output = Expr;

    fn mul(self, rhs: Expr) -> Expr {
        Expr::product(vec![self, rhs])
    }
}

impl Div for Expr {
    type Output = Expr;

    /// Canonical quotient. Dividing by the exact zero constant is not checked
    /// here; the parser rejects `1/0`, and [`crate::eval::eval`] reports it.
    fn div(self, rhs: Expr) -> Expr {
        Expr::quotient(self, rhs)
    }
}

impl Neg for Expr {
    type Output = Expr;

    fn neg(self) -> Expr {
        Expr::negate(self)
    }
}

impl From<i64> for Expr {
    fn from(n: i64) -> Expr {
        Expr::int(i128::from(n))
    }
}

impl From<Number> for Expr {
    fn from(n: Number) -> Expr {
        Expr::Const(n)
    }
}

#[cfg(test)]
mod tests {
    use crate::expr::Expr;
    use crate::parse::parse;

    #[test]
    fn operators_match_parsed_text() {
        let x = Expr::var("x");
        let built = x.clone() * x.clone() + Expr::from(1i64) - x.clone() / Expr::from(2i64);
        let parsed = parse("x*x + 1 - x/2").unwrap();
        assert_eq!(built, parsed);
        assert_eq!(-Expr::var("y"), parse("-y").unwrap());
    }
}
