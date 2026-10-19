# tpt-symbolic — TODO

Source of truth: `spec.txt`. Tick boxes as you go.

## Decisions (confirmed)
- Deps (`tpt-mathr`, `tpt-uir`, `tpt-testkit`): **local stubs/traits** now; swap for real crates later.
- AST: **extend `Expr`** with `Eq`, boolean nodes (`And/Or/Not`, `Bool` consts); Complex comes via `Number`.
- Scope now: **checklist only**; no code until you approve.
- Solver output: **enum result** — `Exact(Vec<Expr>)`, `Parametric`, `Numeric(SymbolicRoot)`, `NoSolution`, `Infinite` (recommended, not yet confirmed by you).

## Phase 0 — Project setup
- [x] Create Cargo crate `tpt-symbolic` (`#![no_std]` + `extern crate alloc`)
- [x] Feature flags: `std` (off by default), `stub-mathr`, `uir`, `testkit`
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
- [ ] Solvers: linear, quadratic, cubic, quartic (radical forms) — linear and quadratic done; cubic and quartic done when they have rational roots (rational root theorem and deflation); pure powers `a·x^n + c = 0` now exact (`x^3 - 2`, `x^4 - 2`); irrational cubic/quartic radicals in general still pending
- [x] Equation isolation for single variable (`x²+y²=25` for `y` → `±sqrt(25-x²)`)
- [x] `SymbolicRoot` for transcendental equations (Newton–Raphson hand-off to tpt-math)
- [x] Test: `x²-2=0` → `sqrt(2)`; 10,000-digit check via exact integer sqrt (`bigint::tests::sqrt2_to_ten_thousand_decimal_places`), not tpt-mathr
- [ ] (deferred) tpt-vertex integration: CAD constraint pipeline example (distance = 5)
- [ ] (deferred) tpt-chemistry integration: balance `C3H8 + O2 → CO2 + H2O` as integer nullspace — the nullspace core is done and tested; `examples/balance_chemistry.rs` demonstrates it locally; wiring into tpt-chemistry is blocked on that repo
- [ ] (deferred) tpt-concord integration: boolean satisfiability/contradiction check API
- [ ] (deferred) tpt-math hand-off: AST export for numeric compilation
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
- [ ] GitHub URLs for tpt-mathr, tpt-uir, tpt-testkit (and tpt-vertex / tpt-chemistry / tpt-concord / tpt-math). Also needed for `repository` and `documentation` in `Cargo.toml`.
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
- [ ] Solver: keep partial rational roots when the remaining factor has none — **decision**: needs a new `SolveResult` variant, because today the whole equation falls back to `Numeric`
- [ ] Solver: general biquadratic substitution (pure powers `x^n = c` are done; `x^4 + x^2 - 2` is not)
- [x] Solver: `divisors(leading)` is computed once, outside the loop
- [ ] Split the overloaded `Parametric` result (free unknowns vs. "does not contain the variable") — **decision**: changes the public meaning of a variant
- [x] `simplify_checked` reports non-convergence and over-deep trees. `simplify` is unchanged
- [x] Integration substitution symbol is chosen so it can't clash with the variable or a name in the integrand (no wrong answer found in testing)
- [x] `Expr` and `SymbolicFunc` are `#[non_exhaustive]`
- [x] `Number::cmp_value` gives the numeric order; the derived `Ord` stays structural
- [x] Removed the `unreachable!` in `calculus.rs`. Stale comments fixed in `bigint.rs` and `expr.rs`. The review's `expr.rs:188` and `:455` were not stale on inspection, so they are unchanged
- [x] `sqrt` folds perfect squares above the 10^4 bound (`sqrt(10007^2)` is `10007`)
- [ ] Performance: the binary gcd in `Number::from_parts`, the quadratic like-term grouping and the trial division in `sqrt_const` are not yet measured or changed. The benches need to show a cost first

### Tier 2 — Missing features
- [x] `eval(expr, env)` (exact, rational) and `to_decimal(n, digits)` (truncated) in `eval.rs`
- [ ] Constants `pi`, `e` — **decision**: needs a representation in `Expr` (a `Const` holds only `Number`). `sqrt` as sugar is done
- [x] Ergonomics: `+ - * / -` on `Expr`, `From<i64>`, `From<Number>`, `subs(expr, var, value)`. `From<&str>` is not added; use `str::parse`, which `FromStr` already provides
- [x] LaTeX printer (`latex.rs`). MathML later
- [ ] Calculus: done — n-th and partial derivatives (`diff_n`, `gradient`) and Asin/Acos/Atan antiderivatives. Not done — Tanh antiderivative, integration by parts, definite integrals, the `expand` fallback for `(x^2+1)^2`
- [ ] Algebra: `factor`, polynomial gcd, partial fractions, cubic/quartic radicals, inversion for `exp`/`log`/`sin`/`1/x` equations
- [ ] Assumptions API (`positive`, `real`, `nonzero`) guarding `x*x^-1`, `log`, `abs`
- [ ] Symbolic matrix type (det, inverse); series and limits; complex numbers

### Tier 3 — Innovative additions
- [ ] Proof-carrying results: checkable `Certificate` for integrals and roots
- [ ] `simplify_trace`: step-by-step `(rule, before, after)` from the `RULES` table
- [x] Property-based rule checker: `simplify_preserves_value_at_sample_points` checks 1500 random expressions at exact sample points. It covers the rational rules only, because trigonometric values are not exact
- [ ] Exact-first hybrid with rigorous interval bounds on numeric fallbacks
- [ ] WASM playground and `wasm-bindgen` wrapper
- [x] Chemistry-balancing demo (`examples/balance_chemistry.rs`)
- [ ] `serde` feature and binary `uir` format — **decision**: adding `serde` adds the crate's first dependency

### Tier 4 — Usability and automation
- [ ] Parse errors with line/column, caret snippet and "did you mean"
  - [x] `tpt-sym` shows the line and column, and the source line with a caret under the error (tested for multi-line input)
  - [ ] "Did you mean" hint for unknown names — needs the parser to report the bad identifier, so it touches `Error::Parse` and is not done
  - [ ] The library `Error::Parse` is still a byte offset plus a message. Line and column, and the "did you mean" hint, are not done
  - [ ] `Unsupported` messages are still generic
- [x] `tpt-sym` command-line tool (`src/bin/tpt-sym.rs`). A REPL is not built
- [x] CI: `cargo fmt --check`, MSRV job on 1.87 (checked locally with `cargo +1.87.0`), Windows and macOS test runners, a feature matrix, a coverage job, and the float grep now ignores comment-only lines
- [ ] CI: `cargo-semver-checks` — needs a published baseline to compare against
- [ ] Release automation
  - [x] CHANGELOG.md, a tag-triggered `cargo publish --dry-run`, Dependabot for GitHub Actions, `rust-version`, keywords and categories
  - [ ] The publish step itself — excluded until v1.0.0 is approved
  - [ ] `repository` and `documentation` in `Cargo.toml` — need the URL
- [x] Fuzz generator widened: `testkit::random_expr_wide` uses all 12 functions, negative and fractional exponents, a third variable, and larger constants. The original `random_expr` is unchanged so existing seeds still give the same expressions. Booleans and deeper nesting are not in the wide generator yet
- [x] `BigInt` differential tests against `i128` and a division identity check beyond `i128`
- [x] Tests for the `MAX_PASSES` exit (`pass_limit_exit_is_reported_as_not_converged`, via a pass-limit parameter on the driver). The `MAX_DEPTH` exit is covered through `simplify_checked`

### Tier 5 — Adoption
- [x] `examples/`: quickstart, calculus, solve_equations, linear_systems, balance_chemistry, exact_numbers, parse_and_print (all run in CI through `cargo test`). `custom_rules` was dropped because the rule table is a `const`, so the example would describe an API that does not exist
- [x] README rewrite: quickstart, command line, syntax table, exact-versus-approximate table, limits and assumptions, examples index
- [x] `docs/cookbook.md`, with every block compiled and run as a doctest through `src/lib.rs`
- [x] Starter `template/` project (builds and runs; uses a path dependency until a release exists). No `no_std` variant yet
- [ ] docs.rs polish: the README is the crate overview. Per-function examples exist for `eval`, `latex` and the cookbook, but not for every public item
- [x] CONTRIBUTING.md, with a walkthrough for adding a simplification rule
- [x] Open decisions resolved by the recommended option: solver result enum is done (`SolveResult`). Partial-root variant, `Parametric` split, `pi`/`e` constants and `serde` stay deferred, so no public API change and no new dependency (see the review decisions above)
- [ ] Sibling-crate URLs (`tpt-mathr`, `tpt-uir`, `tpt-testkit`, integration crates) — needs the user; no recommendation can supply them
