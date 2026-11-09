//! Zirc compiler: translates the AST into stack-based bytecode.
//!
//! [`Compiler::compile`] takes a [`zirc_syntax::ast::Program`] and
//! produces a [`zirc_bytecode::Program`] for the canonical VM backend
//! in `zirc-vm`. The compiler resolves builtin names through the
//! single registry in `zirc-bytecode`, compiles `push`/`pop` on a
//! variable to the dedicated `PushToVar`/`PopFromVar` instructions,
//! and gives `if`/`while`/`for` bodies their own local scopes.

pub mod builder;
pub mod compiler;

pub use compiler::Compiler;

#[cfg(test)]
mod tests {
    use super::*;
    use zirc_bytecode::{Builtin, Instruction};
    use zirc_syntax::ast::*;

    fn create_simple_program(items: Vec<Item>) -> Program {
        Program { items }
    }

    fn span() -> zirc_syntax::error::Span {
        zirc_syntax::error::Span::default()
    }

    fn st(kind: StmtKind) -> Stmt {
        Stmt { span: span(), kind }
    }

    fn it(kind: ItemKind) -> Item {
        Item { span: span(), kind }
    }

    fn param(name: &str) -> Param {
        Param {
            name: name.to_string(),
            ty: None,
            span: span(),
        }
    }

    fn func(name: &str, params: Vec<Param>, body: Vec<Stmt>) -> Function {
        Function {
            name: name.to_string(),
            params,
            return_type: None,
            body,
            span: span(),
        }
    }

    #[test]
    fn test_compiler_new() {
        let compiler = Compiler::new();
        assert!(compiler.function_names().is_empty());
    }

    #[test]
    fn test_compile_simple_expression() {
        let mut compiler = Compiler::new();

        // Program: let x = 5 + 3
        let program = create_simple_program(vec![it(ItemKind::Stmt(st(StmtKind::Let {
            name: "x".to_string(),
            ty: None,
            expr: Expr::BinaryAdd(Box::new(Expr::LiteralInt(5)), Box::new(Expr::LiteralInt(3))),
        })))]);
        let bytecode = compiler.compile(program).unwrap();

        assert_eq!(bytecode.main.name, "__main");
        assert_eq!(bytecode.main.arity, 0);

        // Constant folding: `5 + 3` compiles to a single PushInt(8).
        let expected_ops = [
            Instruction::PushInt(8),
            Instruction::StoreGlobal("x".to_string()),
        ];
        for (i, expected) in expected_ops.iter().enumerate() {
            assert_eq!(&bytecode.main.code[i], expected);
        }
    }

    #[test]
    fn test_compile_function_definition() {
        let mut compiler = Compiler::new();

        // Program: fun add(a, b): a + b end
        let program = create_simple_program(vec![it(ItemKind::Function(func(
            "add",
            vec![param("a"), param("b")],
            vec![st(StmtKind::Return(Some(Expr::BinaryAdd(
                Box::new(Expr::Ident("a".to_string())),
                Box::new(Expr::Ident("b".to_string())),
            ))))],
        )))]);

        let bytecode = compiler.compile(program).unwrap();
        assert_eq!(compiler.function_names(), vec!["add".to_string()]);
        assert_eq!(bytecode.functions.len(), 1);
        let func = &bytecode.functions[0];
        assert_eq!(func.name, "add");
        assert_eq!(func.arity, 2);
        assert_eq!(func.code[0], Instruction::LoadLocal(0)); // load a
        assert_eq!(func.code[1], Instruction::LoadLocal(1)); // load b
        assert_eq!(func.code[2], Instruction::Add);
        assert_eq!(func.code[3], Instruction::Return);
    }

    #[test]
    fn test_compile_builtin_function_call() {
        let mut compiler = Compiler::new();

        // Program: show(42)
        let program = create_simple_program(vec![it(ItemKind::Stmt(st(StmtKind::ExprStmt(
            Expr::Call {
                name: "show".to_string(),
                args: vec![Expr::LiteralInt(42)],
            },
        ))))]);
        let bytecode = compiler.compile(program).unwrap();

        assert_eq!(bytecode.main.code[0], Instruction::PushInt(42));
        assert_eq!(
            bytecode.main.code[1],
            Instruction::BuiltinCall(Builtin::Show, 1)
        );
        assert_eq!(bytecode.main.code[2], Instruction::Pop);
    }

    #[test]
    fn test_compile_if_statement() {
        let mut compiler = Compiler::new();

        // Program: if true: let x = 1 else let x = 2 end
        let program = create_simple_program(vec![it(ItemKind::Stmt(st(StmtKind::If {
            cond: Expr::LiteralBool(true),
            then_body: vec![st(StmtKind::Let {
                name: "x".to_string(),
                ty: None,
                expr: Expr::LiteralInt(1),
            })],
            else_body: vec![st(StmtKind::Let {
                name: "x".to_string(),
                ty: None,
                expr: Expr::LiteralInt(2),
            })],
        })))]);
        let bytecode = compiler.compile(program).unwrap();

        assert_eq!(bytecode.main.code[0], Instruction::PushBool(true));
        assert!(matches!(bytecode.main.code[1], Instruction::JumpIfFalse(_)));
    }

    #[test]
    fn test_compile_while_loop() {
        let mut compiler = Compiler::new();

        // Program: while true: break end
        let program = create_simple_program(vec![it(ItemKind::Stmt(st(StmtKind::While {
            cond: Expr::LiteralBool(true),
            body: vec![st(StmtKind::Break)],
        })))]);
        let bytecode = compiler.compile(program).unwrap();

        assert_eq!(bytecode.main.code[0], Instruction::PushBool(true));
        assert!(matches!(bytecode.main.code[1], Instruction::JumpIfFalse(_)));
        assert!(matches!(bytecode.main.code[2], Instruction::Jump(_))); // break
    }

    #[test]
    fn test_compile_list_operations() {
        let mut compiler = Compiler::new();

        // Program: let arr = [1, 2, 3]; arr[1]
        let program = create_simple_program(vec![
            it(ItemKind::Stmt(st(StmtKind::Let {
                name: "arr".to_string(),
                ty: None,
                expr: Expr::List(vec![
                    Expr::LiteralInt(1),
                    Expr::LiteralInt(2),
                    Expr::LiteralInt(3),
                ]),
            }))),
            it(ItemKind::Stmt(st(StmtKind::ExprStmt(Expr::Index(
                Box::new(Expr::Ident("arr".to_string())),
                Box::new(Expr::LiteralInt(1)),
            ))))),
        ]);
        let bytecode = compiler.compile(program).unwrap();

        assert_eq!(bytecode.main.code[0], Instruction::PushInt(1));
        assert_eq!(bytecode.main.code[1], Instruction::PushInt(2));
        assert_eq!(bytecode.main.code[2], Instruction::PushInt(3));
        assert_eq!(bytecode.main.code[3], Instruction::MakeList(3));
        assert_eq!(
            bytecode.main.code[4],
            Instruction::StoreGlobal("arr".to_string())
        );
        assert_eq!(
            bytecode.main.code[5],
            Instruction::LoadGlobal("arr".to_string())
        );
        assert_eq!(bytecode.main.code[6], Instruction::PushInt(1));
        assert_eq!(bytecode.main.code[7], Instruction::Index);
    }

    #[test]
    fn test_builtin_of_function() {
        use crate::compiler::builtin_of;

        assert_eq!(builtin_of("show"), Some(Builtin::Show));
        assert_eq!(builtin_of("showf"), Some(Builtin::ShowF));
        assert_eq!(builtin_of("len"), Some(Builtin::Len));
        assert_eq!(builtin_of("unknown"), None);
    }

    #[test]
    fn test_duplicate_function_error() {
        let mut compiler = Compiler::new();

        let program = create_simple_program(vec![
            it(ItemKind::Function(func("test", vec![], vec![]))),
            it(ItemKind::Function(func("test", vec![], vec![]))),
        ]);
        let result = compiler.compile(program);
        assert!(result.is_err());
        assert!(result.unwrap_err().msg.contains("Duplicate function"));
    }

    #[test]
    fn test_compiler_default() {
        let compiler = Compiler::default();
        assert!(compiler.function_names().is_empty());
    }

    #[test]
    fn test_debug_info_records_statement_lines() {
        let mut compiler = Compiler::new();
        let program = create_simple_program(vec![it(ItemKind::Stmt(Stmt {
            span: zirc_syntax::error::Span::new(7, 3),
            kind: StmtKind::Let {
                name: "x".to_string(),
                ty: None,
                expr: Expr::LiteralInt(1),
            },
        }))]);
        let bytecode = compiler.compile(program).unwrap();
        assert!(!bytecode.main.debug.is_empty());
        assert!(bytecode.main.debug.iter().flatten().all(|(l, _)| *l == 7));
    }
}
