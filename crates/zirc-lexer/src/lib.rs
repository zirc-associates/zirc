//! Zirc lexer: converts source text into tokens.
//!
//! The lexer scans Zirc source code in a single pass and produces a
//! `Vec<Token>` terminated by an `Eof` token. Comments start with
//! `~` and run to the end of the line. Lexical failures (invalid
//! numbers, unterminated strings, unexpected characters) are reported
//! as [`zirc_syntax::error::Error`]s with kind `Lex` and a 1-based
//! line/column span.
//!
//! # Example
//!
//! ```rust
//! use zirc_lexer::Lexer;
//!
//! let mut lexer = Lexer::new("let x = 42");
//! let tokens = lexer.tokenize().unwrap();
//! assert!(tokens.len() > 1);
//! ```

pub mod lexer;

pub use lexer::Lexer;

#[cfg(test)]
mod tests {
    use super::*;
    use zirc_syntax::token::TokenKind;

    fn expect_tokens(input: &str, expected: Vec<TokenKind>) {
        let mut lexer = Lexer::new(input);
        let tokens = lexer.tokenize().expect("Lexing should succeed");

        let actual_kinds: Vec<TokenKind> = tokens.iter().map(|t| t.kind.clone()).collect();
        assert_eq!(
            actual_kinds, expected,
            "Token mismatch for input: '{}'",
            input
        );
    }

    #[test]
    fn test_basic_tokens() {
        expect_tokens(
            "()",
            vec![TokenKind::LParen, TokenKind::RParen, TokenKind::Eof],
        );
        expect_tokens(
            "[]",
            vec![TokenKind::LBracket, TokenKind::RBracket, TokenKind::Eof],
        );
        expect_tokens(
            ",:",
            vec![TokenKind::Comma, TokenKind::Colon, TokenKind::Eof],
        );
    }

    #[test]
    fn test_operators() {
        expect_tokens("+", vec![TokenKind::Plus, TokenKind::Eof]);
        expect_tokens("-", vec![TokenKind::Minus, TokenKind::Eof]);
        expect_tokens("*", vec![TokenKind::Star, TokenKind::Eof]);
        expect_tokens("/", vec![TokenKind::Slash, TokenKind::Eof]);
        expect_tokens("=", vec![TokenKind::Equal, TokenKind::Eof]);
        expect_tokens("==", vec![TokenKind::EqEq, TokenKind::Eof]);
        expect_tokens("!=", vec![TokenKind::NotEq, TokenKind::Eof]);
        expect_tokens("!", vec![TokenKind::Bang, TokenKind::Eof]);
        expect_tokens("<", vec![TokenKind::Less, TokenKind::Eof]);
        expect_tokens("<=", vec![TokenKind::LessEq, TokenKind::Eof]);
        expect_tokens(">", vec![TokenKind::Greater, TokenKind::Eof]);
        expect_tokens(">=", vec![TokenKind::GreaterEq, TokenKind::Eof]);
        expect_tokens("&&", vec![TokenKind::AndAnd, TokenKind::Eof]);
        expect_tokens("||", vec![TokenKind::OrOr, TokenKind::Eof]);
        expect_tokens("..", vec![TokenKind::DotDot, TokenKind::Eof]);
    }

    #[test]
    fn test_keywords() {
        expect_tokens("fun", vec![TokenKind::Fun, TokenKind::Eof]);
        expect_tokens("end", vec![TokenKind::End, TokenKind::Eof]);
        expect_tokens("if", vec![TokenKind::If, TokenKind::Eof]);
        expect_tokens("else", vec![TokenKind::Else, TokenKind::Eof]);
        expect_tokens("while", vec![TokenKind::While, TokenKind::Eof]);
        expect_tokens("for", vec![TokenKind::For, TokenKind::Eof]);
        expect_tokens("in", vec![TokenKind::In, TokenKind::Eof]);
        expect_tokens("let", vec![TokenKind::Let, TokenKind::Eof]);
        expect_tokens("return", vec![TokenKind::Return, TokenKind::Eof]);
        expect_tokens("break", vec![TokenKind::Break, TokenKind::Eof]);
        expect_tokens("continue", vec![TokenKind::Continue, TokenKind::Eof]);
        expect_tokens("true", vec![TokenKind::True, TokenKind::Eof]);
        expect_tokens("false", vec![TokenKind::False, TokenKind::Eof]);
    }

    #[test]
    fn test_numbers() {
        expect_tokens("42", vec![TokenKind::Number(42), TokenKind::Eof]);
        expect_tokens("0", vec![TokenKind::Number(0), TokenKind::Eof]);
        expect_tokens(
            "123456789",
            vec![TokenKind::Number(123456789), TokenKind::Eof],
        );
    }

    #[test]
    fn test_strings() {
        expect_tokens(
            "\"hello\"",
            vec![TokenKind::String("hello".to_string()), TokenKind::Eof],
        );
        expect_tokens(
            "\"\"",
            vec![TokenKind::String("".to_string()), TokenKind::Eof],
        );
        expect_tokens(
            "\"hello world\"",
            vec![TokenKind::String("hello world".to_string()), TokenKind::Eof],
        );
    }

    #[test]
    fn test_string_escapes() {
        expect_tokens(
            "\"\\n\"",
            vec![TokenKind::String("\n".to_string()), TokenKind::Eof],
        );
        expect_tokens(
            "\"\\t\"",
            vec![TokenKind::String("\t".to_string()), TokenKind::Eof],
        );
        expect_tokens(
            "\"\\r\"",
            vec![TokenKind::String("\r".to_string()), TokenKind::Eof],
        );
        expect_tokens(
            "\"\\\\\"",
            vec![TokenKind::String("\\".to_string()), TokenKind::Eof],
        );
        expect_tokens(
            "\"\\\"\"",
            vec![TokenKind::String("\"".to_string()), TokenKind::Eof],
        );
    }

    #[test]
    fn test_identifiers() {
        expect_tokens(
            "foo",
            vec![TokenKind::Ident("foo".to_string()), TokenKind::Eof],
        );
        expect_tokens(
            "my_var",
            vec![TokenKind::Ident("my_var".to_string()), TokenKind::Eof],
        );
        expect_tokens(
            "test123",
            vec![TokenKind::Ident("test123".to_string()), TokenKind::Eof],
        );
        expect_tokens(
            "_underscore",
            vec![TokenKind::Ident("_underscore".to_string()), TokenKind::Eof],
        );
    }

    #[test]
    fn test_comments() {
        // Comments are tokens now (preserved for the formatter);
        // the parser separates them out again.
        expect_tokens(
            "~ this is a comment",
            vec![
                TokenKind::Comment("this is a comment".to_string()),
                TokenKind::Eof,
            ],
        );
        expect_tokens(
            "42 ~ comment",
            vec![
                TokenKind::Number(42),
                TokenKind::Comment("comment".to_string()),
                TokenKind::Eof,
            ],
        );
        expect_tokens(
            "let x = 5 ~ variable",
            vec![
                TokenKind::Let,
                TokenKind::Ident("x".to_string()),
                TokenKind::Equal,
                TokenKind::Number(5),
                TokenKind::Comment("variable".to_string()),
                TokenKind::Eof,
            ],
        );
    }

    #[test]
    fn test_whitespace_handling() {
        expect_tokens("  42   ", vec![TokenKind::Number(42), TokenKind::Eof]);
        expect_tokens("\n42\n", vec![TokenKind::Number(42), TokenKind::Eof]);
        expect_tokens("\t42\t", vec![TokenKind::Number(42), TokenKind::Eof]);
    }

    #[test]
    fn test_complex_expression() {
        expect_tokens(
            "let x = (42 + 13) * 2",
            vec![
                TokenKind::Let,
                TokenKind::Ident("x".to_string()),
                TokenKind::Equal,
                TokenKind::LParen,
                TokenKind::Number(42),
                TokenKind::Plus,
                TokenKind::Number(13),
                TokenKind::RParen,
                TokenKind::Star,
                TokenKind::Number(2),
                TokenKind::Eof,
            ],
        );
    }

    #[test]
    fn test_function_definition() {
        expect_tokens(
            "fun add(x, y): x + y end",
            vec![
                TokenKind::Fun,
                TokenKind::Ident("add".to_string()),
                TokenKind::LParen,
                TokenKind::Ident("x".to_string()),
                TokenKind::Comma,
                TokenKind::Ident("y".to_string()),
                TokenKind::RParen,
                TokenKind::Colon,
                TokenKind::Ident("x".to_string()),
                TokenKind::Plus,
                TokenKind::Ident("y".to_string()),
                TokenKind::End,
                TokenKind::Eof,
            ],
        );
    }

    #[test]
    fn test_line_and_column_tracking() {
        let mut lexer = Lexer::new("hello\nworld");
        let tokens = lexer.tokenize().expect("Lexing should succeed");

        assert_eq!(tokens[0].line, 1);
        assert_eq!(tokens[0].col, 1);
        assert_eq!(tokens[1].line, 2);
        assert_eq!(tokens[1].col, 1);
    }
}
