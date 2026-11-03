//! Diagnostics, stable error codes and English/French messages for the Peras compiler.
//!
//! Day 1 only defines the error code type. The full `Diagnostic`, `ariadne`
//! rendering, JSON output and Fluent messages arrive on day 2.

use std::fmt;

/// A stable error code such as `E0204`. Codes are never reused or renumbered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ErrorCode(u16);

impl ErrorCode {
    /// Creates an error code from its number, e.g. `ErrorCode::new(204)` for `E0204`.
    #[must_use]
    pub const fn new(number: u16) -> Self {
        Self(number)
    }

    /// The numeric part of the code.
    #[must_use]
    pub const fn number(self) -> u16 {
        self.0
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "E{:04}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::ErrorCode;

    #[test]
    fn codes_are_printed_with_four_digits() {
        assert_eq!(ErrorCode::new(204).to_string(), "E0204");
        assert_eq!(ErrorCode::new(1).to_string(), "E0001");
    }
}
