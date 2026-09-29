//! 分句与分词。
//!
//! 中文没有空格，"以数字化转型为抓手"该怎么切？先用 Aho-Corasick 找出词典里所有能匹配上的词，
//! 连同"单个未知字"一起画成一张**词图**：结点是字与字之间的位置，每条边是一个候选词。
//! 从头走到尾的每条路径都是一种切法，挑**代价最小**的那条（动态规划，和求最短路一样）：
//! 词典里的词代价小，未知的字代价大，所以会尽量用词典里的词、尽量用长词。
//! 最后把连在一起的未知字合并成一个"普通词"（它们是讲话的实际内容，编译器不改它们）。

use bsc_core::Span;

use crate::lexicon::{Cat, Lexicon};

/// 分句的标点。句号类的结束一整句，逗号类的只结束分句。
const SENTENCE_END: &[char] = &['。', '！', '？', '!', '?', '.', '\n'];
const CLAUSE_END: &[char] = &['，', '；', '：', ',', ';', ':'];

/// 词图的一条边：第 `start`..`end` 个字，是词条 `entry`（`None` 表示一个未知的字）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Edge {
    pub start: usize,
    pub end: usize,
    pub entry: Option<usize>,
    pub cost: u32,
}

#[derive(Clone, Debug)]
pub struct Token {
    pub text: String,
    /// 在整段输入里的字节位置。
    pub span: Span,
    /// 词典里的词条；`None` 表示未收录的普通词。
    pub entry: Option<usize>,
}

impl Token {
    pub fn cat(&self, lex: &Lexicon) -> Cat {
        self.entry.map_or(Cat::Content, |e| lex.entries[e].cat)
    }
}

#[derive(Clone, Debug)]
pub struct Clause {
    /// 分句在输入里的字节位置（不含标点）。
    pub span: Span,
    pub text: String,
    /// 后面跟的标点（没有就是空串）。
    pub punct: String,
    /// 后面跟的标点结束了一整句。
    pub ends_sentence: bool,
    pub edges: Vec<Edge>,
    /// 最优路径上的边（下标）。
    pub best: Vec<usize>,
    pub tokens: Vec<Token>,
}

const DICT_COST: u32 = 10;
const UNKNOWN_COST: u32 = 12;

/// 把输入切成分句，每个分句分好词。
pub fn split(input: &str, lex: &Lexicon) -> Vec<Clause> {
    let mut out = Vec::new();
    let mut start = 0;
    let chars: Vec<(usize, char)> = input.char_indices().collect();
    for (k, &(i, c)) in chars.iter().enumerate() {
        let end_sentence = SENTENCE_END.contains(&c);
        if end_sentence || CLAUSE_END.contains(&c) {
            push_clause(&mut out, input, (start, i), c.to_string(), end_sentence, lex);
            start = i + c.len_utf8();
        }
        if k + 1 == chars.len() && start < input.len() {
            push_clause(&mut out, input, (start, input.len()), String::new(), true, lex);
        }
    }
    out
}

fn push_clause(
    out: &mut Vec<Clause>,
    input: &str,
    (start, end): (usize, usize),
    punct: String,
    ends_sentence: bool,
    lex: &Lexicon,
) {
    let raw = &input[start..end];
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        if ends_sentence && let Some(last) = out.last_mut() {
            last.ends_sentence = true;
            if last.punct.is_empty() || !SENTENCE_END.iter().any(|c| last.punct.contains(*c)) {
                last.punct = punct;
            }
        }
        return;
    }
    let s = start + (raw.len() - raw.trim_start().len());
    out.push(segment(input, Span::new(s, s + trimmed.len()), punct, ends_sentence, lex));
}

/// 给一段文字分词（不分句）。
pub fn segment(input: &str, span: Span, punct: String, ends_sentence: bool, lex: &Lexicon) -> Clause {
    let text = span.text(input);
    let chars: Vec<char> = text.chars().collect();
    let offsets: Vec<usize> = text.char_indices().map(|(i, _)| span.start + i).chain([span.end]).collect();
    let n = chars.len();

    let mut edges: Vec<Edge> = lex
        .ac
        .find_all(&chars)
        .into_iter()
        .map(|m| Edge { start: m.start, end: m.end, entry: Some(m.entry), cost: DICT_COST })
        .collect();
    // 每个字都可以是一个未知字；空白字符是代价为 0 的"跳过"边
    for (i, c) in chars.iter().enumerate() {
        let cost = if c.is_whitespace() { 0 } else { UNKNOWN_COST };
        edges.push(Edge { start: i, end: i + 1, entry: None, cost });
    }

    // 动态规划：best[j] = 切到第 j 个字为止的最小代价
    let mut best = vec![u32::MAX; n + 1];
    let mut back: Vec<Option<usize>> = vec![None; n + 1];
    best[0] = 0;
    let mut by_start: Vec<Vec<usize>> = vec![Vec::new(); n + 1];
    for (k, e) in edges.iter().enumerate() {
        by_start[e.start].push(k);
    }
    for i in 0..n {
        if best[i] == u32::MAX {
            continue;
        }
        for &k in &by_start[i] {
            let e = edges[k];
            let c = best[i] + e.cost;
            if c < best[e.end] {
                best[e.end] = c;
                back[e.end] = Some(k);
            }
        }
    }
    let mut path = Vec::new();
    let mut j = n;
    while j > 0 {
        let k = back[j].expect("每个位置都有未知字的边，一定能走通");
        path.push(k);
        j = edges[k].start;
    }
    path.reverse();

    // 连续的未知字合并成一个普通词；空白丢掉
    let mut tokens: Vec<Token> = Vec::new();
    let mut run: Option<(usize, usize)> = None;
    let flush = |run: &mut Option<(usize, usize)>, tokens: &mut Vec<Token>| {
        if let Some((a, b)) = run.take() {
            let sp = Span::new(offsets[a], offsets[b]);
            tokens.push(Token { text: sp.text(input).to_owned(), span: sp, entry: None });
        }
    };
    for &k in &path {
        let e = edges[k];
        match e.entry {
            Some(id) => {
                flush(&mut run, &mut tokens);
                let sp = Span::new(offsets[e.start], offsets[e.end]);
                tokens.push(Token { text: sp.text(input).to_owned(), span: sp, entry: Some(id) });
            }
            None if chars[e.start].is_whitespace() => flush(&mut run, &mut tokens),
            None => {
                run = Some(run.map_or((e.start, e.end), |(a, _)| (a, e.end)));
            }
        }
    }
    flush(&mut run, &mut tokens);
    Clause { span, text: text.to_owned(), punct, ends_sentence, edges, best: path, tokens }
}
