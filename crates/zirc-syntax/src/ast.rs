//! AST (abstract syntax tree) types for the Zirc language.
//!
//! Statements, items and functions carry a source [`Span`]
//! (1-based line/column of their first token) so runtime errors
//! can point at the offending statement. Synthetic nodes built by
//! tools use `Span::default()` (1:1).

use crate::error::Span;

/// Static type tags used for runtime checks and annotations.
#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Int,
    Float,
    String,
    Bool,
    List,
    Unit,
}

/// Expressions (literals, operations, calls, containers).
#[derive(Debug, Clone)]
pub enum Expr {
    LiteralInt(i64),
    LiteralFloat(f64),
    LiteralString(String),
    LiteralBool(bool),
    Ident(String),
    /// Member access like `a.b` (used for module access)
    Member(Box<Expr>, String),
    // arithmetic
    BinaryAdd(Box<Expr>, Box<Expr>),
    BinarySub(Box<Expr>, Box<Expr>),
    BinaryMul(Box<Expr>, Box<Expr>),
    BinaryDiv(Box<Expr>, Box<Expr>),
    // logical
    LogicalAnd(Box<Expr>, Box<Expr>),
    LogicalOr(Box<Expr>, Box<Expr>),
    LogicalNot(Box<Expr>),
    /// Unary numeric negation: `-x`.
    Neg(Box<Expr>),
    // comparisons
    Eq(Box<Expr>, Box<Expr>),
    Ne(Box<Expr>, Box<Expr>),
    Lt(Box<Expr>, Box<Expr>),
    Le(Box<Expr>, Box<Expr>),
    Gt(Box<Expr>, Box<Expr>),
    Ge(Box<Expr>, Box<Expr>),
    Call {
        name: String,
        args: Vec<Expr>,
    },
    /// Anonymous function (closure): `fun(x, y): x + y end`.
    /// The `Function` has no name; free variables are captured by
    /// value when the closure is created.
    Closure(Box<Function>),
    /// Call of an arbitrary expression: `(fun(x): x end)(5)`,
    /// `closures[0](x)`. The callee must evaluate to a closure.
    CallValue(Box<Expr>, Vec<Expr>),
    List(Vec<Expr>),
    Index(Box<Expr>, Box<Expr>),
}

/// A statement with its source span.
#[derive(Debug, Clone)]
pub struct Stmt {
    /// Source span of the statement's first token.
    pub span: Span,
    pub kind: StmtKind,
}

/// Statement kinds (see [`Stmt`] for the spanned wrapper).
#[derive(Debug, Clone)]
pub enum StmtKind {
    Let {
        name: String,
        ty: Option<Type>,
        expr: Expr,
    },
    Assign {
        name: String,
        expr: Expr,
    },
    Return(Option<Expr>),
    If {
        cond: Expr,
        then_body: Vec<Stmt>,
        else_body: Vec<Stmt>,
    },
    While {
        cond: Expr,
        body: Vec<Stmt>,
    },
    For {
        var: String,
        start: Expr,
        end: Expr,
        body: Vec<Stmt>,
    },
    Break,
    Continue,
    ExprStmt(Expr),
    /// A `~` line comment. Ignored by both engines; preserved by
    /// the formatter.
    Comment(String),
}

/// Function parameter with optional type annotation.
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    pub ty: Option<Type>,
    pub span: Span,
}

/// Function definition.
#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Option<Type>,
    pub body: Vec<Stmt>,
    /// Source span of the `fun` keyword (or closure literal).
    pub span: Span,
}

/// A top-level program item with its source span.
#[derive(Debug, Clone)]
pub struct Item {
    /// Source span of the item's first token.
    pub span: Span,
    pub kind: ItemKind,
}

/// Top-level program item kinds (see [`Item`]).
#[derive(Debug, Clone)]
pub enum ItemKind {
    /// A `~` line comment between items. Ignored by both engines;
    /// preserved by the formatter.
    Comment(String),
    /// Module declaration: module "name"
    Module(String),
    /// Import declaration: import name [as alias]
    Import {
        module: String,
        alias: Option<String>,
    },
    Function(Function),
    Stmt(Stmt),
}

/// Entire program consisting of items.
#[derive(Debug, Clone)]
pub struct Program {
    pub items: Vec<Item>,
}
