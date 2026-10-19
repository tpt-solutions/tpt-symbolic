//! `tpt-symbolic`: exact, pure-Rust symbolic mathematics.
//!
//! Expressions are canonicalized on construction, so `a + b` and `b + a` are
//! the same tree. Constants are exact rationals; no floating point is used
//! anywhere in the core.
//!
//! The crate is `no_std` and needs only `alloc`.

#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

#[cfg(any(feature = "std", test))]
extern crate std;

#[cfg(not(feature = "stub-mathr"))]
compile_error!("the real tpt-mathr is not wired in yet; build with the default `stub-mathr` feature");

pub mod calculus;
pub mod error;
pub mod expr;
pub mod mathr;
pub mod parse;
pub mod simplify;
pub mod solve;

#[cfg(feature = "uir")]
pub mod uir;

#[cfg(any(test, feature = "testkit"))]
pub mod testkit;

pub use error::Error;
pub use mathr::Number;
