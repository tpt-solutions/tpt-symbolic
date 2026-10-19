//! Expression AST and canonical constructors.
//!
//! Every node is built through the constructors on [`Expr`] (`sum`,
//! `product`, `power`, ...). They flatten nested sums and products, fold
//! exact constants, collect like terms and factors, and sort operands. Two
//! expressions that are equal by these rules therefore compare equal with `==`.

use crate::mathr::Number;
use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::fmt;

/// Built-in elementary functions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SymbolicFunc {
    /// Sine.
    Sin,
    /// Cosine.
    Cos,
    /// Tangent.
    Tan,
    /// Inverse sine.
    Asin,
    /// Inverse cosine.
    Acos,
    /// Inverse tangent.
    Atan,
    /// Hyperbolic sine.
    Sinh,
    /// Hyperbolic cosine.
    Cosh,
    /// Hyperbolic tangent.
    Tanh,
    /// Natural exponential.
    Exp,
    /// Natural logarithm (`ln` is accepted as an alias by the parser).
    Log,
    /// Absolute value.
    Abs,
}

impl SymbolicFunc {
    /// Canonical name used by the printer and parser.
    pub fn name(self) -> &'static str {
        match self {
            SymbolicFunc::Sin => "sin",
            SymbolicFunc::Cos => "cos",
            SymbolicFunc::Tan => "tan",
            SymbolicFunc::Asin => "asin",
            SymbolicFunc::Acos => "acos",
            SymbolicFunc::Atan => "atan",
            SymbolicFunc::Sinh => "sinh",
            SymbolicFunc::Cosh => "cosh",
            SymbolicFunc::Tanh => "tanh",
            SymbolicFunc::Exp => "exp",
            SymbolicFunc::Log => "log",
            SymbolicFunc::Abs => "abs",
        }
    }

    /// Looks up a function by name. Accepts `ln` for `log`.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "sin" => SymbolicFunc::Sin,
            "cos" => SymbolicFunc::Cos,
            "tan" => SymbolicFunc::Tan,
            "asin" => SymbolicFunc::Asin,
            "acos" => SymbolicFunc::Acos,
            "atan" => SymbolicFunc::Atan,
            "sinh" => SymbolicFunc::Sinh,
            "cosh" => SymbolicFunc::Cosh,
            "tanh" => SymbolicFunc::Tanh,
            "exp" => SymbolicFunc::Exp,
            "log" | "ln" => SymbolicFunc::Log,
            "abs" => SymbolicFunc::Abs,
            _ => return None,
        })
    }
}

/// A symbolic expression. Build values with the canonical constructors
/// rather than the variants directly.
///
/// Operand order inside `Add`, `Mul`, `And` and `Or` is the derived `Ord`
/// order. `Mul` keeps its exact constant, if any, as the first operand.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Expr {
    /// A named variable.
    Var(Arc<str>),
    /// An exact constant.
    Const(Number),
    /// A sum of two or more terms. Never contains a nested `Add`.
    Add(Vec<Expr>),
    /// A product of two or more factors. Never contains a nested `Mul`.
    Mul(Vec<Expr>),
    /// `base ^ exponent`.
    Pow(Box<Expr>, Box<Expr>),
    /// An elementary function applied to one argument.
    Func(SymbolicFunc, Box<Expr>),
    /// Equality between two expressions. Operands are stored in sorted order.
    Eq(Box<Expr>, Box<Expr>),
    /// A boolean constant.
    Bool(bool),
    /// Conjunction of two or more operands.
    And(Vec<Expr>),
    /// Disjunction of two or more operands.
    Or(Vec<Expr>),
    /// Logical negation.
    Not(Box<Expr>),
}

impl Expr {
    /// A variable with the given name.
    pub fn var(name: &str) -> Expr {
        Expr::Var(Arc::from(name))
    }

    /// The exact integer `n`.
    pub fn int(n: i128) -> Expr {
        Expr::Const(Number::from_int(n))
    }

    /// The exact rational `num / den`, or `None` if `den` is zero.
    pub fn rational(num: i128, den: i128) -> Option<Expr> {
        Number::new(num, den).map(Expr::Const)
    }

    /// The exact value `0`.
    pub fn zero() -> Expr {
        Expr::Const(Number::ZERO)
    }

    /// The exact value `1`.
    pub fn one() -> Expr {
        Expr::Const(Number::ONE)
    }

    /// The exact value of this node, if it is a constant.
    pub fn as_number(&self) -> Option<Number> {
        match self {
            Expr::Const(n) => Some(*n),
            _ => None,
        }
    }

    /// `true` if `name` occurs anywhere in this expression.
    pub fn contains_var(&self, name: &str) -> bool {
        match self {
            Expr::Var(v) => &**v == name,
            Expr::Const(_) | Expr::Bool(_) => false,
            Expr::Add(v) | Expr::Mul(v) | Expr::And(v) | Expr::Or(v) => {
                v.iter().any(|e| e.contains_var(name))
            }
            Expr::Pow(a, b) | Expr::Eq(a, b) => a.contains_var(name) || b.contains_var(name),
            Expr::Func(_, a) | Expr::Not(a) => a.contains_var(name),
        }
    }

    /// Canonical sum of `terms`.
    ///
    /// Flattens nested sums, folds constants, combines like terms (`x + x`
    /// becomes `2*x`), drops zero terms, and sorts the result.
    pub fn sum(terms: Vec<Expr>) -> Expr {
        let mut constant = Number::ZERO;
        let mut flat: Vec<Expr> = Vec::new();
        for t in terms {
            match t {
                Expr::Add(inner) => {
                    for x in inner {
                        absorb_term(x, &mut constant, &mut flat);
                    }
                }
                other => absorb_term(other, &mut constant, &mut flat),
            }
        }

        // Like terms: group by the non-numeric part, add the numeric coefficients.
        let mut groups: Vec<(Expr, Number)> = Vec::new();
        let mut out: Vec<Expr> = Vec::new();
        for t in flat {
            let (coef, base) = t.split_coeff();
            match groups.iter().position(|(b, _)| *b == base) {
                Some(i) => match groups[i].1.checked_add(coef) {
                    Some(sum) => groups[i].1 = sum,
                    // Coefficient overflow: keep the term unmerged rather than round.
                    None => out.push(t),
                },
                None => groups.push((base, coef)),
            }
        }
        for (base, coef) in groups {
            if !coef.is_zero() {
                out.push(if coef.is_one() {
                    base
                } else {
                    Expr::product(vec![Expr::Const(coef), base])
                });
            }
        }
        if !constant.is_zero() {
            out.push(Expr::Const(constant));
        }

        out.sort();
        match out.len() {
            0 => Expr::zero(),
            1 => out.pop().unwrap_or_else(Expr::zero),
            _ => Expr::Add(out),
        }
    }

    /// Canonical product of `factors`.
    ///
    /// Flattens nested products, folds constants (a zero constant gives `0`),
    /// merges equal bases by adding exponents (`x*x^2` becomes `x^3`), and
    /// distributes a numeric coefficient over a single sum.
    pub fn product(factors: Vec<Expr>) -> Expr {
        let mut flat: Vec<Expr> = Vec::new();
        for f in factors {
            match f {
                Expr::Mul(inner) => flat.extend(inner),
                other => flat.push(other),
            }
        }

        let mut coef = Number::ONE;
        let mut rest: Vec<Expr> = Vec::new();
        for f in flat {
            absorb_factor(f, &mut coef, &mut rest);
        }
        if coef.is_zero() {
            return Expr::zero();
        }

        let mut groups: Vec<(Expr, Vec<Expr>)> = Vec::new();
        for f in rest {
            let (base, exp) = match f {
                Expr::Pow(b, e) => (*b, *e),
                other => (other, Expr::one()),
            };
            match groups.iter().position(|(b, _)| *b == base) {
                Some(i) => groups[i].1.push(exp),
                None => groups.push((base, vec![exp])),
            }
        }

        let mut out: Vec<Expr> = Vec::new();
        for (base, exps) in groups {
            let built = Expr::power(base, Expr::sum(exps));
            match built {
                Expr::Mul(inner) => {
                    for x in inner {
                        absorb_factor(x, &mut coef, &mut out);
                    }
                }
                other => absorb_factor(other, &mut coef, &mut out),
            }
        }
        if coef.is_zero() {
            return Expr::zero();
        }

        if out.len() == 1 && !coef.is_one() {
            if let Expr::Add(terms) = &out[0] {
                let terms = terms.clone();
                return Expr::sum(
                    terms
                        .into_iter()
                        .map(|t| Expr::product(vec![Expr::Const(coef), t]))
                        .collect(),
                );
            }
        }

        out.sort();
        if out.is_empty() {
            return Expr::Const(coef);
        }
        if coef.is_one() && out.len() == 1 {
            return out.pop().unwrap_or_else(Expr::one);
        }
        let mut v = Vec::with_capacity(out.len() + 1);
        if !coef.is_one() {
            v.push(Expr::Const(coef));
        }
        v.extend(out);
        Expr::Mul(v)
    }

    /// Canonical power `base ^ exp`.
    ///
    /// `x^0` is `1` (including `0^0`, by convention). `x^1` is `x`.
    /// Exact constants fold where the result is exact: integer powers of
    /// rationals, and square roots (`sqrt(12)` becomes `2*3^(1/2)`). Integer
    /// exponents distribute over products and collapse nested powers.
    pub fn power(base: Expr, exp: Expr) -> Expr {
        if let Some(n) = exp.as_number() {
            if n.is_zero() {
                return Expr::one();
            }
            if n.is_one() {
                return base;
            }
            if n.is_integer() {
                if let Ok(k) = i64::try_from(n.numer()) {
                    match &base {
                        Expr::Const(b) => {
                            if let Some(p) = b.checked_pow(k) {
                                return Expr::Const(p);
                            }
                        }
                        Expr::Mul(v) => {
                            let parts = v.iter().map(|f| Expr::power(f.clone(), exp.clone()));
                            return Expr::product(parts.collect());
                        }
                        Expr::Pow(b2, e2) => {
                            let e = Expr::product(vec![(**e2).clone(), exp.clone()]);
                            return Expr::power((**b2).clone(), e);
                        }
                        _ => {}
                    }
                }
            }
            if n == Number::new(1, 2).unwrap_or(Number::ONE) {
                if let Expr::Const(b) = &base {
                    if let Some(root) = sqrt_const(*b) {
                        return root;
                    }
                }
            }
        }
        Expr::Pow(Box::new(base), Box::new(exp))
    }

    /// Canonical negation, expressed as `-1 * e`.
    pub fn negate(e: Expr) -> Expr {
        Expr::product(vec![Expr::Const(Number::NEG_ONE), e])
    }

    /// Canonical difference `a - b`.
    pub fn difference(a: Expr, b: Expr) -> Expr {
        Expr::sum(vec![a, Expr::negate(b)])
    }

    /// Canonical quotient `a / b`, expressed as `a * b^(-1)`.
    pub fn quotient(a: Expr, b: Expr) -> Expr {
        Expr::product(vec![a, Expr::power(b, Expr::int(-1))])
    }

    /// An elementary function applied to `arg`.
    pub fn func(f: SymbolicFunc, arg: Expr) -> Expr {
        Expr::Func(f, Box::new(arg))
    }

    /// Equation `a = b`. Operands are sorted. Two constants fold to a boolean.
    pub fn equation(a: Expr, b: Expr) -> Expr {
        if let (Some(x), Some(y)) = (a.as_number(), b.as_number()) {
            return Expr::Bool(x == y);
        }
        let (l, r) = if a <= b { (a, b) } else { (b, a) };
        Expr::Eq(Box::new(l), Box::new(r))
    }

    /// Boolean constant.
    pub fn boolean(b: bool) -> Expr {
        Expr::Bool(b)
    }

    /// Canonical conjunction. Flattens, drops `true`, short-circuits on
    /// `false`, removes duplicates, and sorts.
    pub fn and_all(items: Vec<Expr>) -> Expr {
        junction(items, true)
    }

    /// Canonical disjunction. Flattens, drops `false`, short-circuits on
    /// `true`, removes duplicates, and sorts.
    pub fn or_all(items: Vec<Expr>) -> Expr {
        junction(items, false)
    }

    /// Canonical negation of a boolean expression. Removes double negation.
    pub fn logical_not(e: Expr) -> Expr {
        match e {
            Expr::Bool(b) => Expr::Bool(!b),
            Expr::Not(inner) => *inner,
            other => Expr::Not(Box::new(other)),
        }
    }

    /// Returns this expression with every occurrence of `target` replaced by
    /// `repl`. The result is rebuilt through the constructors, so it is canonical.
    pub fn replace(&self, target: &Expr, repl: &Expr) -> Expr {
        if self == target {
            return repl.clone();
        }
        match self {
            Expr::Var(_) | Expr::Const(_) | Expr::Bool(_) => self.clone(),
            Expr::Add(v) => Expr::sum(v.iter().map(|e| e.replace(target, repl)).collect()),
            Expr::Mul(v) => Expr::product(v.iter().map(|e| e.replace(target, repl)).collect()),
            Expr::Pow(b, e) => Expr::power(b.replace(target, repl), e.replace(target, repl)),
            Expr::Func(f, a) => Expr::func(*f, a.replace(target, repl)),
            Expr::Eq(a, b) => Expr::equation(a.replace(target, repl), b.replace(target, repl)),
            Expr::And(v) => Expr::and_all(v.iter().map(|e| e.replace(target, repl)).collect()),
            Expr::Or(v) => Expr::or_all(v.iter().map(|e| e.replace(target, repl)).collect()),
            Expr::Not(a) => Expr::logical_not(a.replace(target, repl)),
        }
    }

    /// Splits a term into its numeric coefficient and the remaining factor.
    /// `2*x*y` gives `(2, x*y)` and `x` gives `(1, x)`. A bare constant
    /// `c` gives `(1, c)`, so that overflowed constants still group correctly.
    pub(crate) fn split_coeff(&self) -> (Number, Expr) {
        match self {
            Expr::Mul(v) if v.len() >= 2 => {
                if let Expr::Const(c) = &v[0] {
                    let rest = v[1..].to_vec();
                    let base = if rest.len() == 1 {
                        rest.into_iter().next().unwrap_or_else(Expr::one)
                    } else {
                        Expr::Mul(rest)
                    };
                    return (*c, base);
                }
                (Number::ONE, self.clone())
            }
            Expr::Const(c) => (Number::ONE, Expr::Const(*c)),
            _ => (Number::ONE, self.clone()),
        }
    }
}

/// Adds a term to the running constant or to the list of non-constant terms.
fn absorb_term(t: Expr, constant: &mut Number, flat: &mut Vec<Expr>) {
    if let Expr::Const(n) = &t {
        match constant.checked_add(*n) {
            Some(c) => *constant = c,
            None => flat.push(t),
        }
    } else {
        flat.push(t);
    }
}

/// Multiplies a factor into the running coefficient or the list of other factors.
fn absorb_factor(f: Expr, coef: &mut Number, rest: &mut Vec<Expr>) {
    if let Expr::Const(n) = &f {
        match coef.checked_mul(*n) {
            Some(c) => *coef = c,
            None => rest.push(f),
        }
    } else {
        rest.push(f);
    }
}

/// Exact square root of a non-negative rational, when it has a closed form.
///
/// Uses `sqrt(p/q) = sqrt(p*q)/q`, then pulls out square factors up to 10^4.
/// The result is `k/q * sqrt(m)`, so `sqrt(12)` gives `2*sqrt(3)`.
/// Square factors above 10^4 are left inside the root (not fully reduced).
fn sqrt_const(b: Number) -> Option<Expr> {
    if b.is_negative() {
        return None;
    }
    if b.is_zero() {
        return Some(Expr::zero());
    }
    let mut rest = b.numer().checked_mul(b.denom())?;
    let mut k: i128 = 1;
    let mut d: i128 = 2;
    while d <= 10_000 && d * d <= rest {
        while rest % (d * d) == 0 {
            rest /= d * d;
            k *= d;
        }
        d += 1;
    }
    let coef = Number::new(k, b.denom())?;
    if rest == 1 {
        return Some(Expr::Const(coef));
    }
    let root = Expr::Pow(
        Box::new(Expr::int(rest)),
        Box::new(Expr::Const(Number::new(1, 2)?)),
    );
    if coef.is_one() {
        Some(root)
    } else {
        Some(Expr::Mul(vec![Expr::Const(coef), root]))
    }
}

/// Shared logic for `and` / `or`.
fn junction(items: Vec<Expr>, is_and: bool) -> Expr {
    let identity = is_and;
    let mut out: Vec<Expr> = Vec::new();
    for it in items {
        let flat = match it {
            Expr::And(v) if is_and => v,
            Expr::Or(v) if !is_and => v,
            other => vec![other],
        };
        for x in flat {
            match x {
                Expr::Bool(b) if b == identity => {}
                Expr::Bool(b) => return Expr::Bool(b),
                other => out.push(other),
            }
        }
    }
    out.sort();
    out.dedup();
    match out.len() {
        0 => Expr::Bool(identity),
        1 => out.pop().unwrap_or(Expr::Bool(identity)),
        _ if is_and => Expr::And(out),
        _ => Expr::Or(out),
    }
}

// Precedence levels used by the printer. Higher binds tighter.
const P_OR: u8 = 1;
const P_AND: u8 = 2;
const P_NOT: u8 = 3;
const P_EQ: u8 = 4;
const P_ADD: u8 = 5;
const P_MUL: u8 = 6;
const P_POW: u8 = 9;
const P_ATOM: u8 = 10;

fn precedence(e: &Expr) -> u8 {
    match e {
        Expr::Var(_) | Expr::Func(..) | Expr::Bool(_) => P_ATOM,
        Expr::Const(n) if n.is_negative() => P_ADD,
        Expr::Const(n) if n.is_integer() => P_ATOM,
        Expr::Const(_) => P_MUL,
        Expr::Add(_) => P_ADD,
        Expr::Mul(_) if has_negative_coefficient(e) => P_ADD,
        Expr::Mul(_) => P_MUL,
        Expr::Pow(..) => P_POW,
        Expr::Eq(..) => P_EQ,
        Expr::And(_) => P_AND,
        Expr::Or(_) => P_OR,
        Expr::Not(_) => P_NOT,
    }
}

/// True for a term that prints with a leading minus sign.
fn has_negative_coefficient(t: &Expr) -> bool {
    match t {
        Expr::Const(c) => c.is_negative(),
        Expr::Mul(v) => matches!(v.first(), Some(Expr::Const(c)) if c.is_negative()),
        _ => false,
    }
}

/// The same term with its sign flipped, for printing `a - b` instead of `a + -b`.
fn negate_term(t: &Expr) -> Expr {
    match t {
        // i128::MIN has no positive counterpart; the parser never produces it.
        Expr::Const(c) => Expr::Const(c.checked_neg().unwrap_or(*c)),
        Expr::Mul(v) => {
            let c = v[0].as_number().unwrap_or(Number::ONE);
            let flipped = c.checked_neg().unwrap_or(c);
            let rest = v[1..].to_vec();
            if flipped.is_one() {
                if rest.len() == 1 {
                    rest.into_iter().next().unwrap_or_else(Expr::one)
                } else {
                    Expr::Mul(rest)
                }
            } else {
                let mut out = vec![Expr::Const(flipped)];
                out.extend(rest);
                Expr::Mul(out)
            }
        }
        other => other.clone(),
    }
}

fn write_at(f: &mut fmt::Formatter<'_>, e: &Expr, min: u8) -> fmt::Result {
    if precedence(e) < min {
        f.write_str("(")?;
        write_bare(f, e)?;
        f.write_str(")")
    } else {
        write_bare(f, e)
    }
}

fn write_bare(f: &mut fmt::Formatter<'_>, e: &Expr) -> fmt::Result {
    match e {
        Expr::Var(name) => f.write_str(name),
        Expr::Const(n) => write!(f, "{n}"),
        Expr::Bool(b) => f.write_str(if *b { "true" } else { "false" }),
        Expr::Func(func, arg) => {
            write!(f, "{}(", func.name())?;
            write_at(f, arg, 0)?;
            f.write_str(")")
        }
        Expr::Add(v) => {
            for (i, t) in v.iter().enumerate() {
                if has_negative_coefficient(t) {
                    f.write_str(if i == 0 { "-" } else { " - " })?;
                    write_at(f, &negate_term(t), P_MUL)?;
                } else {
                    if i > 0 {
                        f.write_str(" + ")?;
                    }
                    write_at(f, t, P_ADD)?;
                }
            }
            Ok(())
        }
        Expr::Mul(_) if has_negative_coefficient(e) => {
            f.write_str("-")?;
            write_at(f, &negate_term(e), P_MUL)
        }
        Expr::Mul(v) => {
            for (i, x) in v.iter().enumerate() {
                if i > 0 {
                    f.write_str("*")?;
                }
                write_at(f, x, P_MUL)?;
            }
            Ok(())
        }
        Expr::Pow(b, x) => {
            write_at(f, b, P_ATOM)?;
            f.write_str("^")?;
            write_at(f, x, P_POW)
        }
        Expr::Eq(a, b) => {
            write_at(f, a, P_ADD)?;
            f.write_str(" = ")?;
            write_at(f, b, P_ADD)
        }
        Expr::And(v) => {
            for (i, x) in v.iter().enumerate() {
                if i > 0 {
                    f.write_str(" and ")?;
                }
                write_at(f, x, P_NOT)?;
            }
            Ok(())
        }
        Expr::Or(v) => {
            for (i, x) in v.iter().enumerate() {
                if i > 0 {
                    f.write_str(" or ")?;
                }
                write_at(f, x, P_AND)?;
            }
            Ok(())
        }
        Expr::Not(a) => {
            f.write_str("not ")?;
            write_at(f, a, P_NOT)
        }
    }
}

/// Prints the expression in the same syntax the parser accepts. Parsing the
/// output gives back an identical canonical tree.
impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_at(f, self, 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    fn x() -> Expr {
        Expr::var("x")
    }
    fn y() -> Expr {
        Expr::var("y")
    }

    #[test]
    fn addition_is_commutative_at_construction() {
        let a = Expr::sum(vec![x(), y()]);
        let b = Expr::sum(vec![y(), x()]);
        assert_eq!(a, b);
    }

    #[test]
    fn constants_fold_and_like_terms_combine() {
        // 2 + x + 3 -> x + 5
        let e = Expr::sum(vec![Expr::int(2), x(), Expr::int(3)]);
        assert_eq!(e, Expr::sum(vec![x(), Expr::int(5)]));
        // x + x -> 2x
        let twice = Expr::sum(vec![x(), x()]);
        assert_eq!(twice, Expr::product(vec![Expr::int(2), x()]));
        // x - x -> 0
        assert_eq!(Expr::difference(x(), x()), Expr::zero());
    }

    #[test]
    fn exact_thirds_sum_to_one() {
        let third = Expr::rational(1, 3).unwrap();
        let e = Expr::sum(vec![third.clone(), third.clone(), third]);
        assert_eq!(e, Expr::one());
    }

    #[test]
    fn multiplicative_identities() {
        assert_eq!(Expr::product(vec![x(), Expr::zero()]), Expr::zero());
        assert_eq!(Expr::product(vec![x(), Expr::one()]), x());
        assert_eq!(Expr::power(x(), Expr::zero()), Expr::one());
        assert_eq!(Expr::power(x(), Expr::one()), x());
    }

    #[test]
    fn equal_bases_merge_into_powers() {
        // x * x^2 -> x^3
        let e = Expr::product(vec![x(), Expr::power(x(), Expr::int(2))]);
        assert_eq!(e, Expr::power(x(), Expr::int(3)));
        // x * x^-1 -> 1
        let one = Expr::product(vec![x(), Expr::power(x(), Expr::int(-1))]);
        assert_eq!(one, Expr::one());
    }

    #[test]
    fn nested_sums_and_products_flatten() {
        let inner = Expr::sum(vec![x(), y()]);
        let outer = Expr::sum(vec![inner, Expr::int(1)]);
        assert_eq!(outer, Expr::sum(vec![x(), y(), Expr::int(1)]));
    }

    #[test]
    fn integer_powers_of_powers_collapse() {
        let e = Expr::power(Expr::power(x(), Expr::int(2)), Expr::int(3));
        assert_eq!(e, Expr::power(x(), Expr::int(6)));
    }

    #[test]
    fn constant_powers_fold_exactly() {
        let e = Expr::power(Expr::rational(2, 3).unwrap(), Expr::int(-2));
        assert_eq!(e, Expr::rational(9, 4).unwrap());
    }

    #[test]
    fn square_roots_extract_square_factors() {
        assert_eq!(Expr::power(Expr::int(4), Expr::rational(1, 2).unwrap()), Expr::int(2));
        let root12 = Expr::power(Expr::int(12), Expr::rational(1, 2).unwrap());
        let expected = Expr::product(vec![
            Expr::int(2),
            Expr::power(Expr::int(3), Expr::rational(1, 2).unwrap()),
        ]);
        assert_eq!(root12, expected);
    }

    #[test]
    fn canonicalization_is_idempotent() {
        let e = Expr::sum(vec![
            Expr::product(vec![Expr::int(3), x(), y()]),
            Expr::int(7),
            Expr::negate(x()),
        ]);
        let rebuilt = Expr::sum(vec![e.clone()]);
        assert_eq!(e, rebuilt);
    }

    #[test]
    fn constant_distributes_over_single_sum() {
        // 2 * (x + 1) -> 2x + 2
        let e = Expr::product(vec![Expr::int(2), Expr::sum(vec![x(), Expr::int(1)])]);
        assert_eq!(e, Expr::sum(vec![Expr::product(vec![Expr::int(2), x()]), Expr::int(2)]));
    }

    #[test]
    fn boolean_connectives_canonicalize() {
        let a = Expr::equation(x(), Expr::int(1));
        let b = Expr::equation(y(), Expr::int(2));
        assert_eq!(Expr::and_all(vec![a.clone(), b.clone()]), Expr::and_all(vec![b.clone(), a.clone(), a.clone()]));
        assert_eq!(Expr::and_all(vec![a.clone(), Expr::boolean(false)]), Expr::boolean(false));
        assert_eq!(Expr::or_all(vec![a.clone(), Expr::boolean(true)]), Expr::boolean(true));
        assert_eq!(Expr::logical_not(Expr::logical_not(a.clone())), a);
        assert_eq!(Expr::equation(Expr::int(2), Expr::int(2)), Expr::boolean(true));
    }

    #[test]
    fn printer_shapes() {
        // Variables sort before constants, so the leading term is the variable.
        assert_eq!(Expr::sum(vec![x(), Expr::int(5)]).to_string(), "x + 5");
        assert_eq!(Expr::difference(x(), Expr::int(3)).to_string(), "x - 3");
        assert_eq!(Expr::difference(Expr::int(3), x()).to_string(), "3 - x");
        assert_eq!(Expr::negate(x()).to_string(), "-x");
        assert_eq!(Expr::power(x(), Expr::int(-1)).to_string(), "x^(-1)");
        assert_eq!(Expr::func(SymbolicFunc::Sin, x()).to_string(), "sin(x)");
    }
}
