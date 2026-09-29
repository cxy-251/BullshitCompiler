//! mini-lang 的手写词法分析器。
//!
//! 核心就是一个循环：看当前字符，决定接下来是哪种记号，然后把属于它的字符全部读完。
//!
//! ```text
//! 循环：
//!   空白         → 跳过
//!   `//`         → 行注释，跳到行尾
//!   `/*`         → 块注释，跳到 `*/`
//!   字母或 `_`   → 一直读字母、数字、`_`；读完查关键字表：在表里是关键字，否则是标识符
//!   数字         → 一直读数字，得到整数
//!   `=` `<` 等   → 偷看下一个字符：是 `=` 就组成 `==` `<=`，否则就是单个符号
//!   其他单个符号 → 直接产出
//!   都不是       → 报错，跳过这个字符继续（这样一次能报告多个错误）
//! ```
//!
//! 为了教学，每识别一个记号（或跳过一段空白、注释）都记一个 [`Step`]，
//! 说明"凭什么这么切"。

use bsc_core::{Diagnostic, Span};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TokenKind {
    Ident,
    Int(i64),
    // 关键字
    Fn,
    Let,
    Mut,
    If,
    Else,
    While,
    Return,
    True,
    False,
    // 运算符
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Assign,
    EqEq,
    NotEq,
    Lt,
    Le,
    Gt,
    Ge,
    AndAnd,
    OrOr,
    Bang,
    Arrow,
    // 分隔符
    LParen,
    RParen,
    LBrace,
    RBrace,
    Comma,
    Semi,
    Colon,
    Eof,
}

/// 记号的大类，界面用它决定颜色。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenClass {
    Keyword,
    Ident,
    Number,
    Operator,
    Punct,
    End,
}

/// 关键字表：标识符读完后在这里查。
pub const KEYWORDS: &[(&str, TokenKind)] = &[
    ("fn", TokenKind::Fn),
    ("let", TokenKind::Let),
    ("mut", TokenKind::Mut),
    ("if", TokenKind::If),
    ("else", TokenKind::Else),
    ("while", TokenKind::While),
    ("return", TokenKind::Return),
    ("true", TokenKind::True),
    ("false", TokenKind::False),
];

impl TokenKind {
    pub fn class(self) -> TokenClass {
        use TokenKind::*;
        match self {
            Ident => TokenClass::Ident,
            Int(_) => TokenClass::Number,
            Fn | Let | Mut | If | Else | While | Return | True | False => TokenClass::Keyword,
            Plus | Minus | Star | Slash | Percent | Assign | EqEq | NotEq | Lt | Le | Gt | Ge | AndAnd | OrOr
            | Bang | Arrow => TokenClass::Operator,
            LParen | RParen | LBrace | RBrace | Comma | Semi | Colon => TokenClass::Punct,
            Eof => TokenClass::End,
        }
    }

    /// 记号种类的名字（表格里显示）。
    pub fn name(self) -> &'static str {
        use TokenKind::*;
        match self {
            Ident => "标识符",
            Int(_) => "整数",
            Fn | Let | Mut | If | Else | While | Return | True | False => "关键字",
            Plus => "加号",
            Minus => "减号",
            Star => "乘号",
            Slash => "除号",
            Percent => "取余",
            Assign => "赋值",
            EqEq => "等于",
            NotEq => "不等于",
            Lt => "小于",
            Le => "小于等于",
            Gt => "大于",
            Ge => "大于等于",
            AndAnd => "并且",
            OrOr => "或者",
            Bang => "取反",
            Arrow => "箭头",
            LParen => "左圆括号",
            RParen => "右圆括号",
            LBrace => "左花括号",
            RBrace => "右花括号",
            Comma => "逗号",
            Semi => "分号",
            Colon => "冒号",
            Eof => "输入结束",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

/// 扫描器做出的一次决定。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepKind {
    Whitespace,
    LineComment,
    BlockComment,
    /// 标识符或关键字：先当作标识符读完，再查关键字表。
    Word {
        keyword: bool,
    },
    Number,
    /// 运算符或分隔符。`peeked` 表示为了区分单字符/双字符而偷看了下一个字符，
    /// `lookahead` 是偷看到的字符，`two_char` 表示最终组成了双字符记号。
    Symbol {
        peeked: bool,
        lookahead: Option<char>,
        two_char: bool,
    },
    /// 出错的地方（错误信息在 `LexResult::errors` 里）。
    Error,
    Eof,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Step {
    pub span: Span,
    pub kind: StepKind,
    /// 这一步产出的记号（跳过空白、注释、出错时为 `None`）。
    pub token: Option<Token>,
}

#[derive(Clone, Debug)]
pub struct LexResult {
    /// 记号序列，最后一个总是 `Eof`。
    pub tokens: Vec<Token>,
    pub steps: Vec<Step>,
    pub errors: Vec<Diagnostic>,
}

pub fn lex(src: &str) -> LexResult {
    let mut lx = Lexer { src, pos: 0, tokens: Vec::new(), steps: Vec::new(), errors: Vec::new() };
    lx.run();
    LexResult { tokens: lx.tokens, steps: lx.steps, errors: lx.errors }
}

fn is_ident_start(c: char) -> bool {
    c.is_alphabetic() || c == '_'
}

fn is_ident_continue(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

struct Lexer<'s> {
    src: &'s str,
    pos: usize,
    tokens: Vec<Token>,
    steps: Vec<Step>,
    errors: Vec<Diagnostic>,
}

impl Lexer<'_> {
    fn peek(&self) -> Option<char> {
        self.src[self.pos..].chars().next()
    }

    fn peek2(&self) -> Option<char> {
        let mut it = self.src[self.pos..].chars();
        it.next();
        it.next()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.pos += c.len_utf8();
        Some(c)
    }

    fn eat_while(&mut self, f: impl Fn(char) -> bool) {
        while self.peek().is_some_and(&f) {
            self.bump();
        }
    }

    fn step(&mut self, start: usize, kind: StepKind, token: Option<TokenKind>) {
        let span = Span::new(start, self.pos);
        let token = token.map(|kind| Token { kind, span });
        if let Some(t) = token {
            self.tokens.push(t);
        }
        self.steps.push(Step { span, kind, token });
    }

    fn run(&mut self) {
        while let Some(c) = self.peek() {
            let start = self.pos;
            if c.is_whitespace() {
                self.eat_while(char::is_whitespace);
                self.step(start, StepKind::Whitespace, None);
            } else if c == '/' && self.peek2() == Some('/') {
                self.eat_while(|c| c != '\n');
                self.step(start, StepKind::LineComment, None);
            } else if c == '/' && self.peek2() == Some('*') {
                self.block_comment(start);
            } else if is_ident_start(c) {
                self.eat_while(is_ident_continue);
                let text = &self.src[start..self.pos];
                let keyword = KEYWORDS.iter().find(|(k, _)| *k == text).map(|(_, kind)| *kind);
                self.step(
                    start,
                    StepKind::Word { keyword: keyword.is_some() },
                    Some(keyword.unwrap_or(TokenKind::Ident)),
                );
            } else if c.is_ascii_digit() {
                self.number(start);
            } else {
                self.symbol(start, c);
            }
        }
        let end = self.src.len();
        self.tokens.push(Token { kind: TokenKind::Eof, span: Span::empty_at(end) });
        self.steps.push(Step {
            span: Span::empty_at(end),
            kind: StepKind::Eof,
            token: Some(Token { kind: TokenKind::Eof, span: Span::empty_at(end) }),
        });
    }

    fn block_comment(&mut self, start: usize) {
        self.pos += 2;
        match self.src[self.pos..].find("*/") {
            Some(i) => {
                self.pos += i + 2;
                self.step(start, StepKind::BlockComment, None);
            }
            None => {
                self.pos = self.src.len();
                self.errors.push(
                    Diagnostic::error("块注释没有结束")
                        .with_code("E1003")
                        .with_primary(Span::new(start, start + 2), "注释从这里开始，一直到文件末尾都没有遇到 `*/`")
                        .with_help("在注释结尾补上 `*/`"),
                );
                self.step(start, StepKind::Error, None);
            }
        }
    }

    fn number(&mut self, start: usize) {
        self.eat_while(|c| c.is_ascii_digit());
        let digits_end = self.pos;
        // `12abc` 这种写法：数字后面紧跟字母。把整段当成一个错误，而不是切成 12 和 abc。
        if self.peek().is_some_and(is_ident_continue) {
            self.eat_while(is_ident_continue);
            self.errors.push(
                Diagnostic::error("数字后面紧跟着字母")
                    .with_code("E1002")
                    .with_primary(Span::new(start, self.pos), "这不是一个合法的数字，也不是合法的名字")
                    .with_note("名字（标识符）不能以数字开头")
                    .with_help("如果是变量名，把数字挪到后面，比如 `abc12`；如果是两样东西，中间加个空格或运算符"),
            );
            self.step(start, StepKind::Error, None);
            return;
        }
        match self.src[start..digits_end].parse::<i64>() {
            Ok(v) => self.step(start, StepKind::Number, Some(TokenKind::Int(v))),
            Err(_) => {
                self.errors.push(
                    Diagnostic::error("整数太大了")
                        .with_code("E1004")
                        .with_primary(Span::new(start, self.pos), "超出了 64 位整数的范围")
                        .with_note(format!("能表示的最大整数是 {}", i64::MAX)),
                );
                self.step(start, StepKind::Error, None);
            }
        }
    }

    fn symbol(&mut self, start: usize, c: char) {
        use TokenKind::*;
        self.bump();
        // 这些符号自己就是完整的记号，不需要偷看下一个字符。
        let single = match c {
            '+' => Some(Plus),
            '*' => Some(Star),
            '%' => Some(Percent),
            '(' => Some(LParen),
            ')' => Some(RParen),
            '{' => Some(LBrace),
            '}' => Some(RBrace),
            ',' => Some(Comma),
            ';' => Some(Semi),
            ':' => Some(Colon),
            _ => None,
        };
        if let Some(kind) = single {
            self.step(start, StepKind::Symbol { peeked: false, lookahead: None, two_char: false }, Some(kind));
            return;
        }

        // 这些符号可能是双字符记号的开头：偷看下一个字符再决定（"向前看一个字符"）。
        let next = self.peek();
        let (one, two): (Option<TokenKind>, Option<(char, TokenKind)>) = match c {
            '-' => (Some(Minus), Some(('>', Arrow))),
            '/' => (Some(Slash), None),
            '=' => (Some(Assign), Some(('=', EqEq))),
            '!' => (Some(Bang), Some(('=', NotEq))),
            '<' => (Some(Lt), Some(('=', Le))),
            '>' => (Some(Gt), Some(('=', Ge))),
            '&' => (None, Some(('&', AndAnd))),
            '|' => (None, Some(('|', OrOr))),
            _ => (None, None),
        };
        let peeked = two.is_some();
        match (one, two) {
            (_, Some((second, kind))) if next == Some(second) => {
                self.bump();
                self.step(start, StepKind::Symbol { peeked, lookahead: next, two_char: true }, Some(kind));
            }
            (Some(kind), _) => {
                self.step(start, StepKind::Symbol { peeked, lookahead: next, two_char: false }, Some(kind));
            }
            (None, Some((second, _))) => {
                self.errors.push(
                    Diagnostic::error(format!("单独一个 `{c}` 不是合法的运算符"))
                        .with_code("E1001")
                        .with_primary(Span::new(start, self.pos), "")
                        .with_help(format!(
                            "mini-lang 里\"{}\"写作 `{c}{second}`",
                            if c == '&' { "并且" } else { "或者" }
                        )),
                );
                self.step(start, StepKind::Error, None);
            }
            (None, None) => {
                self.errors.push(unknown_char(start, c));
                self.step(start, StepKind::Error, None);
            }
        }
    }
}

fn unknown_char(start: usize, c: char) -> Diagnostic {
    let d = Diagnostic::error(format!("不认识的字符 `{c}`"))
        .with_code("E1000")
        .with_primary(Span::new(start, start + c.len_utf8()), "mini-lang 里没有这个符号");
    let halfwidth = match c {
        '（' => Some("("),
        '）' => Some(")"),
        '；' => Some(";"),
        '，' => Some(","),
        '：' => Some(":"),
        '｛' => Some("{"),
        '｝' => Some("}"),
        '＝' => Some("="),
        '＋' => Some("+"),
        '－' => Some("-"),
        '＊' | '×' => Some("*"),
        '／' | '÷' => Some("/"),
        _ => None,
    };
    match halfwidth {
        Some(h) => d.with_help(format!("这是中文（全角）符号，请换成半角的 `{h}`")),
        None => d,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use TokenKind::*;

    fn kinds(src: &str) -> Vec<TokenKind> {
        let r = lex(src);
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        r.tokens.into_iter().map(|t| t.kind).collect()
    }

    #[test]
    fn keywords_and_identifiers() {
        assert_eq!(kinds("let mut x"), [Let, Mut, Ident, Eof]);
        // 最长匹配：先读完整个单词再查表，所以 iffy 是标识符，不是 if + fy
        assert_eq!(kinds("if iffy"), [If, Ident, Eof]);
        assert_eq!(kinds("_tmp1 x2"), [Ident, Ident, Eof]);
        // 中文也可以做名字
        assert_eq!(kinds("let 总和 = 0;"), [Let, Ident, Assign, Int(0), Semi, Eof]);
    }

    #[test]
    fn operators_with_lookahead() {
        assert_eq!(kinds("a<=b"), [Ident, Le, Ident, Eof]);
        assert_eq!(kinds("a<b"), [Ident, Lt, Ident, Eof]);
        assert_eq!(kinds("a == b = c != d"), [Ident, EqEq, Ident, Assign, Ident, NotEq, Ident, Eof]);
        assert_eq!(kinds("-> - >"), [Arrow, Minus, Gt, Eof]);
        assert_eq!(kinds("a&&b||!c"), [Ident, AndAnd, Ident, OrOr, Bang, Ident, Eof]);
        assert_eq!(kinds("(){},;:%"), [LParen, RParen, LBrace, RBrace, Comma, Semi, Colon, Percent, Eof]);
    }

    #[test]
    fn comments_and_division() {
        assert_eq!(kinds("a / b // 注释\nc"), [Ident, Slash, Ident, Ident, Eof]);
        assert_eq!(kinds("a /* 多行\n注释 */ b"), [Ident, Ident, Eof]);
        let r = lex("x // c");
        assert!(r.steps.iter().any(|s| s.kind == StepKind::LineComment));
    }

    #[test]
    fn spans_and_steps() {
        let src = "let x = 10;";
        let r = lex(src);
        let texts: Vec<&str> = r.tokens.iter().map(|t| t.span.text(src)).collect();
        assert_eq!(texts, ["let", "x", "=", "10", ";", ""]);
        // 每一步首尾相接，覆盖整个源码
        let mut pos = 0;
        for s in &r.steps {
            assert_eq!(s.span.start, pos);
            pos = s.span.end;
        }
        assert_eq!(pos, src.len());
    }

    #[test]
    fn errors_recover_and_continue() {
        let r = lex("let x = 1 # 2；\n12abc & y /* 没完");
        let codes: Vec<_> = r.errors.iter().map(|d| d.code.unwrap()).collect();
        assert_eq!(codes, ["E1000", "E1000", "E1002", "E1001", "E1003"]);
        assert!(r.errors[1].help.as_ref().unwrap().contains(';'));
        // 出错后仍然继续扫描：y 被识别出来了
        assert!(r.tokens.iter().any(|t| t.kind == Ident && t.span.text("let x = 1 # 2；\n12abc & y /* 没完") == "y"));
    }

    #[test]
    fn huge_number() {
        let r = lex("99999999999999999999");
        assert_eq!(r.errors[0].code, Some("E1004"));
    }
}
