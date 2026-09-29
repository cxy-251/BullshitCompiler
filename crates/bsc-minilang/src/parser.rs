//! mini-lang 的语法分析器：语句用递归下降，表达式用 Pratt 分析法。
//!
//! ## 递归下降
//!
//! 每种语法结构一个函数：`parse_function` 解析函数定义、`parse_block` 解析代码块、
//! `parse_statement` 看第一个记号决定是哪种语句……函数之间互相调用，调用关系和语法树的形状一致。
//!
//! ## Pratt 分析法（表达式）
//!
//! 每个二元运算符有一对"绑定力"(左, 右)，数字越大绑得越紧：
//!
//! ```text
//! parse_expr(最小绑定力 m):
//!     左 = 解析一个"最小单位"（数字、变量、括号、负号开头的……）
//!     循环：
//!         看下一个记号 op。不是运算符，或者 op 的左绑定力 < m：停，返回左
//!         否则吃掉 op，右 = parse_expr(op 的右绑定力)，左 = (左 op 右)
//! ```
//!
//! 左绑定力 < 右绑定力的运算符是左结合的（`8-3-2` = `(8-3)-2`），反过来就是右结合的（赋值 `a=b=c`）。
//!
//! ## 错误恢复
//!
//! 语句里出错时，报告错误，然后跳过记号直到 `;`（吃掉）、`}` 或下一个语句开头的关键字，
//! 接着分析下一条语句——这叫"恐慌模式"恢复，一次就能发现多个错误。
//! 行末缺分号时则用"插入式"恢复：报错后假装分号在那里，直接继续。

use bsc_core::{Diagnostic, Span};

use crate::ast::{Ast, BinOp, NodeId, NodeKind, UnOp};
use crate::lexer::{Token, TokenKind};

/// 前缀运算符（`-x`、`!x`）的右绑定力：比所有二元运算符都高。
pub const PREFIX_BP: u8 = 15;

/// 二元运算符：(左绑定力, 右绑定力)。`None` 表示不是二元运算符。
pub fn infix_bp(kind: TokenKind) -> Option<(u8, u8)> {
    use TokenKind::*;
    Some(match kind {
        Assign => (2, 1),
        OrOr => (3, 4),
        AndAnd => (5, 6),
        EqEq | NotEq => (7, 8),
        Lt | Le | Gt | Ge => (9, 10),
        Plus | Minus => (11, 12),
        Star | Slash | Percent => (13, 14),
        _ => return None,
    })
}

/// 运算符表（界面展示用）：(运算符, 左绑定力, 右绑定力, 说明)。
pub const OPERATOR_TABLE: &[(&str, u8, u8, &str)] = &[
    ("=", 2, 1, "赋值，右结合"),
    ("||", 3, 4, "或者"),
    ("&&", 5, 6, "并且"),
    ("== !=", 7, 8, "相等比较"),
    ("< <= > >=", 9, 10, "大小比较"),
    ("+ -", 11, 12, "加减"),
    ("* / %", 13, 14, "乘除取余"),
    ("前缀 - !", 0, PREFIX_BP, "负号、取反，只有右绑定力"),
];

fn binop(kind: TokenKind) -> Option<BinOp> {
    use TokenKind::*;
    Some(match kind {
        Plus => BinOp::Add,
        Minus => BinOp::Sub,
        Star => BinOp::Mul,
        Slash => BinOp::Div,
        Percent => BinOp::Rem,
        EqEq => BinOp::Eq,
        NotEq => BinOp::Ne,
        Lt => BinOp::Lt,
        Le => BinOp::Le,
        Gt => BinOp::Gt,
        Ge => BinOp::Ge,
        AndAnd => BinOp::And,
        OrOr => BinOp::Or,
        _ => return None,
    })
}

/// 分析函数（也就是文法规则）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rule {
    Program,
    Function,
    Param,
    Type,
    Block,
    Statement,
    Let,
    If,
    While,
    Return,
    ExprStmt,
    /// Pratt 的 parse_expr，带最小绑定力。
    Expr(u8),
    Primary,
}

impl Rule {
    pub fn name(self) -> String {
        match self {
            Rule::Program => "parse_program".to_owned(),
            Rule::Function => "parse_function".to_owned(),
            Rule::Param => "parse_param".to_owned(),
            Rule::Type => "parse_type".to_owned(),
            Rule::Block => "parse_block".to_owned(),
            Rule::Statement => "parse_statement".to_owned(),
            Rule::Let => "parse_let".to_owned(),
            Rule::If => "parse_if".to_owned(),
            Rule::While => "parse_while".to_owned(),
            Rule::Return => "parse_return".to_owned(),
            Rule::ExprStmt => "parse_expr_stmt".to_owned(),
            Rule::Expr(bp) => format!("parse_expr({bp})"),
            Rule::Primary => "parse_primary".to_owned(),
        }
    }

    /// 这个函数负责什么（给新手的一句话）。
    pub fn meaning(self) -> &'static str {
        match self {
            Rule::Program => "整个程序：若干函数定义",
            Rule::Function => "函数定义：fn 名字(参数) -> 类型 { … }",
            Rule::Param => "一个参数：名字: 类型",
            Rule::Type => "类型名",
            Rule::Block => "代码块：{ 若干语句 }",
            Rule::Statement => "一条语句：看第一个记号决定是哪种",
            Rule::Let => "变量声明：let [mut] 名字 [: 类型] = 表达式;",
            Rule::If => "条件语句：if 条件 { … } [else …]",
            Rule::While => "循环：while 条件 { … }",
            Rule::Return => "返回：return [表达式];",
            Rule::ExprStmt => "表达式语句：表达式;",
            Rule::Expr(_) => "表达式（Pratt 分析法）",
            Rule::Primary => "表达式的最小单位：数字、变量、调用、括号、负号……",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// 调用分析函数。
    Enter(Rule),
    /// 分析函数返回（出错时也会返回，调用栈总是配对的）。
    Exit(Rule),
    /// 吃掉第 n 个记号。
    Consume(usize),
    /// 新建语法树节点。
    Build(NodeId),
    /// Pratt 循环里的一次判断：下一个记号 `op`（不是二元运算符时为 `None`），
    /// 它的绑定力，以及结论：`bind` 为真表示吃掉它继续组合，否则返回。
    Pratt { min_bp: u8, op: Option<usize>, lbp: u8, rbp: u8, bind: bool },
    /// 报告了第 n 个错误。
    Error(usize),
    /// 错误恢复：跳过了第 `from` 到 `to`（不含）个记号。
    Recover { from: usize, to: usize },
}

#[derive(Clone, Debug)]
pub struct ParseOutput {
    pub ast: Ast,
    pub root: NodeId,
    pub events: Vec<Event>,
    pub errors: Vec<Diagnostic>,
}

/// 分析整个程序。`tokens` 必须以 `Eof` 结尾。
pub fn parse_program(src: &str, tokens: &[Token]) -> ParseOutput {
    let mut p = Parser::new(src, tokens);
    let root = p.program();
    p.finish(root)
}

/// 只分析一个表达式（2.4 课用）。表达式后面还有多余的记号时报错。
pub fn parse_expression(src: &str, tokens: &[Token]) -> ParseOutput {
    let mut p = Parser::new(src, tokens);
    let root = match p.expr(0) {
        Ok(e) => {
            if p.peek().kind != TokenKind::Eof {
                let t = p.peek();
                p.error(
                    Diagnostic::error("表达式后面还有多余的内容")
                        .with_code("E1108")
                        .with_primary(t.span, "从这里开始是多余的")
                        .with_help("两个操作数之间是不是少了运算符？"),
                );
            }
            e
        }
        Err(Fail) => p.ast.push(NodeKind::Error, Span::empty_at(0), vec![]),
    };
    p.finish(root)
}

/// 出错了。具体的错误信息已经记录在 `Parser::errors` 里。
struct Fail;

type PResult = Result<NodeId, Fail>;

struct Parser<'a> {
    src: &'a str,
    tokens: &'a [Token],
    pos: usize,
    ast: Ast,
    events: Vec<Event>,
    errors: Vec<Diagnostic>,
}

impl<'a> Parser<'a> {
    fn new(src: &'a str, tokens: &'a [Token]) -> Self {
        assert!(matches!(tokens.last(), Some(t) if t.kind == TokenKind::Eof), "记号序列必须以 Eof 结尾");
        Self { src, tokens, pos: 0, ast: Ast::default(), events: Vec::new(), errors: Vec::new() }
    }

    fn finish(self, root: NodeId) -> ParseOutput {
        ParseOutput { ast: self.ast, root, events: self.events, errors: self.errors }
    }

    fn peek(&self) -> Token {
        self.tokens[self.pos]
    }

    fn peek_kind(&self) -> TokenKind {
        self.tokens[self.pos].kind
    }

    fn bump(&mut self) -> Token {
        let t = self.peek();
        if t.kind != TokenKind::Eof {
            self.events.push(Event::Consume(self.pos));
            self.pos += 1;
        }
        t
    }

    /// 上一个吃掉的记号的结束位置。
    fn last_end(&self) -> usize {
        if self.pos == 0 { 0 } else { self.tokens[self.pos - 1].span.end }
    }

    fn build(&mut self, kind: NodeKind, start: usize, children: Vec<NodeId>) -> NodeId {
        let span = Span::new(start, self.last_end().max(start));
        let id = self.ast.push(kind, span, children);
        self.events.push(Event::Build(id));
        id
    }

    fn rule<T>(&mut self, r: Rule, f: impl FnOnce(&mut Self) -> Result<T, Fail>) -> Result<T, Fail> {
        self.events.push(Event::Enter(r));
        let res = f(self);
        self.events.push(Event::Exit(r));
        res
    }

    fn error(&mut self, d: Diagnostic) {
        self.events.push(Event::Error(self.errors.len()));
        self.errors.push(d);
    }

    /// 期望下一个记号是 `kind`：是就吃掉，否则报错。
    fn expect(&mut self, kind: TokenKind, what: &str) -> Result<Token, Fail> {
        if self.peek_kind() == kind {
            return Ok(self.bump());
        }
        let t = self.peek();
        let found = describe(self.src, t);
        let mut d = Diagnostic::error(format!("这里应该是 {what}")).with_code("E1101");
        if kind == TokenKind::Semi || t.kind == TokenKind::Eof {
            // 缺分号时指向上一个记号的末尾，比指向下一行开头更直观
            d = d.with_primary(Span::empty_at(self.last_end()), format!("在这里需要 {what}"));
            if kind == TokenKind::Semi {
                d = d.with_help("语句末尾要写分号 `;`");
            }
        } else {
            d = d.with_primary(t.span, format!("却遇到了{found}"));
        }
        self.error(d);
        // 插入式恢复：缺的是分号、而下一个记号已经换行了，多半只是忘了写分号。
        // 报告错误后假装它在那里，继续往下分析，不必跳过下一行（那样会把下一行的错误也吞掉）。
        if kind == TokenKind::Semi && self.src[self.last_end()..t.span.start].contains('\n') {
            return Ok(Token { kind, span: Span::empty_at(self.last_end()) });
        }
        Err(Fail)
    }

    fn expect_ident(&mut self, what: &str) -> Result<(String, Span), Fail> {
        let t = self.peek();
        if t.kind == TokenKind::Ident {
            self.bump();
            return Ok((t.span.text(self.src).to_owned(), t.span));
        }
        let mut d = Diagnostic::error(format!("这里应该是{what}")).with_code("E1102");
        d = d.with_primary(t.span, format!("却遇到了{}", describe(self.src, t)));
        if t.kind.class() == crate::lexer::TokenClass::Keyword {
            d = d.with_note(format!("`{}` 是关键字，不能用作名字", t.span.text(self.src)));
        }
        self.error(d);
        Err(Fail)
    }

    // ---------------------------------------------------------------- 程序与函数

    fn program(&mut self) -> NodeId {
        self.events.push(Event::Enter(Rule::Program));
        let mut items = Vec::new();
        while self.peek_kind() != TokenKind::Eof {
            if self.peek_kind() == TokenKind::Fn {
                match self.function() {
                    Ok(f) => items.push(f),
                    Err(Fail) => {
                        let start = self.peek().span.start;
                        items.push(self.build(NodeKind::Error, start, vec![]));
                        self.skip_until(|k| k == TokenKind::Fn);
                    }
                }
            } else {
                let t = self.peek();
                self.error(
                    Diagnostic::error("程序的最外层只能写函数定义")
                        .with_code("E1103")
                        .with_primary(t.span, format!("这里是{}", describe(self.src, t)))
                        .with_help("把语句放进函数里，比如 `fn main() { … }`"),
                );
                self.skip_until(|k| k == TokenKind::Fn);
            }
        }
        let root = self.build(NodeKind::Program, 0, items);
        self.events.push(Event::Exit(Rule::Program));
        root
    }

    /// 跳过记号直到满足条件（或到文件末尾），记录一次错误恢复。
    fn skip_until(&mut self, stop: impl Fn(TokenKind) -> bool) {
        let from = self.pos;
        while self.peek_kind() != TokenKind::Eof && !stop(self.peek_kind()) {
            self.pos += 1;
        }
        self.events.push(Event::Recover { from, to: self.pos });
    }

    fn function(&mut self) -> PResult {
        self.rule(Rule::Function, |p| {
            let start = p.bump().span.start; // fn
            let (name, _) = p.expect_ident("函数名")?;
            p.expect(TokenKind::LParen, "`(`")?;
            let mut children = Vec::new();
            if p.peek_kind() != TokenKind::RParen {
                loop {
                    children.push(p.param()?);
                    if p.peek_kind() == TokenKind::Comma {
                        p.bump();
                    } else {
                        break;
                    }
                }
            }
            p.expect(TokenKind::RParen, "`)`")?;
            let has_ret = p.peek_kind() == TokenKind::Arrow;
            if has_ret {
                p.bump();
                children.push(p.ty()?);
            }
            children.push(p.block()?);
            Ok(p.build(NodeKind::Function { name, has_ret }, start, children))
        })
    }

    fn param(&mut self) -> PResult {
        self.rule(Rule::Param, |p| {
            let start = p.peek().span.start;
            let (name, _) = p.expect_ident("参数名")?;
            p.expect(TokenKind::Colon, "`:`（参数名后面要写类型，比如 `n: int`）")?;
            let ty = p.ty()?;
            Ok(p.build(NodeKind::Param { name }, start, vec![ty]))
        })
    }

    fn ty(&mut self) -> PResult {
        self.rule(Rule::Type, |p| {
            let start = p.peek().span.start;
            let (name, _) = p.expect_ident("类型名（比如 `int` 或 `bool`）")?;
            Ok(p.build(NodeKind::Type { name }, start, vec![]))
        })
    }

    // ---------------------------------------------------------------- 语句

    fn block(&mut self) -> PResult {
        self.rule(Rule::Block, |p| {
            let open = p.expect(TokenKind::LBrace, "`{`")?;
            let mut stmts = Vec::new();
            while !matches!(p.peek_kind(), TokenKind::RBrace | TokenKind::Eof | TokenKind::Fn) {
                let before = p.pos;
                match p.statement() {
                    Ok(s) => stmts.push(s),
                    Err(Fail) => {
                        let start = p.tokens[before].span.start;
                        stmts.push(p.build(NodeKind::Error, start, vec![]));
                        p.recover_statement();
                    }
                }
            }
            if p.peek_kind() != TokenKind::RBrace {
                let at = Span::empty_at(p.last_end());
                p.error(
                    Diagnostic::error("代码块没有闭合")
                        .with_code("E1104")
                        .with_primary(at, "这里需要 `}`")
                        .with_secondary(open.span, "代码块从这里开始"),
                );
                return Err(Fail);
            }
            p.bump();
            Ok(p.build(NodeKind::Block, open.span.start, stmts))
        })
    }

    /// 恐慌模式：跳到 `;`（吃掉）、`}` 或下一条语句的开头。
    fn recover_statement(&mut self) {
        use TokenKind::*;
        let from = self.pos;
        loop {
            match self.peek_kind() {
                Semi => {
                    self.pos += 1;
                    break;
                }
                RBrace | Eof | Fn | Let | If | While | Return => break,
                _ => self.pos += 1,
            }
        }
        // 什么都没跳过又没有前进时，至少跳过一个记号，防止死循环
        if self.pos == from && !matches!(self.peek_kind(), RBrace | Eof | Fn) {
            self.pos += 1;
        }
        self.events.push(Event::Recover { from, to: self.pos });
    }

    fn statement(&mut self) -> PResult {
        self.rule(Rule::Statement, |p| match p.peek_kind() {
            TokenKind::Let => p.let_stmt(),
            TokenKind::If => p.if_stmt(),
            TokenKind::While => p.while_stmt(),
            TokenKind::Return => p.return_stmt(),
            TokenKind::LBrace => p.block(),
            _ => p.expr_stmt(),
        })
    }

    fn let_stmt(&mut self) -> PResult {
        self.rule(Rule::Let, |p| {
            let start = p.bump().span.start; // let
            let mutable = p.peek_kind() == TokenKind::Mut;
            if mutable {
                p.bump();
            }
            let (name, _) = p.expect_ident("变量名")?;
            let mut children = Vec::new();
            let has_type = p.peek_kind() == TokenKind::Colon;
            if has_type {
                p.bump();
                children.push(p.ty()?);
            }
            p.expect(TokenKind::Assign, "`=`（声明变量时必须给初值）")?;
            children.push(p.expr(0)?);
            p.expect(TokenKind::Semi, "`;`")?;
            Ok(p.build(NodeKind::Let { name, mutable, has_type }, start, children))
        })
    }

    fn if_stmt(&mut self) -> PResult {
        self.rule(Rule::If, |p| {
            let start = p.bump().span.start; // if
            let cond = p.expr(0)?;
            let then = p.block()?;
            let mut children = vec![cond, then];
            if p.peek_kind() == TokenKind::Else {
                p.bump();
                let els = if p.peek_kind() == TokenKind::If { p.if_stmt()? } else { p.block()? };
                children.push(els);
            }
            Ok(p.build(NodeKind::If, start, children))
        })
    }

    fn while_stmt(&mut self) -> PResult {
        self.rule(Rule::While, |p| {
            let start = p.bump().span.start; // while
            let cond = p.expr(0)?;
            let body = p.block()?;
            Ok(p.build(NodeKind::While, start, vec![cond, body]))
        })
    }

    fn return_stmt(&mut self) -> PResult {
        self.rule(Rule::Return, |p| {
            let start = p.bump().span.start; // return
            let mut children = Vec::new();
            if p.peek_kind() != TokenKind::Semi {
                children.push(p.expr(0)?);
            }
            p.expect(TokenKind::Semi, "`;`")?;
            Ok(p.build(NodeKind::Return, start, children))
        })
    }

    fn expr_stmt(&mut self) -> PResult {
        self.rule(Rule::ExprStmt, |p| {
            let start = p.peek().span.start;
            let e = p.expr(0)?;
            p.expect(TokenKind::Semi, "`;`")?;
            Ok(p.build(NodeKind::ExprStmt, start, vec![e]))
        })
    }

    // ---------------------------------------------------------------- 表达式（Pratt）

    fn expr(&mut self, min_bp: u8) -> PResult {
        self.rule(Rule::Expr(min_bp), |p| {
            let start = p.peek().span.start;
            let mut lhs = p.primary()?;
            loop {
                let t = p.peek();
                let Some((lbp, rbp)) = infix_bp(t.kind) else {
                    p.events.push(Event::Pratt { min_bp, op: None, lbp: 0, rbp: 0, bind: false });
                    break;
                };
                let bind = lbp >= min_bp;
                p.events.push(Event::Pratt { min_bp, op: Some(p.pos), lbp, rbp, bind });
                if !bind {
                    break;
                }
                let op_tok = p.bump();
                let rhs = p.expr(rbp)?;
                lhs = if t.kind == TokenKind::Assign {
                    match p.ast.node(lhs).kind.clone() {
                        NodeKind::Var(name) => p.build(NodeKind::Assign { name }, start, vec![rhs]),
                        _ => {
                            let target = p.ast.node(lhs).span;
                            p.error(
                                Diagnostic::error("只能给变量赋值")
                                    .with_code("E1107")
                                    .with_primary(target, "赋值号左边必须是一个变量名")
                                    .with_secondary(op_tok.span, "")
                                    .with_help("如果想比较是否相等，用 `==`"),
                            );
                            return Err(Fail);
                        }
                    }
                } else {
                    let op = binop(t.kind).expect("infix_bp 与 binop 一致");
                    p.build(NodeKind::Binary(op), start, vec![lhs, rhs])
                };
            }
            Ok(lhs)
        })
    }

    fn primary(&mut self) -> PResult {
        self.rule(Rule::Primary, |p| {
            let t = p.peek();
            let start = t.span.start;
            match t.kind {
                TokenKind::Int(n) => {
                    p.bump();
                    Ok(p.build(NodeKind::Int(n), start, vec![]))
                }
                TokenKind::True | TokenKind::False => {
                    p.bump();
                    Ok(p.build(NodeKind::Bool(t.kind == TokenKind::True), start, vec![]))
                }
                TokenKind::Ident => {
                    p.bump();
                    let name = t.span.text(p.src).to_owned();
                    if p.peek_kind() != TokenKind::LParen {
                        return Ok(p.build(NodeKind::Var(name), start, vec![]));
                    }
                    // 函数调用：名字 ( 实参, … )
                    p.bump();
                    let mut args = Vec::new();
                    if p.peek_kind() != TokenKind::RParen {
                        loop {
                            args.push(p.expr(0)?);
                            if p.peek_kind() == TokenKind::Comma {
                                p.bump();
                            } else {
                                break;
                            }
                        }
                    }
                    p.expect(TokenKind::RParen, "`)`")?;
                    Ok(p.build(NodeKind::Call { name }, start, args))
                }
                TokenKind::LParen => {
                    p.bump();
                    let inner = p.expr(0)?;
                    p.expect(TokenKind::RParen, "`)`")?;
                    // 括号不产生节点，只把范围扩大到包含括号
                    let end = p.last_end();
                    p.ast.nodes[inner.index()].span = Span::new(start, end);
                    Ok(inner)
                }
                TokenKind::Minus | TokenKind::Bang => {
                    p.bump();
                    let operand = p.expr(PREFIX_BP)?;
                    let op = if t.kind == TokenKind::Minus { UnOp::Neg } else { UnOp::Not };
                    Ok(p.build(NodeKind::Unary(op), start, vec![operand]))
                }
                _ => {
                    let mut d = Diagnostic::error("这里应该是一个表达式")
                        .with_code("E1105")
                        .with_primary(t.span, format!("却遇到了{}", describe(p.src, t)));
                    d = match t.kind {
                        TokenKind::Semi | TokenKind::RParen | TokenKind::Eof => d.with_help("表达式还没写完"),
                        k if infix_bp(k).is_some() => d.with_help("两个运算符连在一起了，中间缺一个操作数"),
                        _ => d,
                    };
                    p.error(d);
                    Err(Fail)
                }
            }
        })
    }
}

/// 在错误信息里怎么称呼一个记号。
fn describe(src: &str, t: Token) -> String {
    match t.kind {
        TokenKind::Eof => "文件末尾".to_owned(),
        TokenKind::Ident => format!("名字 `{}`", t.span.text(src)),
        TokenKind::Int(_) => format!("数字 `{}`", t.span.text(src)),
        k => format!("{} `{}`", k.name(), t.span.text(src)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;

    fn expr(src: &str) -> String {
        let out = parse_expression(src, &lex(src).tokens);
        assert!(out.errors.is_empty(), "{src}: {:?}", out.errors);
        out.ast.sexpr(out.root)
    }

    #[test]
    fn pratt_precedence_and_associativity() {
        assert_eq!(expr("1 + 2 * 3"), "(+ 1 (* 2 3))");
        assert_eq!(expr("8 - 3 - 2"), "(- (- 8 3) 2)");
        assert_eq!(expr("a = b = 1"), "(a = (b = 1))");
        assert_eq!(expr("-x * !y"), "(* (-1 x) (!1 y))");
        assert_eq!(expr("a < b + 1 && c == d || e"), "(|| (&& (< a (+ b 1)) (== c d)) e)");
        assert_eq!(expr("f(1, g(x) * 2)"), "(f(…) 1 (* (g(…) x) 2))");
        assert_eq!(expr("(1 + 2) * 3"), "(* (+ 1 2) 3)");
    }

    #[test]
    fn program_structure() {
        let src = "fn sum(n: int) -> int {\n  let mut s = 0;\n  while s < n { s = s + 1; }\n  if s == n { return s; } else { return 0; }\n}";
        let out = parse_program(src, &lex(src).tokens);
        assert!(out.errors.is_empty(), "{:?}", out.errors);
        assert_eq!(
            out.ast.sexpr(out.root),
            "(程序 (fn sum (参数 n int) int ({ } (let mut s 0) (while (< s n) ({ } (表达式语句 (s = (+ s 1))))) \
             (if (== s n) ({ } (return s)) ({ } (return 0))))))"
        );
        // 调用栈事件总是配对的
        let enters = out.events.iter().filter(|e| matches!(e, Event::Enter(_))).count();
        let exits = out.events.iter().filter(|e| matches!(e, Event::Exit(_))).count();
        assert_eq!(enters, exits);
    }

    #[test]
    fn error_recovery_reports_several_errors() {
        let src = "fn main() {\n  let x = ;\n  let y = 1\n  x = y * ;\n  return y;\n}";
        let out = parse_program(src, &lex(src).tokens);
        let codes: Vec<_> = out.errors.iter().map(|d| d.code.unwrap()).collect();
        assert_eq!(codes, ["E1105", "E1101", "E1105"]);
        // 出错后还是分析出了最后的 return 语句
        assert!(out.ast.nodes.iter().any(|n| n.kind == NodeKind::Return));
        let enters = out.events.iter().filter(|e| matches!(e, Event::Enter(_))).count();
        let exits = out.events.iter().filter(|e| matches!(e, Event::Exit(_))).count();
        assert_eq!(enters, exits);
    }
}
