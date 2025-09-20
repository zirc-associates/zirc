//! AST (abstract syntax tree) types for the Zirc language.

/// Static type tags used for runtime checks and annotations.
#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Int,
    String,
    Bool,
    List,
    Unit,
}

/// Expressions (literals, operations, calls, containers).
#[derive(Debug, Clone)]
pub enum Expr {
    LiteralInt(i64),
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
    // comparisons
    Eq(Box<Expr>, Box<Expr>),
    Ne(Box<Expr>, Box<Expr>),
    Lt(Box<Expr>, Box<Expr>),
    Le(Box<Expr>, Box<Expr>),
    Gt(Box<Expr>, Box<Expr>),
    Ge(Box<Expr>, Box<Expr>),
    Call { name: String, args: Vec<Expr> },
    List(Vec<Expr>),
    Index(Box<Expr>, Box<Expr>),
}

/// Statements (variable bindings, control flow, etc.).
#[derive(Debug, Clone)]
pub enum Stmt {
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
}

/// Function parameter with optional type annotation.
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    pub ty: Option<Type>,
}

/// Function definition.
#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Option<Type>,
    pub body: Vec<Stmt>,
}

/// Top-level program items.
#[derive(Debug, Clone)]
pub enum Item {
    /// Module declaration: module "name"
    Module(String),
    /// Import declaration: import name [as alias]
    Import { module: String, alias: Option<String> },
    Function(Function),
    Stmt(Stmt),
}

/// Entire program consisting of items.
#[derive(Debug, Clone)]
pub struct Program {
    pub items: Vec<Item>,
}
