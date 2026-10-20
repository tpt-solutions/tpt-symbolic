//! Numeric checks of the exact cubic solutions in complex radicals.
//!
//! The closed forms are exact, and each root is evaluated here in `f64` against its
//! equation. This is a test-only check; the library itself stays exact.

mod common;

use common::{eval, poly_at};
use tpt_symbolic::parse::parse;
use tpt_symbolic::solve::{solve, solve_complex, SolveResult};
use tpt_symbolic::Expr;

/// `coeffs` are highest degree first. Every root must satisfy the polynomial numerically.
fn assert_roots_satisfy(roots: &[Expr], coeffs: &[f64]) {
    for r in roots {
        let z = eval(r, "x", (0.0, 0.0)).unwrap_or_else(|| panic!("cannot evaluate {r}"));
        let value = poly_at(coeffs, z);
        assert!(
            value.0.abs() < 1e-7 && value.1.abs() < 1e-7,
            "{r} gives {value:?}"
        );
    }
}

#[test]
fn three_real_roots_are_exact_in_complex_radicals() {
    // x^3 - 3x + 1 has three real roots (near -1.879, 0.347, 1.532).
    match solve(&parse("x^3 - 3*x + 1 = 0").unwrap(), "x").unwrap() {
        SolveResult::Exact(v) => {
            assert_eq!(v.len(), 3);
            assert_roots_satisfy(&v, &[1.0, 0.0, -3.0, 1.0]);
        }
        other => panic!("expected exact roots, got {other:?}"),
    }
}

#[test]
fn solve_complex_gives_all_three_roots_of_a_cubic() {
    // x^3 + x + 1: one real root and a complex pair.
    match solve_complex(&parse("x^3 + x + 1 = 0").unwrap(), "x").unwrap() {
        SolveResult::Exact(v) => {
            assert_eq!(v.len(), 3);
            assert_roots_satisfy(&v, &[1.0, 0.0, 1.0, 1.0]);
        }
        other => panic!("expected exact roots, got {other:?}"),
    }
    // x^3 - 1: the rational root 1 and the complex pair -1/2 ± i sqrt(3)/2.
    match solve_complex(&parse("x^3 - 1 = 0").unwrap(), "x").unwrap() {
        SolveResult::Exact(v) => {
            assert_eq!(v.len(), 3);
            assert_roots_satisfy(&v, &[1.0, 0.0, 0.0, -1.0]);
        }
        other => panic!("expected exact roots, got {other:?}"),
    }
}
