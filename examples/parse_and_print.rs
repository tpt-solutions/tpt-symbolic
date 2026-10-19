//! Reading and writing expressions: the text syntax, plain output, and LaTeX.
//!
//! Run with `cargo run --example parse_and_print`.

use tpt_symbolic::latex::to_latex;
use tpt_symbolic::parse::parse;

fn main() {
    // Equivalent spellings parse to the same expression.
    for s in ["x**2 + 2*x*y", "x^2+2*x*y", "2*x*y + x^2"] {
        println!("{s:<14} -> {}", parse(s).expect("valid input"));
    }

    // Output parses back to the same expression, so it is safe to store and reload.
    let e = parse("(x + 1)/(x - 1)").expect("valid input");
    let printed = e.to_string();
    assert_eq!(parse(&printed).expect("round trip"), e);
    println!("round trip ok        : {printed}");

    // LaTeX for documents and notebooks.
    let latex = to_latex(&parse("sqrt(x)/2").expect("valid input"));
    println!("latex                : {latex}");

    // Errors point at the problem.
    match parse("sqrt(2) + foo(x)") {
        Ok(e) => println!("unexpected success   : {e}"),
        Err(err) => println!("error                : {err}"),
    }
}
