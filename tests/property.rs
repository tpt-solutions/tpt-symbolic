//! Property tests for the solver and the integrator, on random inputs.
//!
//! Each exact answer is checked numerically with the test-only evaluator in
//! `numeric_check`: a root must satisfy its equation, and an antiderivative's derivative
//! must match its integrand. The seeds are fixed, so failures reproduce.

use tpt_symbolic::calculus::{integrate, Integral};
mod common;
use common::{eval, near_zero, poly_at, poly_scale, Lcg, C};
use tpt_symbolic::parse::parse;
use tpt_symbolic::solve::{solve, SolveResult};

/// Random polynomial of degree `deg` with small integer coefficients, highest first.
/// The leading coefficient is non-zero.
fn random_poly(g: &mut Lcg, deg: usize) -> Vec<i64> {
    let mut c = Vec::with_capacity(deg + 1);
    c.push(g.range(1, 3));
    for _ in 0..deg {
        c.push(g.range(-6, 6));
    }
    c
}

fn poly_text(c: &[i64]) -> String {
    let deg = c.len() - 1;
    let mut parts = Vec::new();
    for (i, k) in c.iter().enumerate() {
        let power = deg - i;
        if *k == 0 {
            continue;
        }
        let term = match power {
            0 => format!("({k})"),
            1 => format!("({k})*x"),
            _ => format!("({k})*x^{power}"),
        };
        parts.push(term);
    }
    if parts.is_empty() {
        return String::from("0");
    }
    parts.join(" + ")
}

#[test]
fn exact_roots_satisfy_random_polynomial_equations() {
    let mut checked = 0;
    let mut cubics_checked = 0;
    for seed in 1..=400u64 {
        let mut g = Lcg::new(seed);
        let deg = if seed % 2 == 0 { 3 } else { 4 };
        let coeffs = random_poly(&mut g, deg);
        let text = format!("{} = 0", poly_text(&coeffs));
        let eq = parse(&text).unwrap();
        let SolveResult::Exact(roots) = solve(&eq, "x").unwrap() else {
            continue;
        };
        let f64s: Vec<f64> = coeffs.iter().map(|c| *c as f64).collect();
        for r in &roots {
            let z: C = eval(r, "x", (0.0, 0.0))
                .unwrap_or_else(|| panic!("cannot evaluate root {r} of {text}"));
            let value = poly_at(&f64s, z);
            let scale = poly_scale(&f64s, z);
            assert!(
                near_zero(value, scale),
                "{text}: root {r} = {z:?} gives {value:?}"
            );
        }
        checked += 1;
        if deg == 3 {
            cubics_checked += 1;
        }
    }
    // Every cubic is exact, so a meaningful share of the cases must have been checked.
    assert!(
        cubics_checked >= 150,
        "only {cubics_checked} cubics were checked"
    );
    assert!(checked >= 150, "only {checked} cases were checked");
}

/// Random rational function text, and the function's value check points.
fn random_rational(g: &mut Lcg) -> String {
    let deg = g.range(0, 2) as usize;
    let num = random_poly(g, deg);
    let den = match g.range(0, 3) {
        0 => {
            let a = g.range(-2, 2);
            format!("(x - ({a}))")
        }
        1 => {
            // Irreducible quadratic: discriminant b^2 - 4c is negative.
            let b = g.range(-2, 2);
            let c = g.range(b * b / 4 + 1, 6);
            format!("(x^2 + ({b})*x + ({c}))")
        }
        2 => {
            let a = g.range(-2, 2);
            format!("((x - ({a}))*(x^2 + 1))")
        }
        _ => {
            let a = g.range(-2, 2);
            format!("((x - ({a}))^2)")
        }
    };
    format!("({})/{}", poly_text(&num), den)
}

#[test]
fn antiderivatives_of_random_rational_functions_differentiate_back() {
    let points = [0.37, 1.3, -2.1, 2.7, -0.6];
    let mut checked = 0;
    for seed in 1..=200u64 {
        let mut g = Lcg::new(seed);
        let text = random_rational(&mut g);
        let e = parse(&text).unwrap();
        let Integral::Exact(f) = integrate(&e, "x") else {
            continue;
        };
        let h = 1e-5;
        for x0 in points {
            let Some(fx) = eval(&e, "x", (x0, 0.0)) else {
                continue;
            };
            let Some(plus) = eval(&f, "x", (x0 + h, 0.0)) else {
                continue;
            };
            let Some(minus) = eval(&f, "x", (x0 - h, 0.0)) else {
                continue;
            };
            // The antiderivative is real on the real line, so its imaginary part must vanish.
            assert!(
                plus.1.abs() < 1e-9 && minus.1.abs() < 1e-9,
                "{text}: antiderivative is complex at {x0}"
            );
            let derivative = (plus.0 - minus.0) / (2.0 * h);
            let scale = 1.0 + fx.0.abs();
            assert!(
                (derivative - fx.0).abs() <= 1e-5 * scale,
                "{text}: F'({x0}) = {derivative}, integrand = {}",
                fx.0
            );
            checked += 1;
        }
    }
    assert!(checked >= 300, "only {checked} point checks were made");
}
