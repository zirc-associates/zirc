use zirc_syntax::ast::*;
use zirc_syntax::error::{Result, Span};
use zirc_syntax::token::{Token, TokenKind};

/// Builds an AST from a token stream handwritten recursive-descent parser with precedence climbing.
pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    /// `(index of next real token, comment text)` for every
    /// `Comment` token, in source order. Comments are recorded
    /// here instead of interleaving with real tokens, so they
    /// never disturb parsing; [`Parser::drain_comments`] reattaches
    /// them as `Item::Comment`/`Stmt::Comment` at item boundaries.
    comments: Vec<(usize, String)>,
}

impl Parser {
    /// Create a new parser from a vector of tokens.
    ///
    /// An empty token vector is treated as an empty (EOF-only)
    /// program instead of panicking on `peek`. `Comment` tokens are
    /// separated out and preserved (see [`Parser::comments`]).
    pub fn new(mut tokens: Vec<Token>) -> Self {
        if tokens.is_empty() {
            tokens.push(Token {
                kind: TokenKind::Eof,
                line: 1,
                col: 1,
            });
        }
        let mut comments: Vec<(usize, String)> = Vec::new();
        let mut real: Vec<Token> = Vec::with_capacity(tokens.len());
        for tk in tokens {
            match tk.kind {
                TokenKind::Comment(text) => comments.push((real.len(), text)),
                _ => real.push(tk),
            }
        }
        Self {
            tokens: real,
            pos: 0,
            comments,
        }
    }

    /// Drain (in source order) every comment whose following real
    /// token is at or before the current position.
    fn drain_comments(&mut self) -> Vec<String> {
        let pos = self.pos;
        let mut drained = Vec::new();
        let mut rest = Vec::new();
        for (idx, text) in self.comments.drain(..) {
            if idx <= pos {
                drained.push(text);
            } else {
                rest.push((idx, text));
            }
        }
        self.comments = rest;
        drained
    }

    fn peek(&self) -> &Token {
        // `new` guarantees at least an EOF token, and `advance` never
        // moves past it, so this index is always in bounds.
        self.tokens.get(self.pos).unwrap_or_else(|| {
            self.tokens
                .last()
                .expect("parser always has at least an EOF token")
        })
    }
    fn is_eof(&self) -> bool {
        matches!(self.peek().kind, TokenKind::Eof)
    }
    fn advance(&mut self) {
        if !self.is_eof() {
            self.pos += 1;
        }
    }

    fn consume_ident(&mut self) -> Result<String> {
        let tk = self.peek().clone();
        match tk.kind {
            TokenKind::Ident(name) => {
                self.advance();
                Ok(name)
            }
            _ => zirc_syntax::error::error_at_kind(
                zirc_syntax::error::ErrorKind::Parse,
                tk.line,
                tk.col,
                "Expected identifier",
            ),
        }
    }

    /// Span of the token at the cursor.
    fn span_here(&self) -> Span {
        let tk = self.peek();
        Span::new(tk.line, tk.col)
    }

    /// Parse a full program (sequence of items until Eof).
    pub fn parse_program(&mut self) -> Result<Program> {
        let mut items = Vec::new();
        loop {
            // Comments before the next item become items themselves.
            for text in self.drain_comments() {
                items.push(Item {
                    span: Span::default(),
                    kind: ItemKind::Comment(text),
                });
            }
            if self.is_eof() {
                break;
            }
            match self.peek().kind.clone() {
                TokenKind::Module => {
                    let span = self.span_here();
                    self.advance();
                    // module "name"
                    let tk = self.peek().clone();
                    match tk.kind {
                        TokenKind::String(s) => {
                            self.advance();
                            items.push(Item {
                                span,
                                kind: ItemKind::Module(s),
                            });
                        }
                        _ => {
                            return zirc_syntax::error::error_at_kind(
                                zirc_syntax::error::ErrorKind::Parse,
                                tk.line,
                                tk.col,
                                "Expected module name as string literal",
                            );
                        }
                    }
                }
                TokenKind::Import => {
                    items.push(self.parse_import()?);
                }
                TokenKind::Fun => {
                    let span = self.span_here();
                    let f = self.parse_function()?;
                    items.push(Item {
                        span,
                        kind: ItemKind::Function(f),
                    });
                }
                _ => {
                    let span = self.span_here();
                    let s = self.parse_stmt()?;
                    items.push(Item {
                        span,
                        kind: ItemKind::Stmt(s),
                    });
                }
            }
        }
        Ok(Program { items })
    }

    fn parse_type_name(&mut self) -> Result<Type> {
        let name = self.consume_ident()?;
        let ty = match name.as_str() {
            "int" => Type::Int,
            "float" => Type::Float,
            "string" => Type::String,
            "bool" => Type::Bool,
            "unit" => Type::Unit,
            "list" => Type::List,
            _ => {
                return zirc_syntax::error::error_at_kind(
                    zirc_syntax::error::ErrorKind::Parse,
                    self.peek().line,
                    self.peek().col,
                    format!("Unknown type '{}'", name),
                );
            }
        };
        Ok(ty)
    }

    fn parse_function(&mut self) -> Result<Function> {
        let span = self.span_here();
        self.expect(TokenKind::Fun)?;
        let name = self.consume_ident()?;
        let (params, return_type, body) = self.parse_function_tail()?;
        Ok(Function {
            name,
            params,
            return_type,
            body,
            span,
        })
    }

    /// Parse `(params) [(return_type)] : body end` — shared by named
    /// functions and anonymous closures. Assumes `fun` was consumed
    /// (and, for named functions, the name too).
    fn parse_function_tail(&mut self) -> Result<(Vec<Param>, Option<Type>, Vec<Stmt>)> {
        self.expect(TokenKind::LParen)?;
        let mut params: Vec<Param> = Vec::new();
        if !matches!(self.peek().kind, TokenKind::RParen) {
            params.push(self.parse_param()?);
            while matches!(self.peek().kind, TokenKind::Comma) {
                self.advance();
                params.push(self.parse_param()?);
            }
        }
        self.expect(TokenKind::RParen)?;
        let mut return_type = None;
        if matches!(self.peek().kind, TokenKind::LParen) {
            self.advance();
            let ty = self.parse_type_name()?;
            self.expect(TokenKind::RParen)?;
            return_type = Some(ty);
        }
        self.expect(TokenKind::Colon)?;
        let body = self.parse_block_until_end()?;
        self.expect(TokenKind::End)?;
        Ok((params, return_type, body))
    }

    /// Parse an anonymous closure: `fun(params): body end`.
    fn parse_closure(&mut self) -> Result<Function> {
        let span = self.span_here();
        self.expect(TokenKind::Fun)?;
        let (params, return_type, body) = self.parse_function_tail()?;
        Ok(Function {
            name: String::new(),
            params,
            return_type,
            body,
            span,
        })
    }

    fn parse_param(&mut self) -> Result<Param> {
        let span = self.span_here();
        let name = self.consume_ident()?;
        let mut ty = None;
        if matches!(self.peek().kind, TokenKind::Colon) {
            self.advance();
            ty = Some(self.parse_type_name()?);
        }
        Ok(Param { name, ty, span })
    }

    fn parse_block_until_end(&mut self) -> Result<Vec<Stmt>> {
        let mut stmts = Vec::new();
        loop {
            for text in self.drain_comments() {
                stmts.push(Stmt {
                    span: Span::default(),
                    kind: StmtKind::Comment(text),
                });
            }
            if matches!(self.peek().kind, TokenKind::End | TokenKind::Eof) {
                break;
            }
            stmts.push(self.parse_stmt()?);
        }
        Ok(stmts)
    }

    fn parse_block_until_else_or_end(&mut self) -> Result<Vec<Stmt>> {
        let mut stmts = Vec::new();
        loop {
            for text in self.drain_comments() {
                stmts.push(Stmt {
                    span: Span::default(),
                    kind: StmtKind::Comment(text),
                });
            }
            if matches!(
                self.peek().kind,
                TokenKind::Else | TokenKind::End | TokenKind::Eof
            ) {
                break;
            }
            stmts.push(self.parse_stmt()?);
        }
        Ok(stmts)
    }

    fn parse_stmt(&mut self) -> Result<Stmt> {
        let span = self.span_here();
        let mk = |kind: StmtKind| Stmt { span, kind };
        Ok(match self.peek().kind.clone() {
            TokenKind::Let => {
                self.advance();
                let name = self.consume_ident()?;
                let mut ty = None;
                if matches!(self.peek().kind, TokenKind::Colon) {
                    self.advance();
                    ty = Some(self.parse_type_name()?);
                }
                self.expect(TokenKind::Equal)?;
                let expr = self.parse_expr()?;
                mk(StmtKind::Let { name, ty, expr })
            }
            TokenKind::Return => {
                self.advance();
                // optional expression (return without value)
                if matches!(self.peek().kind, TokenKind::End | TokenKind::Else) {
                    mk(StmtKind::Return(None))
                } else {
                    let expr = self.parse_expr()?;
                    mk(StmtKind::Return(Some(expr)))
                }
            }
            TokenKind::If => {
                self.advance();
                let cond = self.parse_expr()?;
                self.expect(TokenKind::Colon)?;
                let then_body = self.parse_block_until_else_or_end()?;
                let mut else_body = Vec::new();
                if matches!(self.peek().kind, TokenKind::Else) {
                    self.advance();
                    self.expect(TokenKind::Colon)?;
                    else_body = self.parse_block_until_end()?;
                }
                self.expect(TokenKind::End)?;
                mk(StmtKind::If {
                    cond,
                    then_body,
                    else_body,
                })
            }
            TokenKind::While => {
                self.advance();
                let cond = self.parse_expr()?;
                self.expect(TokenKind::Colon)?;
                let body = self.parse_block_until_end()?;
                self.expect(TokenKind::End)?;
                mk(StmtKind::While { cond, body })
            }
            TokenKind::For => {
                self.advance();
                let var = self.consume_ident()?;
                self.expect(TokenKind::In)?;
                let start = self.parse_expr()?;
                self.expect(TokenKind::DotDot)?;
                let end = self.parse_expr()?;
                self.expect(TokenKind::Colon)?;
                let body = self.parse_block_until_end()?;
                self.expect(TokenKind::End)?;
                mk(StmtKind::For {
                    var,
                    start,
                    end,
                    body,
                })
            }
            TokenKind::Break => {
                self.advance();
                mk(StmtKind::Break)
            }
            TokenKind::Continue => {
                self.advance();
                mk(StmtKind::Continue)
            }
            TokenKind::Ident(_) => {
                // assignment or expression statement
                if let TokenKind::Ident(name) = self.peek().kind.clone() {
                    let is_assign = matches!(
                        self.tokens.get(self.pos + 1).map(|t| &t.kind),
                        Some(TokenKind::Equal)
                    );
                    if is_assign {
                        self.advance();
                        self.expect(TokenKind::Equal)?;
                        let expr = self.parse_expr()?;
                        return Ok(mk(StmtKind::Assign { name, expr }));
                    }
                }
                let expr = self.parse_expr()?;
                mk(StmtKind::ExprStmt(expr))
            }
            _ => {
                let expr = self.parse_expr()?;
                mk(StmtKind::ExprStmt(expr))
            }
        })
    }

    pub fn parse_expr(&mut self) -> Result<Expr> {
        self.parse_logical_or()
    }

    fn parse_logical_or(&mut self) -> Result<Expr> {
        let mut left = self.parse_logical_and()?;
        while matches!(self.peek().kind, TokenKind::OrOr) {
            self.advance();
            let right = self.parse_logical_and()?;
            left = Expr::LogicalOr(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_logical_and(&mut self) -> Result<Expr> {
        let mut left = self.parse_equality()?;
        while matches!(self.peek().kind, TokenKind::AndAnd) {
            self.advance();
            let right = self.parse_equality()?;
            left = Expr::LogicalAnd(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_equality(&mut self) -> Result<Expr> {
        let mut left = self.parse_comparison()?;
        loop {
            match self.peek().kind.clone() {
                TokenKind::EqEq => {
                    self.advance();
                    let right = self.parse_comparison()?;
                    left = Expr::Eq(Box::new(left), Box::new(right));
                }
                TokenKind::NotEq => {
                    self.advance();
                    let right = self.parse_comparison()?;
                    left = Expr::Ne(Box::new(left), Box::new(right));
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_comparison(&mut self) -> Result<Expr> {
        let mut left = self.parse_term()?;
        loop {
            match self.peek().kind.clone() {
                TokenKind::Less => {
                    self.advance();
                    let right = self.parse_term()?;
                    left = Expr::Lt(Box::new(left), Box::new(right));
                }
                TokenKind::LessEq => {
                    self.advance();
                    let right = self.parse_term()?;
                    left = Expr::Le(Box::new(left), Box::new(right));
                }
                TokenKind::Greater => {
                    self.advance();
                    let right = self.parse_term()?;
                    left = Expr::Gt(Box::new(left), Box::new(right));
                }
                TokenKind::GreaterEq => {
                    self.advance();
                    let right = self.parse_term()?;
                    left = Expr::Ge(Box::new(left), Box::new(right));
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_term(&mut self) -> Result<Expr> {
        let mut left = self.parse_factor()?;
        loop {
            match self.peek().kind.clone() {
                TokenKind::Plus => {
                    self.advance();
                    let right = self.parse_factor()?;
                    left = Expr::BinaryAdd(Box::new(left), Box::new(right));
                }
                TokenKind::Minus => {
                    self.advance();
                    let right = self.parse_factor()?;
                    left = Expr::BinarySub(Box::new(left), Box::new(right));
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_factor(&mut self) -> Result<Expr> {
        let mut left = self.parse_unary()?;
        loop {
            match self.peek().kind.clone() {
                TokenKind::Star => {
                    self.advance();
                    let right = self.parse_unary()?;
                    left = Expr::BinaryMul(Box::new(left), Box::new(right));
                }
                TokenKind::Slash => {
                    self.advance();
                    let right = self.parse_unary()?;
                    left = Expr::BinaryDiv(Box::new(left), Box::new(right));
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expr> {
        match self.peek().kind.clone() {
            TokenKind::Bang => {
                self.advance();
                let e = self.parse_unary()?;
                Ok(Expr::LogicalNot(Box::new(e)))
            }
            TokenKind::Minus => {
                self.advance();
                let e = self.parse_unary()?;
                Ok(Expr::Neg(Box::new(e)))
            }
            _ => self.parse_primary(),
        }
    }

    fn parse_primary(&mut self) -> Result<Expr> {
        let tk = self.peek().clone();
        let mut node = match tk.kind {
            TokenKind::Number(n) => {
                self.advance();
                Ok(Expr::LiteralInt(n))
            }
            TokenKind::Float(f) => {
                self.advance();
                Ok(Expr::LiteralFloat(f))
            }
            TokenKind::String(s) => {
                self.advance();
                Ok(Expr::LiteralString(s))
            }
            TokenKind::True => {
                self.advance();
                Ok(Expr::LiteralBool(true))
            }
            TokenKind::False => {
                self.advance();
                Ok(Expr::LiteralBool(false))
            }
            TokenKind::Ident(name) => {
                // Qualified call: `mod.f(args)` / `a.b.c(args)`.
                // Scan ahead over `(Dot Ident)*`; if a `(` follows,
                // this is a call whose name is the dotted path.
                let mut scan = self.pos + 1;
                let mut is_call = matches!(
                    self.tokens.get(scan).map(|t| &t.kind),
                    Some(TokenKind::LParen)
                );
                while !is_call {
                    match (
                        self.tokens.get(scan).map(|t| &t.kind),
                        self.tokens.get(scan + 1).map(|t| &t.kind),
                    ) {
                        (Some(TokenKind::Dot), Some(TokenKind::Ident(_))) => {
                            scan += 2;
                            is_call = matches!(
                                self.tokens.get(scan).map(|t| &t.kind),
                                Some(TokenKind::LParen)
                            );
                        }
                        _ => break,
                    }
                }
                if is_call {
                    let mut call_name = name.clone();
                    self.advance(); // first ident
                    while matches!(self.peek().kind, TokenKind::Dot) {
                        self.advance();
                        let field = self.consume_ident()?;
                        call_name.push('.');
                        call_name.push_str(&field);
                    }
                    self.expect(TokenKind::LParen)?;
                    let mut args = Vec::new();
                    if !matches!(self.peek().kind, TokenKind::RParen) {
                        args.push(self.parse_expr()?);
                        while matches!(self.peek().kind, TokenKind::Comma) {
                            self.advance();
                            args.push(self.parse_expr()?);
                        }
                    }
                    self.expect(TokenKind::RParen)?;
                    Ok(Expr::Call {
                        name: call_name,
                        args,
                    })
                } else {
                    self.advance();
                    Ok(Expr::Ident(name))
                }
            }
            TokenKind::Fun => {
                let f = self.parse_closure()?;
                Ok(Expr::Closure(Box::new(f)))
            }
            TokenKind::LParen => {
                self.advance();
                let e = self.parse_expr()?;
                self.expect(TokenKind::RParen)?;
                Ok(e)
            }
            TokenKind::LBracket => {
                // list literal
                self.advance();
                let mut elems = Vec::new();
                if !matches!(self.peek().kind, TokenKind::RBracket) {
                    elems.push(self.parse_expr()?);
                    while matches!(self.peek().kind, TokenKind::Comma) {
                        self.advance();
                        elems.push(self.parse_expr()?);
                    }
                }
                self.expect(TokenKind::RBracket)?;
                Ok(Expr::List(elems))
            }
            _ => zirc_syntax::error::error_at_kind(
                zirc_syntax::error::ErrorKind::Parse,
                tk.line,
                tk.col,
                format!("Unexpected token {:?}", tk.kind),
            ),
        }?;
        // Postfix indexing, member access and calling a value
        // (e.g. `(fun(x): x end)(5)`, `closures[0](5)`).
        loop {
            if matches!(self.peek().kind, TokenKind::LBracket) {
                self.advance();
                let idx = self.parse_expr()?;
                self.expect(TokenKind::RBracket)?;
                node = Expr::Index(Box::new(node), Box::new(idx));
                continue;
            }
            if matches!(self.peek().kind, TokenKind::LParen) {
                self.advance();
                let mut args = Vec::new();
                if !matches!(self.peek().kind, TokenKind::RParen) {
                    args.push(self.parse_expr()?);
                    while matches!(self.peek().kind, TokenKind::Comma) {
                        self.advance();
                        args.push(self.parse_expr()?);
                    }
                }
                self.expect(TokenKind::RParen)?;
                node = Expr::CallValue(Box::new(node), args);
                continue;
            }
            if matches!(self.peek().kind, TokenKind::Dot) {
                self.advance();
                let field = self.consume_ident()?;
                node = Expr::Member(Box::new(node), field);
                continue;
            }
            break;
        }
        Ok(node)
    }

    fn expect(&mut self, kind: TokenKind) -> Result<()> {
        let tk = self.peek().clone();
        if std::mem::discriminant(&tk.kind) == std::mem::discriminant(&kind) {
            self.advance();
            Ok(())
        } else {
            zirc_syntax::error::error_at_kind(
                zirc_syntax::error::ErrorKind::Parse,
                tk.line,
                tk.col,
                format!("Expected {:?}, found {:?}", kind, tk.kind),
            )
        }
    }
}

impl Parser {
    fn parse_import(&mut self) -> Result<Item> {
        let span = self.span_here();
        // current token is 'import'
        self.advance();
        // expect module ident
        let module = self.consume_ident()?;
        let mut alias: Option<String> = None;
        if matches!(self.peek().kind, TokenKind::As) {
            self.advance();
            alias = Some(self.consume_ident()?);
        }
        Ok(Item {
            span,
            kind: ItemKind::Import { module, alias },
        })
    }
}
