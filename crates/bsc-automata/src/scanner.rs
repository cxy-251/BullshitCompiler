//! 由正则表达式生成词法分析器（lex / flex 的原理）。
//!
//! 给每种记号写一条正则，比如：
//!
//! ```text
//! IF      if
//! IDENT   [a-z_][a-z0-9_]*
//! NUMBER  [0-9]+
//! 空白    \s+          （跳过）
//! ```
//!
//! 生成过程就是本章前面几课的流水线：每条正则 → NFA，用一个新起点把它们并起来，
//! 子集构造成 DFA，再最小化。接受状态记着"是哪条规则"。
//!
//! 扫描时有两条规则解决歧义：
//! 1. **最长匹配**：尽量往后读，`iffy` 是一个标识符，而不是 `if` + `fy`。
//!    做法是一直走 DFA，记住最近一次经过接受状态的位置，走不动了就退回那里。
//! 2. **优先级**：同样长时，写在前面的规则优先，所以 `if` 是关键字而不是标识符。

use bsc_core::{Diagnostic, Span};

use crate::dfa::{Dfa, SubsetStep, subset_construction};
use crate::minimize::{Minimization, minimize};
use crate::nfa::{Construction, thompson_many};
use crate::regex::Regex;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rule {
    pub name: String,
    pub pattern: String,
    /// 匹配到后直接丢弃（空白、注释）。
    pub skip: bool,
}

impl Rule {
    pub fn new(name: &str, pattern: &str) -> Self {
        Self { name: name.to_owned(), pattern: pattern.to_owned(), skip: false }
    }

    pub fn skip(name: &str, pattern: &str) -> Self {
        Self { skip: true, ..Self::new(name, pattern) }
    }
}

/// 生成好的词法分析器，以及生成过程中的全部中间产物。
#[derive(Clone, Debug)]
pub struct Lexer {
    pub rules: Vec<Rule>,
    pub regexes: Vec<Regex>,
    pub nfa: Construction,
    pub dfa: Dfa,
    pub subset_steps: Vec<SubsetStep>,
    pub min: Minimization,
}

/// 生成失败：第几条规则出了什么问题。
#[derive(Clone, Debug)]
pub struct BuildError {
    pub rule: usize,
    pub diagnostic: Diagnostic,
}

impl Lexer {
    pub fn build(rules: Vec<Rule>) -> Result<Lexer, Box<BuildError>> {
        let mut regexes = Vec::with_capacity(rules.len());
        for (i, r) in rules.iter().enumerate() {
            let re = Regex::parse(&r.pattern).map_err(|diagnostic| Box::new(BuildError { rule: i, diagnostic }))?;
            if re.matches("") {
                return Err(Box::new(BuildError {
                    rule: i,
                    diagnostic: Diagnostic::error(format!("规则 {} 能匹配空串", r.name))
                        .with_code("E0410")
                        .with_primary(Span::new(0, r.pattern.len()), "这条正则什么都不读也能匹配成功")
                        .with_note("那样的话扫描器会原地不动地产出无穷多个空记号")
                        .with_help("把 `*` 换成 `+`，或者去掉最外层的 `?`"),
                }));
            }
            regexes.push(re);
        }
        let nfa = thompson_many(&regexes.iter().collect::<Vec<_>>());
        let (dfa, subset_steps) = subset_construction(&nfa.nfa);
        let min = minimize(&dfa);
        Ok(Lexer { rules, regexes, nfa, dfa, subset_steps, min })
    }

    /// 用最小 DFA 扫描输入。
    pub fn scan(&self, input: &str) -> Scan {
        let dfa = &self.min.dfa;
        let mut events = Vec::new();
        let mut tokens = Vec::new();
        let mut pos = 0;

        while pos < input.len() {
            events.push(ScanEvent::Begin { pos });
            let mut state = dfa.start;
            let mut last_accept: Option<(usize, usize)> = None; // (结束位置, 规则)
            let mut cur = pos;
            for (off, c) in input[pos..].char_indices() {
                let Some(next) = dfa.next(state, c) else {
                    events.push(ScanEvent::Stuck { pos: pos + off, ch: Some(c), state });
                    break;
                };
                state = next;
                cur = pos + off + c.len_utf8();
                let accept = dfa.states[state].accept;
                if let Some(rule) = accept {
                    last_accept = Some((cur, rule));
                }
                events.push(ScanEvent::Advance { pos: cur, ch: c, state, accept });
            }
            if cur == input.len() && events.last().is_some_and(|e| matches!(e, ScanEvent::Advance { .. })) {
                events.push(ScanEvent::Stuck { pos: cur, ch: None, state });
            }

            match last_accept {
                Some((end, rule)) => {
                    let lexeme = Lexeme { rule, span: Span::new(pos, end) };
                    let skipped = self.rules[rule].skip;
                    events.push(ScanEvent::Emit { lexeme, skipped, backtrack: input[end..cur].chars().count() });
                    if !skipped {
                        tokens.push(lexeme);
                    }
                    pos = end;
                }
                None => {
                    let c = input[pos..].chars().next().unwrap();
                    let span = Span::new(pos, pos + c.len_utf8());
                    events.push(ScanEvent::Error { pos });
                    let diagnostic = Diagnostic::error(format!("没有任何规则能匹配从 `{c}` 开始的内容"))
                        .with_code("E0411")
                        .with_primary(span, "扫描在这里卡住了")
                        .with_help("检查是不是漏写了某种记号的规则");
                    return Scan { tokens, events, error: Some(diagnostic) };
                }
            }
        }
        Scan { tokens, events, error: None }
    }
}

/// 识别出的一个记号：哪条规则，源码哪一段。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lexeme {
    pub rule: usize,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScanEvent {
    /// 从 `pos` 开始识别一个新记号，DFA 回到起点。
    Begin { pos: usize },
    /// 读入字符 `ch`（读完后位置为 `pos`），DFA 走到 `state`；
    /// `accept` 不为空说明这里可以结束一个记号——先记下来，但还要继续往后试。
    Advance { pos: usize, ch: char, state: usize, accept: Option<usize> },
    /// 在状态 `state` 读到 `ch` 时无路可走（`ch` 为 `None` 表示输入结束）。
    Stuck { pos: usize, ch: Option<char>, state: usize },
    /// 退回到最近一次接受的位置，产出记号。`backtrack` 是多读了、需要吐回去的字符数。
    Emit { lexeme: Lexeme, skipped: bool, backtrack: usize },
    /// 一个字符都匹配不了：报错。
    Error { pos: usize },
}

#[derive(Clone, Debug)]
pub struct Scan {
    /// 识别出的记号（不含被跳过的）。
    pub tokens: Vec<Lexeme>,
    pub events: Vec<ScanEvent>,
    pub error: Option<Diagnostic>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lexer() -> Lexer {
        Lexer::build(vec![
            Rule::new("IF", "if"),
            Rule::new("IDENT", "[a-z_][a-z0-9_]*"),
            Rule::new("NUMBER", "[0-9]+"),
            Rule::new("LE", "<="),
            Rule::new("LT", "<"),
            Rule::skip("空白", "\\s+"),
        ])
        .unwrap()
    }

    fn names(l: &Lexer, input: &str) -> Vec<String> {
        let scan = l.scan(input);
        assert!(scan.error.is_none(), "{:?}", scan.error);
        scan.tokens.iter().map(|t| format!("{}:{}", l.rules[t.rule].name, t.span.text(input))).collect()
    }

    #[test]
    fn priority_and_longest_match() {
        let l = lexer();
        assert_eq!(names(&l, "if iffy"), ["IF:if", "IDENT:iffy"]);
        assert_eq!(names(&l, "x<=10"), ["IDENT:x", "LE:<=", "NUMBER:10"]);
        assert_eq!(names(&l, "a<b"), ["IDENT:a", "LT:<", "IDENT:b"]);
    }

    #[test]
    fn backtracking_to_last_accept() {
        // 只有 "ab" 和 "abcd" 两种记号时，"abc" 会读到 c 之后才发现走不通，退回到 "ab"。
        let l = Lexer::build(vec![Rule::new("AB", "ab"), Rule::new("ABCD", "abcd"), Rule::new("C", "c")]).unwrap();
        let scan = l.scan("abc");
        assert!(scan.error.is_none());
        assert_eq!(scan.tokens.len(), 2);
        assert!(scan.events.iter().any(|e| matches!(e, ScanEvent::Emit { backtrack: 1, .. })));
    }

    #[test]
    fn errors() {
        let l = lexer();
        let scan = l.scan("x = 1");
        assert_eq!(scan.error.unwrap().code, Some("E0411"));
        assert_eq!(scan.tokens.len(), 1);

        let e = Lexer::build(vec![Rule::new("A", "a*")]).unwrap_err();
        assert_eq!((e.rule, e.diagnostic.code), (0, Some("E0410")));
        let e = Lexer::build(vec![Rule::new("A", "a"), Rule::new("B", "(b")]).unwrap_err();
        assert_eq!((e.rule, e.diagnostic.code), (1, Some("E0401")));
    }
}
