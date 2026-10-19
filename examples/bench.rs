//! Rough timings on large expressions. Run with `cargo run --release --example bench`.
//!
//! This is a plain timing harness, not a statistical benchmark.

use std::time::Instant;
use tpt_symbolic::calculus::{diff, integrate};
use tpt_symbolic::simplify::{expand, simplify};
use tpt_symbolic::solve::solve;
use tpt_symbolic::{Expr, SymbolicFunc};

fn time<T>(label: &str, f: impl FnOnce() -> T) -> T {
    let start = Instant::now();
    let out = f();
    println!("{label:<36} {:>10.3?}", start.elapsed());
    out
}

fn main() {
    let x = Expr::var("x");
    let y = Expr::var("y");

    // A sum of 500 monomials with trigonometric wrapping.
    let terms: Vec<Expr> = (1..=500)
        .map(|k| {
            let mono = Expr::product(vec![Expr::int(k), Expr::power(x.clone(), Expr::int(k % 7))]);
            Expr::func(SymbolicFunc::Sin, mono)
        })
        .collect();
    let big = Expr::sum(terms);
    time("simplify: 500-term sum of sines", || simplify(&big));
    time("diff: 500-term sum of sines", || diff(&big, "x").unwrap());

    let poly = Expr::power(
        Expr::sum(vec![x.clone(), y.clone(), Expr::int(1)]),
        Expr::int(6),
    );
    time("expand: (x + y + 1)^6", || expand(&poly));

    let integrand = Expr::product(vec![
        x.clone(),
        Expr::func(SymbolicFunc::Cos, Expr::power(x.clone(), Expr::int(2))),
    ]);
    time("integrate: x*cos(x^2)", || integrate(&integrand, "x"));

    let deg8 = Expr::sum(
        (0..=8)
            .map(|k| Expr::power(x.clone(), Expr::int(k)))
            .collect(),
    );
    let _ = time("solve: degree-8 polynomial", || {
        solve(&Expr::equation(deg8, Expr::zero()), "x")
    });
}
