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
- [x] `mathr` shim: `Number` (BigInt / Rational / Complex), arithmetic, `Ord`, `Hash`, `Display`
- [ ] Define the trait surface we need from real tpt-mathr so the swap is mechanical
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
- [ ] Golden files: thousands of equivalent-syntax inputs → same AST (partial: small corpus in `parse::tests`; scale-up pending)
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
- [ ] Solvers: linear, quadratic, cubic, quartic (radical forms) — linear and quadratic done; cubic and quartic pending
- [x] Equation isolation for single variable (`x²+y²=25` for `y` → `±sqrt(25-x²)`)
- [x] `SymbolicRoot` for transcendental equations (Newton–Raphson hand-off to tpt-math)
- [x] Test: `x²-2=0` → `sqrt(2)` (10,000-digit check vs tpt-mathr pending the real crate)
- [ ] tpt-vertex integration: CAD constraint pipeline example (distance = 5)
- [ ] tpt-chemistry integration: balance `C3H8 + O2 → CO2 + H2O` as integer nullspace — the nullspace core is done and tested; wiring into tpt-chemistry is blocked on that repo
- [ ] tpt-concord integration: boolean satisfiability/contradiction check API
- [ ] tpt-math hand-off: AST export for numeric compilation
- [ ] Docs, examples, API review — README and pipeline doctest done; API review pending
- [ ] Publish v1.0.0 — excluded by you (not published)

## Cross-cutting quality
- [x] Property tests for identities (via testkit)
- [ ] Fuzz targets for parser and simplifier with time limits — seeded random tests done; time-limited fuzzing pending
- [ ] Benchmarks on large expressions
- [x] `no_std` CI matrix (native, wasm32, riscv)
- [x] Rustdoc for every public item; README with pipeline example

## Proposed decisions (recommended; awaiting your OK)
- [x] `SymbolicFunc`: Sin, Cos, Tan, Asin, Acos, Atan, Sinh, Cosh, Tanh, Exp, Log (natural), Abs. No Sqrt (use `Pow(x, 1/2)`).
- [x] Like-term/like-factor collection (`x+x→2x`, `x*x→x^2`) happens in canonicalization; expansion and trig identities stay in `simplify`.
- [x] Parser: `^` (accept `**`), implicit multiplication, `=`, `and`/`or`/`not`, ASCII only.
- [x] Rust edition 2021; MSRV = current stable; licence MIT OR Apache-2.0 (match other TPT repos if different).
- [x] Other TPT repos are on GitHub: use stubs now, switch to git dependencies once URLs are available.

## Still needed from you
- [ ] GitHub URLs for tpt-mathr, tpt-uir, tpt-testkit (and tpt-vertex / tpt-chemistry / tpt-concord / tpt-math)
- [ ] Licence used by the other TPT repos
