//! Solving single equations: exact roots when they exist, a clear hand-off otherwise.
//!
//! Run with `cargo run --example solve_equations`.

use tpt_symbolic::parse::parse;
use tpt_symbolic::solve::{solve, SolveResult};

fn main() {
    let cases = [
        ("2*x + 6 = 0", "x"),
        ("x^2 - 5*x + 6 = 0", "x"),
        ("x^3 - 2 = 0", "x"),
        ("x^4 - 2 = 0", "x"),
        ("x^2 + 1 = 0", "x"),
        // Circle of radius 5, solved for y. The roots carry x.
        ("x^2 + y^2 = 25", "y"),
        // No closed form here, so the solver hands it to numeric root-finding.
        ("x = cos(x)", "x"),
    ];
    for (equation, var) in cases {
        let eq = parse(equation).expect("valid input");
        let outcome = match solve(&eq, var).expect("solvable form") {
            SolveResult::Exact(roots) if roots.is_empty() => String::from("no solutions"),
            SolveResult::Exact(roots) => roots
                .iter()
                .map(|r| r.to_string())
                .collect::<Vec<_>>()
                .join(" , "),
            SolveResult::NoSolution => String::from("no real solution"),
            SolveResult::Numeric(root) => format!("numeric hand-off for {}", root.expr),
            SolveResult::Parametric => String::from("depends on free parameters"),
            SolveResult::Infinite => String::from("every value"),
            _ => String::from("unrecognised result"),
        };
        println!("{equation:<22} solve for {var}: {outcome}");
    }
}
