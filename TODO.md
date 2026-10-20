# tpt-symbolic — TODO

Source of truth: `spec.txt`. Tick boxes as you go.

## Decisions (confirmed)
- Deps (`tpt-mathr`, `tpt-uir`, `tpt-testkit`): **local stubs/traits** now; swap for real crates later.
- AST: **extend `Expr`** with `Eq`, boolean nodes (`And/Or/Not`, `Bool` consts); Complex comes via `Number`.
- Scope now: **checklist only**; no code until you approve.
- Solver output: **enum result** — `Exact(Vec<Expr>)`, `Parametric`, `Numeric(SymbolicRoot)`, `NoSolution`, `Infinite` (recommended, not yet confirmed by you).

## Phase 0 — Project setup
- [x] Create Cargo crate `tpt-symbolic` (`#![no_std]` + `extern crate alloc`)
- [x] Feature flags: `std` (off by default), `uir`, `testkit` (`stub-mathr` was removed by the tpt-mathr swap)
- [x] Module skeleton: `expr`, `simplify`, `calculus`, `solve`
- [x] Error type (no_std-friendly, no `std::error::Error` unless `std`)
- [x] Confirm builds for `wasm32-unknown-unknown` and a bare-metal target (e.g. `riscv32imac-unknown-none-elf`)
- [x] Enforce: zero C-FFI, zero floats in core AST (CI grep/clippy lint for `f32`/`f64`)

## Phase 0b — Dependency stubs
- [x] `mathr` shim: `Number` (arbitrary-precision rationals over BigInt; Complex not yet), arithmetic, `Ord`, `Hash`, `Display`
- [x] Define the trait surface we need from real tpt-mathr so the swap is mechanical (`mathr::ExactNumber`, implemented by `Number`)
- [x] `uir` shim: serialize/deserialize trait for `Expr`
- [x] `testkit` shim: property-test + golden-file helpers (or temporary use of a public crate in dev-deps only)

## Phase 1 — AST & Canonicalization (Weeks 1–3)
- [x] `Expr` enum: `Var(Arc<str>)`, `Const(Number)`, `Add`, `Mul`, `Pow`, `Func`, `Eq`, `And`, `Or`, `Not`, `Bool`
- [x] `SymbolicFunc` enum (Sin, Cos, Tan, Log, Exp, Sqrt?, … — decide list)
- [x] Smart constructors (`Expr::add`, `mul`, `pow`, …) so un-canonical nodes can't be built
- [x] Total ordering on `Expr` (variable name, then complexity) for deterministic sort
- [x] Flatten nested Add/Mul; fold constants (`2 + x + 3` → `x + 5`)
- [x] Identity handling in constructors (empty Add → 0, single-child unwrap, etc.)
- [x] Combine like terms / repeated factors policy (decide: canonical vs. simplify stage)
- [x] Subtraction/division/negation representation (via `Add`/`Mul`/`Pow(-1)`)
- [x] String parser (precedence, implicit multiplication?, functions, `=`)
- [x] Pretty printer (round-trips with parser)
- [x] `tpt-uir` serialization / deserialization
- [x] Tests: `a+b == b+a`, idempotent canonicalization, 1/3+1/3+1/3 == 1
- [x] Golden files: thousands of equivalent-syntax inputs → same AST (1000 random expressions × 5 spellings in `parse::tests`, plus a hand-written corpus)
- [x] **Milestone:** parse, store, perfectly canonicalize expressions

## Phase 2 — Simplification Engine (Weeks 4–6)
- [x] Pattern matching: implemented as per-rule matchers (`identity_pair`, `square_arg`), not a general pattern language; general commutative matcher deferred
- [x] Rule struct with priority and category
- [x] Algebraic rules: `x*0→0`, `x^1→x`, `x^0→1`, `x*x^n→x^(n+1)`, like-term collection
- [x] `expand` (e.g. `(x+y)^2` → `x^2+2xy+y^2`)
- [x] Trig rules: `sin²+cos²→1` and friends (ordered to avoid loops)
- [x] Log/exp rules
- [x] Boolean rules: absorption, De Morgan, idempotence, complement, double negation
- [x] Fixed-point driver with strict depth/iteration limit
- [x] Termination argument documented (rule ordering / measure that strictly decreases)
- [x] Simplify is idempotent: `simplify(simplify(e)) == simplify(e)`
- [x] Tests + fuzz (seeded random, deeply nested exprs terminate; time-limited fuzzing deferred to cross-cutting)
- [x] **Milestone:** reduce messy expressions to simplest exact form

## Phase 3 — Calculus (Weeks 7–9)
- [x] `diff`: constants, vars, sum, product, quotient, power, chain rule
- [x] Derivatives of all `SymbolicFunc` members
- [x] Route `diff` output through `simplify` (`d/dx sin(x²)` = `2x·cos(x²)`)
- [x] `integrate`: power rule (incl. `x^-1` → log), linear combos, constants
- [x] Basic trig / exp integrals
- [x] Simple u-substitution detection
- [x] Return "unevaluated" result when integration fails (no panic, no wrong answer)
- [x] Property tests: `diff(integrate(f)) == f`
- [x] **Milestone:** exact derivatives/integrals for common physics/engineering cases

## Phase 4 — Solvers & Integration (Weeks 10–12)
- [x] `SolveResult` enum (Exact / Parametric / Numeric(SymbolicRoot) / NoSolution / Infinite)
- [x] Linear systems: exact Gaussian elimination over `Number`
- [x] Polynomial detection + degree/coefficient extraction
- [x] Solvers: linear, quadratic, cubic, quartic (radical forms). Done: linear, quadratic, rational roots with deflation, pure powers, biquadratics, Cardano for cubics (real radicals with one real root, complex radicals for three real roots), Ferrari for quartics whose resolvent cubic has a positive rational root. Not done: quartics with no positive rational resolvent root stay numeric and can be bracketed with `SymbolicRoot::isolate`.
- [x] Equation isolation for single variable (`x²+y²=25` for `y` → `±sqrt(25-x²)`)
- [x] `SymbolicRoot` for transcendental equations (Newton–Raphson hand-off to tpt-math)
- [x] Test: `x²-2=0` → `sqrt(2)`; 10,000-digit check via exact integer sqrt (`bigint::tests::sqrt2_to_ten_thousand_decimal_places`), not tpt-mathr
- [-] tpt-vertex integration: dropped by the user; not planned
- [-] tpt-chemistry integration: dropped by the user; not planned
- [-] tpt-concord integration: dropped by the user; not planned
- [x] tpt-math hand-off: `program::compile` gives a postfix program with no floating-point values, and `run_exact` checks it against exact arithmetic.
- [x] Docs, examples, API review — README and pipeline doctest done; API review done (growable enums marked `#[non_exhaustive]`)
- [ ] Publish v1.0.0 — excluded by you (not published)

## Cross-cutting quality
- [x] Property tests for identities (via testkit)
- [x] Fuzz targets for parser and simplifier with time limits (`tests/fuzz.rs`, 2000 deep inputs each, 5 s per-case budget)
- [x] Benchmarks on large expressions (`examples/bench.rs`, plain timing harness; not statistical)
- [x] `no_std` CI matrix (native, wasm32, riscv)
- [x] Rustdoc for every public item; README with pipeline example

## Proposed decisions (recommended; awaiting your OK)
- [x] `SymbolicFunc`: Sin, Cos, Tan, Asin, Acos, Atan, Sinh, Cosh, Tanh, Exp, Log (natural), Abs. No Sqrt (use `Pow(x, 1/2)`).
- [x] Like-term/like-factor collection (`x+x→2x`, `x*x→x^2`) happens in canonicalization. Products of sums expand automatically (up to 4096 terms) so the canonical form does not depend on build order; powers of sums and trig identities stay in `simplify`/`expand`.
- [x] Parser: `^` (accept `**`), implicit multiplication, `=`, `and`/`or`/`not`, ASCII only.
- [x] Rust edition 2021; MSRV = current stable; licence MIT OR Apache-2.0 (match other TPT repos if different).
- [x] Other TPT repos are on GitHub: use stubs now, switch to git dependencies once URLs are available.

## Still needed from you
- [x] GitHub URLs for tpt-mathr, tpt-uir, tpt-testkit (all under `github.com/tpt-solutions/`; tpt-math is there too). `repository` and `documentation` are set in `Cargo.toml`
- [x] Swap the stubs for the real crates. `Number` now wraps `tpt-mathr-arith`'s `BigRat` (pinned git revision `f736d3e`). The `no_std` arithmetic crate is used directly, because the facade `tpt-mathr` is std-only and does not build for riscv. `numer()` and `denom()` now return owned values of our `BigInt` (breaking, allowed while unpublished). The `stub-mathr` feature and its compile gate are gone. Not swapped, by decision: `tpt-testkit` (needs Rust 1.97, above the 1.87 MSRV) and `tpt-uir` (its IR is not our `Expr`). Performance: the first swap was 1.4x to 2x slower than the stub. Caching the integer conversions (`i64` fast paths for `numer`, `denom`, `new`, `from_parts`, `as_i128`, `is_integer`, `is_one`) brought it below the stub: `examples/bench.rs` in release now gives diff of the 500-term sum 24 ms (stub 36 ms), simplify 5.8 ms (stub 8.8 ms), expand 5.5 ms (stub 7.9 ms), solve 0.76 ms (stub 1.0 ms). Git dependencies block crates.io publishing, so this must be settled before any publish. tpt-vertex, tpt-chemistry and tpt-concord URLs are still missing
- [x] Licence used by the other TPT repos (MIT OR Apache-2.0; LICENSE-MIT and LICENSE-APACHE added)
- [x] Decisions for the review items marked "decision" below: the conservative option was taken for each, so no public API change and no new dependency. Partial solver roots, the `Parametric` split, `pi`/`e` constants, and `serde` remain open for later work

## Review follow-ups (platform review, 2026-10-19)
Findings came from code inspection. The bugs that were reproduced before the fix
are: big literals not reparsing, `sqrt(2)` and `foo(x)` misparsing, deep nesting
overflowing the stack, `nullspace` panicking on ragged input, and `sqrt(10007^2)`
not folding. Items marked "hardening" were not reproduced as bugs. Each done item
has a test or was checked by hand.

### Tier 1 — Bugs
- [x] Parser: number literals accumulate in `BigInt`, so large printed constants reparse (`2^200` round-trips)
- [x] Parser: `foo(x)` (unknown name directly followed by `(`) is an error; `sqrt(x)` is sugar for `x^(1/2)`
  - Decision taken: `2e3` stays `2*e3`, because `2x` must stay valid. `pi` and `e` are plain variables until the Tier 2 constants land.
- [x] Parser depth limit (128 levels). Drop, Clone, Eq and Display are still recursive for trees built in code, so they are not capped yet
- [x] `solve.rs`: the rational-root search refuses coefficients of 2^127 or more, so the `i128` casts cannot wrap
- [x] `nullspace` rejects ragged matrices (reproduced: it panicked with an index out of bounds)
- [x] `expand_product` checks `MAX_EXPAND_TERMS` before allocating (hardening)
- [x] `negate_term` no longer indexes into an empty `Mul` (hardening; not reached by canonical trees in testing)
- [x] `diff`, `integrate` and `solve` reject `And`/`Or`/`Not`/`Eq`/`Bool` operands with `Unsupported`. A top-level `true`/`false` in `solve` still means always/never
- [x] Solver: keep partial rational roots when the remaining factor has none. New `SolveResult::Partial { exact, rest }` (non-breaking: `SolveResult` is `#[non_exhaustive]`)
- [x] Solver: general biquadratic substitution (`a·x^4 + b·x^2 + c`, solved exactly in `u = x^2`; signs of irrational `u` decided exactly; tests in `biquadratic_equations_have_exact_real_roots`)
- [x] Solver: `divisors(leading)` is computed once, outside the loop
- [x] Split the overloaded `Parametric` result. The "no unknown in the equation" case is now `SolveResult::Conditional(expr)`. `Parametric` is kept for free unknowns in linear systems. The variant's meaning is narrower, so callers that matched `Parametric` for `a = 0` now get `Conditional`. The enum is `#[non_exhaustive]`, so the addition itself is not breaking, but this behaviour change should be noted (it is in CHANGELOG)
- [x] `simplify_checked` reports non-convergence and over-deep trees. `simplify` is unchanged
- [x] Integration substitution symbol is chosen so it can't clash with the variable or a name in the integrand (no wrong answer found in testing)
- [x] `Expr` and `SymbolicFunc` are `#[non_exhaustive]`
- [x] `Number::cmp_value` gives the numeric order; the derived `Ord` stays structural
- [x] Removed the `unreachable!` in `calculus.rs`. Stale comments fixed in `bigint.rs` and `expr.rs`. The review's `expr.rs:188` and `:455` were not stale on inspection, so they are unchanged
- [x] `sqrt` folds perfect squares above the 10^4 bound (`sqrt(10007^2)` is `10007`)
- [x] Performance: measured, and no change needed. Harmonic sum of 1/k for k up to 300 (heavy gcd): 5 ms. `sqrt` of 30–40-digit constants (perfect-square search): 2–4 ms. Neither is slow enough to justify a change. The benchmark in `examples/bench.rs` does not isolate these paths, so these probe timings are the evidence.

### Tier 2 — Missing features
- [x] `eval(expr, env)` (exact, rational) and `to_decimal(n, digits)` (truncated) in `eval.rs`
- [x] Constants `pi`, `e`: new `Expr::Pi` and `Expr::E` variants (appended, so existing `Ord` order is unchanged). Parsed from the names `pi` and `e`, which are no longer variables. This is a behaviour change: `e` used as a variable name now means Euler's number (it is in CHANGELOG). `eval` returns `Unsupported`, because they are irrational. `diff` treats them as constants. `sqrt` as sugar is done
- [x] Ergonomics: `+ - * / -` on `Expr`, `From<i64>`, `From<Number>`, `subs(expr, var, value)`. `From<&str>` is not added; use `str::parse`, which `FromStr` already provides
- [x] LaTeX printer (`latex.rs`). MathML later
- [x] Calculus: n-th and partial derivatives, Asin/Acos/Atan and tanh antiderivatives, integration by parts (`x^n · g(x)`, and `u` differentiated for `log(x)·x^2`), `e^(ax)·sin/cos(bx)` in closed form, rational functions by partial fractions (linear and irreducible-quadratic terms, with `log|…|` and `atan`), expansion fallback for `integrate`, and conservative definite integrals (regular integrands only). `x^n·atan(x)` is done by parts, with the rational remainder checked at exact points. Not done: `atan` of a non-trivial argument times a power, and rational integrands with an irreducible cubic or higher factor, which have no closed form here.
- [x] Algebra: `factor` (complete to degree 4, marked incomplete beyond), polynomial gcd, partial fractions, cubic and quartic radicals (see line 70), and inversion of one layer of `exp`, `log`, reciprocal or integer powers.
- [x] Assumptions API. Decision (recommendation followed): keep the default simplifier's `x*x^-1 → 1` (it assumes `x ≠ 0`), documented in README and in `simplify`. `positive` drives `simplify_assuming` (`abs`, `log(abs)`, `sqrt(x^n)`). `nonzero` and `real` are not added, since no rule would use them yet.
- [x] Symbolic matrix type (det, inverse, product, solve; cofactor limit 7 for symbolic entries); series and limits (`taylor`, `limit`, `limit_at_infinity`); complex numbers: the unit `i` with `solve_complex`, and `real_part`, `imag_part` and `conjugate` (`complex.rs`, for expressions that are polynomials in `i`). Also `modulus` (sqrt(re^2 + im^2)), and `arg` and `log` for numeric values, with the principal branch. Not done for complex: `log` and `arg` of symbolic values (the sign of the parts must be known), and functions of complex arguments in general.

### Tier 3 — Innovative additions
- [x] Proof-carrying results: `certificate::Certificate` for roots, antiderivatives, factorisations and root-free intervals. Radical roots often do not reduce to a symbolic zero, so they are reported as not confirmed rather than wrong.
- [x] `simplify_trace`: step-by-step `(rule, before, after)` from the `RULES` table. Canonical construction is not listed, only rule applications.
- [x] Property-based rule checker: `simplify_preserves_value_at_sample_points` checks 1500 random expressions at exact sample points. It covers the rational rules only, because trigonometric values are not exact
- [x] Exact-first hybrid with rigorous interval bounds: `interval` gives exact rational enclosures for polynomials and rational functions, proves root exclusion, and isolates roots. Transcendental functions have no exact rational enclosure and are refused.
- [x] WASM build: `wasm` feature (`src/wasm.rs`) and `playground/` (a separate crate with a browser page). A Node smoke test passes. The browser page itself is not run here.
- [x] Chemistry-balancing demo (`examples/balance_chemistry.rs`)
- [-] `serde` feature and binary `uir` format. Decision (recommendation followed): not added. The text `uir` codec covers serialisation, and `serde` would be the crate's first dependency for no current consumer. Revisit when a consumer asks.

### Tier 4 — Usability and automation
- [x] Parse errors with line/column, caret snippet and "did you mean" (for unknown function names)
  - [x] `tpt-sym` shows the line and column, and the source line with a caret under the error (tested for multi-line input)
  - [x] "Did you mean" for unknown function names (`Error::UnknownFunction`). Unknown variable names are not errors by design, since any name is a variable.
  - [x] The library gives line and column through `error::line_col(src, pos)`. `Error::Parse` keeps its fields, so the change is non-breaking. An unknown function name is `Error::UnknownFunction { pos, name, suggestion }`, with a suggestion by optimal-string-alignment distance
  - [x] `Unsupported` messages name the operation. All 20 sites were reviewed.
- [x] `tpt-sym` command-line tool (`src/bin/tpt-sym.rs`). A REPL is not built
- [x] CI: `cargo fmt --check`, MSRV job on 1.87 (checked locally with `cargo +1.87.0`), Windows and macOS test runners, a feature matrix, a coverage job, and the float grep now ignores comment-only lines
- [ ] CI: `cargo-semver-checks` — blocked: needs a published baseline, and the crate is unpublished (publishing is excluded)
- [-] Release automation: the publish step is excluded until v1.0.0 is approved (see line 79)
  - [x] CHANGELOG.md, a tag-triggered `cargo publish --dry-run`, Dependabot for GitHub Actions, `rust-version`, keywords and categories
  - [ ] The publish step itself — excluded until v1.0.0 is approved
  - [x] `repository` and `documentation` in `Cargo.toml` are set
- [x] Fuzz generator widened: `testkit::random_expr_wide` uses all 12 functions, negative and fractional exponents, a third variable, and larger constants. The original `random_expr` is unchanged so existing seeds still give the same expressions. Booleans and deeper nesting are not in the wide generator yet
- [x] `BigInt` differential tests against `i128` and a division identity check beyond `i128`
- [x] Tests for the `MAX_PASSES` exit (`pass_limit_exit_is_reported_as_not_converged`, via a pass-limit parameter on the driver). The `MAX_DEPTH` exit is covered through `simplify_checked`

### Tier 5 — Adoption
- [x] `examples/`: quickstart, calculus, solve_equations, linear_systems, balance_chemistry, exact_numbers, parse_and_print (all run in CI through `cargo test`). `custom_rules` was dropped because the rule table is a `const`, so the example would describe an API that does not exist
- [x] README rewrite: quickstart, command line, syntax table, exact-versus-approximate table, limits and assumptions, examples index
- [x] `docs/cookbook.md`, with every block compiled and run as a doctest through `src/lib.rs`
- [x] Starter `template/` project (builds and runs; uses a path dependency until a release exists). No `no_std` variant yet
- [x] docs.rs: examples added to the new public functions, and to the core ones (`simplify`, `expand`, `parse`, `diff`, `integrate`, `solve`) (`factor`, `gcd`, `partial_fractions`, `taylor`, `limit`, `limit_at_infinity`, `Matrix::det` and `inverse`, `enclose`, `compile`, `run_exact`, `simplify_trace`, `simplify_assuming`, `definite_integral`, `root_certificate`, `solve_complex`). Older items still have fewer examples than they could.
- [x] CONTRIBUTING.md, with a walkthrough for adding a simplification rule
- [x] Open decisions resolved by the recommended option: solver result enum is done (`SolveResult`). Partial-root variant, `Parametric` split and `pi`/`e` constants are done as additions. The `Parametric` split and the `e` name change alter behaviour and are recorded in CHANGELOG (see the review decisions above). `serde` stays deferred, so no new dependency
- [x] Sibling-crate URLs: `tpt-mathr`, `tpt-uir`, `tpt-testkit` found and recorded. The tpt-vertex, tpt-chemistry and tpt-concord integrations are dropped by the user (see above)
