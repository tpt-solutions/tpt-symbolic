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

### Changed
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
