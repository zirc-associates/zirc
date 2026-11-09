//! Static analyses shared by both engines.
//!
//! Currently: free-variable computation for closures. The compiler
//! uses it to decide which variables a closure captures (and in
//! which order); the interpreter uses it to snapshot capture values
//! at closure creation time. Sharing the analysis guarantees both
//! engines capture the same variables.

use std::collections::HashSet;

use crate::ast::{Expr, Stmt, StmtKind};

/// Free variables of a function body, given its parameter names.
///
/// A variable is free when it is used but not bound in the body.
/// `let` binds from that statement onward (block-scoped), the
/// `for` variable is bound inside its body, and free variables of
/// nested closures that are not bound here propagate upward.
/// Results are in first-use order, without duplicates.
pub fn free_vars(params: &[String], body: &[Stmt]) -> Vec<String> {
    let mut bound: HashSet<String> = params.iter().cloned().collect();
    let mut free: Vec<String> = Vec::new();
    let mut free_set: HashSet<String> = HashSet::new();
    collect_stmts(body, &mut bound, &mut free, &mut free_set);
    free
}

fn note_use(
    name: &str,
    bound: &HashSet<String>,
    free: &mut Vec<String>,
    free_set: &mut HashSet<String>,
) {
    if !bound.contains(name) && free_set.insert(name.to_string()) {
        free.push(name.to_string());
    }
}

fn collect_stmts(
    stmts: &[Stmt],
    bound: &mut HashSet<String>,
    free: &mut Vec<String>,
    free_set: &mut HashSet<String>,
) {
    for s in stmts {
        collect_stmt(s, bound, free, free_set);
    }
}

fn collect_block(
    stmts: &[Stmt],
    bound: &mut HashSet<String>,
    free: &mut Vec<String>,
    free_set: &mut HashSet<String>,
) {
    // Block scope: bindings created inside do not escape.
    let saved: Vec<String> = bound.iter().cloned().collect();
    collect_stmts(stmts, bound, free, free_set);
    *bound = saved.into_iter().collect();
}

fn collect_stmt(
    s: &Stmt,
    bound: &mut HashSet<String>,
    free: &mut Vec<String>,
    free_set: &mut HashSet<String>,
) {
    match &s.kind {
        StmtKind::Comment(_) => {}
        StmtKind::Let { name, expr, .. } => {
            collect_expr(expr, bound, free, free_set);
            bound.insert(name.clone());
        }
        StmtKind::Assign { name, expr } => {
            note_use(name, bound, free, free_set);
            collect_expr(expr, bound, free, free_set);
        }
        StmtKind::Return(e) => {
            if let Some(e) = e {
                collect_expr(e, bound, free, free_set);
            }
        }
        StmtKind::If {
            cond,
            then_body,
            else_body,
        } => {
            collect_expr(cond, bound, free, free_set);
            collect_block(then_body, bound, free, free_set);
            collect_block(else_body, bound, free, free_set);
        }
        StmtKind::While { cond, body } => {
            collect_expr(cond, bound, free, free_set);
            collect_block(body, bound, free, free_set);
        }
        StmtKind::For {
            var,
            start,
            end,
            body,
        } => {
            collect_expr(start, bound, free, free_set);
            collect_expr(end, bound, free, free_set);
            let mut inner = bound.clone();
            inner.insert(var.clone());
            collect_block_with(body, &mut inner, free, free_set);
        }
        StmtKind::Break | StmtKind::Continue => {}
        StmtKind::ExprStmt(e) => collect_expr(e, bound, free, free_set),
    }
}

fn collect_block_with(
    stmts: &[Stmt],
    bound: &mut HashSet<String>,
    free: &mut Vec<String>,
    free_set: &mut HashSet<String>,
) {
    collect_stmts(stmts, bound, free, free_set);
}

fn collect_expr(
    e: &Expr,
    bound: &mut HashSet<String>,
    free: &mut Vec<String>,
    free_set: &mut HashSet<String>,
) {
    match e {
        Expr::LiteralInt(_)
        | Expr::LiteralFloat(_)
        | Expr::LiteralString(_)
        | Expr::LiteralBool(_) => {}
        Expr::Ident(name) => note_use(name, bound, free, free_set),
        Expr::Member(base, _) => collect_expr(base, bound, free, free_set),
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
            collect_expr(a, bound, free, free_set);
            collect_expr(b, bound, free, free_set);
        }
        Expr::LogicalNot(a) | Expr::Neg(a) => collect_expr(a, bound, free, free_set),
        Expr::Call { args, .. } => {
            for a in args {
                collect_expr(a, bound, free, free_set);
            }
        }
        Expr::CallValue(callee, args) => {
            collect_expr(callee, bound, free, free_set);
            for a in args {
                collect_expr(a, bound, free, free_set);
            }
        }
        Expr::Closure(f) => {
            // Free variables of the nested closure that are not
            // bound here are free here too (captured through).
            let inner_params: Vec<String> = f.params.iter().map(|p| p.name.clone()).collect();
            for name in free_vars(&inner_params, &f.body) {
                note_use(&name, bound, free, free_set);
            }
        }
        Expr::List(elems) => {
            for e in elems {
                collect_expr(e, bound, free, free_set);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn param(name: &str) -> crate::ast::Param {
        crate::ast::Param {
            name: name.to_string(),
            ty: None,
            span: crate::error::Span::default(),
        }
    }

    fn stmt(kind: StmtKind) -> Stmt {
        Stmt {
            span: crate::error::Span::default(),
            kind,
        }
    }

    #[test]
    fn free_vars_basic() {
        let body = vec![stmt(StmtKind::ExprStmt(Expr::BinaryAdd(
            Box::new(Expr::Ident("x".into())),
            Box::new(Expr::Ident("y".into())),
        )))];
        assert_eq!(free_vars(&["x".to_string()], &body), vec!["y"]);
    }

    #[test]
    fn let_binds_after_rhs() {
        let body = vec![stmt(StmtKind::Let {
            name: "a".into(),
            ty: None,
            expr: Expr::Ident("a".into()),
        })];
        // RHS use of `a` is free; the binding itself is not.
        assert_eq!(free_vars(&[], &body), vec!["a"]);
    }

    #[test]
    fn nested_closure_propagates() {
        let inner = crate::ast::Function {
            name: String::new(),
            params: vec![param("v")],
            return_type: None,
            body: vec![stmt(StmtKind::ExprStmt(Expr::BinaryAdd(
                Box::new(Expr::Ident("v".into())),
                Box::new(Expr::Ident("outer".into())),
            )))],
            span: crate::error::Span::default(),
        };
        let body = vec![stmt(StmtKind::ExprStmt(Expr::Closure(Box::new(inner))))];
        assert_eq!(free_vars(&[], &body), vec!["outer"]);
    }
}
