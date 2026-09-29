use crate::ast::*;
use crate::token::{Span, Token, TokenKind};

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    pub fn parse_program(&mut self) -> Result<Program, String> {
        let mut functions = Vec::new();

        while !self.is_at_end() {
            functions.push(self.parse_function()?);
        }

        Ok(Program { functions })
    }

    fn parse_function(&mut self) -> Result<Function, String> {
        let fn_tok = self.consume(TokenKind::Fn, "Expected 'fn' keyword to declare function")?;
        let name_tok = self.consume_ident("Expected function name")?;
        self.consume(TokenKind::LParen, "Expected '(' after function name")?;

        let mut params = Vec::new();
        if !self.check(&TokenKind::RParen) {
            loop {
                let p_name = self.consume_ident("Expected parameter name")?.raw;
                self.consume(TokenKind::Colon, "Expected ':' after parameter name")?;
                let p_type = self.parse_type()?;
                params.push((p_name, p_type));

                if self.match_tok(&TokenKind::Comma) {
                    continue;
                } else {
                    break;
                }
            }
        }
        self.consume(TokenKind::RParen, "Expected ')' after parameters")?;

        let ret_type = if self.match_tok(&TokenKind::Arrow) {
            self.parse_type()?
        } else {
            Type::Void
        };

        self.consume(TokenKind::LBrace, "Expected '{' before function body")?;
        let mut body = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            body.push(self.parse_stmt()?);
        }
        let rbrace = self.consume(TokenKind::RBrace, "Expected '}' after function body")?;

        let span = Span::new(fn_tok.span.start, rbrace.span.end, fn_tok.span.line, fn_tok.span.col);
        Ok(Function {
            name: name_tok.raw,
            params,
            ret_type,
            body,
            span,
        })
    }

    fn parse_type(&mut self) -> Result<Type, String> {
        let tok = self.peek();
        match &tok.kind {
            TokenKind::Ident(s) if s.eq_ignore_ascii_case("int") => { self.advance(); Ok(Type::Int) }
            TokenKind::Ident(s) if s.eq_ignore_ascii_case("bool") => { self.advance(); Ok(Type::Bool) }
            TokenKind::Ident(s) if s.eq_ignore_ascii_case("void") => { self.advance(); Ok(Type::Void) }
            _ => Err(format!("Expected type (int/Int, bool/Bool, void/Void) at {}:{}", tok.span.line, tok.span.col)),
        }
    }

    fn parse_stmt(&mut self) -> Result<Stmt, String> {
        let tok = self.peek();
        match &tok.kind {
            TokenKind::Let => self.parse_let_stmt(),
            TokenKind::If => self.parse_if_stmt(),
            TokenKind::While => self.parse_while_stmt(),
            TokenKind::Return => self.parse_return_stmt(),
            TokenKind::Print => self.parse_print_stmt(),
            TokenKind::Ident(_) => {
                // Check if this is assignment
                if self.peek_offset(1).map(|t| t.kind == TokenKind::Eq).unwrap_or(false) {
                    self.parse_assign_stmt()
                } else {
                    let expr = self.parse_expr()?;
                    let semi = self.consume(TokenKind::Semi, "Expected ';' after expression")?;
                    let span = Span::new(expr.span().start, semi.span.end, expr.span().line, expr.span().col);
                    Ok(Stmt::Expr(expr, span))
                }
            }
            _ => {
                let expr = self.parse_expr()?;
                let semi = self.consume(TokenKind::Semi, "Expected ';' after expression")?;
                let span = Span::new(expr.span().start, semi.span.end, expr.span().line, expr.span().col);
                Ok(Stmt::Expr(expr, span))
            }
        }
    }

    fn parse_let_stmt(&mut self) -> Result<Stmt, String> {
        let let_tok = self.consume(TokenKind::Let, "Expected 'let'")?;
        let name_tok = self.consume_ident("Expected variable name after 'let'")?;

        let ty = if self.match_tok(&TokenKind::Colon) {
            self.parse_type()?
        } else {
            Type::Int // Default to Int if omitted
        };

        self.consume(TokenKind::Eq, "Expected '=' in variable declaration")?;
        let init_expr = self.parse_expr()?;
        let semi = self.consume(TokenKind::Semi, "Expected ';' after variable declaration")?;

        let span = Span::new(let_tok.span.start, semi.span.end, let_tok.span.line, let_tok.span.col);
        Ok(Stmt::Let(name_tok.raw, ty, init_expr, span))
    }

    fn parse_assign_stmt(&mut self) -> Result<Stmt, String> {
        let name_tok = self.consume_ident("Expected variable name in assignment")?;
        self.consume(TokenKind::Eq, "Expected '=' in assignment")?;
        let val_expr = self.parse_expr()?;
        let semi = self.consume(TokenKind::Semi, "Expected ';' after assignment")?;

        let span = Span::new(name_tok.span.start, semi.span.end, name_tok.span.line, name_tok.span.col);
        Ok(Stmt::Assign(name_tok.raw, val_expr, span))
    }

    fn parse_if_stmt(&mut self) -> Result<Stmt, String> {
        let if_tok = self.consume(TokenKind::If, "Expected 'if'")?;
        let cond = self.parse_expr()?;

        self.consume(TokenKind::LBrace, "Expected '{' after if condition")?;
        let mut then_branch = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            then_branch.push(self.parse_stmt()?);
        }
        let end_brace = self.consume(TokenKind::RBrace, "Expected '}' after if block")?;

        let (else_branch, end_span) = if self.match_tok(&TokenKind::Else) {
            if self.check(&TokenKind::If) {
                let nested_if = self.parse_if_stmt()?;
                let sp = match &nested_if { Stmt::If(_, _, _, s) => *s, _ => end_brace.span };
                (Some(vec![nested_if]), sp)
            } else {
                self.consume(TokenKind::LBrace, "Expected '{' after else")?;
                let mut el = Vec::new();
                while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
                    el.push(self.parse_stmt()?);
                }
                let elb = self.consume(TokenKind::RBrace, "Expected '}' after else block")?;
                (Some(el), elb.span)
            }
        } else {
            (None, end_brace.span)
        };

        let span = Span::new(if_tok.span.start, end_span.end, if_tok.span.line, if_tok.span.col);
        Ok(Stmt::If(cond, then_branch, else_branch, span))
    }

    fn parse_while_stmt(&mut self) -> Result<Stmt, String> {
        let while_tok = self.consume(TokenKind::While, "Expected 'while'")?;
        let cond = self.parse_expr()?;

        self.consume(TokenKind::LBrace, "Expected '{' after while condition")?;
        let mut body = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.is_at_end() {
            body.push(self.parse_stmt()?);
        }
        let rbrace = self.consume(TokenKind::RBrace, "Expected '}' after while body")?;

        let span = Span::new(while_tok.span.start, rbrace.span.end, while_tok.span.line, while_tok.span.col);
        Ok(Stmt::While(cond, body, span))
    }

    fn parse_return_stmt(&mut self) -> Result<Stmt, String> {
        let ret_tok = self.consume(TokenKind::Return, "Expected 'return'")?;
        let (expr, end_span) = if self.check(&TokenKind::Semi) {
            let semi = self.consume(TokenKind::Semi, "")?;
            (None, semi.span)
        } else {
            let e = self.parse_expr()?;
            let semi = self.consume(TokenKind::Semi, "Expected ';' after return expression")?;
            (Some(e), semi.span)
        };

        let span = Span::new(ret_tok.span.start, end_span.end, ret_tok.span.line, ret_tok.span.col);
        Ok(Stmt::Return(expr, span))
    }

    fn parse_print_stmt(&mut self) -> Result<Stmt, String> {
        let prt_tok = self.consume(TokenKind::Print, "Expected 'print'")?;
        self.consume(TokenKind::LParen, "Expected '(' after print")?;
        let expr = self.parse_expr()?;
        self.consume(TokenKind::RParen, "Expected ')' after print argument")?;
        let semi = self.consume(TokenKind::Semi, "Expected ';' after print")?;

        let span = Span::new(prt_tok.span.start, semi.span.end, prt_tok.span.line, prt_tok.span.col);
        Ok(Stmt::Print(expr, span))
    }

    // Expression parsing with precedence climbing
    pub fn parse_expr(&mut self) -> Result<Expr, String> {
        self.parse_logical_or()
    }

    fn parse_logical_or(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_logical_and()?;
        while self.match_tok(&TokenKind::OrOr) {
            let right = self.parse_logical_and()?;
            let span = Span::new(left.span().start, right.span().end, left.span().line, left.span().col);
            left = Expr::Binary(BinOp::Or, Box::new(left), Box::new(right), span);
        }
        Ok(left)
    }

    fn parse_logical_and(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_equality()?;
        while self.match_tok(&TokenKind::AndAnd) {
            let right = self.parse_equality()?;
            let span = Span::new(left.span().start, right.span().end, left.span().line, left.span().col);
            left = Expr::Binary(BinOp::And, Box::new(left), Box::new(right), span);
        }
        Ok(left)
    }

    fn parse_equality(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_comparison()?;
        while let Some(op) = self.match_equality_op() {
            let right = self.parse_comparison()?;
            let span = Span::new(left.span().start, right.span().end, left.span().line, left.span().col);
            left = Expr::Binary(op, Box::new(left), Box::new(right), span);
        }
        Ok(left)
    }

    fn match_equality_op(&mut self) -> Option<BinOp> {
        if self.match_tok(&TokenKind::EqEq) { Some(BinOp::Eq) }
        else if self.match_tok(&TokenKind::NotEq) { Some(BinOp::Ne) }
        else { None }
    }

    fn parse_comparison(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_term()?;
        while let Some(op) = self.match_comparison_op() {
            let right = self.parse_term()?;
            let span = Span::new(left.span().start, right.span().end, left.span().line, left.span().col);
            left = Expr::Binary(op, Box::new(left), Box::new(right), span);
        }
        Ok(left)
    }

    fn match_comparison_op(&mut self) -> Option<BinOp> {
        if self.match_tok(&TokenKind::Lt) { Some(BinOp::Lt) }
        else if self.match_tok(&TokenKind::LtEq) { Some(BinOp::Le) }
        else if self.match_tok(&TokenKind::Gt) { Some(BinOp::Gt) }
        else if self.match_tok(&TokenKind::GtEq) { Some(BinOp::Ge) }
        else { None }
    }

    fn parse_term(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_factor()?;
        while let Some(op) = self.match_term_op() {
            let right = self.parse_factor()?;
            let span = Span::new(left.span().start, right.span().end, left.span().line, left.span().col);
            left = Expr::Binary(op, Box::new(left), Box::new(right), span);
        }
        Ok(left)
    }

    fn match_term_op(&mut self) -> Option<BinOp> {
        if self.match_tok(&TokenKind::Plus) { Some(BinOp::Add) }
        else if self.match_tok(&TokenKind::Minus) { Some(BinOp::Sub) }
        else { None }
    }

    fn parse_factor(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_unary()?;
        while let Some(op) = self.match_factor_op() {
            let right = self.parse_unary()?;
            let span = Span::new(left.span().start, right.span().end, left.span().line, left.span().col);
            left = Expr::Binary(op, Box::new(left), Box::new(right), span);
        }
        Ok(left)
    }

    fn match_factor_op(&mut self) -> Option<BinOp> {
        if self.match_tok(&TokenKind::Star) { Some(BinOp::Mul) }
        else if self.match_tok(&TokenKind::Slash) { Some(BinOp::Div) }
        else if self.match_tok(&TokenKind::Percent) { Some(BinOp::Mod) }
        else { None }
    }

    fn parse_unary(&mut self) -> Result<Expr, String> {
        if self.match_tok(&TokenKind::Minus) {
            let start = self.tokens[self.pos - 1].span;
            let expr = self.parse_unary()?;
            let span = Span::new(start.start, expr.span().end, start.line, start.col);
            return Ok(Expr::Unary(UnOp::Neg, Box::new(expr), span));
        }
        if self.match_tok(&TokenKind::Not) {
            let start = self.tokens[self.pos - 1].span;
            let expr = self.parse_unary()?;
            let span = Span::new(start.start, expr.span().end, start.line, start.col);
            return Ok(Expr::Unary(UnOp::Not, Box::new(expr), span));
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<Expr, String> {
        let tok = self.peek().clone();
        match tok.kind {
            TokenKind::IntLit(val) => {
                self.advance();
                Ok(Expr::Literal(val, tok.span))
            }
            TokenKind::True => {
                self.advance();
                Ok(Expr::BoolLit(true, tok.span))
            }
            TokenKind::False => {
                self.advance();
                Ok(Expr::BoolLit(false, tok.span))
            }
            TokenKind::Ident(ref name) => {
                self.advance();
                // Check if function call: name(...)
                if self.match_tok(&TokenKind::LParen) {
                    let mut args = Vec::new();
                    if !self.check(&TokenKind::RParen) {
                        loop {
                            args.push(self.parse_expr()?);
                            if self.match_tok(&TokenKind::Comma) {
                                continue;
                            } else {
                                break;
                            }
                        }
                    }
                    let rparen = self.consume(TokenKind::RParen, "Expected ')' after call arguments")?;
                    let span = Span::new(tok.span.start, rparen.span.end, tok.span.line, tok.span.col);
                    Ok(Expr::Call(name.clone(), args, span))
                } else {
                    Ok(Expr::Variable(name.clone(), tok.span))
                }
            }
            TokenKind::LParen => {
                self.advance();
                let expr = self.parse_expr()?;
                self.consume(TokenKind::RParen, "Expected ')' after grouped expression")?;
                Ok(expr)
            }
            _ => Err(format!("Unexpected token '{:?}' at {}:{}", tok.kind, tok.span.line, tok.span.col)),
        }
    }

    // Helper utilities
    fn peek(&self) -> &Token {
        &self.tokens[self.pos]
    }

    fn peek_offset(&self, offset: usize) -> Option<&Token> {
        self.tokens.get(self.pos + offset)
    }

    fn check(&self, kind: &TokenKind) -> bool {
        if self.is_at_end() { false } else { &self.peek().kind == kind }
    }

    fn match_tok(&mut self, kind: &TokenKind) -> bool {
        if self.check(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn advance(&mut self) -> Token {
        if !self.is_at_end() {
            self.pos += 1;
        }
        self.tokens[self.pos - 1].clone()
    }

    fn is_at_end(&self) -> bool {
        self.peek().kind == TokenKind::Eof
    }

    fn consume(&mut self, kind: TokenKind, err: &str) -> Result<Token, String> {
        if self.check(&kind) {
            Ok(self.advance())
        } else {
            let tok = self.peek();
            Err(format!("{} at {}:{} (got {:?})", err, tok.span.line, tok.span.col, tok.kind))
        }
    }

    fn consume_ident(&mut self, err: &str) -> Result<Token, String> {
        let tok = self.peek().clone();
        if let TokenKind::Ident(_) = tok.kind {
            self.advance();
            Ok(tok)
        } else {
            Err(format!("{} at {}:{}", err, tok.span.line, tok.span.col))
        }
    }
}
