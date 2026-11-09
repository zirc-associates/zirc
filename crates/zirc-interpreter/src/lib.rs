//! Zirc interpreter: evaluates AST nodes with a simple tree-walking interpreter.
//!
//! This crate provides the runtime evaluation system for the Zirc programming language.
//! It implements a tree-walking interpreter that directly executes Abstract Syntax Tree (AST) nodes
//! produced by the parser.

pub mod env;
pub mod flow;
pub mod interpreter;
pub mod value;

pub use env::Env;
pub use interpreter::{Interpreter, MemoryStats};
pub use value::Value;

#[cfg(test)]
mod tests {
    use super::*;
    use zirc_lexer::Lexer;
    use zirc_parser::Parser;

    fn run_program(input: &str) -> Result<Option<Value>, String> {
        let mut lexer = Lexer::new(input);
        let tokens = lexer
            .tokenize()
            .map_err(|e| format!("Lex error: {}", e.msg))?;
        let mut parser = Parser::new(tokens);
        let program = parser
            .parse_program()
            .map_err(|e| format!("Parse error: {}", e.msg))?;
        let mut interpreter = Interpreter::new();
        interpreter
            .run_with_env(program, &mut Env::new_root())
            .map_err(|e| format!("Runtime error: {}", e.msg))
    }

    fn expect_value(input: &str, expected: Value) {
        match run_program(input) {
            Ok(Some(actual)) => assert_eq!(actual, expected, "Program: {}", input),
            Ok(None) => panic!("Expected value but got None for: {}", input),
            Err(e) => panic!("Program failed: {}\nInput: {}", e, input),
        }
    }

    fn expect_unit(input: &str) {
        match run_program(input) {
            Ok(result) => assert!(
                result.is_none() || result == Some(Value::Unit),
                "Expected unit for: {}",
                input
            ),
            Err(e) => panic!("Program failed: {}\nInput: {}", e, input),
        }
    }

    fn expect_error(input: &str) {
        if run_program(input).is_ok() {
            panic!("Expected error but program succeeded: {}", input);
        }
    }

    #[test]
    fn test_literal_values() {
        expect_value("42", Value::Int(42));
        expect_value("\"hello\"", Value::Str("hello".to_string()));
        expect_value("true", Value::Bool(true));
        expect_value("false", Value::Bool(false));
    }

    #[test]
    fn test_arithmetic_operations() {
        expect_value("1 + 2", Value::Int(3));
        expect_value("5 - 3", Value::Int(2));
        expect_value("4 * 6", Value::Int(24));
        expect_value("8 / 2", Value::Int(4));
        expect_value("2 + 3 * 4", Value::Int(14)); // Tests precedence
        expect_value("(2 + 3) * 4", Value::Int(20)); // Tests parentheses
    }

    #[test]
    fn test_string_operations() {
        expect_value(
            "\"hello\" + \" \" + \"world\"",
            Value::Str("hello world".to_string()),
        );
        expect_value("\"test\"[0]", Value::Str("t".to_string()));
        expect_value("\"test\"[1]", Value::Str("e".to_string()));
    }

    #[test]
    fn test_comparison_operations() {
        expect_value("5 > 3", Value::Bool(true));
        expect_value("3 > 5", Value::Bool(false));
        expect_value("5 >= 5", Value::Bool(true));
        expect_value("3 < 5", Value::Bool(true));
        expect_value("5 <= 5", Value::Bool(true));
        expect_value("5 == 5", Value::Bool(true));
        expect_value("5 != 3", Value::Bool(true));
    }

    #[test]
    fn test_logical_operations() {
        expect_value("true && true", Value::Bool(true));
        expect_value("true && false", Value::Bool(false));
        expect_value("false || true", Value::Bool(true));
        expect_value("false || false", Value::Bool(false));
        expect_value("!true", Value::Bool(false));
        expect_value("!false", Value::Bool(true));
    }

    #[test]
    fn test_variables() {
        expect_value("let x = 42\nx", Value::Int(42));
        expect_value("let x = 10\nlet y = 20\nx + y", Value::Int(30));
        expect_unit("let x = 5\nx = 10"); // Assignment returns unit
    }

    #[test]
    fn test_lists() {
        expect_value(
            "[1, 2, 3]",
            Value::List(vec![Value::Int(1), Value::Int(2), Value::Int(3)]),
        );
        expect_value("[]", Value::List(vec![]));
        expect_value("[1, 2, 3][1]", Value::Int(2));
        expect_value(
            "[1, 2] + [3, 4]",
            Value::List(vec![
                Value::Int(1),
                Value::Int(2),
                Value::Int(3),
                Value::Int(4),
            ]),
        );
    }

    #[test]
    fn test_control_flow() {
        expect_value("if true: 42 else: 0 end", Value::Int(42));
        expect_value("if false: 42 else: 0 end", Value::Int(0));
        expect_value("if true: 42 end", Value::Int(42));
    }

    #[test]
    fn test_loops() {
        expect_unit("while false: break end"); // Never executes body
        expect_unit("for i in 0..0: break end"); // Empty range
    }

    #[test]
    fn test_functions() {
        expect_value("fun double(x): x * 2 end\ndouble(21)", Value::Int(42));
        expect_value("fun add(x, y): x + y end\nadd(10, 20)", Value::Int(30));
        expect_value(
            "fun fact(n): if n == 0: 1 else: n * fact(n - 1) end end\nfact(5)",
            Value::Int(120),
        );
    }

    #[test]
    fn test_builtin_functions() {
        // Test len
        expect_value("len(\"hello\")", Value::Int(5));
        expect_value("len([1, 2, 3, 4])", Value::Int(4));
        expect_value("len([])", Value::Int(0));

        // Test slice
        expect_value("slice(\"hello\", 1, 4)", Value::Str("ell".to_string()));
        expect_value(
            "slice([1, 2, 3, 4, 5], 1, 4)",
            Value::List(vec![Value::Int(2), Value::Int(3), Value::Int(4)]),
        );
    }

    #[test]
    fn test_error_cases() {
        expect_error("undefined_var");
        expect_error("1 + \"string\""); // Type mismatch
        expect_error("[1, 2, 3][10]"); // Index out of bounds
        expect_error("undefined_function()");
        expect_error("len(42)"); // len expects string or list
    }

    #[test]
    fn test_type_checking() {
        expect_value("let x: int = 42\nx", Value::Int(42));
        expect_error("let x: int = \"string\""); // Type mismatch
        expect_error("let x: string = 42"); // Type mismatch
    }

    #[test]
    fn test_division_by_zero_is_error() {
        expect_error("10 / 0");
        expect_error("let x = 1\nx / 0");
    }

    #[test]
    fn test_list_type_annotation() {
        expect_value(
            "let xs: list = [1, 2]\nxs",
            Value::List(vec![Value::Int(1), Value::Int(2)]),
        );
        expect_error("let xs: list = 42");
    }

    #[test]
    fn test_block_scoping() {
        // `let` inside an if-block does not leak out.
        expect_error("if true: let y = 1 end\ny");
        // Loop variable is scoped to the loop.
        expect_error("for i in 0..3: show(i) end\ni");
        // Assignments to outer bindings persist after a block.
        expect_value("let x = 1\nif true: x = 42 end\nx", Value::Int(42));
        // A pre-existing loop variable is restored after the loop.
        expect_value("let i = 99\nfor i in 0..2: show(i) end\ni", Value::Int(99));
    }

    #[test]
    fn test_complex_programs() {
        let fibonacci = r#"
            fun fib(n):
                if n <= 1:
                    return n
                else:
                    return fib(n - 1) + fib(n - 2)
                end
            end
            fib(10)
        "#;
        expect_value(fibonacci, Value::Int(55));

        let list_processing = r#"
            let nums = [1, 2, 3, 4, 5]
            let doubled = []
            for i in 0..len(nums):
                push(doubled, nums[i] * 2)
            end
            doubled
        "#;
        expect_value(
            list_processing,
            Value::List(vec![
                Value::Int(2),
                Value::Int(4),
                Value::Int(6),
                Value::Int(8),
                Value::Int(10),
            ]),
        );
    }
}
