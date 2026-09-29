//! 迷你 ML 的词法与语法分析（递归下降）。
//!
//! 优先级从低到高：`fun` / `let` / `if`（向右延伸到尽头）< 比较 `<` `==` < 加减 < 乘 < 函数调用 < 原子。

use bsc_core::{Diagnostic, Span};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ExprId(pub u32);

impl ExprId {
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Lt,
    Eq,
}

impl BinOp {
    pub fn symbol(self) -> &'static str {
        match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Lt => "<",
            BinOp::Eq => "==",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExprKind {
    Int(i64),
    Bool(bool),
    Var(String),
    Fun { param: String, body: ExprId },
    App { func: ExprId, arg: ExprId },
    Let { name: String, value: ExprId, body: ExprId },
    If { cond: ExprId, then: ExprId, els: ExprId },
    Bin { op: BinOp, lhs: ExprId, rhs: ExprId },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Program {
    pub src: String,
    pub exprs: Vec<Expr>,
    pub root: ExprId,
}

impl Program {
    pub fn expr(&self, id: ExprId) -> &Expr {
        &self.exprs[id.index()]
    }

    pub fn text(&self, id: ExprId) -> &str {
        self.expr(id).span.text(&self.src)
    }

    pub fn children(&self, id: ExprId) -> Vec<ExprId> {
        match &self.expr(id).kind {
            ExprKind::Int(_) | ExprKind::Bool(_) | ExprKind::Var(_) => vec![],
            ExprKind::Fun { body, .. } => vec![*body],
            ExprKind::App { func, arg } => vec![*func, *arg],
            ExprKind::Let { value, body, .. } => vec![*value, *body],
            ExprKind::If { cond, then, els } => vec![*cond, *then, *els],
            ExprKind::Bin { lhs, rhs, .. } => vec![*lhs, *rhs],
        }
    }

    /// 语法树节点的标签。
    pub fn label(&self, id: ExprId) -> String {
        match &self.expr(id).kind {
            ExprKind::Int(n) => n.to_string(),
            ExprKind::Bool(b) => b.to_string(),
            ExprKind::Var(x) => x.clone(),
            ExprKind::Fun { param, .. } => format!("fun {param} ->"),
            ExprKind::App { .. } => "调用".to_owned(),
            ExprKind::Let { name, .. } => format!("let {name}"),
            ExprKind::If { .. } => "if".to_owned(),
            ExprKind::Bin { op, .. } => op.symbol().to_owned(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tok {
    Int(i64),
    Ident,
    Fun,
    Let,
    In,
    If,
    Then,
    Else,
    True,
    False,
    Arrow,
    Eq,
    EqEq,
    Lt,
    Plus,
    Minus,
    Star,
    LParen,
    RParen,
    Eof,
}

fn lex(src: &str) -> Result<Vec<(Tok, Span)>, Diagnostic> {
    let mut out = Vec::new();
    let b = src.as_bytes();
    let mut i = 0;
    while i < src.len() {
        let c = src[i..].chars().next().unwrap();
        let start = i;
        if c.is_whitespace() {
            i += c.len_utf8();
            continue;
        }
        if c.is_ascii_digit() {
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            let v = src[start..i].parse().map_err(|_| {
                Diagnostic::error("数字太大了").with_code("E1301").with_primary(Span::new(start, i), "")
            })?;
            out.push((Tok::Int(v), Span::new(start, i)));
            continue;
        }
        if c.is_alphabetic() || c == '_' {
            while i < src.len() {
                let ch = src[i..].chars().next().unwrap();
                if !(ch.is_alphanumeric() || ch == '_' || ch == '\'') {
                    break;
                }
                i += ch.len_utf8();
            }
            let tok = match &src[start..i] {
                "fun" => Tok::Fun,
                "let" => Tok::Let,
                "in" => Tok::In,
                "if" => Tok::If,
                "then" => Tok::Then,
                "else" => Tok::Else,
                "true" => Tok::True,
                "false" => Tok::False,
                _ => Tok::Ident,
            };
            out.push((tok, Span::new(start, i)));
            continue;
        }
        let two = src.get(i..i + 2).unwrap_or("");
        let (tok, len) = match (two, c) {
            ("->", _) => (Tok::Arrow, 2),
            ("==", _) => (Tok::EqEq, 2),
            (_, '=') => (Tok::Eq, 1),
            (_, '<') => (Tok::Lt, 1),
            (_, '+') => (Tok::Plus, 1),
            (_, '-') => (Tok::Minus, 1),
            (_, '*') => (Tok::Star, 1),
            (_, '(') => (Tok::LParen, 1),
            (_, ')') => (Tok::RParen, 1),
            _ => {
                return Err(Diagnostic::error(format!("不认识的字符 `{c}`"))
                    .with_code("E1301")
                    .with_primary(Span::new(start, start + c.len_utf8()), "")
                    .with_note("这门小语言只有：数字、true、false、名字、fun -> let = in if then else + - * < == ( )"));
            }
        };
        i += len;
        out.push((tok, Span::new(start, i)));
    }
    out.push((Tok::Eof, Span::empty_at(src.len())));
    Ok(out)
}

pub fn parse(src: &str) -> Result<Program, Diagnostic> {
    let toks = lex(src)?;
    let mut p = Parser { src, toks, pos: 0, exprs: Vec::new() };
    let root = p.expr()?;
    if p.peek() != Tok::Eof {
        let sp = p.toks[p.pos].1;
        return Err(Diagnostic::error("表达式已经结束了，后面还有多余的内容")
            .with_code("E1302")
            .with_primary(sp, "")
            .with_help("如果想调用函数，把整个表达式用括号括起来"));
    }
    Ok(Program { src: src.to_owned(), exprs: p.exprs, root })
}

struct Parser<'s> {
    src: &'s str,
    toks: Vec<(Tok, Span)>,
    pos: usize,
    exprs: Vec<Expr>,
}

type PResult = Result<ExprId, Diagnostic>;

impl Parser<'_> {
    fn peek(&self) -> Tok {
        self.toks[self.pos].0
    }

    fn bump(&mut self) -> (Tok, Span) {
        let t = self.toks[self.pos];
        if t.0 != Tok::Eof {
            self.pos += 1;
        }
        t
    }

    fn push(&mut self, kind: ExprKind, span: Span) -> ExprId {
        self.exprs.push(Expr { kind, span });
        ExprId(self.exprs.len() as u32 - 1)
    }

    fn span(&self, id: ExprId) -> Span {
        self.exprs[id.index()].span
    }

    fn expect(&mut self, t: Tok, what: &str) -> Result<Span, Diagnostic> {
        if self.peek() == t {
            return Ok(self.bump().1);
        }
        let sp = self.toks[self.pos].1;
        Err(Diagnostic::error(format!("这里应该是 {what}")).with_code("E1302").with_primary(sp, ""))
    }

    fn ident(&mut self) -> Result<String, Diagnostic> {
        let (t, sp) = self.toks[self.pos];
        if t == Tok::Ident {
            self.bump();
            Ok(sp.text(self.src).to_owned())
        } else {
            Err(Diagnostic::error("这里应该是一个名字").with_code("E1302").with_primary(sp, ""))
        }
    }

    fn expr(&mut self) -> PResult {
        let start = self.toks[self.pos].1.start;
        match self.peek() {
            Tok::Fun => {
                self.bump();
                let param = self.ident()?;
                self.expect(Tok::Arrow, "`->`")?;
                let body = self.expr()?;
                let sp = Span::new(start, self.span(body).end);
                Ok(self.push(ExprKind::Fun { param, body }, sp))
            }
            Tok::Let => {
                self.bump();
                let name = self.ident()?;
                self.expect(Tok::Eq, "`=`")?;
                let value = self.expr()?;
                self.expect(Tok::In, "`in`")?;
                let body = self.expr()?;
                let sp = Span::new(start, self.span(body).end);
                Ok(self.push(ExprKind::Let { name, value, body }, sp))
            }
            Tok::If => {
                self.bump();
                let cond = self.expr()?;
                self.expect(Tok::Then, "`then`")?;
                let then = self.expr()?;
                self.expect(Tok::Else, "`else`")?;
                let els = self.expr()?;
                let sp = Span::new(start, self.span(els).end);
                Ok(self.push(ExprKind::If { cond, then, els }, sp))
            }
            _ => self.compare(),
        }
    }

    fn binary_level(&mut self, ops: &[(Tok, BinOp)], next: fn(&mut Self) -> PResult) -> PResult {
        let mut lhs = next(self)?;
        while let Some(&(_, op)) = ops.iter().find(|(t, _)| *t == self.peek()) {
            self.bump();
            // 右边允许直接写 fun / let / if（它们会向右延伸到尽头）
            let rhs = if matches!(self.peek(), Tok::Fun | Tok::Let | Tok::If) { self.expr()? } else { next(self)? };
            let sp = self.span(lhs).to(self.span(rhs));
            lhs = self.push(ExprKind::Bin { op, lhs, rhs }, sp);
        }
        Ok(lhs)
    }

    fn compare(&mut self) -> PResult {
        self.binary_level(&[(Tok::Lt, BinOp::Lt), (Tok::EqEq, BinOp::Eq)], Self::sum)
    }

    fn sum(&mut self) -> PResult {
        self.binary_level(&[(Tok::Plus, BinOp::Add), (Tok::Minus, BinOp::Sub)], Self::product)
    }

    fn product(&mut self) -> PResult {
        self.binary_level(&[(Tok::Star, BinOp::Mul)], Self::app)
    }

    /// 函数调用：连续的原子，左结合（`f x y` = `(f x) y`）。
    fn app(&mut self) -> PResult {
        let mut f = self.atom()?;
        while matches!(self.peek(), Tok::Int(_) | Tok::Ident | Tok::True | Tok::False | Tok::LParen) {
            let arg = self.atom()?;
            let sp = self.span(f).to(self.span(arg));
            f = self.push(ExprKind::App { func: f, arg }, sp);
        }
        Ok(f)
    }

    fn atom(&mut self) -> PResult {
        let (t, sp) = self.toks[self.pos];
        match t {
            Tok::Int(n) => {
                self.bump();
                Ok(self.push(ExprKind::Int(n), sp))
            }
            Tok::True | Tok::False => {
                self.bump();
                Ok(self.push(ExprKind::Bool(t == Tok::True), sp))
            }
            Tok::Ident => {
                self.bump();
                Ok(self.push(ExprKind::Var(sp.text(self.src).to_owned()), sp))
            }
            Tok::LParen => {
                self.bump();
                let inner = self.expr()?;
                let close = self.expect(Tok::RParen, "`)`")?;
                self.exprs[inner.index()].span = sp.to(close);
                Ok(inner)
            }
            _ => Err(Diagnostic::error("这里应该是一个表达式").with_code("E1302").with_primary(sp, "")),
        }
    }
}
