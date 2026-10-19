//! Command-line front end for `tpt-symbolic`.
//!
//! ```text
//! tpt-sym simplify "x*x + x*x"
//! tpt-sym diff "x^3" x
//! tpt-sym integrate "2*x" x
//! tpt-sym solve "x^2 - 2 = 0" x
//! tpt-sym eval "x^2 + 1" x=1/2
//! tpt-sym decimal "1/3" 6
//! tpt-sym latex "x/2"
//! ```

use std::process::ExitCode;

use tpt_symbolic::calculus::{diff, integrate, Integral};
use tpt_symbolic::eval::{eval, to_decimal};
use tpt_symbolic::latex::to_latex;
use tpt_symbolic::parse::parse;
use tpt_symbolic::simplify::simplify;
use tpt_symbolic::solve::{solve, SolveResult};
use tpt_symbolic::{Error, Expr, Number};

const USAGE: &str = "usage:
  tpt-sym simplify <expr>
  tpt-sym diff <expr> <var>
  tpt-sym integrate <expr> <var>
  tpt-sym solve <equation> <var>
  tpt-sym eval <expr> <name=value>...
  tpt-sym decimal <expr> <digits>
  tpt-sym latex <expr>";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(out) => {
            println!("{out}");
            ExitCode::SUCCESS
        }
        Err(Failure::Usage) => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
        Err(Failure::Message(msg)) => {
            eprintln!("{msg}");
            ExitCode::FAILURE
        }
    }
}

enum Failure {
    Usage,
    Message(String),
}

impl From<Error> for Failure {
    fn from(err: Error) -> Failure {
        Failure::Message(match err {
            Error::Parse { pos, message } => format!("parse error: {message}\n  {pos}"),
            other => other.to_string(),
        })
    }
}

fn run(args: &[String]) -> Result<String, Failure> {
    let (cmd, rest) = args.split_first().ok_or(Failure::Usage)?;
    let text = |i: usize| rest.get(i).map(String::as_str).ok_or(Failure::Usage);
    match cmd.as_str() {
        "simplify" => Ok(simplify(&parse_arg(text(0)?)?).to_string()),
        "diff" => Ok(diff(&parse_arg(text(0)?)?, text(1)?)?.to_string()),
        "integrate" => Ok(describe_integral(&integrate(
            &parse_arg(text(0)?)?,
            text(1)?,
        ))),
        "solve" => Ok(describe_solve(&solve(&parse_arg(text(0)?)?, text(1)?)?)),
        "latex" => Ok(to_latex(&parse_arg(text(0)?)?)),
        "decimal" => {
            let value = constant(&parse_arg(text(0)?)?)?;
            let digits: usize = text(1)?.parse().map_err(|_| Failure::Usage)?;
            Ok(to_decimal(&value, digits))
        }
        "eval" => {
            let expr = parse_arg(text(0)?)?;
            let env = rest[1..]
                .iter()
                .map(|binding| parse_binding(binding))
                .collect::<Result<Vec<_>, _>>()?;
            let refs: Vec<(&str, Number)> =
                env.iter().map(|(n, v)| (n.as_str(), v.clone())).collect();
            Ok(eval(&expr, &refs)?.to_string())
        }
        _ => Err(Failure::Usage),
    }
}

/// Reads `name=value`, where the value is any exact constant such as `3/4` or `0.5`.
fn parse_binding(binding: &str) -> Result<(String, Number), Failure> {
    let (name, value) = binding
        .split_once('=')
        .ok_or_else(|| Failure::Message(format!("expected name=value, got {binding:?}")))?;
    let value = constant(&parse_arg(value)?)?;
    Ok((name.to_string(), value))
}

/// Parses a command-line expression. A parse error gives its line and column,
/// and the line it is on is shown with a caret under the error.
fn parse_arg(src: &str) -> Result<Expr, Failure> {
    parse(src).map_err(|err| match err {
        Error::Parse { pos, message } => Failure::Message(describe_parse_error(src, pos, message)),
        other => Failure::from(other),
    })
}

/// Formats a parse error at byte offset `pos` in `src`, with a 1-based line and column.
fn describe_parse_error(src: &str, pos: usize, message: &str) -> String {
    let pos = pos.min(src.len());
    let before = &src[..pos];
    let line_no = before.matches('\n').count() + 1;
    let line_start = before.rfind('\n').map_or(0, |i| i + 1);
    let line_end = src[pos..].find('\n').map_or(src.len(), |i| pos + i);
    let line = &src[line_start..line_end];
    let column = before[line_start..].chars().count() + 1;
    let caret_pad = " ".repeat(column - 1);
    format!("parse error at line {line_no}, column {column}: {message}\n{line}\n{caret_pad}^")
}

fn constant(e: &Expr) -> Result<Number, Failure> {
    e.as_number()
        .ok_or_else(|| Failure::Message(String::from("expected a plain number")))
}

fn describe_integral(result: &Integral) -> String {
    match result {
        Integral::Exact(f) => format!("{f} + C"),
        Integral::Unevaluated(e) => format!("no closed form found for the integral of {e}"),
        _ => String::from("unrecognised integration result"),
    }
}

fn describe_solve(result: &SolveResult) -> String {
    match result {
        SolveResult::Exact(roots) if roots.is_empty() => String::from("no solutions"),
        SolveResult::Exact(roots) => roots
            .iter()
            .map(|r| r.to_string())
            .collect::<Vec<_>>()
            .join(", "),
        SolveResult::Parametric => String::from("solutions depend on free parameters"),
        SolveResult::Numeric(root) => {
            format!(
                "no closed form; numeric root-finding needed for {} = 0",
                root.expr
            )
        }
        SolveResult::NoSolution => String::from("no real solution"),
        SolveResult::Infinite => String::from("every value is a solution"),
        _ => String::from("unrecognised solver result"),
    }
}

#[cfg(test)]
mod tests {
    use super::describe_parse_error;

    #[test]
    fn parse_error_reports_line_and_column_on_its_own_line() {
        let text = describe_parse_error("x +\n2 * )", 8, "unexpected token");
        assert_eq!(
            text,
            "parse error at line 2, column 5: unexpected token\n2 * )\n    ^"
        );
    }
}
