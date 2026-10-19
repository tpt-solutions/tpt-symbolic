# Changelog

All notable changes to this project are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project will
use semantic versioning once v1.0.0 is released.

## Unreleased

### Added
- `tpt-sym` command-line tool: `simplify`, `diff`, `integrate`, `solve`, `eval`, `decimal`, `latex`.
- `eval` module: exact evaluation at rational points, `subs`, and `to_decimal` (truncated decimal output).
- `latex` module: `to_latex` for math-mode output with fractions, roots and grouping.
- `ops` impls on `Expr`: `+ - * / -x`, plus `From<i64>` and `From<Number>`.
- `sqrt(x)` parses as `x^(1/2)`.
- `diff_n` (n-th derivative) and `gradient` (partial derivatives).
- Integration of `asin`, `acos` and `atan` with a linear argument.
- `simplify_checked`, which reports non-convergence and over-deep trees instead of returning a partial result.
- `Number::cmp_value`, the numeric order (the derived `Ord` is structural).
- Exact real roots of pure power equations such as `x^4 - 2 = 0`.
- Examples: `quickstart`, `calculus`, `solve_equations`, `linear_systems`, `balance_chemistry`, `exact_numbers`, `parse_and_print`.
- `docs/cookbook.md`, with each recipe checked as a doctest.
- `CONTRIBUTING.md` with a walkthrough for adding a simplification rule.

- `Expr::Pi` and `Expr::E` (the constants π and Euler's number). They are appended, so the existing variant order is unchanged.
- `SolveResult::Partial { exact, rest }`: exact roots, plus a numeric hand-off for a remaining factor with no closed form.
- `SolveResult::Conditional(expr)`: an equation with no unknown holds when `expr` is zero.
- `Error::UnknownFunction { pos, name, suggestion }`, with a "did you mean" suggestion for likely typos such as `sni(x)`.
- `error::line_col(src, pos)`: the 1-based line and column of a byte offset.
- Integration by parts for `x^n · g(x)` where `g` has a basic antiderivative, such as `x*sin(x)` and `x^2*exp(x)`.
- Integration of `tanh`, with a simplifier rule `sinh(x)*cosh(x)^-1` → `tanh(x)`.
- Biquadratic equations `a·x^4 + b·x^2 + c = 0` have exact real roots.

- `factor`: rational roots, quartics into integer quadratics, and marked incomplete beyond degree 4. `Factorization::describe` prints the factors separately.
- `apart::partial_fractions`: exact decomposition over the rationals, checked at sample points.
- `factor::gcd`: polynomial gcd over the rationals.
- `limits`: `taylor` (exact series), `limit` and `limit_at_infinity` (direct substitution, L'Hôpital, degree comparison).
- `matrix::Matrix`: determinant, inverse, product, transpose, and a linear solve, exact for numeric entries and cofactor-based for symbolic ones.
- `calculus::definite_integral`, conservative: only regular integrands are evaluated. `integrate` tries the expansion when the direct form fails.
- `simplify::simplify_trace`: the rule applications that produced a result.
- Imaginary unit `i` (`Expr::I`), `solve_complex` (complex roots of quadratics), and `real_part`, `imag_part` and `conjugate`.
- Rational integration by partial fractions, with `log|…|` and `atan` terms, checked at exact points.
- `x^n * atan(x)` by parts, with the rational remainder checked at exact points.
- Power folding: `c^(-1/2)` for perfect-square constants, and multi-factor quotients in limits and integration.
- Exact `sin` and `cos` at multiples of pi/12 (the `special-angle` rule).
- Integration of `e^(ax)·sin(bx)` and `e^(ax)·cos(bx)` in closed form, and integration by parts with the logarithm differentiated (`x^2*log(x)`).
- Doc examples for the new public functions.
- Radical solutions: Cardano for cubics with one real root, and Ferrari for quartics whose resolvent cubic has a positive rational root.
- Equations of the form `exp(u) = c`, `log(u) = c`, `1/u = c`, and `u^n = c` are inverted, and the inner equation is solved exactly.
- `assume::simplify_assuming`: `abs` and `log(abs)` rules for variables declared positive.
- `interval`: exact rational interval enclosures for polynomial and rational expressions, root exclusion, and root isolation. `SymbolicRoot::isolate` uses it on numeric hand-offs.
- `certificate::Certificate`: checkable records for roots, antiderivatives, factorisations and root-free intervals.
- `program::compile` and `run_exact`: a postfix program for numeric back ends such as `tpt-math`, with no floating-point values in the core.
- `wasm` feature (`src/wasm.rs`) and a browser page in `playground/`.
- CLI commands: `factor`, `apart`, `limit`, `taylor`, `definite`, `trace`.

### Changed
- `i` parses as the imaginary unit, not a variable, as `e` and `pi` do.
- `eval` refuses `pi`, `e` and `i`, which have no exact rational value.
- The `wasm` feature is off by default, and the `no_std` targets are unaffected.
- Exact arithmetic now uses `tpt-mathr-arith` (the `no_std` arithmetic crate of tpt-mathr, pinned to git revision `f736d3e`). Breaking: `Number::numer()` and `Number::denom()` return an owned `BigInt` instead of a reference. The `stub-mathr` feature and its compile gate are removed. Git dependencies cannot be published to crates.io, so this must be resolved before any release. On `examples/bench.rs`, arithmetic is faster than the old stub after the integer fast paths.
- `pi` and `e` parse as constants, not variables. `eval` returns `Unsupported` for them, and `diff` treats them as constants.
- `solve` on an equation with no unknown returns `SolveResult::Conditional` instead of `Parametric`. `Parametric` now means free unknowns in linear systems only.
- Number literals of any length parse exactly. Previously literals over about 38 digits were rejected, so large printed constants did not reparse.
- A name followed directly by `(` that is not a built-in function is a parse error. Before, it was read as a product, so `foo(x)` became `foo*x`.
- Parsing rejects input nested deeper than 128 levels instead of risking a stack overflow.
- `diff` rejects boolean and equation operands with `Unsupported`. `Bool` used to differentiate to `0`.
- `solve` rejects logical connectives (`and`, `or`, `not`) with `Unsupported`.
- `Expr` and `SymbolicFunc` are `#[non_exhaustive]`.

### Fixed
- `nullspace` returns an error for ragged input instead of panicking with an index out of bounds.
- Perfect squares above the 10^4 trial-division bound stayed inside the root: `sqrt(10007^2)` did not fold to `10007`.
- `sqrt(2)` is now a square root. It used to parse as `2*sqrt`, a product with a variable named `sqrt`.
- Number literals and bignum constants round-trip through the printer, as described under Changed.
- Hardening, with no wrong answer seen in testing: the rational-root search is guarded against coefficients of 2^127 or more, `negate_term` no longer indexes into an empty product, expansion checks its size before allocating, and the integration substitution symbol is chosen so it cannot clash with a name in the integrand.
- Stale comments in `bigint.rs` and `expr.rs` describing the old division algorithm and overflow behaviour.
