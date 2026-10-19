//! The shortest useful tour: parse, simplify, expand, differentiate, solve.
//!
//! Run with `cargo run --example quickstart`.

use tpt_symbolic::calculus::diff;
use tpt_symbolic::parse::parse;
use tpt_symbolic::simplify::{expand, simplify};
use tpt_symbolic::solve::{solve, SolveResult};

fn main() {
    // Parsing canonicalizes: equivalent spellings give the same expression.
    let messy = parse("3 + x*2 + x - 1 + x*x").expect("valid input");
    println!("parsed and canonical : {messy}");

    // Simplify reduces to the simplest exact form.
    let trig = parse("sin(x)^2 + cos(x)^2").expect("valid input");
    println!("sin^2 + cos^2        : {}", simplify(&trig));

    // Expand multiplies out powers of sums.
    let square = parse("(x + y)^2").expect("valid input");
    println!("expand (x+y)^2       : {}", expand(&square));

    // Exact derivative, simplified.
    let f = parse("x^3 + 2*x").expect("valid input");
    println!(
        "d/dx (x^3 + 2x)      : {}",
        diff(&f, "x").expect("differentiable")
    );

    // Exact roots: sqrt(2) stays symbolic, it is not rounded.
    let eq = parse("x^2 - 2 = 0").expect("valid input");
    if let SolveResult::Exact(roots) = solve(&eq, "x").expect("solvable form") {
        let shown: Vec<String> = roots.iter().map(|r| r.to_string()).collect();
        println!("solve x^2 - 2 = 0    : {}", shown.join(" , "));
    }
}
