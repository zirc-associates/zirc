use zirc_syntax::error::Result;
use zirc_syntax::token::{Token, TokenKind};

/// Streaming character scanner that produces tokens with positions.
pub struct Lexer {
    src: Vec<char>,
    pos: usize,
    line: usize,
    col: usize,
}

impl Lexer {
    /// Create a new lexer over the given source string.
    pub fn new(input: &str) -> Self {
        Self {
            src: input.chars().collect(),
            pos: 0,
            line: 1,
            col: 1,
        }
    }

    fn peek(&self) -> Option<char> {
        self.src.get(self.pos).copied()
    }
    fn peek_next(&self) -> Option<char> {
        self.src.get(self.pos + 1).copied()
    }
    fn advance(&mut self) -> Option<char> {
        let ch = self.src.get(self.pos).copied();
        if let Some(c) = ch {
            self.pos += 1;
            if c == '\n' {
                self.line += 1;
                self.col = 1;
            } else {
                self.col += 1;
            }
        }
        ch
    }

    fn make_token(&self, kind: TokenKind) -> Token {
        Token {
            kind,
            line: self.line,
            col: self.col,
        }
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.peek() {
            if c.is_whitespace() {
                self.advance();
            } else if c == '~' {
                while let Some(c2) = self.peek() {
                    self.advance();
                    if c2 == '\n' {
                        break;
                    }
                }
            } else {
                break;
            }
        }
    }

    fn read_number(&mut self) -> Result<Token> {
        let start_line = self.line;
        let start_col = self.col;
        let mut s = String::new();
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                s.push(c);
                self.advance();
            } else {
                break;
            }
        }
        let val: i64 = s.parse().map_err(|_| {
            zirc_syntax::error::Error::with_span("Invalid number", start_line, start_col)
        })?;
        Ok(Token {
            kind: TokenKind::Number(val),
            line: start_line,
            col: start_col,
        })
    }

    fn read_ident_or_keyword(&mut self) -> Token {
        let start_line = self.line;
        let start_col = self.col;
        let mut s = String::new();
        while let Some(c) = self.peek() {
            if c.is_ascii_alphanumeric() || c == '_' {
                s.push(c);
                self.advance();
            } else {
                break;
            }
        }
        let kind = match s.as_str() {
            "module" => TokenKind::Module,
            "import" => TokenKind::Import,
            "as" => TokenKind::As,
            "fun" => TokenKind::Fun,
            "end" => TokenKind::End,
            "if" => TokenKind::If,
            "else" => TokenKind::Else,
            "return" => TokenKind::Return,
            "let" => TokenKind::Let,
            "while" => TokenKind::While,
            "break" => TokenKind::Break,
            "continue" => TokenKind::Continue,
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            "for" => TokenKind::For,
            "in" => TokenKind::In,
            _ => TokenKind::Ident(s),
        };
        Token {
            kind,
            line: start_line,
            col: start_col,
        }
    }

    fn read_string(&mut self) -> Result<Token> {
        let start_line = self.line;
        let start_col = self.col;
        let mut s = String::new();
        while let Some(c) = self.advance() {
            match c {
                '"' => {
                    return Ok(Token {
                        kind: TokenKind::String(s),
                        line: start_line,
                        col: start_col,
                    });
                }
                '\\' => {
                    if let Some(n) = self.advance() {
                        let esc = match n {
                            'n' => '\n',
                            't' => '\t',
                            'r' => '\r',
                            '\\' => '\\',
                            '"' => '"',
                            other => other,
                        };
                        s.push(esc);
                    } else {
                        return zirc_syntax::error::error_at(
                            start_line,
                            start_col,
                            "Unterminated string",
                        );
                    }
                }
                other => s.push(other),
            }
        }
        zirc_syntax::error::error_at(start_line, start_col, "Unterminated string")
    }

    /// Tokenize the entire input into a vector of tokens ending with Eof.
    pub fn tokenize(&mut self) -> Result<Vec<Token>> {
        let mut tokens = Vec::new();
        loop {
            self.skip_whitespace();
            let line = self.line;
            let col = self.col;
            let tk = match self.peek() {
                None => {
                    tokens.push(Token {
                        kind: TokenKind::Eof,
                        line,
                        col,
                    });
                    break;
                }
                Some('(') => {
                    self.advance();
                    self.make_token(TokenKind::LParen)
                }
                Some(')') => {
                    self.advance();
                    self.make_token(TokenKind::RParen)
                }
                Some(',') => {
                    self.advance();
                    self.make_token(TokenKind::Comma)
                }
                Some(':') => {
                    self.advance();
                    self.make_token(TokenKind::Colon)
                }
                Some('[') => {
                    self.advance();
                    self.make_token(TokenKind::LBracket)
                }
                Some(']') => {
                    self.advance();
                    self.make_token(TokenKind::RBracket)
                }
                Some('=') => {
                    if self.peek_next() == Some('=') {
                        self.advance();
                        self.advance();
                        Token {
                            kind: TokenKind::EqEq,
                            line,
                            col,
                        }
                    } else {
                        self.advance();
                        self.make_token(TokenKind::Equal)
                    }
                }
                Some('!') => {
                    if self.peek_next() == Some('=') {
                        self.advance();
                        self.advance();
                        Token {
                            kind: TokenKind::NotEq,
                            line,
                            col,
                        }
                    } else {
                        self.advance();
                        Token {
                            kind: TokenKind::Bang,
                            line,
                            col,
                        }
                    }
                }
                Some('<') => {
                    self.advance();
                    if self.peek() == Some('=') {
                        self.advance();
                        Token {
                            kind: TokenKind::LessEq,
                            line,
                            col,
                        }
                    } else {
                        Token {
                            kind: TokenKind::Less,
                            line,
                            col,
                        }
                    }
                }
                Some('>') => {
                    self.advance();
                    if self.peek() == Some('=') {
                        self.advance();
                        Token {
                            kind: TokenKind::GreaterEq,
                            line,
                            col,
                        }
                    } else {
                        Token {
                            kind: TokenKind::Greater,
                            line,
                            col,
                        }
                    }
                }
                Some('+') => {
                    self.advance();
                    self.make_token(TokenKind::Plus)
                }
                Some('-') => {
                    self.advance();
                    self.make_token(TokenKind::Minus)
                }
                Some('*') => {
                    self.advance();
                    self.make_token(TokenKind::Star)
                }
                Some('/') => {
                    self.advance();
                    self.make_token(TokenKind::Slash)
                }
                Some('&') => {
                    if self.peek_next() == Some('&') {
                        self.advance();
                        self.advance();
                        Token {
                            kind: TokenKind::AndAnd,
                            line,
                            col,
                        }
                    } else {
                        return zirc_syntax::error::error_at(
                            line,
                            col,
                            "Unexpected '&' (did you mean '&&'?)",
                        );
                    }
                }
                Some('|') => {
                    if self.peek_next() == Some('|') {
                        self.advance();
                        self.advance();
                        Token {
                            kind: TokenKind::OrOr,
                            line,
                            col,
                        }
                    } else {
                        return zirc_syntax::error::error_at(
                            line,
                            col,
                            "Unexpected '|' (did you mean '||'?)",
                        );
                    }
                }
                Some('.') => {
                    if self.peek_next() == Some('.') {
                        self.advance();
                        self.advance();
                        Token {
                            kind: TokenKind::DotDot,
                            line,
                            col,
                        }
                    } else {
                        self.advance();
                        self.make_token(TokenKind::Dot)
                    }
                }
                Some('"') => {
                    self.advance();
                    self.read_string()?
                }
                Some(c) if c.is_ascii_digit() => self.read_number()?,
                Some(c) if c.is_ascii_alphabetic() || c == '_' => self.read_ident_or_keyword(),
                Some(other) => {
                    return zirc_syntax::error::error_at(
                        line,
                        col,
                        format!("Unexpected character '{}'", other),
                    );
                }
            };
            tokens.push(tk);
        }
        Ok(tokens)
    }
}

