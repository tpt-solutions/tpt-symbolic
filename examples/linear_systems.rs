//! Exact Gaussian elimination: unique solutions, free parameters, and null spaces.
//!
//! Run with `cargo run --example linear_systems`.

use tpt_symbolic::mathr::Number;
use tpt_symbolic::solve::{nullspace, solve_linear_system};

fn q(n: i128) -> Number {
    Number::from_int(n)
}

fn main() {
    // 2x + 3y = 8 and x - y = -1, so x = 1 and y = 2.
    let a = vec![vec![q(2), q(3)], vec![q(1), q(-1)]];
    let b = vec![q(8), q(-1)];
    println!(
        "unique system        : {:?}",
        solve_linear_system(&a, &b).expect("well-formed system")
    );

    // One equation in two unknowns leaves a free parameter.
    let under = vec![vec![q(1), q(1)]];
    let under_b = vec![q(4)];
    println!(
        "x + y = 4            : {:?}",
        solve_linear_system(&under, &under_b).expect("well-formed system")
    );

    // The null space of x + 2y + 3z = 0, one basis vector per free column.
    let m = vec![vec![q(1), q(2), q(3)]];
    println!(
        "null space of [1 2 3]: {:?}",
        nullspace(&m).expect("well-formed matrix")
    );
}
