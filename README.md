# tpt-symbolic

Exact, pure-Rust symbolic mathematics for the TPT ecosystem. It builds
expressions as canonical trees, simplifies them, differentiates and integrates
them, solves equations, and evaluates them exactly at rational points.

- No floating point anywhere. Constants are exact rationals of any size.
- `no_std` + `alloc`. Builds for `wasm32-unknown-unknown` and `riscv32imac-unknown-none-elf`.
- Canonical on construction: `a + b` and `b + a` are the same tree.
- Zero dependencies and zero C-FFI.

## Quickstart

Add it to `Cargo.toml` (it is not on crates.io yet, so use a path or git dependency):

```toml
[dependencies]
tpt-symbolic = { path = "../tpt-symbolic" }
```

```rust
use tpt_symbolic::calculus::diff;
use tpt_symbolic::parse::parse;
use tpt_symbolic::simplify::simplify;

let e = parse("sin(x)^2 + cos(x)^2").unwrap();
assert_eq!(simplify(&e).to_string(), "1");

let f = parse("x^3 + 2*x").unwrap();
assert_eq!(diff(&f, "x").unwrap().to_string(), "2 + 3*x^2");
```

Run the full tour with `cargo run --example quickstart`.

## Command line

A small `tpt-sym` binary is included:

```sh
cargo run -- simplify "x*x + x*x"       # 2*x^2
cargo run -- diff "x^3" x               # 3*x^2
cargo run -- integrate "2*x" x          # x^2 + C
cargo run -- solve "x^2 - 2 = 0" x      # -2^(1/2), 2^(1/2)
cargo run -- eval "x^2 + 1" x=1/2       # 5/4
cargo run -- decimal "1/3" 6            # 0.333333
cargo run -- latex "x/2"                # \frac{x}{2}
cargo run -- factor "x^4 - 5*x^2 + 6" x # (-2 + x^2) * (-3 + x^2)
cargo run -- apart "1" "x^2 - 1" x      # -1/2*(x + 1)^(-1) + 1/2*(x - 1)^(-1)
cargo run -- limit "sin(x)/x" x 0       # 1
cargo run -- taylor "exp(x)" x 0 3      # x + 1 + 1/2*x^2 + 1/6*x^3
cargo run -- definite "x^2" x 0 1       # 1/3
cargo run -- trace "sin(x)^2 + cos(x)^2" # pythagorean-identity: ... -> 1, then result: 1
```

## Syntax

| you write               | meaning                                         |
|-------------------------|-------------------------------------------------|
| `2*x`, `2x`, `x y`      | product (juxtaposition multiplies)             |
| `x^2`, `x**2`           | power, right-associative: `2^3^2` is `2^(3^2)`  |
| `x/2`, `0.25`           | exact rational: `0.25` is `1/4`, never a float  |
| `a + b`, `a - b`, `-a`  | sum, difference, negation                       |
| `a = b`                 | equation (used by `solve`)                      |
| `and`, `or`, `not`      | logical connectives                             |
| `true`, `false`         | logical constants                               |
| `sin(x)`, `log(x)`, ... | built-in functions: `sin cos tan asin acos atan sinh cosh tanh exp log abs` |
| `sqrt(x)`               | same as `x^(1/2)`                               |
| `pi`, `e`               | the constants π and Euler's number (not variables; `eval` refuses them) |

A name followed directly by `(` must be a built-in function. Anything else is
an error, so `foo(x)` is rejected rather than read as `foo*x`. A likely typo
such as `sni(x)` also gets a "did you mean" suggestion. Names are ASCII
identifiers, and a multi-letter name is one variable: `xy` is not `x*y`.

## Exact and approximate

| operation                        | result                                           |
|----------------------------------|--------------------------------------------------|
| parsing, printing, `simplify`    | exact                                            |
| `diff`, `integrate`              | exact; each antiderivative is checked by differentiating it |
| `eval` at rational points        | exact rational, or `Unsupported` when irrational |
| `to_decimal(n, digits)`          | decimal digits, **truncated** (not rounded)      |
| `solve` (degree 1, 2, rational roots, `x^n = c`) | exact radicals such as `2^(1/3)`  |
| `solve` (anything else)          | `SolveResult::Numeric`, a hand-off for numeric root-finding |
| `solve` (some rational roots)    | `SolveResult::Partial`: the exact roots, plus a numeric hand-off for the rest |
| `solve` (no unknown in the equation) | `SolveResult::Conditional`, the expression that must vanish |

Nothing rounds on its own. An irrational value stays symbolic until you ask for
digits, and then only the digits you ask for are produced.

## Limits and assumptions

- **Real numbers only.** Discriminants and radicands are assumed non-negative
  when they are symbolic. `x*x^-1` simplifies to `1` without checking `x != 0`.
- **Products of sums expand.** `(x + 1)/(x - 1)` is canonicalised to
  `x*(x - 1)^(-1) + (x - 1)^(-1)`. The constructors always expand, so use
  `factor` to see factors. `Factorization::describe` prints them separately.
- **Constants.** `pi`, `e` and `i` are constants, not variables. `eval` refuses them,
  because they have no exact rational value.
- **Solver scope.** Exact answers exist for degree 1 and 2 (including complex roots
  through `solve_complex`), for polynomials with rational roots, for `x^n = c`, for
  biquadratics, for cubics with one real root (Cardano), and for quartics whose resolvent
  cubic has a positive rational root (Ferrari). Three real roots of a cubic need complex
  intermediates and are reported as numeric. Other leftovers use `SymbolicRoot::isolate`
  for rigorous brackets.
- **Factoring.** Complete over the rationals for degree up to 4, and marked incomplete
  beyond that. Partial fractions need a completely factored denominator.
- **Integration scope.** Power rule, linear arguments, basic trig and exponential forms,
  `1/x`, `asin`, `acos`, `atan`, simple u-substitution, integration by parts for
  `x^n * g(x)`, and expansion when the direct form fails. Definite integrals are
  conservative: they are returned only for integrands with no pole, log, abs or
  fractional power, so a divergent integral is never reported as a number.
- **Limits.** Direct substitution, L'Hôpital for `0/0` quotients, and degree comparison
  at infinity. Other limits are unevaluated, never guessed.
- **Assumptions.** The default simplifier combines `x*x^-1` into `1` (that is,
  it assumes `x != 0`). `simplify_assuming` applies `abs` and `log` rules only for
  variables declared positive.
- **Matrices.** Exact for numeric entries at any size. Symbolic determinants and
  inverses are limited to size 7. A symbolic inverse divides by the determinant, and
  the determinant is assumed non-zero.
- **Nesting limit.** The parser accepts up to 128 levels of nesting and reports
  deeper input as an error.

## Features

| feature       | default | purpose                                                    |
|---------------|---------|------------------------------------------------------------|
| `std`         | no      | `std::error::Error` impl for `Error`                       |
| `uir`         | no      | text IR codec (`UirCodec`), standing in for `tpt-uir`      |
| `testkit`     | no      | deterministic random-expression generator for property tests |

## Examples

| example               | shows                                                  |
|-----------------------|--------------------------------------------------------|
| `quickstart`          | parse, simplify, expand, differentiate, solve          |
| `calculus`            | derivatives, gradients, verified integrals             |
| `solve_equations`     | exact roots, no-solution cases, numeric hand-off       |
| `linear_systems`      | unique, parametric and null-space results              |
| `balance_chemistry`   | a balanced reaction from an integer null space         |
| `exact_numbers`       | big integers, evaluation, truncated decimals           |
| `parse_and_print`     | syntax, round trips, LaTeX, error messages             |
| `bench`               | timing on large expressions                            |

Run any of them with `cargo run --example <name>`.

## Pipeline example

```rust
use tpt_symbolic::calculus::{integrate, Integral};
use tpt_symbolic::parse::parse;
use tpt_symbolic::simplify::simplify;
use tpt_symbolic::solve::{solve, SolveResult};
use tpt_symbolic::Expr;

// Simplify with trigonometric identities.
let e = parse("sin(x)^2 + cos(x)^2").unwrap();
assert_eq!(simplify(&e), Expr::one());

// Solve a circle constraint for y: y = ±sqrt(25 - x^2).
let circle = parse("x^2 + y^2 = 25").unwrap();
if let SolveResult::Exact(roots) = solve(&circle, "y").unwrap() {
    assert_eq!(roots.len(), 2);
}

// Integrate with verification. Unsupported forms return Unevaluated.
match integrate(&parse("x*cos(x^2)").unwrap(), "x") {
    Integral::Exact(f) => assert_eq!(f, parse("1/2*sin(x^2)").unwrap()),
    _ => unreachable!("the README example integral is supported"),
}
```

## Status

Implemented: canonical AST, parser and printer, prioritised rewrite engine with a
rule trace, `expand`, `diff` (higher derivatives and gradients), verified and
definite `integrate`, exact evaluation and truncated decimals, LaTeX output, exact
linear systems and nullspace, the solvers described above, factoring, partial fractions,
symbolic matrices, Taylor series and limits, complex numbers (the unit `i`), assumptions
on variables, interval enclosures with root isolation, certificates that re-check a
result, a numeric program export for `tpt-math`, and a WebAssembly build (`playground/`).

Not yet implemented: the integrations with other TPT crates, release automation, and
the publishing decision for the git dependency. See `TODO.md`.

## Testing

```sh
cargo test --all-features
```

The design is in `spec.txt` and the work plan in `TODO.md`. Contributors should
read `CONTRIBUTING.md` first.

## Licence

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT licence](LICENSE-MIT), at your option.
