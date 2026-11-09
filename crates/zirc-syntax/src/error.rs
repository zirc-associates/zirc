//! Error handling types and utilities for the Zirc language toolchain.
//!
//! This module provides a unified error handling system used throughout all Zirc
//! language components. It includes structured error types with source location
//! information and convenience functions for error creation and propagation.
//!
//! # Error Philosophy
//!
//! The Zirc error system prioritizes:
//!
//! - **Helpful error messages**: Clear, actionable error descriptions
//! - **Precise location information**: Line and column numbers for source errors
//! - **Consistent formatting**: Uniform error presentation across all tools
//! - **Easy propagation**: Convenient creation and handling of errors
//!
//! # Examples
//!
//! ## Basic error creation
//!
//! ```rust
//! use zirc_syntax::error::{Error, Result, error};
//!
//! // Create a simple error
//! let simple_error = Error::new("Something went wrong");
//!
//! // Create an error with source location
//! let located_error = Error::with_span("Unexpected token", 10, 5);
//!
//! // Use the convenience function
//! fn might_fail() -> Result<i32> {
//!     error("Operation failed")
//! }
//! ```
//!
//! ## Error propagation
//!
//! ```rust
//! use zirc_syntax::error::{Result, Error, error};
//!
//! fn parse_number(s: &str) -> Result<i32> {
//!     s.parse().map_err(|_| Error::new(format!("Invalid number: {}", s)))
//! }
//!
//! fn process_input(input: &str) -> Result<i32> {
//!     let num = parse_number(input)?;
//!     if num < 0 {
//!         error("Number must be positive")
//!     } else {
//!         Ok(num * 2)
//!     }
//! }
//! ```

use std::fmt;

/// Classification of a Zirc error, independent of its message text.
///
/// Callers must match on this kind (not on message substrings) when
/// reacting programmatically to an error. The human-readable `msg`
/// remains free to evolve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ErrorKind {
    /// Unclassified / generic error (default, kept for compatibility).
    #[default]
    General,
    /// Lexical analysis failed (bad character, unterminated string, ...).
    Lex,
    /// Parsing failed (unexpected token, missing `end`, ...).
    Parse,
    /// Compilation to bytecode failed.
    Compile,
    /// Runtime evaluation failed in the tree-walking interpreter.
    Runtime,
    /// Execution failed in the bytecode VM (stack underflow, div by zero, ...).
    Vm,
    /// I/O failure (reading/writing files, stdin/stdout).
    Io,
    /// Type mismatch against a declared annotation.
    Type,
    /// Undefined variable or function.
    Undefined,
}

/// Source span: 1-based line/column where a token or error originated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    /// 1-based line number.
    pub line: usize,
    /// 1-based column number.
    pub col: usize,
}

impl Span {
    /// Create a span from a 1-based line and column.
    pub fn new(line: usize, col: usize) -> Self {
        Self { line, col }
    }
}

/// Synthetic spans (built by tools, not the parser) point at 1:1.
impl Default for Span {
    fn default() -> Self {
        Self { line: 1, col: 1 }
    }
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.col)
    }
}

/// An error that occurred during Zirc language processing.
///
/// This structure represents all types of errors that can occur in the Zirc toolchain,
/// from lexical analysis through interpretation. Each error includes a descriptive
/// message and optional source location information.
///
/// # Fields
///
/// - `msg`: Human-readable error description
/// - `line`: Optional 1-based line number in source file
/// - `col`: Optional 1-based column number in source file
///
/// # Design Rationale
///
/// The error type is designed to be:
/// - **Lightweight**: Minimal memory overhead
/// - **Informative**: Includes context for debugging
/// - **Flexible**: Works with or without location info
/// - **Consistent**: Same format across all components
///
/// # Examples
///
/// ```rust
/// use zirc_syntax::Error;
///
/// // Error without location
/// let generic_error = Error::new("File not found");
///
/// // Error with precise location
/// let syntax_error = Error::with_span(
///     "Expected 'end' keyword",
///     15,  // line
///     8    // column
/// );
///
/// println!("{}", syntax_error);  // "Expected 'end' keyword at 15:8"
/// ```
#[derive(Debug, Clone)]
pub struct Error {
    /// Human-readable error message
    pub msg: String,

    /// Optional line number in source file (1-based)
    pub line: Option<usize>,

    /// Optional column number in source file (1-based)
    pub col: Option<usize>,

    /// Machine-readable classification (match on this, not on `msg`).
    pub kind: ErrorKind,
}

impl Error {
    /// Creates a new error with the given message.
    ///
    /// The error will not have source location information. This is suitable
    /// for runtime errors or cases where source location is not relevant.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use zirc_syntax::Error;
    ///
    /// let error = Error::new("File not found");
    /// let error_from_string = Error::new(format!("Failed to parse: {}", "input"));
    /// ```
    pub fn new(msg: impl Into<String>) -> Self {
        Self {
            msg: msg.into(),
            line: None,
            col: None,
            kind: ErrorKind::General,
        }
    }

    /// Create an error of the given kind, without source location.
    pub fn of_kind(kind: ErrorKind, msg: impl Into<String>) -> Self {
        Self {
            msg: msg.into(),
            line: None,
            col: None,
            kind,
        }
    }

    /// Create an error of the given kind at a source location.
    pub fn at(kind: ErrorKind, msg: impl Into<String>, line: usize, col: usize) -> Self {
        Self {
            msg: msg.into(),
            line: Some(line),
            col: Some(col),
            kind,
        }
    }

    /// Source span of this error, if it carries a location.
    pub fn span(&self) -> Option<Span> {
        match (self.line, self.col) {
            (Some(line), Some(col)) => Some(Span::new(line, col)),
            _ => None,
        }
    }

    /// Builder: set the error kind, keeping message and location.
    pub fn with_kind(mut self, kind: ErrorKind) -> Self {
        self.kind = kind;
        self
    }

    /// Attach a span only if this error does not already carry a
    /// location (engines use this to surface the current
    /// statement's span on unlocated runtime errors).
    pub fn with_span_if_missing(mut self, span: Span) -> Self {
        if self.line.is_none() {
            self.line = Some(span.line);
            self.col = Some(span.col);
        }
        self
    }

    /// Creates a new error with the given message and source location.
    ///
    /// This should be used for syntax errors, parse errors, or other issues
    /// that can be precisely located in the source code.
    ///
    /// # Parameters
    ///
    /// - `msg`: The error message
    /// - `line`: 1-based line number in the source file
    /// - `col`: 1-based column number in the source file
    ///
    /// # Examples
    ///
    /// ```rust
    /// use zirc_syntax::Error;
    ///
    /// // Error at line 5, column 12
    /// let error = Error::with_span("Unexpected token 'if'", 5, 12);
    ///
    /// // The error will format as: "Unexpected token 'if' at 5:12"
    /// println!("{}", error);
    /// ```
    pub fn with_span(msg: impl Into<String>, line: usize, col: usize) -> Self {
        Self {
            msg: msg.into(),
            line: Some(line),
            col: Some(col),
            kind: ErrorKind::General,
        }
    }
}

impl std::error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let (Some(l), Some(c)) = (self.line, self.col) {
            write!(f, "{} at {}:{}", self.msg, l, c)
        } else {
            write!(f, "{}", self.msg)
        }
    }
}

impl From<String> for Error {
    fn from(s: String) -> Self {
        Error::new(s)
    }
}
impl From<&str> for Error {
    fn from(s: &str) -> Self {
        Error::new(s)
    }
}

/// A specialized `Result` type for Zirc operations.
///
/// This is a convenience type alias that uses [`Error`] as the error type.
/// It's used throughout the Zirc codebase for consistent error handling.
///
/// # Examples
///
/// ```rust
/// use zirc_syntax::error::{Result, error};
///
/// fn parse_input(input: &str) -> Result<i32> {
///     if input.is_empty() {
///         error("Input cannot be empty")
///     } else {
///         Ok(42)
///     }
/// }
/// ```
pub type Result<T> = std::result::Result<T, Error>;

/// Convenience function to create an error result.
///
/// This function creates an `Err` result containing an [`Error`] with the
/// given message. It's a shorthand for `Err(Error::new(msg))`.
///
/// # Examples
///
/// ```rust
/// use zirc_syntax::error::{Result, error};
///
/// fn validate_input(s: &str) -> Result<()> {
///     if s.is_empty() {
///         error("Input cannot be empty")
///     } else {
///         Ok(())
///     }
/// }
/// ```
pub fn error<T>(msg: impl Into<String>) -> Result<T> {
    Err(Error::new(msg))
}

/// Convenience function to create an error result with source location.
///
/// This function creates an `Err` result containing an [`Error`] with the
/// given message and source location. It's a shorthand for
/// `Err(Error::with_span(msg, line, col))`.
///
/// # Parameters
///
/// - `line`: 1-based line number in the source file
/// - `col`: 1-based column number in the source file  
/// - `msg`: The error message
///
/// # Examples
///
/// ```rust
/// use zirc_syntax::error::{Result, error_at};
///
/// fn parse_token_at_location(line: usize, col: usize) -> Result<String> {
///     // Simulate a parse error at a specific location
///     error_at(line, col, "Unexpected character '&'")
/// }
/// ```
pub fn error_at<T>(line: usize, col: usize, msg: impl Into<String>) -> Result<T> {
    Err(Error::with_span(msg, line, col))
}

/// Convenience function to create an error result with a kind and
/// source location. Prefer this over [`error_at`] in the lexer
/// (kind `Lex`) and parser (kind `Parse`) so callers can classify
/// failures programmatically.
pub fn error_at_kind<T>(
    kind: ErrorKind,
    line: usize,
    col: usize,
    msg: impl Into<String>,
) -> Result<T> {
    Err(Error::at(kind, msg, line, col))
}
