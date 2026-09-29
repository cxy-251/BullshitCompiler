//! 文法的表示与文本解析。
//!
//! 文法写法：
//!
//! ```text
//! # 注释
//! E -> E + T | T
//! T -> T * F
//!    | F                 以 | 开头的行接着上一条规则
//! F -> ( E ) | num
//! A -> a A | ε           ε 表示空串（也可以直接留空：A -> a A |）
//! ```
//!
//! 出现在箭头左边的是**非终结符**，其余都是**终结符**。第一条规则左边的是开始符号。
//! 用引号括起来的 `'|'` `'->'` 总是终结符（用来表示和写法冲突的符号本身）。

use bsc_core::{Diagnostic, Span};

/// 文法符号的编号。
pub type Sym = usize;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Production {
    pub lhs: Sym,
    /// 右部；空表示 ε。
    pub rhs: Vec<Sym>,
    /// 这个候选式在文法源码中的位置（界面高亮用）。
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct Grammar {
    names: Vec<String>,
    terminal: Vec<bool>,
    pub productions: Vec<Production>,
    pub start: Sym,
    /// 输入结束标记 `$`（终结符）。
    pub eof: Sym,
}

impl Grammar {
    pub fn parse(src: &str) -> Result<Grammar, Diagnostic> {
        // 第一遍：按行切出 (左部, 左部位置, [候选式文本与位置])
        struct Rule<'s> {
            lhs: &'s str,
            alts: Vec<(&'s str, usize)>,
        }
        let mut rules: Vec<Rule> = Vec::new();
        let mut offset = 0;
        for line in src.split_inclusive('\n') {
            let line_start = offset;
            offset += line.len();
            let content = line.trim_end_matches(['\n', '\r']);
            let trimmed = content.trim_start();
            if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("//") {
                continue;
            }
            let indent = content.len() - trimmed.len();
            let (lhs, body, body_at) = if let Some(rest) = trimmed.strip_prefix('|') {
                let Some(prev) = rules.last() else {
                    return Err(Diagnostic::error("以 `|` 开头的行前面没有规则")
                        .with_code("E0504")
                        .with_primary(Span::new(line_start + indent, line_start + indent + 1), "")
                        .with_help("`|` 开头的行用来续写上一条规则的候选式"));
                };
                (prev.lhs, rest, line_start + indent + 1)
            } else {
                let arrow = ["->", "→", "::="].iter().filter_map(|a| trimmed.find(a).map(|i| (i, a.len()))).min();
                let Some((i, alen)) = arrow else {
                    return Err(Diagnostic::error("这一行没有箭头")
                        .with_code("E0501")
                        .with_primary(Span::new(line_start + indent, line_start + content.len()), "")
                        .with_help("规则的写法是 `左部 -> 右部`，比如 `E -> E + T`"));
                };
                let lhs = trimmed[..i].trim();
                if lhs.is_empty() || lhs.split_whitespace().count() != 1 {
                    let at = line_start + indent;
                    return Err(Diagnostic::error("箭头左边必须恰好是一个符号")
                        .with_code("E0502")
                        .with_primary(Span::new(at, at + i.max(1)), "这里应该是被定义的那个非终结符")
                        .with_help("上下文无关文法的每条规则，左边只有一个非终结符"));
                }
                (lhs, &trimmed[i + alen..], line_start + indent + i + alen)
            };
            // 按 | 切候选式（引号里的 | 不算）。引号只在单词开头才算"开引号"、在单词末尾才算"关引号"，
            // 这样 E' 这种带撇号的名字不会被误当成引号。
            let mut alts = Vec::new();
            let mut start = 0;
            let mut in_quote: Option<char> = None;
            let chars: Vec<(usize, char)> = body.char_indices().collect();
            for (idx, &(j, c)) in chars.iter().enumerate() {
                let prev_space = idx == 0 || chars[idx - 1].1.is_whitespace();
                let next_space = chars.get(idx + 1).is_none_or(|&(_, n)| n.is_whitespace());
                match (in_quote, c) {
                    (None, '\'' | '"') if prev_space => in_quote = Some(c),
                    (Some(q), c) if c == q && next_space => in_quote = None,
                    (None, '|') => {
                        alts.push((&body[start..j], body_at + start));
                        start = j + 1;
                    }
                    _ => {}
                }
            }
            alts.push((&body[start..], body_at + start));
            match rules.iter_mut().find(|r| r.lhs == lhs) {
                Some(r) if trimmed.starts_with('|') => r.alts.extend(alts),
                Some(r) => r.alts.extend(alts),
                None => rules.push(Rule { lhs, alts }),
            }
        }
        if rules.is_empty() {
            return Err(Diagnostic::error("文法是空的")
                .with_code("E0503")
                .with_help("至少写一条规则，比如 `S -> a S b | ε`"));
        }

        // 第二遍：编号。先是非终结符（按出现顺序），然后是终结符，最后是 $。
        let mut names: Vec<String> = rules.iter().map(|r| r.lhs.to_owned()).collect();
        let mut terminal = vec![false; names.len()];
        let mut productions = Vec::new();
        for (ri, r) in rules.iter().enumerate() {
            for &(alt, at) in &r.alts {
                let mut rhs = Vec::new();
                for word in alt.split_whitespace() {
                    if word == "ε" {
                        continue;
                    }
                    let quoted = word.len() >= 2
                        && ((word.starts_with('\'') && word.ends_with('\''))
                            || (word.starts_with('"') && word.ends_with('"')));
                    let name = if quoted { &word[1..word.len() - 1] } else { word };
                    let sym = match names.iter().position(|n| n == name) {
                        Some(i) if !quoted || terminal[i] => i,
                        _ => {
                            names.push(name.to_owned());
                            terminal.push(true);
                            names.len() - 1
                        }
                    };
                    rhs.push(sym);
                }
                let trimmed_start = alt.len() - alt.trim_start().len();
                let span = Span::new(at + trimmed_start, at + alt.trim_end().len().max(trimmed_start));
                productions.push(Production { lhs: ri, rhs, span });
            }
        }
        names.push("$".to_owned());
        terminal.push(true);
        let eof = names.len() - 1;
        Ok(Grammar { names, terminal, productions, start: 0, eof })
    }

    pub fn name(&self, s: Sym) -> &str {
        &self.names[s]
    }

    pub fn is_terminal(&self, s: Sym) -> bool {
        self.terminal[s]
    }

    pub fn num_symbols(&self) -> usize {
        self.names.len()
    }

    pub fn symbol(&self, name: &str) -> Option<Sym> {
        self.names.iter().position(|n| n == name)
    }

    /// 非终结符（按定义顺序）。
    pub fn nonterminals(&self) -> impl Iterator<Item = Sym> + '_ {
        (0..self.names.len()).filter(|&s| !self.terminal[s])
    }

    /// 终结符（不含 `$`）。
    pub fn terminals(&self) -> impl Iterator<Item = Sym> + '_ {
        (0..self.names.len()).filter(|&s| self.terminal[s] && s != self.eof)
    }

    /// 某个非终结符的所有产生式：(产生式编号, 产生式)。
    pub fn prods_of(&self, nt: Sym) -> impl Iterator<Item = (usize, &Production)> + '_ {
        self.productions.iter().enumerate().filter(move |(_, p)| p.lhs == nt)
    }

    /// 符号串的写法，空串写成 ε。
    pub fn seq_text(&self, syms: &[Sym]) -> String {
        if syms.is_empty() { "ε".to_owned() } else { syms.iter().map(|&s| self.name(s)).collect::<Vec<_>>().join(" ") }
    }

    /// 产生式的写法：`E → E + T`。
    pub fn prod_text(&self, p: usize) -> String {
        let prod = &self.productions[p];
        format!("{} → {}", self.name(prod.lhs), self.seq_text(&prod.rhs))
    }

    /// 增广文法：加一个新的开始符号 S' 和产生式 S' → S（放在最后，原有产生式编号不变）。
    /// LR 分析需要它，这样"归约出 S' 并且输入结束"就是唯一的接受时机。
    pub fn augmented(&self) -> Grammar {
        let mut g = self.clone();
        let mut name = format!("{}'", self.name(self.start));
        while g.names.contains(&name) {
            name.push('\'');
        }
        // 新符号插在 $ 前面会打乱编号，所以放在最后；$ 仍是 eof。
        g.names.push(name);
        g.terminal.push(false);
        let s = g.names.len() - 1;
        g.productions.push(Production { lhs: s, rhs: vec![self.start], span: Span::default() });
        g.start = s;
        g
    }

    /// 把输入句子切成终结符序列：
    /// - 和某个终结符同名的单词或符号就是那个终结符（多字符符号按最长匹配）；
    /// - 数字切成 `num`、其他单词切成 `id`（如果文法里有这两个终结符的话）。
    pub fn tokenize(&self, input: &str) -> Result<Vec<(Sym, Span)>, Diagnostic> {
        let mut out = Vec::new();
        let mut pos = 0;
        let terms: Vec<Sym> = self.terminals().collect();
        while pos < input.len() {
            let rest = &input[pos..];
            let c = rest.chars().next().unwrap();
            if c.is_whitespace() {
                pos += c.len_utf8();
                continue;
            }
            let word_len = if c.is_alphanumeric() || c == '_' {
                rest.char_indices().find(|&(_, ch)| !(ch.is_alphanumeric() || ch == '_')).map_or(rest.len(), |(i, _)| i)
            } else {
                0
            };
            if word_len > 0 {
                let word = &rest[..word_len];
                let span = Span::new(pos, pos + word_len);
                let sym = terms.iter().copied().find(|&t| self.name(t) == word).or_else(|| {
                    let class = if word.chars().all(|ch| ch.is_ascii_digit()) { "num" } else { "id" };
                    self.symbol(class).filter(|&s| self.is_terminal(s))
                });
                match sym {
                    Some(s) => out.push((s, span)),
                    None => {
                        return Err(Diagnostic::error(format!("`{word}` 不是这个文法的终结符"))
                            .with_code("E0505")
                            .with_primary(span, "")
                            .with_note(format!("终结符有：{}", self.terminal_list())));
                    }
                }
                pos += word_len;
                continue;
            }
            // 符号：找最长的同名终结符
            let best = terms
                .iter()
                .copied()
                .filter(|&t| {
                    let n = self.name(t);
                    !n.is_empty() && rest.starts_with(n) && !n.starts_with(|ch: char| ch.is_alphanumeric() || ch == '_')
                })
                .max_by_key(|&t| self.name(t).len());
            match best {
                Some(t) => {
                    let len = self.name(t).len();
                    out.push((t, Span::new(pos, pos + len)));
                    pos += len;
                }
                None => {
                    return Err(Diagnostic::error(format!("`{c}` 不是这个文法的终结符"))
                        .with_code("E0505")
                        .with_primary(Span::new(pos, pos + c.len_utf8()), "")
                        .with_note(format!("终结符有：{}", self.terminal_list())));
                }
            }
        }
        Ok(out)
    }

    fn terminal_list(&self) -> String {
        self.terminals().map(|t| self.name(t).to_owned()).collect::<Vec<_>>().join("  ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_tokenize() {
        let g = Grammar::parse("E -> E + T | T\nT -> T * F\n   | F\nF -> ( E ) | num\nA -> a A | ε |").unwrap();
        assert_eq!(g.productions.len(), 9); // A 有三个候选式：a A、ε、以及最后那个空的
        assert_eq!(g.prod_text(0), "E → E + T");
        assert_eq!(g.prod_text(7), "A → ε");
        assert_eq!(g.prod_text(8), "A → ε"); // 显式的 ε 和空候选式一样
        assert!(!g.is_terminal(g.symbol("F").unwrap()));
        assert!(g.is_terminal(g.symbol("num").unwrap()));
        let toks = g.tokenize("(12 + 3)*4").unwrap();
        let names: Vec<&str> = toks.iter().map(|&(s, _)| g.name(s)).collect();
        assert_eq!(names, ["(", "num", "+", "num", ")", "*", "num"]);
        assert_eq!(g.tokenize("1 - 2").unwrap_err().code, Some("E0505"));
        assert_eq!(Grammar::parse("E E + T").unwrap_err().code, Some("E0501"));
        assert_eq!(Grammar::parse("| a").unwrap_err().code, Some("E0504"));
    }
}
