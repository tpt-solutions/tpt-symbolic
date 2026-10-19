//! Balancing a chemical equation as an integer null-space problem.
//!
//! Each element gives one row of a matrix: its atom count in each species, with
//! products negative. The balanced coefficients form the smallest positive
//! integer vector in the null space.
//!
//! Run with `cargo run --example balance_chemistry`.

use tpt_symbolic::mathr::Number;
use tpt_symbolic::solve::{nullspace, primitive_integer_vector};

fn q(n: i128) -> Number {
    Number::from_int(n)
}

fn main() {
    // C3H8 + O2 -> CO2 + H2O, columns in the order [C3H8, O2, CO2, H2O].
    let species = ["C3H8", "O2", "CO2", "H2O"];
    let matrix = vec![
        vec![q(3), q(0), q(-1), q(0)],  // carbon
        vec![q(8), q(0), q(0), q(-2)],  // hydrogen
        vec![q(0), q(2), q(-2), q(-1)], // oxygen
    ];

    let basis = nullspace(&matrix).expect("well-formed matrix");
    assert_eq!(basis.len(), 1, "a balanced reaction has one free direction");
    let coefficients = primitive_integer_vector(&basis[0]).expect("integer scaling");

    let lhs: Vec<String> = (0..2)
        .map(|i| format!("{} {}", coefficients[i], species[i]))
        .collect();
    let rhs: Vec<String> = (2..4)
        .map(|i| format!("{} {}", coefficients[i], species[i]))
        .collect();
    println!("{} -> {}", lhs.join(" + "), rhs.join(" + "));
}
