//! 语法分析：递归下降。
//!
//! 计算器语言的语法（文法）只有四条规则，每条规则对应下面的一个函数：
//!
//! ```text
//! expr    → term    (('+' | '-') term)*      加减：优先级最低
//! term    → unary   (('*' | '/') unary)*     乘除：优先级更高
//! unary   → '-' unary | primary              负号：优先级最高
//! primary → 整数 | '(' expr ')'              最小的单位
//! ```
//!
//! 规则越靠下，绑定得越紧。`1 + 2 * 3` 里，`expr` 先调用 `term` 读 `1`，
//! 看到 `+` 后再调用 `term`——而这个 `term` 会把 `2 * 3` 整个吃掉，
//! 于是乘法自然成了加法的子树。**运算符优先级就藏在规则的层次里。**
//!
//! 分析过程中每进入/离开一条规则、每吃掉一个记号、每建一个节点都会记一个
//! [`ParseEvent`]，界面据此回放"调用栈"的变化。

use bsc_core::{Diagnostic, Span};

use crate::ast::{Ast, BinOp, NodeId, NodeKind};
use crate::lexer::{Token, TokenKind};

/// 文法规则。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rule {
    Expr,
    Term,
    Unary,
    Primary,
}

impl Rule {
    pub const ALL: [Rule; 4] = [Rule::Expr, Rule::Term, Rule::Unary, Rule::Primary];

    pub fn name(self) -> &'static str {
        match self {
            Rule::Expr => "expr",
            Rule::Term => "term",
            Rule::Unary => "unary",
            Rule::Primary => "primary",
        }
    }

    /// 这条规则的产生式。
    pub fn production(self) -> &'static str {
        match self {
            Rule::Expr => "expr → term (('+' | '-') term)*",
            Rule::Term => "term → unary (('*' | '/') unary)*",
            Rule::Unary => "unary → '-' unary | primary",
            Rule::Primary => "primary → 整数 | '(' expr ')'",
        }
    }

    /// 给新手看的一句话解释。
    pub fn meaning(self) -> &'static str {
        match self {
            Rule::Expr => "若干项用 + 或 - 连起来",
            Rule::Term => "若干因子用 * 或 / 连起来",
            Rule::Unary => "可以带负号的因子",
            Rule::Primary => "一个数，或者括号里的整个表达式",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParseEvent {
    /// 进入一条规则（调用对应的函数）。
    Enter(Rule),
    /// 离开一条规则（函数返回）。
    Exit(Rule),
    /// 吃掉第 n 个记号。
    Consume(usize),
    /// 新建了一个语法树节点。
    Build(NodeId),
}

#[derive(Clone, Debug)]
pub struct ParseResult {
    /// 语法树。出错时里面是出错前已经建好的节点。
    pub ast: Ast,
    /// 分析过程的完整记录。
    pub events: Vec<ParseEvent>,
    /// 根节点，或第一个语法错误。
    pub root: Result<NodeId, Diagnostic>,
}

pub fn parse(tokens: &[Token]) -> ParseResult {
    assert!(matches!(tokens.last(), Some(t) if t.kind == TokenKind::Eof), "记号序列必须以 Eof 结尾");
    let mut p = Parser { tokens, pos: 0, ast: Ast::default(), events: Vec::new() };
    let root = p.parse_all();
    ParseResult { ast: p.ast, events: p.events, root }
}

struct Parser<'t> {
    tokens: &'t [Token],
    pos: usize,
    ast: Ast,
    events: Vec<ParseEvent>,
}

type PResult = Result<NodeId, Diagnostic>;

impl Parser<'_> {
    fn peek(&self) -> Token {
        self.tokens[self.pos]
    }

    fn bump(&mut self) -> Token {
        let t = self.peek();
        self.events.push(ParseEvent::Consume(self.pos));
        if t.kind != TokenKind::Eof {
            self.pos += 1;
        }
        t
    }

    fn build(&mut self, kind: NodeKind, span: Span) -> NodeId {
        let id = self.ast.push(kind, span);
        self.events.push(ParseEvent::Build(id));
        id
    }

    /// 包一层"进入/离开规则"的记录。出错时不记 Exit，调用栈停在出错的位置。
    fn rule(&mut self, rule: Rule, f: impl FnOnce(&mut Self) -> PResult) -> PResult {
        self.events.push(ParseEvent::Enter(rule));
        let r = f(self)?;
        self.events.push(ParseEvent::Exit(rule));
        Ok(r)
    }

    fn parse_all(&mut self) -> PResult {
        if self.peek().kind == TokenKind::Eof {
            return Err(Diagnostic::error("输入是空的")
                .with_code("E0204")
                .with_primary(self.peek().span, "这里需要一个表达式")
                .with_help("试试输入 `1 + 2 * 3`"));
        }
        let root = self.expr()?;
        let t = self.peek();
        if t.kind != TokenKind::Eof {
            let root_span = self.ast.node(root).span;
            let mut d = Diagnostic::error(format!("表达式已经完整了，后面却还有{}", t.kind.describe()))
                .with_code("E0203")
                .with_primary(t.span, "多出来的部分从这里开始")
                .with_secondary(root_span, "这是一个完整的表达式");
            d = match t.kind {
                TokenKind::RParen => d.with_help("多了一个右括号，检查括号是否配对"),
                TokenKind::Int(_) | TokenKind::LParen => {
                    d.with_help("两个操作数之间缺少运算符，比如 `2 3` 应该写成 `2 * 3` 或 `2 + 3`")
                }
                _ => d,
            };
            return Err(d);
        }
        Ok(root)
    }

    /// expr → term (('+' | '-') term)*
    fn expr(&mut self) -> PResult {
        self.rule(Rule::Expr, |p| {
            let mut lhs = p.term()?;
            // 用循环而不是递归处理连续的 + -，这样 `8 - 3 - 2` 会被分析成
            // `(8 - 3) - 2`（左结合），而不是错误的 `8 - (3 - 2)`。
            while let Some(op) = match p.peek().kind {
                TokenKind::Plus => Some(BinOp::Add),
                TokenKind::Minus => Some(BinOp::Sub),
                _ => None,
            } {
                p.bump();
                let rhs = p.term()?;
                let span = p.ast.node(lhs).span.to(p.ast.node(rhs).span);
                lhs = p.build(NodeKind::Binary(op, lhs, rhs), span);
            }
            Ok(lhs)
        })
    }

    /// term → unary (('*' | '/') unary)*
    fn term(&mut self) -> PResult {
        self.rule(Rule::Term, |p| {
            let mut lhs = p.unary()?;
            while let Some(op) = match p.peek().kind {
                TokenKind::Star => Some(BinOp::Mul),
                TokenKind::Slash => Some(BinOp::Div),
                _ => None,
            } {
                p.bump();
                let rhs = p.unary()?;
                let span = p.ast.node(lhs).span.to(p.ast.node(rhs).span);
                lhs = p.build(NodeKind::Binary(op, lhs, rhs), span);
            }
            Ok(lhs)
        })
    }

    /// unary → '-' unary | primary
    fn unary(&mut self) -> PResult {
        self.rule(Rule::Unary, |p| {
            if p.peek().kind == TokenKind::Minus {
                let minus = p.bump();
                let operand = p.unary()?;
                let span = minus.span.to(p.ast.node(operand).span);
                Ok(p.build(NodeKind::Neg(operand), span))
            } else {
                p.primary()
            }
        })
    }

    /// primary → 整数 | '(' expr ')'
    fn primary(&mut self) -> PResult {
        self.rule(Rule::Primary, |p| {
            let t = p.peek();
            match t.kind {
                TokenKind::Int(n) => {
                    p.bump();
                    Ok(p.build(NodeKind::Num(n), t.span))
                }
                TokenKind::LParen => {
                    let open = p.bump();
                    let inner = p.expr()?;
                    let close = p.peek();
                    if close.kind != TokenKind::RParen {
                        return Err(Diagnostic::error("括号没有闭合")
                            .with_code("E0202")
                            .with_primary(close.span, format!("这里需要 `)`，却遇到了{}", close.kind.describe()))
                            .with_secondary(open.span, "与这个左括号配对"));
                    }
                    p.bump();
                    // 括号只影响"怎么分组"，不会在树里留下节点——分组关系已经体现在树的形状上。
                    // 但节点覆盖的源码范围扩大到包含括号，方便高亮。
                    let node = *p.ast.node(inner);
                    p.ast.nodes[inner.index()].span = node.span.to(open.span).to(close.span);
                    Ok(inner)
                }
                _ => {
                    let d = Diagnostic::error("这里应该是一个数字或左括号")
                        .with_code("E0201")
                        .with_primary(t.span, format!("遇到了{}", t.kind.describe()));
                    Err(match t.kind {
                        TokenKind::Eof => d.with_help("表达式还没写完，结尾缺一个数字"),
                        TokenKind::RParen => d.with_help("括号里是空的，或者右括号多了"),
                        TokenKind::Plus | TokenKind::Star | TokenKind::Slash => {
                            d.with_help("两个运算符连在一起了，中间缺一个数字")
                        }
                        _ => d,
                    })
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;

    fn sexpr(src: &str) -> String {
        let r = parse(&lex(src).unwrap());
        let root = r.root.unwrap();
        r.ast.sexpr(root)
    }

    fn err(src: &str) -> Diagnostic {
        parse(&lex(src).unwrap()).root.unwrap_err()
    }

    #[test]
    fn precedence() {
        assert_eq!(sexpr("1 + 2 * 3"), "(+ 1 (* 2 3))");
        assert_eq!(sexpr("(1 + 2) * 3"), "(* (+ 1 2) 3)");
        assert_eq!(sexpr("-2 * 3"), "(* (neg 2) 3)");
        assert_eq!(sexpr("--1"), "(neg (neg 1))");
    }

    #[test]
    fn left_associative() {
        assert_eq!(sexpr("8 - 3 - 2"), "(- (- 8 3) 2)");
        assert_eq!(sexpr("8 / 4 / 2"), "(/ (/ 8 4) 2)");
    }

    #[test]
    fn node_spans_include_parens() {
        let src = "2 * (3 + 4)";
        let r = parse(&lex(src).unwrap());
        let root = r.root.unwrap();
        assert_eq!(r.ast.node(root).span.text(src), src);
        let rhs = r.ast.children(root)[1];
        assert_eq!(r.ast.node(rhs).span.text(src), "(3 + 4)");
    }

    #[test]
    fn events_are_balanced() {
        let r = parse(&lex("1 + 2 * (3 - 4)").unwrap());
        r.root.unwrap();
        let mut depth = 0i32;
        for e in &r.events {
            match e {
                ParseEvent::Enter(_) => depth += 1,
                ParseEvent::Exit(_) => depth -= 1,
                _ => {}
            }
            assert!(depth >= 0);
        }
        assert_eq!(depth, 0);
        let builds = r.events.iter().filter(|e| matches!(e, ParseEvent::Build(_))).count();
        assert_eq!(builds, r.ast.nodes.len());
    }

    #[test]
    fn syntax_errors() {
        assert_eq!(err("").code, Some("E0204"));
        assert_eq!(err("1 +").code, Some("E0201"));
        assert_eq!(err("1 + * 2").code, Some("E0201"));
        assert_eq!(err("1 + * 2").primary_span(), Some(Span::new(4, 5)));
        let d = err("(1 + 2");
        assert_eq!(d.code, Some("E0202"));
        assert_eq!(d.labels[1].span, Span::new(0, 1));
        assert_eq!(err("1 + 2)").code, Some("E0203"));
        assert_eq!(err("2 3").code, Some("E0203"));
    }
}
