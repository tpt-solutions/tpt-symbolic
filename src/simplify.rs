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

use crate::error::Error;
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
#[non_exhaustive]
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
    Rule {
        name: "fold-constant-function",
        priority: 100,
        category: Category::Constant,
        apply: rule_fold_function,
    },
    Rule {
        name: "reflexive-equation",
        priority: 95,
        category: Category::Constant,
        apply: rule_reflexive_equation,
    },
    Rule {
        name: "parity-of-argument",
        priority: 90,
        category: Category::Trigonometric,
        apply: rule_parity,
    },
    Rule {
        name: "pythagorean-identity",
        priority: 85,
        category: Category::Trigonometric,
        apply: rule_pythagorean,
    },
    Rule {
        name: "even-power-of-abs",
        priority: 82,
        category: Category::Algebraic,
        apply: rule_even_power_of_abs,
    },
    Rule {
        name: "tan-definition",
        priority: 80,
        category: Category::Trigonometric,
        apply: rule_tan_definition,
    },
    Rule {
        name: "tanh-definition",
        priority: 80,
        category: Category::Trigonometric,
        apply: rule_tanh_definition,
    },
    Rule {
        name: "imaginary-unit",
        priority: 98,
        category: Category::Algebraic,
        apply: rule_imaginary_unit,
    },
    Rule {
        name: "special-angle",
        priority: 99,
        category: Category::Trigonometric,
        apply: rule_special_angle,
    },
    Rule {
        name: "special-atan",
        priority: 99,
        category: Category::Trigonometric,
        apply: rule_special_atan,
    },
    Rule {
        name: "inverse-log-exp",
        priority: 80,
        category: Category::Exponential,
        apply: rule_log_exp_inverse,
    },
    Rule {
        name: "merge-exponentials",
        priority: 75,
        category: Category::Exponential,
        apply: rule_exp_merge,
    },
    Rule {
        name: "and-absorption",
        priority: 60,
        category: Category::Logical,
        apply: rule_and_absorb,
    },
    Rule {
        name: "or-absorption",
        priority: 60,
        category: Category::Logical,
        apply: rule_or_absorb,
    },
    Rule {
        name: "de-morgan",
        priority: 50,
        category: Category::Logical,
        apply: rule_de_morgan,
    },
];

/// Simplifies `e` to a fixed point of the rule set (bounded by [`MAX_PASSES`]).
///
/// The result is canonical, and `simplify(&simplify(e)) == simplify(e)`.
///
/// If the limits are hit, the partly simplified result is returned without
/// signalling it. Use [`simplify_checked`] to find out.
/// # Example
/// ```
/// use tpt_symbolic::parse::parse;
/// use tpt_symbolic::simplify::simplify;
/// assert_eq!(simplify(&parse("x*x + x*x").unwrap()).to_string(), "2*x^2");
/// ```
pub fn simplify(e: &Expr) -> Expr {
    simplify_bounded(e).0
}

/// Like [`simplify`], but returns an error instead of a partial result when the
/// fixed point is not reached within [`MAX_PASSES`], or when the tree is deeper
/// than [`MAX_DEPTH`] (so some subtrees could not be rewritten).
pub fn simplify_checked(e: &Expr) -> Result<Expr, Error> {
    match simplify_bounded(e) {
        (out, true) if tree_depth(&out) <= MAX_DEPTH => Ok(out),
        (_, true) => Err(Error::Unsupported(
            "expression is nested deeper than the simplifier limit",
        )),
        (_, false) => Err(Error::Unsupported(
            "simplification did not reach a fixed point",
        )),
    }
}

/// The result, and whether it is a fixed point of the rule set.
fn simplify_bounded(e: &Expr) -> (Expr, bool) {
    simplify_with_passes(e, MAX_PASSES)
}

/// [`simplify_bounded`] with an explicit pass limit, so the limit exit can be tested.
fn simplify_with_passes(e: &Expr, max_passes: usize) -> (Expr, bool) {
    let (out, converged, _) = run_passes(e, max_passes, false);
    (out, converged)
}

/// One rule application: which rule fired, and the node before and after it.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Step {
    /// Name from the [`RULES`] table.
    pub rule: &'static str,
    /// The node the rule matched, after its children were simplified.
    pub before: Expr,
    /// The node the rule produced.
    pub after: Expr,
}

/// Simplifies `e` like [`simplify`], and also returns every rule application in order.
///
/// Only rules from [`RULES`] are recorded. Canonical construction (flattening,
/// constant folding, like-term collection) happens in the smart constructors and is not
/// listed, so the steps explain the rule-based part of the result.
/// # Example
/// ```
/// use tpt_symbolic::parse::parse;
/// use tpt_symbolic::simplify::simplify_trace;
/// use tpt_symbolic::Expr;
/// let (out, steps) = simplify_trace(&parse("sin(x)^2 + cos(x)^2").unwrap());
/// assert_eq!(out, Expr::one());
/// assert_eq!(steps[0].rule, "pythagorean-identity");
/// ```
pub fn simplify_trace(e: &Expr) -> (Expr, Vec<Step>) {
    let (out, _, steps) = run_passes(e, MAX_PASSES, true);
    (out, steps)
}

/// Runs passes until a fixed point or `max_passes`. Returns the result, whether it converged,
/// and the rule applications when `trace` is set.
fn run_passes(e: &Expr, max_passes: usize, trace: bool) -> (Expr, bool, Vec<Step>) {
    let mut cur = e.clone();
    let mut steps = if trace { Some(Vec::new()) } else { None };
    for _ in 0..max_passes {
        let next = pass(&cur, 0, &mut steps);
        if next == cur {
            return (cur, true, steps.unwrap_or_default());
        }
        cur = next;
    }
    (cur, false, steps.unwrap_or_default())
}

/// Number of levels in the tree, counting a leaf as 1.
fn tree_depth(e: &Expr) -> usize {
    match e {
        Expr::Var(_) | Expr::Const(_) | Expr::Bool(_) | Expr::Pi | Expr::E | Expr::I => 1,
        Expr::Add(v) | Expr::Mul(v) | Expr::And(v) | Expr::Or(v) => {
            1 + v.iter().map(tree_depth).max().unwrap_or(0)
        }
        Expr::Pow(a, b) | Expr::Eq(a, b) => 1 + tree_depth(a).max(tree_depth(b)),
        Expr::Func(_, a) | Expr::Not(a) => 1 + tree_depth(a),
    }
}

fn pass(e: &Expr, depth: usize, steps: &mut Option<Vec<Step>>) -> Expr {
    if depth > MAX_DEPTH {
        return e.clone();
    }
    let d = depth + 1;
    let rebuilt = match e {
        Expr::Var(_) | Expr::Const(_) | Expr::Bool(_) | Expr::Pi | Expr::E | Expr::I => {
            return e.clone()
        }
        Expr::Add(v) => Expr::sum(v.iter().map(|t| pass(t, d, steps)).collect()),
        Expr::Mul(v) => Expr::product(v.iter().map(|t| pass(t, d, steps)).collect()),
        Expr::Pow(b, x) => {
            let base = pass(b, d, steps);
            Expr::power(base, pass(x, d, steps))
        }
        Expr::Func(f, a) => Expr::func(*f, pass(a, d, steps)),
        Expr::Eq(a, b) => {
            let lhs = pass(a, d, steps);
            Expr::equation(lhs, pass(b, d, steps))
        }
        Expr::And(v) => Expr::and_all(v.iter().map(|t| pass(t, d, steps)).collect()),
        Expr::Or(v) => Expr::or_all(v.iter().map(|t| pass(t, d, steps)).collect()),
        Expr::Not(a) => Expr::logical_not(pass(a, d, steps)),
    };
    match apply_rules(&rebuilt) {
        Some((rule, out)) => {
            if let Some(log) = steps {
                log.push(Step {
                    rule,
                    before: rebuilt,
                    after: out.clone(),
                });
            }
            out
        }
        None => rebuilt,
    }
}

/// The first rule that changes `e`, with its name.
fn apply_rules(e: &Expr) -> Option<(&'static str, Expr)> {
    for rule in RULES {
        if let Some(out) = (rule.apply)(e) {
            if out != *e {
                return Some((rule.name, out));
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
    if v.first()?.as_number()? != Number::neg_one() {
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
    if let (Some(a), Some(b)) = (
        square_arg(b1, SymbolicFunc::Sin),
        square_arg(b2, SymbolicFunc::Cos),
    ) {
        if a == b && c1 == c2 {
            return Some(Expr::Const(c1.clone()));
        }
    }
    if let (Some(a), Some(b)) = (
        square_arg(b1, SymbolicFunc::Cosh),
        square_arg(b2, SymbolicFunc::Sinh),
    ) {
        if a == b && c2.checked_neg().as_ref() == Some(c1) {
            return Some(Expr::Const(c1.clone()));
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

/// `|a|^(2k)` becomes `a^(2k)`, since the absolute value vanishes from even powers.
fn rule_even_power_of_abs(e: &Expr) -> Option<Expr> {
    let Expr::Pow(base, exp) = e else { return None };
    let n = exp.as_number()?;
    if !n.is_integer() || n.as_i128()? % 2 != 0 {
        return None;
    }
    match &**base {
        Expr::Func(SymbolicFunc::Abs, a) => Some(Expr::power((**a).clone(), (**exp).clone())),
        _ => None,
    }
}

/// `sin(a) * cos(a)^-1` becomes `tan(a)`.
fn rule_tan_definition(e: &Expr) -> Option<Expr> {
    ratio_definition(e, SymbolicFunc::Sin, SymbolicFunc::Cos, SymbolicFunc::Tan)
}

/// `sinh(a) * cosh(a)^-1` becomes `tanh(a)`.
/// Reduces powers and products of the imaginary unit with `I^2 = -1`.
///
/// `I^n` for an integer `n` becomes `1`, `I`, `-1` or `-I` by `n mod 4`. A product with
/// `k` copies of `I` becomes `(-1)^(k/2)` times `I` when `k` is odd, and `(-1)^(k/2)` otherwise.
fn rule_imaginary_unit(e: &Expr) -> Option<Expr> {
    match e {
        Expr::Pow(base, exp) if **base == Expr::I => {
            let n = exp.as_number()?;
            if !n.is_integer() {
                return None;
            }
            let k = n.as_i128()?.rem_euclid(4);
            Some(match k {
                0 => Expr::one(),
                1 => Expr::I,
                2 => Expr::int(-1),
                _ => Expr::negate(Expr::I),
            })
        }
        Expr::Mul(v) => {
            let copies = v.iter().filter(|f| **f == Expr::I).count();
            if copies < 2 {
                return None;
            }
            let mut factors: Vec<Expr> = v.iter().filter(|f| **f != Expr::I).cloned().collect();
            if (copies / 2) % 2 == 1 {
                factors.push(Expr::int(-1));
            }
            if copies % 2 == 1 {
                factors.push(Expr::I);
            }
            Some(Expr::product(factors))
        }
        _ => None,
    }
}

/// `atan` at 0 and at ±1: `0`, `±π/4`.
fn rule_special_atan(e: &Expr) -> Option<Expr> {
    let Expr::Func(SymbolicFunc::Atan, arg) = e else {
        return None;
    };
    let c = arg.as_number()?;
    if c.is_zero() {
        Some(Expr::zero())
    } else if c.is_one() {
        Some(Expr::product(vec![Expr::rational(1, 4)?, Expr::Pi]))
    } else if c == Number::neg_one() {
        Some(Expr::product(vec![Expr::rational(-1, 4)?, Expr::Pi]))
    } else {
        None
    }
}

/// Exact `sin` and `cos` at rational multiples of `pi` with denominator dividing 12.
///
/// Other angles are left alone. `tan` is not included.
fn rule_special_angle(e: &Expr) -> Option<Expr> {
    let Expr::Func(f, arg) = e else { return None };
    if !matches!(f, SymbolicFunc::Sin | SymbolicFunc::Cos) {
        return None;
    }
    let q = pi_multiple(arg)?.checked_mul(&Number::from_int(12))?;
    if !q.is_integer() {
        return None;
    }
    // Index in twelfths of a turn, mod 24. cos(x) = sin(x + pi/2), and pi/2 is 6 twelfths.
    let mut k = q.as_i128()?.rem_euclid(24);
    if *f == SymbolicFunc::Cos {
        k = (k + 6).rem_euclid(24);
    }
    sin_at_twelfths(k)
}

/// The rational `q` with `arg = q*pi`, when `arg` has that form.
fn pi_multiple(arg: &Expr) -> Option<Number> {
    match arg {
        Expr::Pi => Some(Number::one()),
        Expr::Const(c) if c.is_zero() => Some(Number::zero()),
        Expr::Mul(v) => match v.as_slice() {
            [Expr::Const(c), Expr::Pi] => Some(c.clone()),
            _ => None,
        },
        _ => None,
    }
}

/// `sin(k*pi/12)` for `k` in `0..24`, where the value is exact.
fn sin_at_twelfths(k: i128) -> Option<Expr> {
    let half = || Expr::rational(1, 2).unwrap_or_else(Expr::one);
    // sqrt(2)/2 and sqrt(3)/2, as canonical products.
    let root = |n: i128| {
        Expr::product(vec![
            Expr::rational(1, 2).unwrap_or_else(Expr::one),
            Expr::power(Expr::int(n), Expr::rational(1, 2).unwrap_or_else(Expr::one)),
        ])
    };
    Some(match k {
        0 | 12 => Expr::zero(),
        2 | 10 => half(),
        3 | 9 => root(2),
        4 | 8 => root(3),
        6 => Expr::one(),
        14 | 22 => Expr::negate(half()),
        15 | 21 => Expr::negate(root(2)),
        16 | 20 => Expr::negate(root(3)),
        18 => Expr::negate(Expr::one()),
        _ => return None,
    })
}

fn rule_tanh_definition(e: &Expr) -> Option<Expr> {
    ratio_definition(
        e,
        SymbolicFunc::Sinh,
        SymbolicFunc::Cosh,
        SymbolicFunc::Tanh,
    )
}

/// Rewrites `num(a) * den(a)^-1` as `out(a)` inside a product.
fn ratio_definition(
    e: &Expr,
    num: SymbolicFunc,
    den: SymbolicFunc,
    out: SymbolicFunc,
) -> Option<Expr> {
    let Expr::Mul(v) = e else { return None };
    for (i, f) in v.iter().enumerate() {
        let Expr::Func(f_name, a) = f else {
            continue;
        };
        if *f_name != num {
            continue;
        }
        for (j, g) in v.iter().enumerate() {
            if i == j {
                continue;
            }
            let Expr::Pow(base, exp) = g else { continue };
            let is_inv_den = exp.as_number() == Some(Number::neg_one())
                && matches!(&**base, Expr::Func(d, b) if *d == den && b == a);
            if is_inv_den {
                let mut rest: Vec<Expr> = v
                    .iter()
                    .enumerate()
                    .filter(|(k, _)| *k != i && *k != j)
                    .map(|(_, t)| t.clone())
                    .collect();
                rest.push(Expr::func(out, (**a).clone()));
                return Some(Expr::product(rest));
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
            return Some(if is_and {
                Expr::and_all(rest)
            } else {
                Expr::or_all(rest)
            });
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
        Expr::And(v) => Some(Expr::or_all(
            v.iter().cloned().map(Expr::logical_not).collect(),
        )),
        Expr::Or(v) => Some(Expr::and_all(
            v.iter().cloned().map(Expr::logical_not).collect(),
        )),
        _ => None,
    }
}

/// Multiplies out products of sums and expands small integer powers of sums.
///
/// Returns the input unchanged if the expansion would exceed [`MAX_EXPAND_TERMS`].
/// The output is canonical. Call [`simplify`] afterwards if further reduction
/// is wanted.
/// # Example
/// ```
/// use tpt_symbolic::simplify::expand;
/// use tpt_symbolic::parse::parse;
/// assert_eq!(expand(&parse("(x+1)^2").unwrap()).to_string(), "1 + 2*x + x^2");
/// ```
pub fn expand(e: &Expr) -> Expr {
    match e {
        Expr::Var(_) | Expr::Const(_) | Expr::Bool(_) | Expr::Pi | Expr::E | Expr::I => e.clone(),
        Expr::Add(v) => Expr::sum(v.iter().map(expand).collect()),
        Expr::Mul(v) => expand_product(e, v),
        Expr::Pow(b, x) => {
            let base = expand(b);
            if let (Expr::Add(_), Some(n)) = (&base, x.as_number()) {
                if n.is_integer() && n.as_i128().is_some_and(|v| (2..=8).contains(&v)) {
                    let copies: Vec<Expr> = (0..n.as_i128().unwrap_or(0))
                        .map(|_| base.clone())
                        .collect();
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
        // Check the size before allocating, so a huge product can't ask for
        // more memory than the cap allows.
        if acc.len().saturating_mul(terms.len()) > MAX_EXPAND_TERMS {
            return original.clone();
        }
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
    fn trace_gives_the_same_result_and_names_the_rules() {
        let e = p("sin(x)^2 + cos(x)^2");
        let (out, steps) = simplify_trace(&e);
        assert_eq!(out, simplify(&e));
        assert!(steps.iter().any(|st| st.rule == "pythagorean-identity"));
        let e = p("sin(pi/6)");
        let (out, steps) = simplify_trace(&e);
        assert_eq!(out, simplify(&e));
        assert!(steps.iter().any(|st| st.rule == "special-angle"));
        // Every recorded step changes something.
        assert!(steps.iter().all(|st| st.before != st.after));
    }

    #[test]
    fn imaginary_unit_reduces() {
        assert_eq!(simplify(&Expr::power(Expr::I, Expr::int(2))), Expr::int(-1));
        assert_eq!(
            simplify(&Expr::power(Expr::I, Expr::int(3))),
            Expr::negate(Expr::I)
        );
        assert_eq!(
            simplify(&Expr::power(Expr::I, Expr::int(-1))),
            Expr::negate(Expr::I)
        );
        assert_eq!(
            simplify(&Expr::product(vec![Expr::I, Expr::I, Expr::var("x")])),
            simplify(&p("-x"))
        );
        assert_eq!(simplify(&p("i*i")), simplify(&p("-1")));
        assert_eq!(simplify(&p("i^4")), simplify(&p("1")));
    }

    #[test]
    fn exact_values_at_twelfths_of_pi() {
        assert_eq!(simplify(&s("sin(pi/6)")), simplify(&s("1/2")));
        assert_eq!(simplify(&s("cos(pi)")), simplify(&s("-1")));
        assert_eq!(simplify(&s("sin(-pi/2)")), simplify(&s("-1")));
        assert_eq!(simplify(&s("cos(0)")), simplify(&s("1")));
        assert_eq!(simplify(&s("cos(pi/4)^2")), simplify(&s("1/2")));
        // pi/5 is not a multiple of pi/12, so it stays symbolic.
        assert_eq!(simplify(&s("sin(pi/5)")), simplify(&s("sin(pi/5)")));
        assert!(matches!(simplify(&s("sin(pi/5)")), Expr::Func(..)));
    }

    #[test]
    fn hyperbolic_ratio_becomes_tanh() {
        assert_eq!(simplify(&s("sinh(x)*cosh(x)^-1")), simplify(&s("tanh(x)")));
        // The tan rule still fires for the circular functions.
        assert_eq!(simplify(&s("sin(x)*cos(x)^-1")), simplify(&s("tan(x)")));
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
        assert_eq!(
            s(&format!("not (({a}) and ({b}))")),
            s(&format!("not ({a}) or not ({b})"))
        );
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
    fn deeper_random_inputs_are_idempotent_and_expand_terminates() {
        let mut rng = Rng::new(0xC0FFEE);
        for _ in 0..500 {
            let e = random_expr(&mut rng, 5);
            let once = simplify(&e);
            assert_eq!(simplify(&once), once, "not idempotent for {e}");
            let _ = expand(&e);
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

    #[test]
    fn pass_limit_exit_is_reported_as_not_converged() {
        // Zero passes never confirm a fixed point, even for an already simple input.
        assert_eq!(simplify_with_passes(&p("x"), 0), (p("x"), false));
        // One pass on a fixed point confirms it, so the limit is not hit.
        assert_eq!(simplify_with_passes(&p("x"), 1), (p("x"), true));
        assert_eq!(
            simplify_with_passes(&p("x + x"), MAX_PASSES),
            (p("2*x"), true)
        );
    }

    #[test]
    fn simplify_checked_reports_limits() {
        assert_eq!(simplify_checked(&p("x + x")).unwrap(), p("2*x"));
        let mut deep = Expr::var("x");
        for _ in 0..(MAX_DEPTH + 8) {
            deep = Expr::func(SymbolicFunc::Sin, deep);
        }
        assert!(matches!(
            simplify_checked(&deep),
            Err(Error::Unsupported(_))
        ));
    }

    /// Property check on the rule set: simplification must not change the value.
    ///
    /// Compares exact values at rational sample points. A point where either side
    /// has no exact value (functions, division by zero) is skipped, so this checks
    /// the rational part of the rules, not the trigonometric ones.
    #[test]
    fn simplify_preserves_value_at_sample_points() {
        use crate::eval::eval;
        let samples = [(0, 1), (1, 1), (-2, 1), (3, 2), (-5, 4)];
        let mut rng = Rng::new(2026);
        let mut compared = 0usize;
        for _ in 0..1500 {
            let e = random_expr(&mut rng, 4);
            let s = simplify(&e);
            for (xn, xd) in samples {
                for (yn, yd) in samples {
                    let env = [
                        ("x", Number::new(xn, xd).unwrap()),
                        ("y", Number::new(yn, yd).unwrap()),
                    ];
                    let (Ok(before), Ok(after)) = (eval(&e, &env), eval(&s, &env)) else {
                        continue;
                    };
                    assert_eq!(
                        before, after,
                        "simplify changed the value of {e} (simplified to {s})"
                    );
                    compared += 1;
                }
            }
        }
        assert!(compared > 1000, "too few exact comparisons: {compared}");
    }
}
