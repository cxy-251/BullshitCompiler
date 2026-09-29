use crate::token::{Span, Token, TokenKind};

pub struct Lexer<'a> {
    source: &'a str,
    chars: Vec<(usize, char)>,
    pos: usize,
    line: usize,
    col: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self {
        let chars: Vec<(usize, char)> = source.char_indices().collect();
        Self {
            source,
            chars,
            pos: 0,
            line: 1,
            col: 1,
        }
    }

    pub fn tokenize(&mut self) -> Result<Vec<Token>, String> {
        let mut tokens = Vec::new();

        while self.pos < self.chars.len() {
            let (idx, ch) = self.chars[self.pos];

            // Whitespace
            if ch.is_whitespace() {
                if ch == '\n' {
                    self.line += 1;
                    self.col = 1;
                } else {
                    self.col += 1;
                }
                self.pos += 1;
                continue;
            }

            // Single line comment //
            if ch == '/' && self.peek_next() == Some('/') {
                while self.pos < self.chars.len() && self.chars[self.pos].1 != '\n' {
                    self.pos += 1;
                }
                continue;
            }

            // Multi-line comment /* */
            if ch == '/' && self.peek_next() == Some('*') {
                self.pos += 2;
                self.col += 2;
                while self.pos < self.chars.len() {
                    if self.chars[self.pos].1 == '\n' {
                        self.line += 1;
                        self.col = 1;
                    } else {
                        self.col += 1;
                    }
                    if self.chars[self.pos].1 == '*' && self.peek_next() == Some('/') {
                        self.pos += 2;
                        self.col += 2;
                        break;
                    }
                    self.pos += 1;
                }
                continue;
            }

            let start_idx = idx;
            let start_line = self.line;
            let start_col = self.col;

            // Identifiers or Keywords
            if ch.is_alphabetic() || ch == '_' {
                let mut ident = String::new();
                while self.pos < self.chars.len() {
                    let (_, c) = self.chars[self.pos];
                    if c.is_alphanumeric() || c == '_' {
                        ident.push(c);
                        self.advance();
                    } else {
                        break;
                    }
                }

                let kind = match ident.as_str() {
                    "fn" => TokenKind::Fn,
                    "let" => TokenKind::Let,
                    "const" => TokenKind::Const,
                    "if" => TokenKind::If,
                    "else" => TokenKind::Else,
                    "while" => TokenKind::While,
                    "return" => TokenKind::Return,
                    "print" => TokenKind::Print,
                    "true" => TokenKind::True,
                    "false" => TokenKind::False,
                    _ => TokenKind::Ident(ident.clone()),
                };

                let end_idx = if self.pos < self.chars.len() { self.chars[self.pos].0 } else { self.source.len() };
                tokens.push(Token {
                    kind,
                    raw: ident,
                    span: Span::new(start_idx, end_idx, start_line, start_col),
                });
                continue;
            }

            // Integer literals
            if ch.is_ascii_digit() {
                let mut num_str = String::new();
                while self.pos < self.chars.len() {
                    let (_, c) = self.chars[self.pos];
                    if c.is_ascii_digit() {
                        num_str.push(c);
                        self.advance();
                    } else {
                        break;
                    }
                }
                let val: i64 = num_str.parse().map_err(|e| format!("Invalid integer at {}:{}: {}", start_line, start_col, e))?;
                let end_idx = if self.pos < self.chars.len() { self.chars[self.pos].0 } else { self.source.len() };
                tokens.push(Token {
                    kind: TokenKind::IntLit(val),
                    raw: num_str,
                    span: Span::new(start_idx, end_idx, start_line, start_col),
                });
                continue;
            }

            // Multi-char operators
            if ch == '=' && self.peek_next() == Some('=') {
                self.advance(); self.advance();
                tokens.push(Token { kind: TokenKind::EqEq, raw: "==".into(), span: Span::new(start_idx, start_idx + 2, start_line, start_col) });
                continue;
            }
            if ch == '!' && self.peek_next() == Some('=') {
                self.advance(); self.advance();
                tokens.push(Token { kind: TokenKind::NotEq, raw: "!=".into(), span: Span::new(start_idx, start_idx + 2, start_line, start_col) });
                continue;
            }
            if ch == '<' && self.peek_next() == Some('=') {
                self.advance(); self.advance();
                tokens.push(Token { kind: TokenKind::LtEq, raw: "<=".into(), span: Span::new(start_idx, start_idx + 2, start_line, start_col) });
                continue;
            }
            if ch == '>' && self.peek_next() == Some('=') {
                self.advance(); self.advance();
                tokens.push(Token { kind: TokenKind::GtEq, raw: ">=".into(), span: Span::new(start_idx, start_idx + 2, start_line, start_col) });
                continue;
            }
            if ch == '&' && self.peek_next() == Some('&') {
                self.advance(); self.advance();
                tokens.push(Token { kind: TokenKind::AndAnd, raw: "&&".into(), span: Span::new(start_idx, start_idx + 2, start_line, start_col) });
                continue;
            }
            if ch == '|' && self.peek_next() == Some('|') {
                self.advance(); self.advance();
                tokens.push(Token { kind: TokenKind::OrOr, raw: "||".into(), span: Span::new(start_idx, start_idx + 2, start_line, start_col) });
                continue;
            }
            if ch == '-' && self.peek_next() == Some('>') {
                self.advance(); self.advance();
                tokens.push(Token { kind: TokenKind::Arrow, raw: "->".into(), span: Span::new(start_idx, start_idx + 2, start_line, start_col) });
                continue;
            }

            // Single char symbols
            let (kind, len) = match ch {
                '+' => (TokenKind::Plus, 1),
                '-' => (TokenKind::Minus, 1),
                '*' => (TokenKind::Star, 1),
                '/' => (TokenKind::Slash, 1),
                '%' => (TokenKind::Percent, 1),
                '=' => (TokenKind::Eq, 1),
                '<' => (TokenKind::Lt, 1),
                '>' => (TokenKind::Gt, 1),
                '!' => (TokenKind::Not, 1),
                '(' => (TokenKind::LParen, 1),
                ')' => (TokenKind::RParen, 1),
                '{' => (TokenKind::LBrace, 1),
                '}' => (TokenKind::RBrace, 1),
                ':' => (TokenKind::Colon, 1),
                ';' => (TokenKind::Semi, 1),
                ',' => (TokenKind::Comma, 1),
                _ => return Err(format!("Unexpected character '{}' at {}:{}", ch, self.line, self.col)),
            };

            self.advance();
            tokens.push(Token {
                kind,
                raw: ch.to_string(),
                span: Span::new(start_idx, start_idx + len, start_line, start_col),
            });
        }

        let end = self.source.len();
        tokens.push(Token {
            kind: TokenKind::Eof,
            raw: "".into(),
            span: Span::new(end, end, self.line, self.col),
        });

        Ok(tokens)
    }

    fn advance(&mut self) {
        self.pos += 1;
        self.col += 1;
    }

    fn peek_next(&self) -> Option<char> {
        self.chars.get(self.pos + 1).map(|(_, c)| *c)
    }
}
