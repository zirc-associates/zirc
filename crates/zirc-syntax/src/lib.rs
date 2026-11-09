//! Zirc language syntax definitions and abstract syntax tree.
//!
//! This crate provides the foundational syntax elements for the Zirc programming language,
//! including token definitions, abstract syntax tree (AST) nodes, and error handling utilities.
//! All other crates in the Zirc ecosystem depend on these fundamental types.
//!
//! # Overview
//!
//! The crate is organized into three main modules:
//!
//! - [`token`]: Token types and lexical elements
//! - [`ast`]: Abstract syntax tree node definitions  
//! - [`error`]: Error handling types and utilities
//!
//! # Architecture
//!
//! The syntax crate follows a layered approach:
//!
//! ```text
//! ┌─────────────────────────────────────┐
//! │              Parser                 │  <- Produces AST
//! ├─────────────────────────────────────┤
//! │               Lexer                 │  <- Produces Tokens
//! ├─────────────────────────────────────┤
//! │           Source Code               │  <- Raw text input
//! └─────────────────────────────────────┘
//! ```
//!
//! # Examples
//!
//! ## Working with tokens
//!
//! ```rust
//! use zirc_syntax::{Token, TokenKind};
//!
//! let token = Token {
//!     kind: TokenKind::Ident("variable".to_string()),
//!     line: 1,
//!     col: 1,
//! };
//! ```
//!
//! ## Building AST nodes
//!
//! ```rust
//! use zirc_syntax::{Expr, Span, Stmt, StmtKind};
//!
//! let expr = Expr::LiteralInt(42);
//! let stmt = Stmt { span: Span::new(1, 1), kind: StmtKind::ExprStmt(expr) };
//! ```
//!
//! # Features
//!
//! - **Zero-cost abstractions**: AST nodes are designed for efficient traversal
//! - **Rich error context**: Error types include location and context information
//! - **Extensible design**: Easy to add new language features
//! - **Type safety**: Leverages Rust's type system to prevent invalid ASTs

/// Token definitions and lexical analysis types.
///
/// This module contains all token types that the lexer can produce,
/// including keywords, operators, literals, and structural elements.
pub mod token;

/// Abstract syntax tree node definitions.
///
/// This module defines the complete AST structure for Zirc programs,
/// including expressions, statements, types, and program structure.
pub mod ast;

/// Static analyses shared by both engines (free variables, ...).
pub mod analysis;

/// Error handling utilities and types.
///
/// This module provides error types, result types, and utility functions
/// for consistent error handling across the Zirc toolchain.
pub mod error;

// Re-export all public items for convenience
pub use ast::*;
pub use error::*;
pub use token::*;
