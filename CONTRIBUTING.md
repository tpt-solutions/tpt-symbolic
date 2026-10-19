# Contributing

Thanks for looking at `tpt-symbolic`. The crate is exact by design, so most
review questions come down to: does this change keep results exact, and does it
keep them canonical?

## Ground rules

- **No floats in the library.** CI rejects `f32` and `f64` in `src/`. Use
  `Number` (exact rationals) for every value.
- **`no_std` + `alloc`.** Only `src/bin/` may use `std`. The library must still
  build for `wasm32-unknown-unknown` and `riscv32imac-unknown-none-elf`.
- **Wrong answers are worse than no answer.** If a rule or integral is not
  certain, return `None` or `Unevaluated`. Never guess.
- **Canonical form.** Equivalent inputs must give identical trees. Add a test
  when you touch construction or simplification.

## Before you open a pull request

```sh
cargo fmt
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo test --no-default-features --features stub-mathr
```

Also run the examples you affect with `cargo run --example <name>`.

## Walkthrough: adding a simplification rule

Rules live in `src/simplify.rs`. Each one is a `Rule`: a name, a priority, a
category, and a function that rewrites a single node whose children are already
simplified. It returns `None` when it does not apply.

1. **Write the function.** Match the shape you want, and bail out early:

   ```rust
   /// `sin(a) * cos(a)^-1` becomes `tan(a)`.
   fn rule_tan_definition(e: &Expr) -> Option<Expr> {
       // ...matches on Expr::Mul, returns Some(Expr::func(SymbolicFunc::Tan, a))
   }
   ```

2. **Register it** in the `RULES` table. Higher priority runs first. Put a rule
   after any rule whose output it would otherwise undo, and make sure no two
   rules can rewrite each other back and forth.

3. **Add a unit test** next to the rule that checks the rewrite, and one that
   checks it does not fire on a near miss.

4. **Keep it terminating.** The driver stops after `MAX_PASSES` passes, but a
   rule that cycles will show up as `simplify_checked` returning an error. Run
   the idempotence tests in `simplify::tests` after adding a rule.

5. **Check the value.** `simplify_preserves_value_at_sample_points` evaluates
   random expressions exactly before and after simplification. It covers the
   rational rules. Trigonometric rules are not checked there, so test them with
   explicit cases.

## Other kinds of change

- **New function (`SymbolicFunc`).** Add the variant, then fix every `match`
  the compiler reports. Add its derivative in `calculus::diff`, its printing in
  `expr.rs`, its LaTeX name in `latex.rs`, and its name in
  `SymbolicFunc::from_name`.
- **New solver case.** Return `SolveResult::Numeric` when you are not sure. An
  exact root must be checked by substituting it back.
- **Public API.** The enums `Expr`, `SymbolicFunc` and `Error` are
  `#[non_exhaustive]`, so adding a variant is not a breaking change. Removing or
  renaming a public item is.

## Documentation

- Every public item needs rustdoc (`#![warn(missing_docs)]` is on).
- Code in `docs/cookbook.md` and the README runs as doctests. Keep it correct.
- Add a line to `CHANGELOG.md` under "Unreleased" for any user-visible change.
