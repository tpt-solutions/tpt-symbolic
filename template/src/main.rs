//! Starter program for an application that uses tpt-symbolic.
//!
//! Copy this `template/` directory, change the dependency in `Cargo.toml`, and
//! replace the body of `main` with your own problem.

use tpt_symbolic::calculus::diff;
use tpt_symbolic::eval::{eval, to_decimal};
use tpt_symbolic::parse::parse;
use tpt_symbolic::simplify::simplify;
use tpt_symbolic::Number;

fn main() {
    // 1. Describe the model as text. Errors say where the problem is.
    let model = match parse("x^2 + 3*x + 1/2") {
        Ok(e) => e,
        Err(err) => {
            eprintln!("bad model: {err}");
            std::process::exit(1);
        }
    };

    // 2. Work with it symbolically.
    let slope = simplify(&diff(&model, "x").expect("polynomial is differentiable"));
    println!("model : {model}");
    println!("slope : {slope}");

    // 3. Evaluate at exact points, and print decimals only when you need them.
    for (num, den) in [(1, 2), (-3, 4)] {
        let x = Number::new(num, den).expect("non-zero denominator");
        let value = eval(&model, &[("x", x)]).expect("polynomial at a rational point");
        println!("x = {num}/{den}: exact {value}, about {}", to_decimal(&value, 6));
    }
}
