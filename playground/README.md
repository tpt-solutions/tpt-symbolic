# Playground

A browser page that runs `tpt-symbolic` through WebAssembly.

Build the module and its JavaScript glue, then serve `www/`:

```sh
cargo build --release --target wasm32-unknown-unknown
wasm-bindgen --target web --out-dir www/pkg \
  target/wasm32-unknown-unknown/release/tpt_symbolic_playground.wasm
python -m http.server --directory www 8000   # then open http://localhost:8000
```

This needs the `wasm32-unknown-unknown` target (`rustup target add wasm32-unknown-unknown`)
and `wasm-bindgen-cli` at the same version as the `wasm-bindgen` crate in `Cargo.lock`
(currently 0.2.129).

The bindings are in `src/wasm.rs` of the main crate, behind the `wasm` feature. This
directory is a separate crate so that the main library stays a plain `rlib` for every
other target.
