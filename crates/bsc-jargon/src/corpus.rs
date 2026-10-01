//! 语料分析：从真实文本里找出编译器认不出的高频词，以及人写的参考译文集。
//!
//! 词典扩充的可靠做法是：先从真实文本里找出编译器"认不出的词"，把这些词连同原文例句交给便宜的 AI 填词条，
//! 而不是让 AI 凭空编词。认不出的词有两种来源：
//!
//! - **未收录片段**：分词后连在一起的未知字。公文里常常是好几个四字格连成一串（"攻坚克难踔厉奋发"），
//!   先统计整篇语料里每个两字、四字窗口出现的次数，再给每个长片段挑一种切法：尽量切成出现过两次以上的四字、两字词，
//!   一样好的切法里选从片段开头对齐的那种（四字格总是一个接一个排的）。只出现一次的长片段切不出东西，不算噪声。
//! - **拼合的四字格**：一半是词典词、一半是未知字（"统筹谋划"里的"统筹"已经收录），分词时被拆开了，
//!   单看"谋划"翻不好。相邻几个词正好凑成四个汉字、里面有未知字、出现两次以上的，整体列出来。

use std::collections::HashMap;

use bsc_core::Span;

use crate::lexicon::{Cat, Lexicon};
use crate::segment;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// 未收录片段（或从里面切出来的词）。
    Unknown,
    /// 词典词和未知字拼成的四字格。
    Phrase,
}

/// 待办词表里的一个候选词。
#[derive(Clone, Debug)]
pub struct Candidate {
    pub text: String,
    pub kind: Kind,
    /// 在语料里出现的次数。
    pub count: usize,
    /// 第一次出现的分句（太长就截取候选词前后各一段，两头加"……"）。
    pub example: String,
    /// 第一次出现的位置（字节），界面上用来高亮。
    pub span: Span,
}

/// 例句里候选词前后各保留的字数。
const CONTEXT: usize = 18;
/// 切分时的得分：四字词、两字词（只算出现过两次以上的）。
const GAIN4: u32 = 8;
const GAIN2: u32 = 3;

fn is_cjk(c: char) -> bool {
    ('\u{4e00}'..='\u{9fff}').contains(&c)
}

/// 分析一段语料，返回按出现次数排好序的候选词。
pub fn analyze(lex: &Lexicon, input: &str) -> Vec<Candidate> {
    let clauses = segment::split(input, lex);

    // 未收录的汉字片段：(分句, 每个字的字节位置和字)
    let mut frags: Vec<(usize, Vec<(usize, char)>)> = Vec::new();
    for (ci, cl) in clauses.iter().enumerate() {
        for t in cl.tokens.iter().filter(|t| t.entry.is_none()) {
            let mut run = Vec::new();
            for (i, c) in t.text.char_indices() {
                if is_cjk(c) {
                    run.push((t.span.start + i, c));
                } else if !run.is_empty() {
                    frags.push((ci, std::mem::take(&mut run)));
                }
            }
            if !run.is_empty() {
                frags.push((ci, run));
            }
        }
    }

    // 第一遍：每个两字、四字窗口在整篇语料里出现几次
    let mut grams: HashMap<String, usize> = HashMap::new();
    for (_, run) in &frags {
        for n in [2, 4] {
            for w in run.windows(n) {
                *grams.entry(w.iter().map(|&(_, c)| c).collect()).or_default() += 1;
            }
        }
    }
    let seen = |w: &[(usize, char)]| grams.get(&w.iter().map(|&(_, c)| c).collect::<String>()).copied().unwrap_or(0);

    // 第二遍：切出候选词
    let mut pieces: Vec<(usize, Span, Kind)> = Vec::new();
    let span_of = |w: &[(usize, char)]| {
        let (last, c) = w[w.len() - 1];
        Span::new(w[0].0, last + c.len_utf8())
    };
    for (ci, run) in &frags {
        let n = run.len();
        if n < 2 {
            continue;
        }
        if n <= 4 {
            pieces.push((*ci, span_of(run), Kind::Unknown));
            continue;
        }
        // 从后往前做动态规划：best[i] = 从第 i 个字切到结尾的最高得分和这一步切多长
        let mut best = vec![(0u32, 1usize); n + 1];
        for i in (0..n).rev() {
            best[i] = (best[i + 1].0, 1);
            for (len, gain) in [(2, GAIN2), (4, GAIN4)] {
                if i + len <= n && seen(&run[i..i + len]) >= 2 {
                    let g = gain + best[i + len].0;
                    // 同分时取长的（循环里后试四字，所以用 >=）
                    if g >= best[i].0 {
                        best[i] = (g, len);
                    }
                }
            }
        }
        let mut i = 0;
        while i < n {
            let len = best[i].1;
            if len > 1 {
                pieces.push((*ci, span_of(&run[i..i + len]), Kind::Unknown));
            }
            i += len;
        }
    }

    // 拼合的四字格：相邻几个词正好四个汉字，里面有未知字，没有功能词
    for (ci, cl) in clauses.iter().enumerate() {
        let toks = &cl.tokens;
        for k in 0..toks.len() {
            let (mut chars, mut unknown) = (0, false);
            for (j, t) in toks.iter().enumerate().skip(k) {
                if !t.text.chars().all(is_cjk) || matches!(t.cat(lex), Cat::Func(_)) {
                    break;
                }
                chars += t.text.chars().count();
                unknown |= t.entry.is_none();
                if chars >= 4 {
                    if chars == 4 && j > k && unknown {
                        pieces.push((ci, Span::new(toks[k].span.start, t.span.end), Kind::Phrase));
                    }
                    break;
                }
            }
        }
    }

    // 汇总：按第一次出现的位置排，保证结果确定
    pieces.sort_by_key(|&(_, s, _)| s.start);
    let mut out: Vec<Candidate> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    for (ci, span, kind) in pieces {
        let text = &input[span.start..span.end];
        if let Some(&i) = index.get(text) {
            out[i].count += 1;
            continue;
        }
        index.insert(text.to_owned(), out.len());
        let example = example(input, clauses[ci].span, span);
        out.push(Candidate { text: text.to_owned(), kind, count: 1, example, span });
    }
    out.retain(|c| lex.get(&c.text).is_none() && (c.kind == Kind::Unknown || c.count >= 2));
    out.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then(b.text.chars().count().cmp(&a.text.chars().count()))
            .then(a.span.start.cmp(&b.span.start))
    });
    out
}

/// 例句：候选词所在的分句，前后各留 `CONTEXT` 个字。
fn example(input: &str, clause: Span, word: Span) -> String {
    let before: Vec<char> = input[clause.start..word.start].chars().collect();
    let after: Vec<char> = input[word.end..clause.end].chars().collect();
    let mut s = String::new();
    if before.len() > CONTEXT {
        s.push_str("……");
    }
    s.extend(&before[before.len().saturating_sub(CONTEXT)..]);
    s.push_str(&input[word.start..word.end]);
    s.extend(after.iter().take(CONTEXT));
    if after.len() > CONTEXT {
        s.push_str("……");
    }
    s
}

/// 参考译文集里的一条：原文和人写的大白话译文。
#[derive(Clone, Debug)]
pub struct Reference {
    pub name: String,
    pub source: String,
    pub reference: String,
}

pub const BUILTIN_REFERENCES: &str = include_str!("../reference/reference.txt");

/// 解析参考译文集。格式见 reference/reference.txt 开头的注释。
pub fn parse_references(text: &str) -> Vec<Reference> {
    let mut out: Vec<Reference> = Vec::new();
    // 当前在写哪一段：false = 原文，true = 参考
    let mut in_ref = false;
    for line in text.lines() {
        let line = line.trim();
        if let Some(name) = line.strip_prefix("## ") {
            out.push(Reference { name: name.trim().to_owned(), source: String::new(), reference: String::new() });
            in_ref = false;
            continue;
        }
        if line.starts_with('#') {
            continue;
        }
        let Some(r) = out.last_mut() else { continue };
        let rest = if let Some(rest) = line.strip_prefix("原文：") {
            in_ref = false;
            rest
        } else if let Some(rest) = line.strip_prefix("参考：") {
            in_ref = true;
            rest
        } else {
            line
        };
        let field = if in_ref { &mut r.reference } else { &mut r.source };
        if !field.is_empty() && !rest.is_empty() {
            field.push('\n');
        }
        field.push_str(rest.trim());
    }
    out.retain(|r| !r.source.is_empty());
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Compiler;

    #[test]
    fn corpus_candidates() {
        let c = Compiler::builtin();
        let lex = &c.lex;
        for w in ["攻坚克难", "踔厉奋发", "砥砺奋进", "谋划", "统筹谋划", "克难踔厉"] {
            assert!(lex.get(w).is_none(), "「{w}」不该在词典里");
        }
        assert!(lex.get("统筹").is_some());
        let corpus = "要攻坚克难踔厉奋发，砥砺奋进。各单位要攻坚克难踔厉奋发。我们统筹谋划，大家统筹谋划。昨天下雨了。";
        let r = analyze(lex, corpus);
        let count = |w: &str| r.iter().find(|c| c.text == w).map(|c| c.count);

        // 长片段按四字对齐切开，跨词的切片（克难踔厉）不出现
        assert_eq!(count("攻坚克难"), Some(2));
        assert_eq!(count("踔厉奋发"), Some(2));
        assert_eq!(count("克难踔厉"), None);
        // 直接出现的短片段即使只有一次也列出
        assert_eq!(count("砥砺奋进"), Some(1));
        // 词典词 + 未知字拼成的四字格
        assert_eq!(r.iter().find(|c| c.text == "统筹谋划").map(|c| (c.kind, c.count)), Some((Kind::Phrase, 2)));
        // 只出现一次的长片段切不出东西
        assert!(r.iter().all(|c| !c.text.contains("下雨")), "{r:?}");
        // 次数多的排前面
        assert!(r[0].count == 2 && r.last().unwrap().count == 1);
        for c in &r {
            assert!(lex.get(&c.text).is_none());
            assert_eq!(&corpus[c.span.start..c.span.end], c.text);
            assert!(c.example.contains(&c.text));
        }
        assert!(parse_references(BUILTIN_REFERENCES).iter().all(|r| !r.reference.is_empty()));
    }
}
