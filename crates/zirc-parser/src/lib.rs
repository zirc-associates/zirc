//! Zirc parser: builds an AST from a token stream.
//!
//! A hand-written recursive-descent parser with precedence climbing
//! for binary operators. It produces a [`zirc_syntax::ast::Program`]
//! from the tokens yielded by the lexer. Syntax failures
//! are reported as [`zirc_syntax::error::Error`]s with kind `Parse`
//! and a 1-based line/column span, never as panics.
//!
//! # Example
//!
//! ```rust
//! use zirc_lexer::Lexer;
//! use zirc_parser::Parser;
//!
//! let mut lexer = Lexer::new("let x = 42");
//! let tokens = lexer.tokenize().unwrap();
//! let mut parser = Parser::new(tokens);
//! let program = parser.parse_program().unwrap();
//! assert_eq!(program.items.len(), 1);
//! ```

pub mod parser;

pub use parser::Parser;

#[cfg(test)]
mod tests {
    use super::*;
    use zirc_lexer::Lexer;
    use zirc_syntax::ast::*;

    fn parse_expr_str(input: &str) -> Expr {
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize().expect("Lexing should succeed");
        let mut parser = Parser::new(tokens);
        parser.parse_expr().expect("Parsing should succeed")
    }

    fn parse_program_str(input: &str) -> Program {
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize().expect("Lexing should succeed");
        let mut parser = Parser::new(tokens);
        parser.parse_program().expect("Parsing should succeed")
    }

    #[test]
    fn test_literal_expressions() {
        assert!(matches!(parse_expr_str("42"), Expr::LiteralInt(42)));
        assert!(matches!(parse_expr_str("\"hello\""), Expr::LiteralString(s) if s == "hello"));
        assert!(matches!(parse_expr_str("true"), Expr::LiteralBool(true)));
        assert!(matches!(parse_expr_str("false"), Expr::LiteralBool(false)));
    }

    #[test]
    fn test_identifier_expressions() {
        assert!(matches!(parse_expr_str("variable"), Expr::Ident(s) if s == "variable"));
        assert!(matches!(parse_expr_str("my_var"), Expr::Ident(s) if s == "my_var"));
    }

    #[test]
    fn test_binary_arithmetic() {
        assert!(matches!(parse_expr_str("1 + 2"), Expr::BinaryAdd(_, _)));
        assert!(matches!(parse_expr_str("5 - 3"), Expr::BinarySub(_, _)));
        assert!(matches!(parse_expr_str("4 * 6"), Expr::BinaryMul(_, _)));
        assert!(matches!(parse_expr_str("8 / 2"), Expr::BinaryDiv(_, _)));
    }

    #[test]
    fn test_comparison_operations() {
        assert!(matches!(parse_expr_str("1 == 2"), Expr::Eq(_, _)));
        assert!(matches!(parse_expr_str("1 != 2"), Expr::Ne(_, _)));
        assert!(matches!(parse_expr_str("1 < 2"), Expr::Lt(_, _)));
        assert!(matches!(parse_expr_str("1 <= 2"), Expr::Le(_, _)));
        assert!(matches!(parse_expr_str("1 > 2"), Expr::Gt(_, _)));
        assert!(matches!(parse_expr_str("1 >= 2"), Expr::Ge(_, _)));
    }

    #[test]
    fn test_logical_operations() {
        assert!(matches!(
            parse_expr_str("true && false"),
            Expr::LogicalAnd(_, _)
        ));
        assert!(matches!(
            parse_expr_str("true || false"),
            Expr::LogicalOr(_, _)
        ));
        assert!(matches!(parse_expr_str("!true"), Expr::LogicalNot(_)));
    }

    #[test]
    fn test_parenthesized_expressions() {
        assert!(matches!(parse_expr_str("(1 + 2)"), Expr::BinaryAdd(_, _)));
    }

    #[test]
    fn test_operator_precedence() {
        // These should parse without errors and have correct structure
        assert!(matches!(parse_expr_str("1 + 2 * 3"), Expr::BinaryAdd(_, _)));
        assert!(matches!(parse_expr_str("2 * 3 + 1"), Expr::BinaryAdd(_, _)));
    }

    #[test]
    fn test_list_expressions() {
        if let Expr::List(items) = parse_expr_str("[1, 2, 3]") {
            assert_eq!(items.len(), 3);
        } else {
            panic!("Expected List");
        }

        if let Expr::List(items) = parse_expr_str("[]") {
            assert_eq!(items.len(), 0);
        } else {
            panic!("Expected empty List");
        }
    }

    #[test]
    fn test_index_expressions() {
        assert!(matches!(parse_expr_str("arr[0]"), Expr::Index(_, _)));
    }

    #[test]
    fn test_function_calls() {
        if let Expr::Call { name, args } = parse_expr_str("foo()") {
            assert_eq!(name, "foo");
            assert_eq!(args.len(), 0);
        } else {
            panic!("Expected Call");
        }

        if let Expr::Call { name, args } = parse_expr_str("add(1, 2)") {
            assert_eq!(name, "add");
            assert_eq!(args.len(), 2);
        } else {
            panic!("Expected Call");
        }
    }

    #[test]
    fn test_basic_parsing() {
        // Just test that basic programs parse without error
        let _ = parse_program_str("let x = 42");
        let _ = parse_program_str("let x: int = 42");
        let _ = parse_program_str("x = 42");
        let _ = parse_program_str("if true: let x = 1 end");
        let _ = parse_program_str("if true: let x = 1 else: let x = 2 end");
        let _ = parse_program_str("while true: break end");
        let _ = parse_program_str("for i in 0..10: break end");
        let _ = parse_program_str("fun add(x, y): x + y end");
        let _ = parse_program_str("fun add(x: int, y: int) (int): x + y end");
        let _ = parse_program_str("return 42");
        let _ = parse_program_str("break");
        let _ = parse_program_str("continue");
    }
}
