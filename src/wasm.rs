//! JavaScript bindings, built with the `wasm` feature for `wasm32-unknown-unknown`.
//!
//! Each function takes and returns strings, so a page needs no knowledge of the Rust types.
//! Errors become a thrown JavaScript string with the message.

use crate::calculus::{diff, integrate, Integral};
use crate::latex::to_latex;
use crate::parse::parse;
use crate::simplify::simplify;
use crate::solve::{solve, SolveResult};
use alloc::format;
use alloc::string::{String, ToString};
use wasm_bindgen::prelude::*;

fn fail(message: impl core::fmt::Display) -> JsValue {
    JsValue::from_str(&message.to_string())
}

/// Simplifies an expression and returns it as text.
#[wasm_bindgen]
pub fn simplify_text(src: &str) -> Result<String, JsValue> {
    let e = parse(src).map_err(fail)?;
    Ok(simplify(&e).to_string())
}

/// Differentiates `src` with respect to `var`.
#[wasm_bindgen]
pub fn diff_text(src: &str, var: &str) -> Result<String, JsValue> {
    let e = parse(src).map_err(fail)?;
    Ok(diff(&e, var).map_err(fail)?.to_string())
}

/// Integrates `src` with respect to `var`. Returns the antiderivative, or a message that
/// no verified closed form was found.
#[wasm_bindgen]
pub fn integrate_text(src: &str, var: &str) -> Result<String, JsValue> {
    let e = parse(src).map_err(fail)?;
    Ok(match integrate(&e, var) {
        Integral::Exact(f) => f.to_string(),
        Integral::Unevaluated(_) => String::from("no closed form found"),
    })
}

/// Solves an equation for `var`, one solution per line.
#[wasm_bindgen]
pub fn solve_text(equation: &str, var: &str) -> Result<String, JsValue> {
    let e = parse(equation).map_err(fail)?;
    Ok(match solve(&e, var).map_err(fail)? {
        SolveResult::Exact(roots) => roots
            .iter()
            .map(|r| r.to_string())
            .collect::<alloc::vec::Vec<_>>()
            .join("\n"),
        SolveResult::Numeric(root) => format!("numeric root-finding needed for {} = 0", root.expr),
        SolveResult::NoSolution => String::from("no real solution"),
        SolveResult::Infinite => String::from("every value is a solution"),
        SolveResult::Conditional(expr) => format!("holds only when {expr} = 0"),
        SolveResult::Parametric => String::from("solutions depend on free parameters"),
        SolveResult::Partial { exact, rest } => format!(
            "exact: {}; numeric root-finding needed for {} = 0",
            exact
                .iter()
                .map(|r| r.to_string())
                .collect::<alloc::vec::Vec<_>>()
                .join(", "),
            rest.expr
        ),
    })
}

/// Renders an expression as LaTeX.
#[wasm_bindgen]
pub fn latex_text(src: &str) -> Result<String, JsValue> {
    let e = parse(src).map_err(fail)?;
    Ok(to_latex(&e))
}
