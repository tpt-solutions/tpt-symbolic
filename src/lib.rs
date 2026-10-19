#![doc = include_str!("../README.md")]
#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

extern crate alloc;

#[cfg(any(feature = "std", test))]
extern crate std;

/// Task-oriented recipes. Every code block is a doctest.
#[doc = include_str!("../docs/cookbook.md")]
pub mod cookbook {}

pub mod apart;
pub mod assume;
pub mod bigint;
pub mod calculus;
pub mod certificate;
pub mod error;
pub mod eval;
pub mod expr;
pub mod factor;
pub mod interval;
pub mod latex;
pub mod limits;
pub mod mathr;
pub mod matrix;
mod ops;
pub mod parse;
pub mod program;
pub mod simplify;
pub mod solve;

#[cfg(feature = "uir")]
pub mod uir;
#[cfg(feature = "wasm")]
pub mod wasm;

#[cfg(any(test, feature = "testkit"))]
pub mod testkit;

pub use error::Error;
pub use expr::{Expr, SymbolicFunc};
pub use mathr::Number;
