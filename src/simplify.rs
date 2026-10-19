//! Simplification: a prioritized, bounded rewrite engine.
//!
//! [`simplify`] rebuilds the tree bottom-up through the canonical constructors,
//! then tries the [`RULES`] on each node, highest priority first. It repeats
//! whole passes until nothing changes or [`MAX_PASSES`] is reached.
//!
//! Termination argument. Canonical construction already gives a normal form for
//! sums, products and powers. The rules fall into three groups:
//!
//! * Size-reducing rules (constant folding, Pythagorean identities, absorption,
//!   complement, `log(exp x) = x`, `|| ||` collapse) strictly shrink the tree.
//! * Negation-moving rules (De Morgan, odd/even argument, `exp` merging) push
//!   `not` and `-1` toward the leaves or into a canonical position. The total
//!   depth of `not` nodes, and the count of exponentials, each strictly decrease.
//! * Rules that could ping-pong would have to undo each other. None does, and
//!   the pass limit is the backstop if a future rule breaks this.
//!
//! The pass limit guarantees that the call returns, even on rule sets that do
//! not terminate. Tests check that idempotence holds on random inputs.

use crate::expr::{Expr, SymbolicFunc};
use crate::mathr::Number;
use alloc::vec;
use alloc::vec::Vec;

/// Maximum number of full passes before the driver gives up on a fixed point.
pub const MAX_PASSES: usize = 64;
/// Maximum tree depth the rewriter descends into. Deeper subtrees are kept as-is.
pub const MAX_DEPTH: usize = 256;
/// Maximum number of terms [`expand`] will produce before giving up.
pub const MAX_EXPAND_TERMS: usize = 4096;

/// Which family a rule belongs to. Used for documentation and filtering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    /// Folding of exact constants and trivial identities.
    Constant,
    /// Algebraic identities that canonical construction does not cover.
    Algebraic,
    /// Trigonometric and hyperbolic identities and parity.
    Trigonometric,
    /// Exponential and logarithm identities.
    Exponential,
    /// Boolean logic.
    Logical,
}

/// A named rewrite rule. `apply` returns `None` when the rule does not fire.
#[derive(Clone, Copy)]
pub struct Rule {
    /// Rule name, for diagnostics and documentation.
    pub name: &'static str,
    /// Higher priority rules are tried first on each node.
    pub priority: u8,
    /// Rule family.
    pub category: Category,
    /// Attempts the rewrite on a single node whose children are already simplified.
    pub apply: fn(&Expr) -> Option<Expr>,
}

/// All rules, in descending priority order.
pub const RULES: &[Rule] = &[
    Rule { name: "fold-constant-function", priority: 100, category: Category::Constant, apply: rule_fold_function },
    Rule { name: "reflexive-equation", priority: 95, category: Category::Constant, apply: rule_reflexive_equation },
    Rule { name: "parity-of-argument", priority: 90, category: Category::Trigonometric, apply: rule_parity },
    Rule { name: "pythagorean-identity", priority: 85, category: Category::Trigonometric, apply: rule_pythagorean },
    Rule { name: "inverse-log-exp", priority: 80, category: Category::Exponential, apply: rule_log_exp_inverse },
    Rule { name: "merge-exponentials", priority: 75, category: Category::Exponential, apply: rule_exp_merge },
    Rule { name: "and-absorption", priority: 60, category: Category::Logical, apply: rule_and_absorb },
    Rule { name: "or-absorption", priority: 60, category: Category::Logical, apply: rule_or_absorb },
    Rule { name: "de-morgan", priority: 50, category: Category::Logical, apply: rule_de_morgan },
];

/// Simplifies `e` to a fixed point of the rule set (bounded by [`MAX_PASSES`]).
///
/// The result is canonical, and `simplify(&simplify(e)) == simplify(e)`.
pub fn simplify(e: &Expr) -> Expr {
    let mut cur = e.clone();
    for _ in 0..MAX_PASSES {
        let next = pass(&cur, 0);
        if next == cur {
            break;
        }
        cur = next;
    }
    cur
}

fn pass(e: &Expr, depth: usize) -> Expr {
    if depth > MAX_DEPTH {
        return e.clone();
    }
    let d = depth + 1;
    let rebuilt = match e {
        Expr::Var(_) | Expr::Const(_) | Expr::Bool(_) => return e.clone(),
        Expr::Add(v) => Expr::sum(v.iter().map(|t| pass(t, d)).collect()),
        Expr::Mul(v) => Expr::product(v.iter().map(|t| pass(t, d)).collect()),
        Expr::Pow(b, x) => Expr::power(pass(b, d), pass(x, d)),
        Expr::Func(f, a) => Expr::func(*f, pass(a, d)),
        Expr::Eq(a, b) => Expr::equation(pass(a, d), pass(b, d)),
        Expr::And(v) => Expr::and_all(v.iter().map(|t| pass(t, d)).collect()),
        Expr::Or(v) => Expr::or_all(v.iter().map(|t| pass(t, d)).collect()),
        Expr::Not(a) => Expr::logical_not(pass(a, d)),
    };
    apply_rules(&rebuilt).unwrap_or(rebuilt)
}

fn apply_rules(e: &Expr) -> Option<Expr> {
    for rule in RULES {
        if let Some(out) = (rule.apply)(e) {
            if out != *e {
                return Some(out);
            }
        }
    }
    None
}

fn rule_fold_function(e: &Expr) -> Option<Expr> {
    let Expr::Func(f, a) = e else { return None };
    let n = a.as_number()?;
    match *f {
        SymbolicFunc::Sin
        | SymbolicFunc::Tan
        | SymbolicFunc::Asin
        | SymbolicFunc::Atan
        | SymbolicFunc::Sinh
        | SymbolicFunc::Tanh
            if n.is_zero() =>
        {
            Some(Expr::zero())
        }
        SymbolicFunc::Cos | SymbolicFunc::Cosh | SymbolicFunc::Exp if n.is_zero() => {
            Some(Expr::one())
        }
        SymbolicFunc::Log if n.is_one() => Some(Expr::zero()),
        SymbolicFunc::Abs => {
            let v = if n.is_negative() { n.checked_neg()? } else { n };
            Some(Expr::Const(v))
        }
        _ => None,
    }
}

fn rule_reflexive_equation(e: &Expr) -> Option<Expr> {
    match e {
        Expr::Eq(a, b) if a == b => Some(Expr::boolean(true)),
        _ => None,
    }
}

/// `f(-x)` becomes `-f(x)` for odd `f`, and `f(x)` for even `f`.
fn rule_parity(e: &Expr) -> Option<Expr> {
    let Expr::Func(f, a) = e else { return None };
    let Expr::Mul(v) = &**a else { return None };
    if v.first()?.as_number()? != Number::NEG_ONE {
        return None;
    }
    let x = Expr::product(v[1..].to_vec());
    match f {
        SymbolicFunc::Sin
        | SymbolicFunc::Tan
        | SymbolicFunc::Asin
        | SymbolicFunc::Atan
        | SymbolicFunc::Sinh
        | SymbolicFunc::Tanh => Some(Expr::negate(Expr::func(*f, x))),
        SymbolicFunc::Cos | SymbolicFunc::Cosh | SymbolicFunc::Abs => Some(Expr::func(*f, x)),
        _ => None,
    }
}

/// The argument `a` when `e` is `f(a)^2`.
fn square_arg(e: &Expr, f: SymbolicFunc) -> Option<&Expr> {
    let Expr::Pow(base, exp) = e else { return None };
    if exp.as_number()? != Number::from_int(2) {
        return None;
    }
    match &**base {
        Expr::Func(g, a) if *g == f => Some(a),
        _ => None,
    }
}

/// Replacement for `c*A + d*B` when the pair is a known identity:
/// `sin^2 a + cos^2 a = 1` (coefficients equal), `cosh^2 a - sinh^2 a = 1`.
fn identity_pair(p: &(Number, Expr), q: &(Number, Expr)) -> Option<Expr> {
    let (c1, b1) = p;
    let (c2, b2) = q;
    if let (Some(a), Some(b)) = (square_arg(b1, SymbolicFunc::Sin), square_arg(b2, SymbolicFunc::Cos)) {
        if a == b && c1 == c2 {
            return Some(Expr::Const(*c1));
        }
    }
    if let (Some(a), Some(b)) = (square_arg(b1, SymbolicFunc::Cosh), square_arg(b2, SymbolicFunc::Sinh)) {
        if a == b && c2.checked_neg() == Some(*c1) {
            return Some(Expr::Const(*c1));
        }
    }
    None
}

fn rule_pythagorean(e: &Expr) -> Option<Expr> {
    let Expr::Add(terms) = e else { return None };
    let parts: Vec<(Number, Expr)> = terms.iter().map(|t| t.split_coeff()).collect();
    for i in 0..parts.len() {
        for j in 0..parts.len() {
            if i == j {
                continue;
            }
            if let Some(replacement) = identity_pair(&parts[i], &parts[j]) {
                let mut out: Vec<Expr> = terms
                    .iter()
                    .enumerate()
                    .filter(|(k, _)| *k != i && *k != j)
                    .map(|(_, t)| t.clone())
                    .collect();
                out.push(replacement);
                return Some(Expr::sum(out));
            }
        }
    }
    None
}

fn rule_log_exp_inverse(e: &Expr) -> Option<Expr> {
    match e {
        // log(exp x) = x holds for every real x.
        Expr::Func(SymbolicFunc::Log, a) => match &**a {
            Expr::Func(SymbolicFunc::Exp, x) => Some((**x).clone()),
            _ => None,
        },
        // |abs(x)| = abs(x)
        Expr::Func(SymbolicFunc::Abs, a) => match &**a {
            Expr::Func(SymbolicFunc::Abs, _) => Some((**a).clone()),
            _ => None,
        },
        _ => None,
    }
}

/// `exp(a) * exp(b)` becomes `exp(a + b)`, and `exp(a)^r` becomes `exp(r*a)`.
fn rule_exp_merge(e: &Expr) -> Option<Expr> {
    match e {
        Expr::Mul(v) => {
            let mut args: Vec<Expr> = Vec::new();
            let mut others: Vec<Expr> = Vec::new();
            for f in v {
                match f {
                    Expr::Func(SymbolicFunc::Exp, a) => args.push((**a).clone()),
                    other => others.push(other.clone()),
                }
            }
            if args.len() < 2 {
                return None;
            }
            others.push(Expr::func(SymbolicFunc::Exp, Expr::sum(args)));
            Some(Expr::product(others))
        }
        Expr::Pow(base, r) => match &**base {
            Expr::Func(SymbolicFunc::Exp, a) => Some(Expr::func(
                SymbolicFunc::Exp,
                Expr::product(vec![(**a).clone(), (**r).clone()]),
            )),
            _ => None,
        },
        _ => None,
    }
}

/// Drops a conjunct or disjunct that is absorbed by a sibling, and detects complements.
/// `A and (A or B)` becomes `A`. `A or (A and B)` becomes `A`.
/// `A and not A` becomes `false`. `A or not A` becomes `true`.
fn junction_rule(e: &Expr, is_and: bool) -> Option<Expr> {
    let items = match e {
        Expr::And(v) if is_and => v,
        Expr::Or(v) if !is_and => v,
        _ => return None,
    };
    for it in items {
        if let Expr::Not(inner) = it {
            if items.contains(&**inner) {
                return Some(Expr::boolean(!is_and));
            }
        }
    }
    for (i, it) in items.iter().enumerate() {
        let inner = match (it, is_and) {
            (Expr::Or(v), true) | (Expr::And(v), false) => v,
            _ => continue,
        };
        let absorbed = items
            .iter()
            .enumerate()
            .any(|(j, other)| j != i && inner.contains(other));
        if absorbed {
            let rest: Vec<Expr> = items
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .map(|(_, x)| x.clone())
                .collect();
            return Some(if is_and { Expr::and_all(rest) } else { Expr::or_all(rest) });
        }
    }
    None
}

fn rule_and_absorb(e: &Expr) -> Option<Expr> {
    junction_rule(e, true)
}

fn rule_or_absorb(e: &Expr) -> Option<Expr> {
    junction_rule(e, false)
}

/// `not (A and B)` becomes `not A or not B`, and dually for `or`.
fn rule_de_morgan(e: &Expr) -> Option<Expr> {
    let Expr::Not(inner) = e else { return None };
    match &**inner {
        Expr::And(v) => Some(Expr::or_all(v.iter().cloned().map(Expr::logical_not).collect())),
        Expr::Or(v) => Some(Expr::and_all(v.iter().cloned().map(Expr::logical_not).collect())),
        _ => None,
    }
}

/// Multiplies out products of sums and expands small integer powers of sums.
///
/// Returns the input unchanged if the expansion would exceed [`MAX_EXPAND_TERMS`].
/// The output is canonical. Call [`simplify`] afterwards if further reduction
/// is wanted.
pub fn expand(e: &Expr) -> Expr {
    match e {
        Expr::Var(_) | Expr::Const(_) | Expr::Bool(_) => e.clone(),
        Expr::Add(v) => Expr::sum(v.iter().map(expand).collect()),
        Expr::Mul(v) => expand_product(e, v),
        Expr::Pow(b, x) => {
            let base = expand(b);
            if let (Expr::Add(_), Some(n)) = (&base, x.as_number()) {
                if n.is_integer() && (2..=8).contains(&n.numer()) {
                    let copies: Vec<Expr> = (0..n.numer()).map(|_| base.clone()).collect();
                    return expand_product(e, &copies);
                }
            }
            Expr::power(base, expand(x))
        }
        Expr::Func(f, a) => Expr::func(*f, expand(a)),
        Expr::Eq(a, b) => Expr::equation(expand(a), expand(b)),
        Expr::And(v) => Expr::and_all(v.iter().map(expand).collect()),
        Expr::Or(v) => Expr::or_all(v.iter().map(expand).collect()),
        Expr::Not(a) => Expr::logical_not(expand(a)),
    }
}

fn expand_product(original: &Expr, factors: &[Expr]) -> Expr {
    let mut acc: Vec<Expr> = alloc::vec![Expr::one()];
    for f in factors {
        let terms: Vec<Expr> = match expand(f) {
            Expr::Add(ts) => ts,
            other => alloc::vec![other],
        };
        let mut next = Vec::with_capacity(acc.len() * terms.len());
        for a in &acc {
            for t in &terms {
                next.push(Expr::product(alloc::vec![a.clone(), t.clone()]));
            }
        }
        if next.len() > MAX_EXPAND_TERMS {
            return original.clone();
        }
        acc = next;
    }
    Expr::sum(acc)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse;
    use crate::testkit::{random_expr, Rng};
    use alloc::format;

    fn p(s: &str) -> Expr {
        parse(s).unwrap_or_else(|e| panic!("parse {s:?}: {e}"))
    }

    fn s(src: &str) -> Expr {
        simplify(&p(src))
    }

    #[test]
    fn trig_pythagorean_identity() {
        assert_eq!(s("sin(x)^2 + cos(x)^2"), Expr::one());
        assert_eq!(s("3*sin(x)^2 + 3*cos(x)^2 + y"), p("y + 3"));
        assert_eq!(s("cosh(x)^2 - sinh(x)^2"), Expr::one());
    }

    #[test]
    fn parity_and_constant_functions() {
        assert_eq!(s("sin(-x)"), s("-sin(x)"));
        assert_eq!(s("cos(-x)"), s("cos(x)"));
        assert_eq!(s("sin(0)"), Expr::zero());
        assert_eq!(s("cos(0)"), Expr::one());
        assert_eq!(s("log(1)"), Expr::zero());
        assert_eq!(s("abs(-3)"), Expr::int(3));
    }

    #[test]
    fn exponentials_merge_and_invert() {
        assert_eq!(s("exp(x)*exp(y)"), s("exp(x + y)"));
        assert_eq!(s("exp(x)^2"), s("exp(2*x)"));
        assert_eq!(s("log(exp(x))"), p("x"));
    }

    #[test]
    fn boolean_absorption_complement_and_de_morgan() {
        let a = "x = 1";
        let b = "y = 2";
        assert_eq!(s(&format!("({a}) and (({a}) or ({b}))")), s(a));
        assert_eq!(s(&format!("({a}) or (({a}) and ({b}))")), s(a));
        assert_eq!(s(&format!("({a}) and not ({a})")), Expr::boolean(false));
        assert_eq!(s(&format!("({a}) or not ({a})")), Expr::boolean(true));
        assert_eq!(s(&format!("not (({a}) and ({b}))")), s(&format!("not ({a}) or not ({b})")));
    }

    #[test]
    fn expand_binomial_square() {
        let e = expand(&p("(x + y)^2"));
        assert_eq!(e, p("x^2 + 2*x*y + y^2"));
        let cube = expand(&p("(x + 1)^3"));
        assert_eq!(cube, p("x^3 + 3*x^2 + 3*x + 1"));
    }

    #[test]
    fn simplify_is_idempotent_on_random_inputs() {
        let mut rng = Rng::new(0x5EED);
        for _ in 0..300 {
            let e = random_expr(&mut rng, 4);
            let once = simplify(&e);
            assert_eq!(simplify(&once), once, "not idempotent for {e}");
        }
    }

    #[test]
    fn terminates_on_nested_chaos() {
        let e = p("sin(cos(tan(x^100)))");
        let _ = simplify(&e);
        let mut deep = p("x");
        for _ in 0..200 {
            deep = Expr::func(SymbolicFunc::Sin, deep);
        }
        let _ = simplify(&deep);
    }
}
