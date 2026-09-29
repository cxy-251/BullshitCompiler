//! 正则表达式的语法分析。
//!
//! 支持的写法：
//!
//! | 写法 | 含义 |
//! |---|---|
//! | `a` | 字符 a 本身 |
//! | `ab` | 连接：先 a 后 b |
//! | `a\|b` | 选择：a 或 b |
//! | `a*` | 重复 0 次或多次 |
//! | `a+` | 重复 1 次或多次 |
//! | `a?` | 可有可无 |
//! | `(…)` | 分组 |
//! | `[a-z0-9]` `[^…]` | 字符类 / 取反 |
//! | `.` | 除换行外的任意字符 |
//! | `\d` `\w` `\s` | 数字 / 单词字符 / 空白 |
//! | `\*` `\(` … | 转义：表示符号本身 |
//!
//! 只有前三种（连接、选择、`*`）是"本质"的，其余都是简写：`a+` = `aa*`，
//! `a?` = `a|ε`，`[abc]` = `a|b|c`。
//!
//! 文法（优先级从低到高）：
//!
//! ```text
//! alt    → concat ('|' concat)*
//! concat → repeat*                  （可以为空，表示 ε）
//! repeat → atom ('*' | '+' | '?')*
//! atom   → 字符 | '.' | 字符类 | 转义 | '(' alt ')'
//! ```

use bsc_core::{Diagnostic, Span};

use crate::charset::CharSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RegexId(pub u32);

impl RegexId {
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegexKind {
    /// ε：空串。
    Empty,
    /// 一个字符集中的任意一个字符（单个字符也是字符集）。
    Set(CharSet),
    Concat(RegexId, RegexId),
    Alt(RegexId, RegexId),
    Star(RegexId),
    Plus(RegexId),
    Optional(RegexId),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegexNode {
    pub kind: RegexKind,
    pub span: Span,
}

/// 解析好的正则表达式：语法树节点存放在数组里，用 [`RegexId`] 引用。
#[derive(Clone, Debug)]
pub struct Regex {
    pub source: String,
    pub nodes: Vec<RegexNode>,
    pub root: RegexId,
}

impl Regex {
    pub fn parse(source: &str) -> Result<Regex, Diagnostic> {
        let mut p = Parser { src: source, pos: 0, nodes: Vec::new() };
        let root = p.alt()?;
        if let Some(c) = p.peek() {
            // alt() 只会在遇到 ')' 或末尾时停下
            debug_assert_eq!(c, ')');
            return Err(Diagnostic::error("多余的右括号")
                .with_code("E0404")
                .with_primary(Span::new(p.pos, p.pos + 1), "没有和它配对的左括号")
                .with_help("如果想匹配字符 `)` 本身，写成 `\\)`"));
        }
        Ok(Regex { source: source.to_owned(), nodes: p.nodes, root })
    }

    pub fn node(&self, id: RegexId) -> &RegexNode {
        &self.nodes[id.index()]
    }

    pub fn children(&self, id: RegexId) -> Vec<RegexId> {
        match self.node(id).kind {
            RegexKind::Empty | RegexKind::Set(_) => vec![],
            RegexKind::Concat(a, b) | RegexKind::Alt(a, b) => vec![a, b],
            RegexKind::Star(a) | RegexKind::Plus(a) | RegexKind::Optional(a) => vec![a],
        }
    }

    /// 节点在树图里显示的文字。
    pub fn label(&self, id: RegexId) -> String {
        match &self.node(id).kind {
            RegexKind::Empty => "ε".to_owned(),
            RegexKind::Set(s) => s.label(),
            RegexKind::Concat(..) => "连接".to_owned(),
            RegexKind::Alt(..) => "或 |".to_owned(),
            RegexKind::Star(_) => "*".to_owned(),
            RegexKind::Plus(_) => "+".to_owned(),
            RegexKind::Optional(_) => "?".to_owned(),
        }
    }

    /// 这种节点是什么意思（给新手的一句话）。
    pub fn explain(&self, id: RegexId) -> String {
        let text = |id: RegexId| self.node(id).span.text(&self.source).to_owned();
        match &self.node(id).kind {
            RegexKind::Empty => "空串 ε：什么都不匹配也算成功".to_owned(),
            RegexKind::Set(s) => format!("匹配一个字符：{}", s.label()),
            RegexKind::Concat(a, b) => format!("先匹配 `{}`，紧接着匹配 `{}`", text(*a), text(*b)),
            RegexKind::Alt(a, b) => format!("匹配 `{}` 或者 `{}`", text(*a), text(*b)),
            RegexKind::Star(a) => format!("把 `{}` 重复 0 次或更多次", text(*a)),
            RegexKind::Plus(a) => format!("把 `{}` 重复 1 次或更多次", text(*a)),
            RegexKind::Optional(a) => format!("`{}` 可有可无", text(*a)),
        }
    }

    /// 参考实现：直接在语法树上判断整串是否匹配（不经过任何自动机）。
    ///
    /// 思路是"位置集合"：对一个节点，给出所有可能的起点，求出所有可能的终点。
    /// 它很慢，但简单到一眼能看出是对的，测试里用来核对自动机的结果。
    pub fn matches(&self, input: &str) -> bool {
        let chars: Vec<char> = input.chars().collect();
        let ends = self.ends(self.root, &[0].into_iter().collect(), &chars);
        ends.contains(&chars.len())
    }

    fn ends(
        &self,
        id: RegexId,
        starts: &std::collections::BTreeSet<usize>,
        s: &[char],
    ) -> std::collections::BTreeSet<usize> {
        match &self.node(id).kind {
            RegexKind::Empty => starts.clone(),
            RegexKind::Set(set) => {
                starts.iter().filter(|&&i| i < s.len() && set.contains(s[i])).map(|i| i + 1).collect()
            }
            RegexKind::Concat(a, b) => {
                let mid = self.ends(*a, starts, s);
                self.ends(*b, &mid, s)
            }
            RegexKind::Alt(a, b) => {
                let mut r = self.ends(*a, starts, s);
                r.extend(self.ends(*b, starts, s));
                r
            }
            RegexKind::Optional(a) => {
                let mut r = self.ends(*a, starts, s);
                r.extend(starts.iter().copied());
                r
            }
            RegexKind::Star(a) | RegexKind::Plus(a) => {
                let mut reached =
                    if matches!(self.node(id).kind, RegexKind::Star(_)) { starts.clone() } else { Default::default() };
                let mut frontier = self.ends(*a, starts, s);
                while !frontier.is_subset(&reached) {
                    reached.extend(frontier.iter().copied());
                    frontier = self.ends(*a, &frontier, s);
                }
                reached
            }
        }
    }
}

struct Parser<'s> {
    src: &'s str,
    pos: usize,
    nodes: Vec<RegexNode>,
}

type PResult = Result<RegexId, Diagnostic>;

impl Parser<'_> {
    fn peek(&self) -> Option<char> {
        self.src[self.pos..].chars().next()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.pos += c.len_utf8();
        Some(c)
    }

    fn push(&mut self, kind: RegexKind, span: Span) -> RegexId {
        let id = RegexId(self.nodes.len() as u32);
        self.nodes.push(RegexNode { kind, span });
        id
    }

    fn span_of(&self, id: RegexId) -> Span {
        self.nodes[id.index()].span
    }

    /// alt → concat ('|' concat)*
    fn alt(&mut self) -> PResult {
        let mut lhs = self.concat()?;
        while self.peek() == Some('|') {
            self.bump();
            let rhs = self.concat()?;
            let span = self.span_of(lhs).to(self.span_of(rhs));
            lhs = self.push(RegexKind::Alt(lhs, rhs), span);
        }
        Ok(lhs)
    }

    /// concat → repeat*
    fn concat(&mut self) -> PResult {
        let mut acc: Option<RegexId> = None;
        while let Some(c) = self.peek() {
            if c == '|' || c == ')' {
                break;
            }
            let next = self.repeat()?;
            acc = Some(match acc {
                None => next,
                Some(prev) => {
                    let span = self.span_of(prev).to(self.span_of(next));
                    self.push(RegexKind::Concat(prev, next), span)
                }
            });
        }
        Ok(match acc {
            Some(id) => id,
            None => self.push(RegexKind::Empty, Span::empty_at(self.pos)),
        })
    }

    /// repeat → atom ('*' | '+' | '?')*
    fn repeat(&mut self) -> PResult {
        let mut inner = self.atom()?;
        while let Some(c) = self.peek() {
            let make: fn(RegexId) -> RegexKind = match c {
                '*' => RegexKind::Star,
                '+' => RegexKind::Plus,
                '?' => RegexKind::Optional,
                _ => break,
            };
            self.bump();
            let span = self.span_of(inner).to(Span::new(self.pos - 1, self.pos));
            inner = self.push(make(inner), span);
        }
        Ok(inner)
    }

    fn atom(&mut self) -> PResult {
        let start = self.pos;
        let c = self.bump().expect("concat 保证这里还有字符");
        let set = match c {
            '(' => {
                let inner = self.alt()?;
                if self.peek() != Some(')') {
                    return Err(Diagnostic::error("括号没有闭合")
                        .with_code("E0401")
                        .with_primary(Span::empty_at(self.pos), "这里需要一个 `)`")
                        .with_secondary(Span::new(start, start + 1), "与这个左括号配对"));
                }
                self.bump();
                // 括号只负责分组，不产生节点；把节点的范围扩大到包含括号，方便高亮。
                self.nodes[inner.index()].span = Span::new(start, self.pos);
                return Ok(inner);
            }
            '*' | '+' | '?' => {
                return Err(Diagnostic::error(format!("`{c}` 前面没有可以重复的东西"))
                    .with_code("E0402")
                    .with_primary(Span::new(start, self.pos), "重复符号必须跟在一个字符或分组后面")
                    .with_help(format!("如果想匹配字符 `{c}` 本身，写成 `\\{c}`")));
            }
            '[' => self.class(start)?,
            '.' => CharSet::any_but_newline(),
            '\\' => self.escape(start, false)?,
            c => CharSet::single(c),
        };
        Ok(self.push(RegexKind::Set(set), Span::new(start, self.pos)))
    }

    /// 反斜杠后面的部分。`in_class` 表示是否在 `[...]` 里面。
    fn escape(&mut self, start: usize, in_class: bool) -> Result<CharSet, Diagnostic> {
        let Some(c) = self.bump() else {
            return Err(Diagnostic::error("反斜杠后面缺少字符")
                .with_code("E0405")
                .with_primary(Span::new(start, self.pos), "`\\` 用来转义后面的一个字符")
                .with_help("如果想匹配 `\\` 本身，写成 `\\\\`"));
        };
        Ok(match c {
            'd' => CharSet::digit(),
            'w' => CharSet::word(),
            's' => CharSet::space(),
            'n' => CharSet::single('\n'),
            't' => CharSet::single('\t'),
            'r' => CharSet::single('\r'),
            c if "()|*+?[].\\^-".contains(c) => CharSet::single(c),
            c => {
                let where_ = if in_class { "字符类里" } else { "" };
                return Err(Diagnostic::error(format!("{where_}不认识的转义 `\\{c}`"))
                    .with_code("E0406")
                    .with_primary(Span::new(start, self.pos), "")
                    .with_note(
                        "可用的转义：\\d 数字、\\w 单词字符、\\s 空白、\\n 换行、\\t 制表符，以及 \\( \\* 等符号本身",
                    ));
            }
        })
    }

    /// `[...]` 字符类。`start` 是 `[` 的位置。
    fn class(&mut self, start: usize) -> Result<CharSet, Diagnostic> {
        let negate = self.peek() == Some('^');
        if negate {
            self.bump();
        }
        let mut set = CharSet::empty();
        let mut first = true;
        loop {
            let item_start = self.pos;
            let Some(c) = self.bump() else {
                return Err(Diagnostic::error("字符类没有闭合")
                    .with_code("E0403")
                    .with_primary(Span::empty_at(self.pos), "这里需要一个 `]`")
                    .with_secondary(Span::new(start, start + 1), "字符类从这里开始"));
            };
            if c == ']' && !first {
                break;
            }
            first = false;
            let lo = match c {
                '\\' => {
                    let esc = self.escape(item_start, true)?;
                    match esc.ranges().next() {
                        Some((a, b)) if a == b && esc.ranges().count() == 1 => a,
                        _ => {
                            set = set.union(&esc);
                            continue;
                        }
                    }
                }
                c => c,
            };
            // 形如 a-z 的区间（`-` 在最后时表示字符本身）
            let rest = &self.src[self.pos..];
            if rest.starts_with('-') && !rest.starts_with("-]") && rest.len() > 1 {
                self.bump();
                let hi_start = self.pos;
                let hi = match self.bump() {
                    Some('\\') => {
                        let esc = self.escape(hi_start, true)?;
                        match esc.ranges().next() {
                            Some((a, b)) if a == b => a,
                            _ => {
                                return Err(Diagnostic::error("区间的终点必须是单个字符")
                                    .with_code("E0407")
                                    .with_primary(Span::new(item_start, self.pos), ""));
                            }
                        }
                    }
                    Some(h) => h,
                    None => unreachable!("rest.len() > 1 保证还有字符"),
                };
                if hi < lo {
                    return Err(Diagnostic::error(format!("区间 `{lo}-{hi}` 的起点比终点大"))
                        .with_code("E0407")
                        .with_primary(Span::new(item_start, self.pos), "字符按编码顺序排列，起点要在前")
                        .with_help(format!("应该写成 `{hi}-{lo}`")));
                }
                set = set.union(&CharSet::range(lo, hi));
            } else {
                set = set.union(&CharSet::single(lo));
            }
        }
        Ok(if negate { set.complement() } else { set })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 把语法树写成带括号的形式，方便断言结构。
    fn shape(src: &str) -> String {
        let r = Regex::parse(src).unwrap();
        fn go(r: &Regex, id: RegexId) -> String {
            match &r.node(id).kind {
                RegexKind::Empty => "ε".into(),
                RegexKind::Set(s) => s.label(),
                RegexKind::Concat(a, b) => format!("({}·{})", go(r, *a), go(r, *b)),
                RegexKind::Alt(a, b) => format!("({}|{})", go(r, *a), go(r, *b)),
                RegexKind::Star(a) => format!("{}*", go(r, *a)),
                RegexKind::Plus(a) => format!("{}+", go(r, *a)),
                RegexKind::Optional(a) => format!("{}?", go(r, *a)),
            }
        }
        go(&r, r.root)
    }

    #[test]
    fn precedence() {
        assert_eq!(shape("ab|c"), "((a·b)|c)");
        assert_eq!(shape("a|bc*"), "(a|(b·c*))");
        assert_eq!(shape("(a|b)*abb"), "((((a|b)*·a)·b)·b)");
        assert_eq!(shape("a|"), "(a|ε)");
        assert_eq!(shape("()"), "ε");
        assert_eq!(shape("a+?"), "a+?");
    }

    #[test]
    fn classes_and_escapes() {
        assert_eq!(shape("[a-c_]"), "[_a-c]");
        assert_eq!(shape("[^0-9]"), "[^0-9]");
        assert_eq!(shape("\\d+"), "[0-9]+");
        assert_eq!(shape("\\*"), "*");
        assert_eq!(shape("[-a]"), "[-a]");
        assert_eq!(shape("[a-]"), "[-a]");
        assert_eq!(shape("[]]"), "]");
        assert_eq!(shape("[\\]a]"), "[]a]");
    }

    #[test]
    fn spans() {
        let r = Regex::parse("x(ab)*").unwrap();
        let RegexKind::Concat(_, star) = r.node(r.root).kind else { panic!() };
        assert_eq!(r.node(star).span.text(&r.source), "(ab)*");
        let RegexKind::Star(group) = r.node(star).kind else { panic!() };
        assert_eq!(r.node(group).span.text(&r.source), "(ab)");
    }

    #[test]
    fn errors() {
        let code = |s: &str| Regex::parse(s).unwrap_err().code.unwrap();
        assert_eq!(code("(ab"), "E0401");
        assert_eq!(code("*a"), "E0402");
        assert_eq!(code("a|*"), "E0402");
        assert_eq!(code("[ab"), "E0403");
        assert_eq!(code("ab)"), "E0404");
        assert_eq!(code("a\\"), "E0405");
        assert_eq!(code("\\q"), "E0406");
        assert_eq!(code("[z-a]"), "E0407");
    }

    #[test]
    fn reference_matcher() {
        let r = Regex::parse("(a|b)*abb").unwrap();
        assert!(r.matches("abb") && r.matches("aababb"));
        assert!(!r.matches("ab") && !r.matches("abba"));
        let r = Regex::parse("[0-9]+(\\.[0-9]+)?").unwrap();
        assert!(r.matches("3") && r.matches("3.14"));
        assert!(!r.matches("3.") && !r.matches(".5"));
        assert!(Regex::parse("").unwrap().matches(""));
        assert!(Regex::parse("(a*)*").unwrap().matches("aaa"));
    }
}
