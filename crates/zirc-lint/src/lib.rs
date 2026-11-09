//! `zirc-lint`: static analysis for Zirc programs.
//!
//! Walks the AST without executing it and reports likely bugs:
//! calls to undefined functions, uses of undefined variables, and
//! duplicate function definitions. Builtin names resolve through
//! the single registry in `zirc-bytecode`, so a misspelled builtin
//! is caught here exactly as in the engines.
//!
//! # Example
//!
//! ```rust
//! use zirc_lint::lint_program;
//! use zirc_lexer::Lexer;
//! use zirc_parser::Parser;
//!
//! let mut lexer = Lexer::new("show(1)\nundefined_fn()");
//! let tokens = lexer.tokenize().unwrap();
//! let mut parser = Parser::new(tokens);
//! let program = parser.parse_program().unwrap();
//! let warnings = lint_program(&program);
//! assert_eq!(warnings.len(), 1);
//! ```

use std::collections::HashSet;

use zirc_syntax::ast::{Expr, Function, ItemKind, Program, Stmt, StmtKind};

/// A single lint finding.
#[derive(Debug, Clone, PartialEq)]
pub struct LintWarning {
    /// Short rule name, e.g. `undefined-function`.
    pub rule: &'static str,
    /// Human-readable explanation.
    pub message: String,
    /// 1-based line of the offending statement, if known.
    pub line: Option<usize>,
    /// 1-based column, if known.
    pub col: Option<usize>,
}

impl LintWarning {
    fn new(rule: &'static str, message: String) -> Self {
        Self {
            rule,
            message,
            line: None,
            col: None,
        }
    }
    fn at(rule: &'static str, message: String, span: zirc_syntax::error::Span) -> Self {
        Self {
            rule,
            message,
            line: Some(span.line),
            col: Some(span.col),
        }
    }
}

/// Statically check a program and return all warnings found.
///
/// Checks performed:
/// - `undefined-function`: call to a name that is neither a builtin
///   nor a defined function (qualified `mod.f` calls check the
///   qualified name, falling back to the last segment).
/// - `undefined-variable`: identifier use with no visible binding
///   (block-scoped, mirroring the engines).
/// - `duplicate-function`: two definitions with the same name.
pub fn lint_program(program: &Program) -> Vec<LintWarning> {
    let mut warnings = Vec::new();
    let mut functions: HashSet<String> = HashSet::new();
    for item in &program.items {
        if let ItemKind::Function(f) = &item.kind {
            if !functions.insert(f.name.clone()) {
                warnings.push(LintWarning::at(
                    "duplicate-function",
                    format!("duplicate function '{}'", f.name),
                    item.span,
                ));
            }
        }
    }
    let mut scopes: Vec<HashSet<String>> = vec![HashSet::new()];
    for item in &program.items {
        match &item.kind {
            ItemKind::Function(f) => lint_function(f, &functions, &mut warnings),
            ItemKind::Stmt(s) => lint_stmt(s, &functions, &mut scopes, &mut warnings),
            ItemKind::Module(_) | ItemKind::Import { .. } | ItemKind::Comment(_) => {}
        }
    }
    warnings
}

fn lint_function(f: &Function, functions: &HashSet<String>, warnings: &mut Vec<LintWarning>) {
    let mut scopes: Vec<HashSet<String>> = vec![f.params.iter().map(|p| p.name.clone()).collect()];
    for s in &f.body {
        lint_stmt(s, functions, &mut scopes, warnings);
    }
}

fn is_defined(scopes: &[HashSet<String>], name: &str) -> bool {
    scopes.iter().rev().any(|s| s.contains(name))
}

fn lint_stmt(
    s: &Stmt,
    functions: &HashSet<String>,
    scopes: &mut Vec<HashSet<String>>,
    warnings: &mut Vec<LintWarning>,
) {
    let span = s.span;
    match &s.kind {
        StmtKind::Comment(_) => {}
        StmtKind::Let { name, expr, .. } => {
            lint_expr(expr, functions, scopes, warnings);
            scopes.last_mut().unwrap().insert(name.clone());
        }
        StmtKind::Assign { name, expr } => {
            lint_expr(expr, functions, scopes, warnings);
            if !is_defined(scopes, name) {
                warnings.push(LintWarning::at(
                    "undefined-variable",
                    format!("assignment to undefined variable '{}'", name),
                    span,
                ));
            }
        }
        StmtKind::Return(e) => {
            if let Some(e) = e {
                lint_expr(e, functions, scopes, warnings);
            }
        }
        StmtKind::If {
            cond,
            then_body,
            else_body,
        } => {
            lint_expr(cond, functions, scopes, warnings);
            lint_block(then_body, functions, scopes, warnings);
            lint_block(else_body, functions, scopes, warnings);
        }
        StmtKind::While { cond, body } => {
            lint_expr(cond, functions, scopes, warnings);
            lint_block(body, functions, scopes, warnings);
        }
        StmtKind::For {
            var,
            start,
            end,
            body,
        } => {
            lint_expr(start, functions, scopes, warnings);
            lint_expr(end, functions, scopes, warnings);
            scopes.push(HashSet::new());
            scopes.last_mut().unwrap().insert(var.clone());
            for s in body {
                lint_stmt(s, functions, scopes, warnings);
            }
            scopes.pop();
        }
        StmtKind::Break | StmtKind::Continue => {}
        StmtKind::ExprStmt(e) => lint_expr(e, functions, scopes, warnings),
    }
}

fn lint_block(
    body: &[Stmt],
    functions: &HashSet<String>,
    scopes: &mut Vec<HashSet<String>>,
    warnings: &mut Vec<LintWarning>,
) {
    scopes.push(HashSet::new());
    for s in body {
        lint_stmt(s, functions, scopes, warnings);
    }
    scopes.pop();
}

fn lint_expr(
    e: &Expr,
    functions: &HashSet<String>,
    scopes: &mut Vec<HashSet<String>>,
    warnings: &mut Vec<LintWarning>,
) {
    lint_expr_at(
        e,
        functions,
        scopes,
        warnings,
        zirc_syntax::error::Span::default(),
    )
}

fn lint_expr_at(
    e: &Expr,
    functions: &HashSet<String>,
    scopes: &mut Vec<HashSet<String>>,
    warnings: &mut Vec<LintWarning>,
    span: zirc_syntax::error::Span,
) {
    match e {
        Expr::LiteralInt(_)
        | Expr::LiteralFloat(_)
        | Expr::LiteralString(_)
        | Expr::LiteralBool(_) => {}
        Expr::Ident(name) => {
            if !is_defined(scopes, name) {
                warnings.push(LintWarning::at(
                    "undefined-variable",
                    format!("undefined variable '{}'", name),
                    span,
                ));
            }
        }
        Expr::Member(base, _) => lint_expr(base, functions, scopes, warnings),
        Expr::BinaryAdd(a, b)
        | Expr::BinarySub(a, b)
        | Expr::BinaryMul(a, b)
        | Expr::BinaryDiv(a, b)
        | Expr::Eq(a, b)
        | Expr::Ne(a, b)
        | Expr::Lt(a, b)
        | Expr::Le(a, b)
        | Expr::Gt(a, b)
        | Expr::Ge(a, b)
        | Expr::LogicalAnd(a, b)
        | Expr::LogicalOr(a, b)
        | Expr::Index(a, b) => {
            lint_expr(a, functions, scopes, warnings);
            lint_expr(b, functions, scopes, warnings);
        }
        Expr::LogicalNot(a) | Expr::Neg(a) => lint_expr(a, functions, scopes, warnings),
        Expr::CallValue(callee, args) => {
            lint_expr(callee, functions, scopes, warnings);
            for a in args {
                lint_expr(a, functions, scopes, warnings);
            }
        }
        Expr::Closure(f) => {
            // Check the body with params (and only params) bound;
            // free variables are reported as undefined if unknown.
            let mut inner: Vec<HashSet<String>> =
                vec![f.params.iter().map(|p| p.name.clone()).collect()];
            for s in &f.body {
                lint_stmt(s, functions, &mut inner, warnings);
            }
        }
        Expr::Call { name, args } => {
            for a in args {
                lint_expr(a, functions, scopes, warnings);
            }
            let known = zirc_bytecode::Builtin::from_name(name).is_some()
                || functions.contains(name)
                || is_defined(scopes, name)
                || name
                    .rsplit('.')
                    .next()
                    .is_some_and(|last| functions.contains(last));
            if !known {
                warnings.push(LintWarning::new(
                    "undefined-function",
                    format!("undefined function '{}'", name),
                ));
            }
        }
        Expr::List(elems) => {
            for e in elems {
                lint_expr(e, functions, scopes, warnings);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zirc_lexer::Lexer;
    use zirc_parser::Parser;

    fn lint(src: &str) -> Vec<LintWarning> {
        let mut lexer = Lexer::new(src);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        lint_program(&parser.parse_program().unwrap())
    }

    #[test]
    fn catches_undefined_function() {
        let warnings = lint("show(1)\nnope(2)");
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].rule, "undefined-function");
    }

    #[test]
    fn catches_undefined_variable_and_block_scope() {
        let warnings = lint("if true: let y = 1 end\ny");
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].rule, "undefined-variable");
    }

    #[test]
    fn clean_program_has_no_warnings() {
        let warnings = lint("fun f(x): x + 1 end\nlet y = f(1)\nshow(y)");
        assert!(warnings.is_empty(), "got: {warnings:?}");
    }

    #[test]
    fn catches_duplicate_function() {
        let warnings = lint("fun f(): 1 end\nfun f(): 2 end");
        assert!(warnings.iter().any(|w| w.rule == "duplicate-function"));
    }
}
