//! String parser for expressions.
//!
//! Grammar, from loosest to tightest binding:
//!
//! | level      | operators                                   |
//! |------------|---------------------------------------------|
//! | disjunction| `or`                                        |
//! | conjunction| `and`                                       |
//! | negation   | `not` (prefix)                              |
//! | equation   | `=`                                         |
//! | sum        | `+`, `-`                                    |
//! | product    | `*`, `/`, implicit juxtaposition           |
//! | unary      | prefix `-`, `+`                             |
//! | power      | `^` and `**` (right-associative)           |
//!
//! Numbers are exact: `0.25` is read as `1/4`, never as a float. Names are ASCII
//! identifiers. Multi-letter names are single variables (`xy` is one name). Use
//! `x*y` for a product.

use crate::error::Error;
use crate::expr::{Expr, SymbolicFunc};
use crate::mathr::Number;
use alloc::string::String;
use alloc::vec::Vec;
use core::str::FromStr;

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Num(Number),
    Ident(String),
    Sym(char),
}

/// Parses `src` into a canonical expression.
pub fn parse(src: &str) -> Result<Expr, Error> {
    let toks = tokenize(src)?;
    if toks.is_empty() {
        return Err(Error::Parse { pos: 0, message: "empty input" });
    }
    let end = src.len();
    let mut p = Parser { toks, pos: 0, end };
    let e = p.or_expr()?;
    if p.pos < p.toks.len() {
        return Err(p.err("unexpected input after the expression"));
    }
    Ok(e)
}

impl FromStr for Expr {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse(s)
    }
}

fn is_keyword(word: &str) -> bool {
    matches!(word, "and" | "or" | "not")
}

fn tokenize(src: &str) -> Result<Vec<(usize, Tok)>, Error> {
    let bytes = src.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        match c {
            b' ' | b'\t' | b'\r' | b'\n' => i += 1,
            b'0'..=b'9' | b'.' => {
                let start = i;
                while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                    i += 1;
                }
                let n = parse_decimal(&src[start..i]).ok_or(Error::Parse {
                    pos: start,
                    message: "malformed or out-of-range number",
                })?;
                out.push((start, Tok::Num(n)));
            }
            b'A'..=b'Z' | b'a'..=b'z' | b'_' => {
                let start = i;
                while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                out.push((start, Tok::Ident(String::from(&src[start..i]))));
            }
            b'*' if bytes.get(i + 1) == Some(&b'*') => {
                out.push((i, Tok::Sym('^')));
                i += 2;
            }
            b'+' | b'-' | b'*' | b'/' | b'^' | b'(' | b')' | b'=' => {
                out.push((i, Tok::Sym(c as char)));
                i += 1;
            }
            _ => {
                return Err(Error::Parse { pos: i, message: "unexpected character" });
            }
        }
    }
    Ok(out)
}

/// Reads `digits` or `digits.digits` as an exact rational.
fn parse_decimal(text: &str) -> Option<Number> {
    let mut parts = text.splitn(2, '.');
    let int_part = parts.next()?;
    let frac_part = parts.next().unwrap_or("");
    if int_part.is_empty() && frac_part.is_empty() {
        return None;
    }
    if frac_part.contains('.') {
        return None;
    }
    let mut num: i128 = 0;
    for ch in int_part.chars().chain(frac_part.chars()) {
        let digit = ch.to_digit(10)? as i128;
        num = num.checked_mul(10)?.checked_add(digit)?;
    }
    let mut den: i128 = 1;
    for _ in frac_part.chars() {
        den = den.checked_mul(10)?;
    }
    Number::new(num, den)
}

struct Parser {
    toks: Vec<(usize, Tok)>,
    pos: usize,
    end: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos).map(|(_, t)| t)
    }

    fn at_sym(&self, c: char) -> bool {
        matches!(self.peek(), Some(Tok::Sym(s)) if *s == c)
    }

    fn at_word(&self, w: &str) -> bool {
        matches!(self.peek(), Some(Tok::Ident(s)) if s == w)
    }

    fn err(&self, message: &'static str) -> Error {
        let pos = self.toks.get(self.pos).map_or(self.end, |(p, _)| *p);
        Error::Parse { pos, message }
    }

    fn bump(&mut self) {
        self.pos += 1;
    }

    fn expect_sym(&mut self, c: char, message: &'static str) -> Result<(), Error> {
        if self.at_sym(c) {
            self.bump();
            Ok(())
        } else {
            Err(self.err(message))
        }
    }

    fn or_expr(&mut self) -> Result<Expr, Error> {
        let mut items = alloc::vec![self.and_expr()?];
        while self.at_word("or") {
            self.bump();
            items.push(self.and_expr()?);
        }
        Ok(Expr::or_all(items))
    }

    fn and_expr(&mut self) -> Result<Expr, Error> {
        let mut items = alloc::vec![self.not_expr()?];
        while self.at_word("and") {
            self.bump();
            items.push(self.not_expr()?);
        }
        Ok(Expr::and_all(items))
    }

    fn not_expr(&mut self) -> Result<Expr, Error> {
        if self.at_word("not") {
            self.bump();
            Ok(Expr::logical_not(self.not_expr()?))
        } else {
            self.eq_expr()
        }
    }

    fn eq_expr(&mut self) -> Result<Expr, Error> {
        let lhs = self.sum_expr()?;
        if self.at_sym('=') {
            self.bump();
            let rhs = self.sum_expr()?;
            Ok(Expr::equation(lhs, rhs))
        } else {
            Ok(lhs)
        }
    }

    fn sum_expr(&mut self) -> Result<Expr, Error> {
        let mut acc = self.term()?;
        loop {
            if self.at_sym('+') {
                self.bump();
                let rhs = self.term()?;
                acc = Expr::sum(alloc::vec![acc, rhs]);
            } else if self.at_sym('-') {
                self.bump();
                let rhs = self.term()?;
                acc = Expr::difference(acc, rhs);
            } else {
                return Ok(acc);
            }
        }
    }

    fn starts_implicit_factor(&self) -> bool {
        match self.peek() {
            Some(Tok::Num(_)) => true,
            Some(Tok::Ident(s)) => !is_keyword(s),
            Some(Tok::Sym('(')) => true,
            _ => false,
        }
    }

    fn term(&mut self) -> Result<Expr, Error> {
        let mut acc = self.unary()?;
        loop {
            if self.at_sym('*') {
                self.bump();
                let rhs = self.unary()?;
                acc = Expr::product(alloc::vec![acc, rhs]);
            } else if self.at_sym('/') {
                self.bump();
                let rhs = self.unary()?;
                if rhs.as_number().is_some_and(|n| n.is_zero()) {
                    return Err(Error::DivisionByZero);
                }
                acc = Expr::quotient(acc, rhs);
            } else if self.starts_implicit_factor() {
                let rhs = self.power()?;
                acc = Expr::product(alloc::vec![acc, rhs]);
            } else {
                return Ok(acc);
            }
        }
    }

    fn unary(&mut self) -> Result<Expr, Error> {
        if self.at_sym('-') {
            self.bump();
            Ok(Expr::negate(self.unary()?))
        } else if self.at_sym('+') {
            self.bump();
            self.unary()
        } else {
            self.power()
        }
    }

    fn power(&mut self) -> Result<Expr, Error> {
        let base = self.atom()?;
        if self.at_sym('^') {
            self.bump();
            let exp = self.unary()?;
            let base_is_zero = base.as_number().is_some_and(|n| n.is_zero());
            let exp_is_negative = exp.as_number().is_some_and(|n| n.is_negative());
            if base_is_zero && exp_is_negative {
                return Err(Error::DivisionByZero);
            }
            Ok(Expr::power(base, exp))
        } else {
            Ok(base)
        }
    }

    fn atom(&mut self) -> Result<Expr, Error> {
        let tok = match self.peek() {
            Some(t) => t.clone(),
            None => return Err(self.err("unexpected end of input")),
        };
        match tok {
            Tok::Num(n) => {
                self.bump();
                Ok(Expr::Const(n))
            }
            Tok::Sym('(') => {
                self.bump();
                let inner = self.or_expr()?;
                self.expect_sym(')', "expected ')'")?;
                Ok(inner)
            }
            Tok::Ident(name) => {
                if name == "true" || name == "false" {
                    self.bump();
                    return Ok(Expr::Bool(name == "true"));
                }
                if is_keyword(&name) {
                    return Err(self.err("keyword used where a value was expected"));
                }
                self.bump();
                match SymbolicFunc::from_name(&name) {
                    Some(func) if self.at_sym('(') => {
                        self.bump();
                        let arg = self.or_expr()?;
                        self.expect_sym(')', "expected ')' after function argument")?;
                        Ok(Expr::func(func, arg))
                    }
                    Some(_) => Err(self.err("function name must be followed by '(...)'")),
                    None => Ok(Expr::var(&name)),
                }
            }
            Tok::Sym(_) => Err(self.err("expected a number, a name or '('")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::format;
    use alloc::string::ToString;

    fn p(s: &str) -> Expr {
        parse(s).unwrap_or_else(|e| panic!("failed to parse {s:?}: {e}"))
    }

    #[test]
    fn decimals_are_exact() {
        assert_eq!(p("0.25"), Expr::rational(1, 4).unwrap());
        assert_eq!(p("1/3 + 1/3 + 1/3"), Expr::one());
    }

    #[test]
    fn equivalent_spellings_give_identical_trees() {
        // Golden set: each group must canonicalize to one tree.
        let groups: &[&[&str]] = &[
            &["x + y", "y + x", "y+x", "(x) + (y)"],
            &["2*x + 3 + x", "3 + 3*x - x*2 + 2*x - 2*x + 2*x - x + x", "x + x + x + 3 + x*0"],
            &["2x", "2*x", "x*2", "(2)*x", "2**1*x"],
            &["x^2 * x", "x**3", "x*x*x"],
            &["-(x + 1)", "-x - 1", "0 - x - 1"],
            &["x - x", "0", "2 - 2"],
            &["x/2", "x*0.5", "(1/2)*x"],
            &["sin(x)^2 + cos(x)^2", "cos(x)^2 + sin(x)^2"],
            &["a = b", "b = a"],
            &["x = 1 and y = 2", "y = 2 and x = 1", "(x = 1) and (y = 2) and true"],
            &["not not (x = 1)", "x = 1"],
        ];
        for group in groups {
            let first = p(group[0]);
            for other in &group[1..] {
                assert_eq!(p(other), first, "{other:?} should equal {:?}", group[0]);
            }
        }
    }

    #[test]
    fn precedence_and_associativity() {
        assert_eq!(p("2^3^2"), p("2^(3^2)"));
        assert_eq!(p("-x^2"), p("-(x^2)"));
        assert_eq!(p("x^-1"), Expr::power(Expr::var("x"), Expr::int(-1)));
        assert_eq!(p("2*3 + 4"), Expr::int(10));
    }

    #[test]
    fn rejects_bad_input_with_positions() {
        assert!(matches!(parse(""), Err(Error::Parse { .. })));
        assert!(matches!(parse("1 / 0"), Err(Error::DivisionByZero)));
        assert!(matches!(parse("x +"), Err(Error::Parse { .. })));
        assert!(matches!(parse("x $ 2"), Err(Error::Parse { pos: 2, .. })));
        assert!(matches!(parse("(x + 1"), Err(Error::Parse { .. })));
        assert!(matches!(parse("sin x"), Err(Error::Parse { .. })));
        assert!(matches!(parse("1.2.3"), Err(Error::Parse { .. })));
    }

    #[test]
    fn random_expressions_parse_back_under_equivalent_spellings() {
        use crate::testkit::{random_expr, Rng};
        let mut rng = Rng::new(11);
        for _ in 0..1000 {
            let e = random_expr(&mut rng, 4);
            let s = e.to_string();
            let variants = [
                format!("({s})"),
                format!("{s} + 0"),
                format!("0 + {s}"),
                format!("1*({s})"),
                format!("({s})^1"),
            ];
            for v in &variants {
                assert_eq!(p(v), e, "{v:?} should equal {s:?}");
            }
        }
    }

    #[test]
    fn printed_output_reparses_to_the_same_tree() {
        let samples = [
            "x + 5",
            "3 - x",
            "-2*x*y + 1/2",
            "(x + 1)^(1/2)",
            "sin(x)^2 + cos(x)^2",
            "x^(-1)*y",
            "(-3)*x^2 - 7",
            "a = b and (not c or d)",
            "not (x = 1 and y = 2)",
        ];
        for s in samples {
            let e = p(s);
            let printed = e.to_string();
            assert_eq!(p(&printed), e, "round trip failed for {s:?} printed as {printed:?}");
        }
        assert_eq!(format!("{}", p("2*x")), "2*x");
    }
}
