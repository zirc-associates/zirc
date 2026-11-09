//! `zirc-fmt` library: canonical pretty-printing of Zirc ASTs.
//!
//! The binary in `main.rs` is a thin CLI over [`format_program`].
//! Formatting is idempotent: formatting formatted output yields
//! the same text. `~` line comments are preserved as
//! `Item::Comment`/`Stmt::Comment` nodes; a comment written at the
//! end of a code line is moved to its own line before the next
//! statement, with its text intact.

use zirc_syntax::ast::*;

pub fn format_program(p: &Program) -> String {
    let mut out = String::new();
    for (i, item) in p.items.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        match &item.kind {
            ItemKind::Comment(text) => out.push_str(&format!("~ {text}\n")),
            ItemKind::Function(f) => out.push_str(&format_function(f)),
            ItemKind::Stmt(s) => out.push_str(&format_stmt(s, 0)),
            ItemKind::Module(name) => {
                // The parser requires a string literal here.
                out.push_str("module ");
                out.push_str(&format!("\"{}\"", name.replace('"', "\\\"")));
                out.push('\n');
            }
            ItemKind::Import { module, alias } => {
                out.push_str("import ");
                out.push_str(module);
                if let Some(a) = alias {
                    out.push_str(" as ");
                    out.push_str(a);
                }
                out.push('\n');
            }
        }
    }
    out
}

fn format_type(t: &Type) -> &'static str {
    match t {
        Type::Int => "int",
        Type::Float => "float",
        Type::String => "string",
        Type::Bool => "bool",
        Type::List => "list",
        Type::Unit => "unit",
    }
}

fn format_function(f: &Function) -> String {
    let mut out = String::new();
    out.push_str("fun ");
    out.push_str(&f.name);
    out.push('(');
    for (i, p) in f.params.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        out.push_str(&p.name);
        if let Some(ty) = &p.ty {
            out.push_str(": ");
            out.push_str(format_type(ty));
        }
    }
    out.push(')');
    if let Some(rt) = &f.return_type {
        out.push(' ');
        out.push('(');
        out.push_str(format_type(rt));
        out.push(')');
    }
    out.push_str(":\n");
    for s in &f.body {
        out.push_str(&format_stmt(s, 2));
    }
    out.push_str("end\n");
    out
}

fn format_stmt(s: &Stmt, indent: usize) -> String {
    let mut out = String::new();
    let pad = " ".repeat(indent);
    match &s.kind {
        StmtKind::Comment(text) => {
            out.push_str(&pad);
            out.push_str(&format!("~ {text}\n"));
        }
        StmtKind::Let { name, ty, expr } => {
            out.push_str(&pad);
            out.push_str("let ");
            out.push_str(name);
            if let Some(t) = ty {
                out.push_str(": ");
                out.push_str(format_type(t));
            }
            out.push_str(" = ");
            out.push_str(&format_expr(expr));
            out.push('\n');
        }
        StmtKind::Assign { name, expr } => {
            out.push_str(&pad);
            out.push_str(name);
            out.push_str(" = ");
            out.push_str(&format_expr(expr));
            out.push('\n');
        }
        StmtKind::Return(e) => {
            out.push_str(&pad);
            out.push_str("return");
            if let Some(x) = e {
                out.push(' ');
                out.push_str(&format_expr(x));
            }
            out.push('\n');
        }
        StmtKind::If {
            cond,
            then_body,
            else_body,
        } => {
            out.push_str(&pad);
            out.push_str("if ");
            out.push_str(&format_expr(cond));
            out.push_str(":\n");
            for st in then_body {
                out.push_str(&format_stmt(st, indent + 2));
            }
            if !else_body.is_empty() {
                out.push_str(&pad);
                out.push_str("else:\n");
                for st in else_body {
                    out.push_str(&format_stmt(st, indent + 2));
                }
            }
            out.push_str(&pad);
            out.push_str("end\n");
        }
        StmtKind::While { cond, body } => {
            out.push_str(&pad);
            out.push_str("while ");
            out.push_str(&format_expr(cond));
            out.push_str(":\n");
            for st in body {
                out.push_str(&format_stmt(st, indent + 2));
            }
            out.push_str(&pad);
            out.push_str("end\n");
        }
        StmtKind::For {
            var,
            start,
            end,
            body,
        } => {
            out.push_str(&pad);
            out.push_str("for ");
            out.push_str(var);
            out.push_str(" in ");
            out.push_str(&format_expr(start));
            out.push_str("..");
            out.push_str(&format_expr(end));
            out.push_str(":\n");
            for st in body {
                out.push_str(&format_stmt(st, indent + 2));
            }
            out.push_str(&pad);
            out.push_str("end\n");
        }
        StmtKind::Break => {
            out.push_str(&pad);
            out.push_str("break\n");
        }
        StmtKind::Continue => {
            out.push_str(&pad);
            out.push_str("continue\n");
        }
        StmtKind::ExprStmt(e) => {
            out.push_str(&pad);
            out.push_str(&format_expr(e));
            out.push('\n');
        }
    }
    out
}

fn format_expr(e: &Expr) -> String {
    match e {
        Expr::LiteralInt(n) => n.to_string(),
        Expr::LiteralFloat(v) => zirc_bytecode::value::format_float(*v),
        Expr::LiteralString(s) => format!("\"{}\"", escape_string(s)),
        Expr::LiteralBool(b) => {
            if *b {
                "true".into()
            } else {
                "false".into()
            }
        }
        Expr::Ident(s) => s.clone(),
        Expr::Member(base, field) => format!("{}.{}", wrap(base), field),
        Expr::BinaryAdd(a, b) => bin("+", a, b),
        Expr::BinarySub(a, b) => bin("-", a, b),
        Expr::BinaryMul(a, b) => bin("*", a, b),
        Expr::BinaryDiv(a, b) => bin("/", a, b),
        Expr::Eq(a, b) => bin("==", a, b),
        Expr::Ne(a, b) => bin("!=", a, b),
        Expr::Lt(a, b) => bin("<", a, b),
        Expr::Le(a, b) => bin("<=", a, b),
        Expr::Gt(a, b) => bin(">", a, b),
        Expr::Ge(a, b) => bin(">=", a, b),
        Expr::LogicalAnd(a, b) => bin("&&", a, b),
        Expr::LogicalOr(a, b) => bin("||", a, b),
        Expr::LogicalNot(x) => format!("!{}", wrap(x)),
        Expr::Neg(x) => format!("-{}", wrap(x)),
        Expr::List(items) => {
            let mut s = String::new();
            s.push('[');
            for (i, it) in items.iter().enumerate() {
                if i > 0 {
                    s.push_str(", ");
                }
                s.push_str(&format_expr(it));
            }
            s.push(']');
            s
        }
        Expr::Index(base, idx) => format!("{}[{}]", wrap(base), format_expr(idx)),
        Expr::Closure(f) => {
            let mut s = String::new();
            s.push_str("fun(");
            for (i, p) in f.params.iter().enumerate() {
                if i > 0 {
                    s.push_str(", ");
                }
                s.push_str(&p.name);
                if let Some(ty) = &p.ty {
                    s.push_str(": ");
                    s.push_str(format_type(ty));
                }
            }
            s.push_str("):\n");
            for st in &f.body {
                s.push_str(&format_stmt(st, 2));
            }
            s.push_str("end");
            s
        }
        Expr::CallValue(callee, args) => {
            let mut s = String::new();
            s.push_str(&wrap(callee));
            s.push('(');
            for (i, a) in args.iter().enumerate() {
                if i > 0 {
                    s.push_str(", ");
                }
                s.push_str(&format_expr(a));
            }
            s.push(')');
            s
        }
        Expr::Call { name, args } => {
            let mut s = String::new();
            s.push_str(name);
            s.push('(');
            for (i, a) in args.iter().enumerate() {
                if i > 0 {
                    s.push_str(", ");
                }
                s.push_str(&format_expr(a));
            }
            s.push(')');
            s
        }
    }
}

fn bin(op: &str, a: &Expr, b: &Expr) -> String {
    format!("{} {} {}", wrap(a), op, wrap(b))
}

/// Escape a string value the way the lexer understands: `\\`, `\"`,
/// `\n`, `\t`, `\r`. Emitting raw control characters would change
/// the string on re-lexing (lossy round-trip).
fn escape_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            other => out.push(other),
        }
    }
    out
}

fn wrap(e: &Expr) -> String {
    match e {
        Expr::LiteralInt(_)
        | Expr::LiteralFloat(_)
        | Expr::LiteralString(_)
        | Expr::LiteralBool(_)
        | Expr::Ident(_)
        | Expr::Member(_, _)
        | Expr::Call { .. }
        | Expr::CallValue(_, _)
        | Expr::Index(_, _)
        | Expr::List(_) => format_expr(e),
        _ => format!("({})", format_expr(e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zirc_lexer::Lexer;
    use zirc_parser::Parser;

    fn parse(src: &str) -> Program {
        let mut lexer = Lexer::new(src);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        parser.parse_program().unwrap()
    }

    #[test]
    fn formatting_is_idempotent() {
        let src = "let x=1+2\nfun f(a,b):a*b end\nif x>1: showf(\"hi\") else: showf(\"lo\") end\nfor i in 0..3: show(i) end\n";
        let once = format_program(&parse(src));
        let twice = format_program(&parse(&once));
        assert_eq!(once, twice);
    }

    #[test]
    fn formats_floats_and_negation() {
        let out = format_program(&parse("let pi = 3.14\nlet n = -pi\n"));
        assert!(out.contains("3.14"), "got: {out}");
        assert!(out.contains("-pi"), "got: {out}");
    }

    #[test]
    fn formats_modules_and_imports() {
        let out = format_program(&parse("module \"math\"\nimport strings as s\n"));
        // Must re-parse (parser requires a string literal after `module`).
        assert!(out.contains("module \"math\""), "got: {out}");
        assert!(out.contains("import strings as s"), "got: {out}");
        let twice = format_program(&parse(&out));
        assert_eq!(out, twice);
    }

    #[test]
    fn string_escapes_round_trip() {
        // `\n`, `\t`, `\\` and `\"` must survive format -> re-lex.
        let src = "let s = \"a\\nb\\tc\\\\d\\\"e\"\n";
        let out = format_program(&parse(src));
        assert!(out.contains("\\n"), "got: {out}");
        assert!(out.contains("\\t"), "got: {out}");
        assert!(out.contains("\\\\"), "got: {out}");
        // The string itself must not contain a raw newline (only
        // the statement-terminating newline at the end may).
        assert_eq!(
            out.lines().count(),
            1,
            "raw newline leaked into string: {out:?}"
        );
        let twice = format_program(&parse(&out));
        assert_eq!(out, twice);
    }

    #[test]
    fn preserves_comments() {
        let src = "~ header comment\nlet x = 1\n~ mid comment\nfun f(a):\n  ~ inner comment\n  return a\nend\n";
        let out = format_program(&parse(src));
        assert!(out.contains("~ header comment"), "got: {out}");
        assert!(out.contains("~ mid comment"), "got: {out}");
        assert!(out.contains("~ inner comment"), "got: {out}");
        // And formatting the result keeps the comments (idempotent).
        let twice = format_program(&parse(&out));
        assert_eq!(out, twice);
    }

    #[test]
    fn all_shipped_zirc_files_are_idempotent() {
        // Every .zirc file in examples/, stdlib/ and benchmark/scripts/
        // must format -> re-parse -> format to the same text. This
        // sweep catches formatter/parser drift (e.g. module quotes,
        // string escapes) on real code instead of snippets.
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..");
        let mut files: Vec<std::path::PathBuf> = Vec::new();
        for dir in ["examples", "stdlib", "benchmark/scripts"] {
            let base = root.join(dir);
            if !base.exists() {
                continue;
            }
            let mut stack = vec![base];
            while let Some(dir) = stack.pop() {
                for entry in std::fs::read_dir(&dir).unwrap().flatten() {
                    let p = entry.path();
                    if p.is_dir() {
                        stack.push(p);
                    } else if p.extension().and_then(|e| e.to_str()) == Some("zirc") {
                        files.push(p);
                    }
                }
            }
        }
        assert!(!files.is_empty(), "no .zirc files found under {root:?}");
        for file in &files {
            let src = std::fs::read_to_string(file).unwrap();
            // Some shipped files are intentionally broken (errors/).
            let Ok(program) = std::panic::catch_unwind(|| parse(&src)) else {
                continue;
            };
            let once = format_program(&program);
            let twice = format_program(&parse(&once));
            assert_eq!(
                once,
                twice,
                "formatting is not idempotent for {}",
                file.display()
            );
        }
    }

    #[test]
    fn formats_closures() {
        let out = format_program(&parse("let add = fun(x, y): x + y end\n"));
        assert!(out.contains("fun(x, y):"), "got: {out}");
        let twice = format_program(&parse(&out));
        assert_eq!(out, twice);
    }
}
