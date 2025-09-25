use std::env;
use std::fs;
use std::path::PathBuf;

use zirc_lexer::Lexer;
use zirc_parser::Parser;
use zirc_syntax::ast::*;

fn main() {
    let mut args = env::args().skip(1).collect::<Vec<_>>();
    if args.is_empty() || args.iter().any(|a| a == "-h" || a == "--help") {
        eprintln!("Usage: zirc-fmt [--check|--write] <file.zirc>");
        std::process::exit(2);
    }
    let mut check = false;
    let mut write = false;
    let mut file = None;
    while let Some(a) = args.first().cloned() {
        if a == "--check" {
            check = true;
            args.remove(0);
        } else if a == "--write" {
            write = true;
            args.remove(0);
        } else {
            file = Some(PathBuf::from(a));
            args.remove(0);
            break;
        }
    }
    let file = file.expect("file required");
    let src = fs::read_to_string(&file).expect("read file");
    let mut lexer = Lexer::new(&src);
    let tokens = lexer.tokenize().unwrap_or_else(|e| {
        eprintln!("Lex error: {}", e);
        std::process::exit(1)
    });
    let mut parser = Parser::new(tokens);
    let program = parser.parse_program().unwrap_or_else(|e| {
        eprintln!("Parse error: {}", e);
        std::process::exit(1)
    });

    let formatted = format_program(&program);

    if check {
        if normalize_newlines(&formatted) != normalize_newlines(&src) {
            eprintln!("{}: not formatted", file.display());
            std::process::exit(1);
        } else {
            println!("{}: ok", file.display());
        }
    } else if write {
        fs::write(&file, formatted).expect("write");
    } else {
        print!("{}", formatted);
    }
}

fn normalize_newlines(s: &str) -> String {
    s.replace("\r\n", "\n")
}

fn format_program(p: &Program) -> String {
    let mut out = String::new();
    for (i, item) in p.items.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        match item {
            Item::Function(f) => out.push_str(&format_function(f)),
            Item::Stmt(s) => out.push_str(&format_stmt(s, 0)),
            Item::Module(name) => {
                out.push_str("module ");
                out.push_str(name);
                out.push('\n');
            }
            Item::Import { module, alias } => {
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
    match s {
        Stmt::Let { name, ty, expr } => {
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
        Stmt::Assign { name, expr } => {
            out.push_str(&pad);
            out.push_str(name);
            out.push_str(" = ");
            out.push_str(&format_expr(expr));
            out.push('\n');
        }
        Stmt::Return(e) => {
            out.push_str(&pad);
            out.push_str("return");
            if let Some(x) = e {
                out.push(' ');
                out.push_str(&format_expr(x));
            }
            out.push('\n');
        }
        Stmt::If {
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
        Stmt::While { cond, body } => {
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
        Stmt::For {
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
        Stmt::Break => {
            out.push_str(&pad);
            out.push_str("break\n");
        }
        Stmt::Continue => {
            out.push_str(&pad);
            out.push_str("continue\n");
        }
        Stmt::ExprStmt(e) => {
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
        Expr::LiteralString(s) => format!("\"{}\"", s.replace('\"', "\\\"")),
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
fn wrap(e: &Expr) -> String {
    match e {
        Expr::LiteralInt(_)
        | Expr::LiteralString(_)
        | Expr::LiteralBool(_)
        | Expr::Ident(_)
        | Expr::Member(_, _)
        | Expr::Call { .. } => format_expr(e),
        _ => format!("({})", format_expr(e)),
    }
}
