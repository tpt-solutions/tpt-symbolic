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

pub mod bigint;
pub mod calculus;
pub mod error;
pub mod eval;
pub mod expr;
pub mod latex;
pub mod mathr;
mod ops;
pub mod parse;
pub mod simplify;
pub mod solve;

#[cfg(feature = "uir")]
pub mod uir;

#[cfg(any(test, feature = "testkit"))]
pub mod testkit;

pub use error::Error;
pub use expr::{Expr, SymbolicFunc};
pub use mathr::Number;
