# tpt-symbolic

Exact, pure-Rust symbolic mathematics for the TPT ecosystem. It builds
expressions as canonical trees, simplifies them, differentiates and integrates
them, and solves equations. Constants are exact rationals and no floating point
is used anywhere in the core.

- `no_std` + `alloc`. Builds for `wasm32-unknown-unknown` and `riscv32imac-unknown-none-elf`.
- Canonical on construction: `a + b` and `b + a` are the same tree.
- Zero C-FFI.

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
    Integral::Unevaluated(_) => unreachable!(),
}
```

## Features

| feature       | default | purpose                                                    |
|---------------|---------|------------------------------------------------------------|
| `stub-mathr`  | yes     | local exact-number shim, standing in for `tpt-mathr`       |
| `std`         | no      | `std::error::Error` impl for `Error`                       |
| `uir`         | no      | text IR codec (`UirCodec`), standing in for `tpt-uir`      |
| `testkit`     | no      | deterministic random-expression generator for property tests |

## Status

Implemented: canonical AST, parser and printer, prioritized rewrite engine,
`expand`, `diff`, verified `integrate` (power rule, linear arguments, simple
u-substitution), exact linear systems and nullspace, linear and quadratic
solvers.

Not yet implemented: cubic and quartic radical formulas (these return
`SolveResult::Numeric`), general integration beyond the rules above, and
the integrations with the other TPT crates. See `TODO.md`.

## Testing

```sh
cargo test --all-features
```

The design is in `spec.txt` and the work plan in `TODO.md`.

## Licence

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT licence](LICENSE-MIT), at your option.
