//! Error type shared by the parser, calculus and solver modules.

use alloc::string::String;
use core::fmt;

/// 1-based `(line, column)` of byte offset `pos` in `src`. Columns count characters.
///
/// An offset past the end is clamped to the end of `src`, and an offset inside a
/// character is moved back to that character's start.
pub fn line_col(src: &str, pos: usize) -> (usize, usize) {
    let mut pos = pos.min(src.len());
    while !src.is_char_boundary(pos) {
        pos -= 1;
    }
    let before = &src[..pos];
    let line = before.matches('\n').count() + 1;
    let line_start = before.rfind('\n').map_or(0, |i| i + 1);
    let column = before[line_start..].chars().count() + 1;
    (line, column)
}

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
    /// A name followed by `(` that is not a known function.
    UnknownFunction {
        /// Byte offset of the name.
        pos: usize,
        /// The name as written.
        name: String,
        /// The closest known function name, if one is close enough to be a likely typo.
        suggestion: Option<&'static str>,
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
            Error::UnknownFunction {
                pos,
                name,
                suggestion,
            } => {
                write!(f, "unknown function `{name}` at byte {pos}")?;
                match suggestion {
                    Some(s) => write!(f, "; did you mean `{s}`?"),
                    None => f.write_str("; write `*` if you mean multiplication"),
                }
            }
            Error::DivisionByZero => f.write_str("division by zero"),
            Error::Overflow => f.write_str("exact constant overflow"),
            Error::Unsupported(what) => write!(f, "unsupported: {what}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}
