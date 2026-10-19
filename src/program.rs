//! Compiling expressions to a postfix program for numeric evaluation.
//!
//! This is the hand-off to a numeric back end such as `tpt-math`. The program is a flat
//! list of operations over a value stack, with variables as indices into a caller-supplied
//! list. It contains no floating-point values, so the core stays exact. The back end
//! chooses how to evaluate each operation.
//!
//! [`run_exact`] evaluates programs that use only exact rational operations. It is for
//! checking the compiler, not a numeric evaluator.

use crate::error::Error;
use crate::expr::{Expr, SymbolicFunc};
use crate::mathr::Number;
use alloc::string::String;
use alloc::vec::Vec;

/// One operation of a postfix program.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Op {
    /// Push an exact constant.
    Const(Number),
    /// Push π.
    Pi,
    /// Push Euler's number.
    E,
    /// Push the imaginary unit.
    I,
    /// Push the value of variable `index` in [`Program::vars`].
    Var(usize),
    /// Pop `n` values and push their sum.
    Add(usize),
    /// Pop `n` values and push their product.
    Mul(usize),
    /// Pop an exponent and a base, and push base to the exponent.
    Pow,
    /// Pop an argument and push the function applied to it.
    Func(SymbolicFunc),
}

/// A compiled expression: the variable names, and the operations in evaluation order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Program {
    /// Variable names. [`Op::Var`] indices refer to this list.
    pub vars: Vec<String>,
    /// Operations, evaluated left to right on a stack. The result is the one value left.
    pub ops: Vec<Op>,
}

/// Compiles `e` over the variables `vars`.
///
/// Fails on booleans, equations and logical connectives, and on any variable not in `vars`.
/// # Example
/// ```
/// use tpt_symbolic::parse::parse;
/// use tpt_symbolic::program::compile;
/// assert!(compile(&parse("x^2").unwrap(), &["x"]).is_ok());
/// ```
pub fn compile(e: &Expr, vars: &[&str]) -> Result<Program, Error> {
    let mut ops = Vec::new();
    emit(e, vars, &mut ops)?;
    Ok(Program {
        vars: vars.iter().map(|v| String::from(*v)).collect(),
        ops,
    })
}

fn emit(e: &Expr, vars: &[&str], out: &mut Vec<Op>) -> Result<(), Error> {
    match e {
        Expr::Const(n) => out.push(Op::Const(n.clone())),
        Expr::Pi => out.push(Op::Pi),
        Expr::E => out.push(Op::E),
        Expr::I => out.push(Op::I),
        Expr::Var(v) => {
            let index = vars
                .iter()
                .position(|name| *name == &**v)
                .ok_or(Error::Unsupported(
                    "expression uses a variable not in the list",
                ))?;
            out.push(Op::Var(index));
        }
        Expr::Add(terms) => {
            for t in terms {
                emit(t, vars, out)?;
            }
            out.push(Op::Add(terms.len()));
        }
        Expr::Mul(factors) => {
            for f in factors {
                emit(f, vars, out)?;
            }
            out.push(Op::Mul(factors.len()));
        }
        Expr::Pow(base, exp) => {
            emit(base, vars, out)?;
            emit(exp, vars, out)?;
            out.push(Op::Pow);
        }
        Expr::Func(f, arg) => {
            emit(arg, vars, out)?;
            out.push(Op::Func(*f));
        }
        Expr::Bool(_) | Expr::Eq(..) | Expr::And(_) | Expr::Or(_) | Expr::Not(_) => {
            return Err(Error::Unsupported(
                "only arithmetic expressions compile to a program",
            ));
        }
    }
    Ok(())
}

/// Evaluates a program exactly, with `env[i]` as the value of `program.vars[i]`.
///
/// Supports constants, variables, sums, products, and powers with integer exponents.
/// Constants `pi`, `e`, `i` and functions have no exact rational value and fail with
/// [`Error::Unsupported`].
/// # Example
/// ```
/// use tpt_symbolic::parse::parse;
/// use tpt_symbolic::program::{compile, run_exact};
/// use tpt_symbolic::Number;
/// let prog = compile(&parse("x^2 + 3*x").unwrap(), &["x"]).unwrap();
/// assert_eq!(run_exact(&prog, &[Number::from_int(2)]).unwrap(), Number::from_int(10));
/// ```
pub fn run_exact(program: &Program, env: &[Number]) -> Result<Number, Error> {
    let mut stack: Vec<Number> = Vec::new();
    for op in &program.ops {
        match op {
            Op::Const(n) => stack.push(n.clone()),
            Op::Var(i) => stack.push(
                env.get(*i)
                    .cloned()
                    .ok_or(Error::Unsupported("missing value for a variable"))?,
            ),
            Op::Add(n) => {
                let items = pop_many(&mut stack, *n)?;
                let mut acc = Number::zero();
                for x in &items {
                    acc = acc.checked_add(x).ok_or(Error::Overflow)?;
                }
                stack.push(acc);
            }
            Op::Mul(n) => {
                let items = pop_many(&mut stack, *n)?;
                let mut acc = Number::one();
                for x in &items {
                    acc = acc.checked_mul(x).ok_or(Error::Overflow)?;
                }
                stack.push(acc);
            }
            Op::Pow => {
                let exp = stack.pop().ok_or(Error::Unsupported("malformed program"))?;
                let base = stack.pop().ok_or(Error::Unsupported("malformed program"))?;
                let k =
                    exp.as_i128()
                        .and_then(|k| i64::try_from(k).ok())
                        .ok_or(Error::Unsupported(
                            "exact evaluation needs integer exponents",
                        ))?;
                stack.push(base.checked_pow(k).ok_or(Error::DivisionByZero)?);
            }
            Op::Pi | Op::E | Op::I | Op::Func(_) => {
                return Err(Error::Unsupported(
                    "no exact rational value for this operation",
                ));
            }
        }
    }
    match (stack.pop(), stack.is_empty()) {
        (Some(v), true) => Ok(v),
        _ => Err(Error::Unsupported("malformed program")),
    }
}

fn pop_many(stack: &mut Vec<Number>, n: usize) -> Result<Vec<Number>, Error> {
    if n > stack.len() {
        return Err(Error::Unsupported("malformed program"));
    }
    Ok(stack.split_off(stack.len() - n))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse;

    fn p(s: &str) -> Expr {
        parse(s).unwrap_or_else(|e| panic!("parse {s:?}: {e}"))
    }

    #[test]
    fn exact_run_matches_the_expression() {
        let prog = compile(&p("x^2 + 3*x - 1/2"), &["x"]).unwrap();
        assert_eq!(
            run_exact(&prog, &[Number::from_int(2)]).unwrap(),
            Number::new(19, 2).unwrap()
        );
    }

    #[test]
    fn several_variables_use_their_index() {
        let prog = compile(&p("y - x"), &["x", "y"]).unwrap();
        assert_eq!(
            run_exact(&prog, &[Number::from_int(2), Number::from_int(7)]).unwrap(),
            Number::from_int(5)
        );
    }

    #[test]
    fn transcendental_and_unknown_symbols_are_not_compiled_exactly() {
        let prog = compile(&p("sin(x)"), &["x"]).unwrap();
        assert!(run_exact(&prog, &[Number::one()]).is_err());
        assert!(compile(&p("y*x"), &["x"]).is_err());
        assert!(compile(&p("x = 1"), &["x"]).is_err());
    }

    #[test]
    fn the_program_has_no_floats_and_uses_the_variable_table() {
        let prog = compile(&p("2*x"), &["x"]).unwrap();
        assert_eq!(prog.vars, alloc::vec![String::from("x")]);
        assert!(prog.ops.contains(&Op::Var(0)));
    }
}
