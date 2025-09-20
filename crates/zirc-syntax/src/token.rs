//! Token definitions and lexical analysis types for the Zirc language.
//!
//! This module defines all the token types that can be produced by the Zirc lexer.
//! Tokens represent the smallest meaningful units of Zirc source code, such as
//! keywords, identifiers, operators, and literals.
//!
//! # Token Categories
//!
//! The lexer recognizes several categories of tokens:
//!
//! - **Identifiers**: Variable names, function names (`foo`, `my_var`)
//! - **Literals**: Numbers and strings (`42`, `"hello"`)
//! - **Keywords**: Language reserved words (`fun`, `if`, `while`)
//! - **Operators**: Arithmetic and comparison operators (`+`, `==`, `&&`)
//! - **Punctuation**: Structural elements (`(`, `)`, `,`)
//! - **Special**: End-of-file marker
//!
//! # Examples
//!
//! ```rust
//! use zirc_syntax::{Token, TokenKind};
//!
//! // Create a keyword token
//! let keyword = Token {
//!     kind: TokenKind::Fun,
//!     line: 1,
//!     col: 1,
//! };
//!
//! // Create an identifier token
//! let identifier = Token {
//!     kind: TokenKind::Ident("factorial".to_string()),
//!     line: 1,
//!     col: 5,
//! };
//!
//! // Create a number literal
//! let number = Token {
//!     kind: TokenKind::Number(42),
//!     line: 2,
//!     col: 1,
//! };
//! ```

/// Token types that can be produced by the Zirc lexer.
///
/// Each variant represents a different kind of lexical element in the Zirc language.
/// Tokens carry their semantic content (like the text of identifiers or the value
/// of number literals) as well as their syntactic category.
///
/// # Variant Categories
///
/// ## Literals
/// - [`Ident`](TokenKind::Ident): Variable and function names
/// - [`Number`](TokenKind::Number): Integer literals  
/// - [`String`](TokenKind::String): String literals
///
/// ## Keywords
/// Language reserved words that have special meaning in Zirc syntax.
///
/// ## Operators
/// Arithmetic, comparison, and logical operators used in expressions.
///
/// ## Punctuation
/// Structural tokens used to delimit and organize code elements.
///
/// # Examples
///
/// ```rust
/// use zirc_syntax::TokenKind;
///
/// // Different token types
/// let var_name = TokenKind::Ident("counter".to_string());
/// let number = TokenKind::Number(100);
/// let text = TokenKind::String("Hello, world!".to_string());
/// let keyword = TokenKind::Fun;
/// let operator = TokenKind::Plus;
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // === Literals ===
    
    /// An identifier token (variable names, function names, etc.)
    /// 
    /// Examples: `foo`, `my_variable`, `factorial`
    Ident(String),
    
    /// A numeric literal token (64-bit signed integers)
    /// 
    /// Examples: `42`, `-123`, `0`
    Number(i64),
    
    /// A string literal token
    /// 
    /// Examples: `"hello"`, `"world!"`, `""`
    String(String),
    
    // === Keywords ===
    
    /// The `module` keyword - declares the module name for this file
    Module,
    
    /// The `import` keyword - imports another module
    Import,
    
    /// The `as` keyword - used to alias imports
    As,
    
    /// The `fun` keyword - used to declare functions
    Fun,
    
    /// The `end` keyword - used to close blocks
    End,
    
    /// The `if` keyword - used for conditional statements
    If,
    
    /// The `else` keyword - used for alternative branches
    Else,
    
    /// The `while` keyword - used for loops
    While,
    
    /// The `break` keyword - used to exit loops early
    Break,
    
    /// The `continue` keyword - used to skip to next loop iteration
    Continue,
    
    /// The `return` keyword - used to return from functions
    Return,
    
    /// The `let` keyword - used for variable declarations
    Let,
    
    /// The `true` keyword - boolean literal
    True,
    
    /// The `false` keyword - boolean literal
    False,
    
    /// The `for` keyword - used for range-based loops
    For,
    
    /// The `in` keyword - used in for-loop syntax
    In,
    
    // === Punctuation ===
    
    /// Comma separator `,`
    Comma,
    
    /// Colon `:` - used in type annotations and block syntax
    Colon,
    
    /// Left parenthesis `(`
    LParen,
    
    /// Right parenthesis `)`
    RParen,
    
    /// Left square bracket `[` 
    LBracket,
    
    /// Right square bracket `]`
    RBracket,
    
    /// Dot `.` - used for member access
    Dot,
    
    // === Operators ===
    
    /// Assignment operator `=`
    Equal,
    
    /// Addition operator `+`
    Plus,
    
    /// Subtraction operator `-`
    Minus,
    
    /// Multiplication operator `*`
    Star,
    
    /// Division operator `/`
    Slash,
    
    /// Equality comparison operator `==`
    EqEq,
    
    /// Inequality comparison operator `!=`
    NotEq,
    
    /// Less-than comparison operator `<`
    Less,
    
    /// Less-than-or-equal comparison operator `<=`
    LessEq,
    
    /// Greater-than comparison operator `>`
    Greater,
    
    /// Greater-than-or-equal comparison operator `>=`
    GreaterEq,
    
    /// Logical AND operator `&&`
    AndAnd,
    
    /// Logical OR operator `||`
    OrOr,
    
    /// Logical NOT operator `!`
    Bang,
    
    /// Range operator `..` used in for-loops
    DotDot,
    
    /// End-of-file marker - indicates no more tokens
    Eof,
}

/// A token with its source location information.
///
/// This struct combines a [`TokenKind`] with source position information,
/// enabling the lexer to report precise error locations and the parser
/// to provide helpful error messages with line and column numbers.
///
/// # Fields
///
/// - `kind`: The type and content of the token
/// - `line`: 1-based line number in the source file
/// - `col`: 1-based column number in the source file
///
/// # Examples
///
/// ```rust
/// use zirc_syntax::{Token, TokenKind};
///
/// // Create a token for a function keyword at the start of a file
/// let fun_token = Token {
///     kind: TokenKind::Fun,
///     line: 1,
///     col: 1,
/// };
///
/// // Create a token for an identifier later in the line
/// let name_token = Token {
///     kind: TokenKind::Ident("factorial".to_string()),
///     line: 1,
///     col: 5,
/// };
///
/// // Position information is useful for error reporting
/// println!("Token '{}' at line {}, column {}", 
///          "factorial", name_token.line, name_token.col);
/// ```
///
/// # Usage in Error Reporting
///
/// The position information is crucial for providing helpful error messages:
///
/// ```text
/// Error at line 3, column 15: Unexpected token 'if'
///   let x = 5 + if y > 0
///               ^
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    /// The type and semantic content of this token
    pub kind: TokenKind,
    
    /// Line number in the source file (1-based)
    pub line: usize,
    
    /// Column number in the source file (1-based)
    pub col: usize,
}
