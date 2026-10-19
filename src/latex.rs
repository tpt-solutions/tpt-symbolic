//! LaTeX output for expressions.
//!
//! The output is meant for display in math mode (`$$ ... $$`). It uses `\frac`
//! for quotients, `\sqrt` for square roots, and `\left( ... \right)` where
//! grouping is needed. Multi-letter names are typeset with `\mathrm`, because
//! the parser reads them as single variables.

use crate::expr::{Expr, SymbolicFunc};
use crate::mathr::Number;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// Renders `e` as LaTeX math-mode source.
///
/// # Examples
///
/// ```
/// use tpt_symbolic::latex::to_latex;
/// use tpt_symbolic::parse::parse;
///
/// assert_eq!(to_latex(&parse("x/2").unwrap()), "\\frac{x}{2}");
/// assert_eq!(to_latex(&parse("sqrt(2)").unwrap()), "\\sqrt{2}");
/// ```
pub fn to_latex(e: &Expr) -> String {
    tex(e)
}

fn tex(e: &Expr) -> String {
    match e {
        Expr::Var(name) => var_tex(name),
        Expr::Const(n) => number_tex(n),
        Expr::Pi => String::from("\\pi"),
        Expr::E => String::from("e"),
        Expr::I => String::from("i"),
        Expr::Add(terms) => {
            let mut out = String::new();
            for (i, t) in terms.iter().enumerate() {
                let s = tex(t);
                if i == 0 {
                    out.push_str(&s);
                } else if let Some(rest) = s.strip_prefix('-') {
                    out.push_str(" - ");
                    out.push_str(rest);
                } else {
                    out.push_str(" + ");
                    out.push_str(&s);
                }
            }
            out
        }
        Expr::Mul(factors) => mul_tex(factors),
        Expr::Pow(base, exp) => pow_tex(base, exp),
        Expr::Func(f, arg) => func_tex(*f, arg),
        Expr::Eq(a, b) => format!("{} = {}", tex(a), tex(b)),
        Expr::Bool(true) => String::from("\\top"),
        Expr::Bool(false) => String::from("\\bot"),
        Expr::And(items) => join_logic(items, " \\land "),
        Expr::Or(items) => join_logic(items, " \\lor "),
        Expr::Not(a) => format!("\\lnot {}", group_if(needs_group_under_not(a), tex(a))),
    }
}

fn var_tex(name: &str) -> String {
    if name.chars().count() == 1 {
        String::from(name)
    } else {
        format!("\\mathrm{{{name}}}")
    }
}

fn number_tex(n: &Number) -> String {
    let sign = if n.is_negative() { "-" } else { "" };
    let num = n.numer().abs().to_string();
    if n.is_integer() {
        format!("{sign}{num}")
    } else {
        format!("{sign}\\frac{{{num}}}{{{}}}", n.denom())
    }
}

/// A product, with negative exponents collected into the denominator.
fn mul_tex(factors: &[Expr]) -> String {
    let mut coef: Option<Number> = None;
    let mut numer: Vec<String> = Vec::new();
    let mut denom: Vec<String> = Vec::new();
    for f in factors {
        match f {
            Expr::Const(c) => coef = Some(c.clone()),
            Expr::Pow(base, exp) => match exp.as_number() {
                Some(k) if k.is_integer() && k.is_negative() => {
                    let positive = k.checked_neg().and_then(|p| p.as_i128()).unwrap_or(1);
                    denom.push(if positive == 1 {
                        atom_tex(base)
                    } else {
                        format!("{}^{{{positive}}}", atom_tex(base))
                    });
                }
                _ => numer.push(factor_tex(f)),
            },
            other => numer.push(factor_tex(other)),
        }
    }
    let c = coef.unwrap_or_else(Number::one);
    let negative = c.is_negative();
    let magnitude = if negative {
        c.checked_neg().unwrap_or_else(|| c.clone())
    } else {
        c
    };
    let mut num_parts = Vec::new();
    if !magnitude.numer().is_one() || numer.is_empty() {
        num_parts.push(magnitude.numer().to_string());
    }
    num_parts.extend(numer);
    let mut den_parts = Vec::new();
    if !magnitude.denom().is_one() {
        den_parts.push(magnitude.denom().to_string());
    }
    den_parts.extend(denom);

    let body = if den_parts.is_empty() {
        num_parts.join(" \\cdot ")
    } else {
        format!(
            "\\frac{{{}}}{{{}}}",
            num_parts.join(" \\cdot "),
            den_parts.join(" \\cdot ")
        )
    };
    if negative {
        format!("-{body}")
    } else {
        body
    }
}

fn factor_tex(e: &Expr) -> String {
    group_if(
        matches!(e, Expr::Add(_) | Expr::Eq(..) | Expr::And(_) | Expr::Or(_)),
        tex(e),
    )
}

fn pow_tex(base: &Expr, exp: &Expr) -> String {
    if let Some(k) = exp.as_number() {
        if k.checked_mul(&Number::from_int(2))
            .is_some_and(|twice| twice.is_one())
        {
            // exp = 1/2 prints as a square root.
            return format!("\\sqrt{{{}}}", tex(base));
        }
        if k.is_integer() && k.is_negative() {
            let positive = k.checked_neg().and_then(|p| p.as_i128()).unwrap_or(1);
            let inner = if positive == 1 {
                atom_tex(base)
            } else {
                format!("{}^{{{positive}}}", atom_tex(base))
            };
            return format!("\\frac{{1}}{{{inner}}}");
        }
    }
    format!("{}^{{{}}}", atom_tex(base), tex(exp))
}

/// The base of a power, grouped unless it is a single token.
fn atom_tex(e: &Expr) -> String {
    let single_token = match e {
        Expr::Var(_) | Expr::Func(..) => true,
        Expr::Const(n) => n.is_integer() && !n.is_negative(),
        _ => false,
    };
    group_if(!single_token, tex(e))
}

fn func_tex(f: SymbolicFunc, arg: &Expr) -> String {
    let body = tex(arg);
    match f {
        SymbolicFunc::Abs => format!("\\left|{body}\\right|"),
        SymbolicFunc::Exp => format!("\\exp\\left({body}\\right)"),
        SymbolicFunc::Log => format!("\\ln\\left({body}\\right)"),
        SymbolicFunc::Sin => format!("\\sin\\left({body}\\right)"),
        SymbolicFunc::Cos => format!("\\cos\\left({body}\\right)"),
        SymbolicFunc::Tan => format!("\\tan\\left({body}\\right)"),
        SymbolicFunc::Asin => format!("\\arcsin\\left({body}\\right)"),
        SymbolicFunc::Acos => format!("\\arccos\\left({body}\\right)"),
        SymbolicFunc::Atan => format!("\\arctan\\left({body}\\right)"),
        SymbolicFunc::Sinh => format!("\\sinh\\left({body}\\right)"),
        SymbolicFunc::Cosh => format!("\\cosh\\left({body}\\right)"),
        SymbolicFunc::Tanh => format!("\\tanh\\left({body}\\right)"),
    }
}

fn join_logic(items: &[Expr], sep: &str) -> String {
    items
        .iter()
        .map(|i| {
            group_if(
                matches!(i, Expr::And(_) | Expr::Or(_) | Expr::Eq(..)),
                tex(i),
            )
        })
        .collect::<Vec<_>>()
        .join(sep)
}

fn needs_group_under_not(e: &Expr) -> bool {
    matches!(
        e,
        Expr::Add(_) | Expr::Mul(_) | Expr::Eq(..) | Expr::And(_) | Expr::Or(_)
    )
}

fn group_if(cond: bool, s: String) -> String {
    if cond {
        format!("\\left({s}\\right)")
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse;

    fn t(src: &str) -> String {
        to_latex(&parse(src).unwrap())
    }

    #[test]
    fn powers_roots_and_fractions() {
        assert_eq!(t("x^2 + 1"), "1 + x^{2}");
        assert_eq!(t("sqrt(2)"), "\\sqrt{2}");
        assert_eq!(t("1/x"), "\\frac{1}{x}");
        assert_eq!(t("x/2"), "\\frac{x}{2}");
        assert_eq!(t("3/4"), "\\frac{3}{4}");
        assert_eq!(t("-x*y/3"), "-\\frac{x \\cdot y}{3}");
        assert_eq!(t("(x+1)^2"), "\\left(x + 1\\right)^{2}");
    }

    #[test]
    fn functions_and_logic() {
        assert_eq!(
            t("sin(x)^2 + cos(x)^2"),
            "\\sin\\left(x\\right)^{2} + \\cos\\left(x\\right)^{2}"
        );
        assert_eq!(
            t("x = 1 and y = 2"),
            "\\left(x = 1\\right) \\land \\left(y = 2\\right)"
        );
        assert_eq!(t("not (x = 1)"), "\\lnot \\left(x = 1\\right)");
    }

    #[test]
    fn multi_letter_names_are_roman() {
        assert_eq!(t("xy + 1"), "\\mathrm{xy} + 1");
    }
}
