//! Error type shared by the parser, calculus and solver modules.

use core::fmt;

/// Errors produced by `tpt-symbolic`.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// The input text could not be parsed. `pos` is a byte offset into the source.
    Parse {
        /// Byte offset of the offending token.
        pos: usize,
        /// What the parser expected or found.
        message: &'static str,
    },
    /// A division or negative power of exact zero was requested.
    DivisionByZero,
    /// An exact constant grew beyond the range of the current number type.
    Overflow,
    /// The operation is not implemented for this kind of expression.
    Unsupported(&'static str),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Parse { pos, message } => write!(f, "parse error at byte {pos}: {message}"),
            Error::DivisionByZero => f.write_str("division by zero"),
            Error::Overflow => f.write_str("exact constant overflow"),
            Error::Unsupported(what) => write!(f, "unsupported: {what}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}
