//! Deterministic random-expression generator for property and fuzz tests.
//!
//! Seeded, so every failure can be reproduced from its seed. No external
//! crates are used, which keeps this module usable under `no_std`.

use crate::expr::{Expr, SymbolicFunc};
use crate::mathr::Number;
use alloc::vec;

/// xorshift64* pseudo-random generator.
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    /// Creates a generator. A zero seed is replaced by a fixed non-zero one.
    pub fn new(seed: u64) -> Self {
        Rng(if seed == 0 {
            0x9E37_79B9_7F4A_7C15
        } else {
            seed
        })
    }

    /// Next 64 random bits.
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// A value in `0..n`. Returns 0 when `n` is 0.
    pub fn below(&mut self, n: u64) -> u64 {
        if n == 0 {
            0
        } else {
            self.next_u64() % n
        }
    }
}

const FUNCS: [SymbolicFunc; 8] = [
    SymbolicFunc::Sin,
    SymbolicFunc::Cos,
    SymbolicFunc::Tan,
    SymbolicFunc::Exp,
    SymbolicFunc::Log,
    SymbolicFunc::Sinh,
    SymbolicFunc::Cosh,
    SymbolicFunc::Abs,
];

/// A random canonical expression over the variables `x` and `y`.
///
/// `depth` bounds the tree height. Exponents are non-negative integers, so no
/// division by zero can occur.
pub fn random_expr(rng: &mut Rng, depth: u32) -> Expr {
    if depth == 0 || rng.below(3) == 0 {
        return random_leaf(rng);
    }
    let d = depth - 1;
    match rng.below(7) {
        0 => {
            let n = 2 + rng.below(2);
            Expr::sum((0..n).map(|_| random_expr(rng, d)).collect())
        }
        1 => {
            let n = 2 + rng.below(2);
            Expr::product((0..n).map(|_| random_expr(rng, d)).collect())
        }
        2 => {
            let exp = Expr::int(rng.below(4) as i128);
            Expr::power(random_expr(rng, d), exp)
        }
        3 => {
            let f = FUNCS[rng.below(FUNCS.len() as u64) as usize];
            Expr::func(f, random_expr(rng, d))
        }
        4 => Expr::negate(random_expr(rng, d)),
        5 => {
            let p = rng.below(9) as i128 - 4;
            let q = 1 + rng.below(4) as i128;
            Expr::Const(Number::new(p, q).unwrap_or(Number::one()))
        }
        _ => {
            let c = rng.below(7) as i128 - 3;
            Expr::product(vec![Expr::int(c), random_leaf(rng)])
        }
    }
}

const ALL_FUNCS: [SymbolicFunc; 12] = [
    SymbolicFunc::Sin,
    SymbolicFunc::Cos,
    SymbolicFunc::Tan,
    SymbolicFunc::Asin,
    SymbolicFunc::Acos,
    SymbolicFunc::Atan,
    SymbolicFunc::Sinh,
    SymbolicFunc::Cosh,
    SymbolicFunc::Tanh,
    SymbolicFunc::Exp,
    SymbolicFunc::Log,
    SymbolicFunc::Abs,
];

/// A wider random expression than [`random_expr`].
///
/// Uses every built-in function, negative and fractional exponents, a third
/// variable, and larger rational constants. Kept separate so existing seeds keep
/// producing the same expressions.
pub fn random_expr_wide(rng: &mut Rng, depth: u32) -> Expr {
    if depth == 0 || rng.below(3) == 0 {
        return match rng.below(3) {
            0 => Expr::var(["x", "y", "z"][rng.below(3) as usize]),
            1 => Expr::int(rng.below(21) as i128 - 10),
            _ => Expr::Const(
                Number::new(rng.below(41) as i128 - 20, 1 + rng.below(9) as i128)
                    .unwrap_or(Number::one()),
            ),
        };
    }
    let d = depth - 1;
    match rng.below(7) {
        0 => Expr::sum(
            (0..2 + rng.below(3))
                .map(|_| random_expr_wide(rng, d))
                .collect(),
        ),
        1 => Expr::product(
            (0..2 + rng.below(3))
                .map(|_| random_expr_wide(rng, d))
                .collect(),
        ),
        2 => {
            // Exponents from -3 to 3, and sometimes one half.
            let exp = if rng.below(4) == 0 {
                Expr::rational(1, 2).unwrap_or_else(Expr::one)
            } else {
                Expr::int(rng.below(7) as i128 - 3)
            };
            let base = random_expr_wide(rng, d);
            // 0^(negative) is undefined, and the parser rejects it, so keep it out.
            let zero_base = base.as_number().is_some_and(|n| n.is_zero());
            let exp = if zero_base && exp.as_number().is_some_and(|n| n.is_negative()) {
                Expr::int(1)
            } else {
                exp
            };
            Expr::power(base, exp)
        }
        3 => {
            let f = ALL_FUNCS[rng.below(ALL_FUNCS.len() as u64) as usize];
            Expr::func(f, random_expr_wide(rng, d))
        }
        4 => Expr::negate(random_expr_wide(rng, d)),
        5 => Expr::Const(
            Number::new(rng.below(41) as i128 - 20, 1 + rng.below(9) as i128)
                .unwrap_or(Number::one()),
        ),
        _ => {
            let c = rng.below(21) as i128 - 10;
            Expr::product(vec![Expr::int(c), random_expr_wide(rng, d)])
        }
    }
}

fn random_leaf(rng: &mut Rng) -> Expr {
    if rng.below(2) == 0 {
        Expr::var(if rng.below(2) == 0 { "x" } else { "y" })
    } else {
        Expr::int(rng.below(7) as i128 - 3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generator_is_reproducible() {
        let a = random_expr(&mut Rng::new(42), 4);
        let b = random_expr(&mut Rng::new(42), 4);
        assert_eq!(a, b);
    }
}
