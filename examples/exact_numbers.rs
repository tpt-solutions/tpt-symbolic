//! Exact arbitrary-precision numbers: no floats anywhere.
//!
//! Run with `cargo run --example exact_numbers`.

use tpt_symbolic::eval::{eval, subs, to_decimal};
use tpt_symbolic::parse::parse;
use tpt_symbolic::Number;

fn main() {
    // Exact integers stay exact however large they get.
    let big = parse("2^200").expect("valid input");
    println!("2^200 exactly        : {big}");

    // Evaluate at a rational point, then print decimals truncated to the digits asked for.
    let f = parse("x^2 - x/3").expect("valid input");
    let x = Number::new(2, 3).expect("non-zero denominator");
    let value = eval(&f, &[("x", x)]).expect("rational point");
    println!("f(2/3) exactly       : {value}");
    println!("f(2/3) to 8 places   : {}", to_decimal(&value, 8));

    // Substitution keeps the expression symbolic until you evaluate it.
    let shifted = subs(&f, "x", &parse("y + 1").expect("valid input"));
    println!("f(y + 1)             : {shifted}");

    // Decimal input is exact too: 0.1 is 1/10, not the nearest binary float.
    let sum = parse("0.1 + 0.2").expect("valid input");
    println!(
        "0.1 + 0.2 == 3/10    : {}",
        sum == parse("3/10").expect("valid input")
    );
}
