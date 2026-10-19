//! Derivatives, higher derivatives, gradients and integrals.
//!
//! Run with `cargo run --example calculus`.

use tpt_symbolic::calculus::{diff, diff_n, gradient, integrate, Integral};
use tpt_symbolic::parse::parse;

fn main() {
    let f = parse("sin(x^2)").expect("valid input");
    println!("f                   : {f}");
    println!(
        "f'                  : {}",
        diff(&f, "x").expect("differentiable")
    );
    println!(
        "f'' (second)        : {}",
        diff_n(&f, "x", 2).expect("differentiable")
    );

    // Partial derivatives: each treats the other names as constants.
    let g = parse("x^2*y + y^3").expect("valid input");
    let grad = gradient(&g, &["x", "y"]).expect("differentiable");
    println!("grad of {g}  : [{}, {}]", grad[0], grad[1]);

    // Integrals are checked by differentiating the answer.
    for src in ["2*x + 1", "exp(3*x)", "1/x", "asin(x)", "sin(x)*cos(x)"] {
        let e = parse(src).expect("valid input");
        match integrate(&e, "x") {
            Integral::Exact(antiderivative) => println!("int {src:<16}: {antiderivative} + C"),
            Integral::Unevaluated(_) => println!("int {src:<16}: no closed form found here"),
            _ => println!("int {src:<16}: unrecognised result"),
        }
    }
}
