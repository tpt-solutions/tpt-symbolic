# Starter template

A minimal application that depends on `tpt-symbolic`.

To use it:

1. Copy this directory somewhere outside the library repository.
2. In `Cargo.toml`, replace `tpt-symbolic = { path = ".." }` with a version or a
   git URL once the crate is released.
3. Replace the body of `main` in `src/main.rs`.
4. Run `cargo run`.

The template shows the usual flow: parse text into an expression, work with it
symbolically, then evaluate it at exact points.
