//! Text codec for expressions: a stand-in for `tpt-uir`.
//!
//! The wire format is the printer's output, which the parser reads back to an
//! identical canonical tree. A binary IR can replace it later without changing
//! callers, because they only use [`UirCodec`].

use crate::error::Error;
use crate::expr::Expr;
use crate::parse::parse;
use alloc::string::{String, ToString};

/// Encode to and decode from the IR text form.
pub trait UirCodec: Sized {
    /// Encodes `self` as IR text.
    fn to_uir(&self) -> String;

    /// Decodes IR text produced by [`UirCodec::to_uir`].
    fn from_uir(src: &str) -> Result<Self, Error>;
}

impl UirCodec for Expr {
    fn to_uir(&self) -> String {
        self.to_string()
    }

    fn from_uir(src: &str) -> Result<Self, Error> {
        parse(src)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_text() {
        let e = Expr::from_uir("3*x^2 - 2*y + 1/2").unwrap();
        assert_eq!(Expr::from_uir(&e.to_uir()).unwrap(), e);
    }
}
