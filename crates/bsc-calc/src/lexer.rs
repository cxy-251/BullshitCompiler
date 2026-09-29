//! 词法分析：把字符流切成记号流。
//!
//! 人读 `12+3` 时会自然地看成"十二、加、三"三个单位，而不是 `1`、`2`、`+`、`3`
//! 四个字符。词法分析器做的就是这件事：从左到右扫描，每次看当前字符决定
//! "接下来是什么记号"，然后把属于这个记号的字符全部吃掉。

use bsc_core::{Diagnostic, Span};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenKind {
    /// 整数字面量，比如 `42`。
    Int(i64),
    Plus,
    Minus,
    Star,
    Slash,
    LParen,
    RParen,
    /// 输入结束。放一个显式的"结束"记号，语法分析器就不用处处判断越界。
    Eof,
}

/// 记号的大类，界面用它决定颜色。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenClass {
    Number,
    Operator,
    Paren,
    End,
}

impl TokenKind {
    pub fn class(self) -> TokenClass {
        match self {
            TokenKind::Int(_) => TokenClass::Number,
            TokenKind::Plus | TokenKind::Minus | TokenKind::Star | TokenKind::Slash => TokenClass::Operator,
            TokenKind::LParen | TokenKind::RParen => TokenClass::Paren,
            TokenKind::Eof => TokenClass::End,
        }
    }

    /// 记号种类的中文名。
    pub fn name(self) -> &'static str {
        match self {
            TokenKind::Int(_) => "整数",
            TokenKind::Plus => "加号",
            TokenKind::Minus => "减号",
            TokenKind::Star => "乘号",
            TokenKind::Slash => "除号",
            TokenKind::LParen => "左括号",
            TokenKind::RParen => "右括号",
            TokenKind::Eof => "输入结束",
        }
    }

    /// 在错误信息里怎么称呼这个记号。
    pub fn describe(self) -> String {
        match self {
            TokenKind::Int(n) => format!("数字 `{n}`"),
            TokenKind::Eof => "输入结束".to_owned(),
            other => format!("{} `{}`", other.name(), other.symbol()),
        }
    }

    pub fn symbol(self) -> &'static str {
        match self {
            TokenKind::Int(_) => "整数",
            TokenKind::Plus => "+",
            TokenKind::Minus => "-",
            TokenKind::Star => "*",
            TokenKind::Slash => "/",
            TokenKind::LParen => "(",
            TokenKind::RParen => ")",
            TokenKind::Eof => "EOF",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

/// 把源码切成记号。成功时最后一个记号总是 [`TokenKind::Eof`]。
pub fn lex(source: &str) -> Result<Vec<Token>, Diagnostic> {
    let mut tokens = Vec::new();
    let mut chars = source.char_indices().peekable();

    while let Some(&(start, c)) = chars.peek() {
        // 1. 空白：直接跳过，它只用来分隔记号。
        if c.is_whitespace() {
            chars.next();
            continue;
        }

        // 2. 数字：一直读到不是数字为止（"最长匹配"——`123` 是一个数，不是三个）。
        if c.is_ascii_digit() {
            let mut end = start;
            while let Some(&(i, d)) = chars.peek() {
                if !d.is_ascii_digit() {
                    break;
                }
                end = i + d.len_utf8();
                chars.next();
            }
            let span = Span::new(start, end);
            let value = span.text(source).parse::<i64>().map_err(|_| {
                Diagnostic::error("数字太大了")
                    .with_code("E0102")
                    .with_primary(span, "超出了 64 位整数的范围")
                    .with_note(format!("能表示的最大整数是 {}", i64::MAX))
            })?;
            tokens.push(Token { kind: TokenKind::Int(value), span });
            continue;
        }

        // 3. 单个字符的符号。
        let kind = match c {
            '+' => TokenKind::Plus,
            '-' => TokenKind::Minus,
            '*' => TokenKind::Star,
            '/' => TokenKind::Slash,
            '(' => TokenKind::LParen,
            ')' => TokenKind::RParen,
            _ => return Err(unknown_char(start, c)),
        };
        chars.next();
        tokens.push(Token { kind, span: Span::new(start, start + c.len_utf8()) });
    }

    tokens.push(Token { kind: TokenKind::Eof, span: Span::empty_at(source.len()) });
    Ok(tokens)
}

fn unknown_char(start: usize, c: char) -> Diagnostic {
    let span = Span::new(start, start + c.len_utf8());
    let diag = Diagnostic::error(format!("不认识的字符 `{c}`"))
        .with_code("E0101")
        .with_primary(span, "计算器语言里没有这个符号");

    // 中文输入法下最常见的几种误输入，给出具体的改法。
    let suggestion = match c {
        '（' => Some("("),
        '）' => Some(")"),
        '＋' => Some("+"),
        '－' | '—' => Some("-"),
        '×' | '＊' | 'x' | 'X' => Some("*"),
        '÷' | '／' => Some("/"),
        _ => None,
    };
    if let Some(s) = suggestion {
        return diag.with_help(format!("你可能想输入半角的 `{s}`"));
    }
    if ('０'..='９').contains(&c) {
        return diag.with_help("这是全角数字，请切换到半角输入");
    }
    if c.is_alphabetic() {
        return diag.with_note("计算器语言只有整数和 + - * / ( )，没有变量或函数");
    }
    diag.with_note("计算器语言只有整数和 + - * / ( )")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(src: &str) -> Vec<TokenKind> {
        lex(src).unwrap().into_iter().map(|t| t.kind).collect()
    }

    #[test]
    fn splits_numbers_and_operators() {
        use TokenKind::*;
        assert_eq!(kinds("12+3"), vec![Int(12), Plus, Int(3), Eof]);
        assert_eq!(
            kinds(" ( 1 -2 )*4/ 5 "),
            vec![LParen, Int(1), Minus, Int(2), RParen, Star, Int(4), Slash, Int(5), Eof]
        );
        assert_eq!(kinds(""), vec![Eof]);
    }

    #[test]
    fn spans_point_back_to_source() {
        let src = "  42 + 7";
        let toks = lex(src).unwrap();
        assert_eq!(toks[0].span.text(src), "42");
        assert_eq!(toks[1].span.text(src), "+");
        assert_eq!(toks[2].span.text(src), "7");
        assert_eq!(toks[3].span, Span::empty_at(src.len()));
    }

    #[test]
    fn fullwidth_paren_gets_a_hint() {
        let err = lex("（1+2)").unwrap_err();
        assert_eq!(err.code, Some("E0101"));
        assert_eq!(err.primary_span(), Some(Span::new(0, 3)));
        assert!(err.help.unwrap().contains('('));
    }

    #[test]
    fn huge_number_is_an_error() {
        let err = lex("1 + 99999999999999999999").unwrap_err();
        assert_eq!(err.code, Some("E0102"));
        assert_eq!(err.primary_span(), Some(Span::new(4, 24)));
    }
}
