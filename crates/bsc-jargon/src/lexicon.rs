//! 词典：黑话编译器的"语言定义"，也是可以不断扩充的映射关系。
//!
//! 词典是纯文本数据，一行一个词条（`词 | 类别 | 映射 | 备注`），分层加载：
//! 第 0 层是手工维护的核心词典，后面是一批批 AI 生成、或者用户临时粘贴的扩充词条。
//!
//! 映射关系一多，就会出现各种问题：同一个词在不同批次里定义得不一样、上位词绕成一个圈、
//! 映射到的说法本身还是黑话、单字词条把普通词切碎……所以加载词典时像编译器检查源码一样检查它，
//! 每个问题都给出带位置的诊断，有问题的词条要么按规则处理，要么不收录。

use std::collections::HashMap;

use bsc_core::{Diagnostic, Span};

use crate::ac::AhoCorasick;

/// 功能词：句式分析靠它们认出结构。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Func {
    Yi,
    Wei,
    WeiLe,
    ZuoWei,
    WeiRao,
    TongGuo,
    Zai,
    Ba,
    Rang,
    De,
    BingLie,
    Subject,
    Modal,
    Link,
}

impl Func {
    pub const ALL: [Func; 14] = [
        Func::Yi,
        Func::Wei,
        Func::WeiLe,
        Func::ZuoWei,
        Func::WeiRao,
        Func::TongGuo,
        Func::Zai,
        Func::Ba,
        Func::Rang,
        Func::De,
        Func::BingLie,
        Func::Subject,
        Func::Modal,
        Func::Link,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Func::Yi => "以",
            Func::Wei => "为",
            Func::WeiLe => "为了",
            Func::ZuoWei => "作为",
            Func::WeiRao => "围绕",
            Func::TongGuo => "通过",
            Func::Zai => "在",
            Func::Ba => "把",
            Func::Rang => "让",
            Func::De => "的",
            Func::BingLie => "并列",
            Func::Subject => "主语",
            Func::Modal => "情态",
            Func::Link => "连接",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Cat {
    /// 膨胀修饰语。
    Modifier,
    /// 空洞动词。
    Hollow,
    /// 和虚指名词搭配时整句没有信息的动词。
    Filler,
    /// 虚指名词。
    Vague,
    /// 名词性固定搭配。
    Fixed,
    /// 动词性固定搭配。
    FixedVerb,
    /// 朴素动词。
    Verb,
    /// 普通词。
    Content,
    Func(Func),
}

impl Cat {
    pub fn parse(s: &str) -> Option<Cat> {
        Some(match s {
            "修饰" => Cat::Modifier,
            "空动" => Cat::Hollow,
            "空转" => Cat::Filler,
            "虚名" => Cat::Vague,
            "固定" => Cat::Fixed,
            "固定动" => Cat::FixedVerb,
            "实动" => Cat::Verb,
            "内容" => Cat::Content,
            _ => return Func::ALL.iter().find(|f| f.name() == s).map(|f| Cat::Func(*f)),
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Cat::Modifier => "修饰",
            Cat::Hollow => "空动",
            Cat::Filler => "空转",
            Cat::Vague => "虚名",
            Cat::Fixed => "固定",
            Cat::FixedVerb => "固定动",
            Cat::Verb => "实动",
            Cat::Content => "内容",
            Cat::Func(f) => f.name(),
        }
    }

    /// 是不是"黑话"（需要被优化掉的词）。
    pub fn is_jargon(self) -> bool {
        matches!(self, Cat::Modifier | Cat::Hollow | Cat::Filler | Cat::Vague | Cat::Fixed | Cat::FixedVerb)
    }

    /// 需要写映射的类别。
    fn needs_target(self) -> bool {
        matches!(self, Cat::Hollow | Cat::Filler | Cat::Vague | Cat::Fixed | Cat::FixedVerb)
    }

    /// 句式文法里的终结符。
    pub fn terminal(self) -> &'static str {
        match self {
            Cat::Modifier => "修饰",
            Cat::Hollow | Cat::Filler | Cat::Verb | Cat::FixedVerb => "动",
            Cat::Vague | Cat::Fixed | Cat::Content => "名",
            Cat::Func(f) => f.name(),
        }
    }
}

/// 一层词典（一个文件）。
#[derive(Clone, Debug)]
pub struct Layer {
    pub name: String,
    pub text: String,
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub word: String,
    pub cat: Cat,
    pub target: Option<String>,
    pub note: Option<String>,
    pub layer: usize,
    /// 词条那一行在层文本里的位置。
    pub line: Span,
}

/// 词典的一条诊断：属于哪一层。
#[derive(Clone, Debug)]
pub struct LexDiag {
    pub layer: usize,
    pub diag: Diagnostic,
}

#[derive(Clone, Debug)]
pub struct Lexicon {
    pub layers: Vec<Layer>,
    pub entries: Vec<Entry>,
    index: HashMap<String, usize>,
    /// 每个词条降级时走过的链：[词, 上位词, 上位词的上位词, …, 朴素说法]。
    chains: Vec<Vec<String>>,
    pub diags: Vec<LexDiag>,
    pub ac: AhoCorasick,
}

const MAX_CHAIN: usize = 5;

impl Lexicon {
    /// 按层加载。第 0 层是核心词典：只有它能定义功能词，冲突时它优先。
    pub fn build(layers: Vec<Layer>) -> Lexicon {
        let mut entries: Vec<Entry> = Vec::new();
        let mut index: HashMap<String, usize> = HashMap::new();
        let mut diags = Vec::new();
        for (li, layer) in layers.iter().enumerate() {
            let mut offset = 0;
            for raw in layer.text.split_inclusive('\n') {
                let line_start = offset;
                offset += raw.len();
                let line = raw.trim_end_matches(['\n', '\r']);
                let content = line.trim();
                if content.is_empty() || content.starts_with('#') {
                    continue;
                }
                let span =
                    Span::new(line_start + (line.len() - line.trim_start().len()), line_start + line.trim_end().len());
                let mut push = |d: Diagnostic| diags.push(LexDiag { layer: li, diag: d });
                let fields: Vec<&str> = content.split('|').map(str::trim).collect();
                if fields.len() < 2 || fields.len() > 4 || fields[0].is_empty() {
                    push(
                        Diagnostic::error("词条格式不对")
                            .with_code("E3001")
                            .with_primary(span, "")
                            .with_help("写成 `词 | 类别 | 映射 | 备注`，映射和备注可以省略，比如 `赋能 | 空动 | 助力`"),
                    );
                    continue;
                }
                let word = fields[0].to_owned();
                let Some(cat) = Cat::parse(fields[1]) else {
                    push(
                        Diagnostic::error(format!("不认识的类别「{}」", fields[1]))
                            .with_code("E3002")
                            .with_primary(span, "")
                            .with_note("类别只能是：修饰、空动、空转、虚名、固定、固定动、实动、内容，或者功能词")
                            .with_help("这个词条没有收录"),
                    );
                    continue;
                };
                if li > 0 && matches!(cat, Cat::Func(_)) {
                    push(
                        Diagnostic::error(format!("功能词「{word}」只能在核心词典里定义"))
                            .with_code("E3003")
                            .with_primary(span, "")
                            .with_note("功能词决定句式分析的结果，随便增加会让已有的句子分析出错")
                            .with_help("这个词条没有收录"),
                    );
                    continue;
                }
                if li > 0 && word.chars().count() == 1 {
                    push(
                        Diagnostic::error(format!("单字词条「{word}」没有收录"))
                            .with_code("E3010")
                            .with_primary(span, "")
                            .with_note("单个字会出现在大量普通词里，收录后容易把普通词切碎（比如把\"化\"当成修饰语，\"变化\"就被切坏了）")
                            .with_help("写成包含它的完整词语"),
                    );
                    continue;
                }
                let target = fields.get(2).filter(|t| !t.is_empty()).map(|t| (*t).to_owned());
                let note = fields.get(3).filter(|t| !t.is_empty()).map(|t| (*t).to_owned());
                if cat.needs_target() && target.is_none() {
                    push(
                        Diagnostic::warning(format!("「{word}」是{}，但没有写映射", cat.name()))
                            .with_code("W3005")
                            .with_primary(span, "")
                            .with_note("降级 / 折叠时不知道换成什么，只能原样保留")
                            .with_help("在第三栏写上它的上位词或大白话"),
                    );
                }
                if !cat.needs_target() && !matches!(cat, Cat::Func(Func::Link)) && target.is_some() {
                    push(
                        Diagnostic::note(format!("「{word}」是{}，映射会被忽略", cat.name()))
                            .with_code("N3006")
                            .with_primary(span, ""),
                    );
                }
                if target.as_deref() == Some(word.as_str()) {
                    push(
                        Diagnostic::warning(format!("「{word}」映射到了它自己"))
                            .with_code("W3013")
                            .with_primary(span, "")
                            .with_help("映射应该是更朴素的说法"),
                    );
                }
                let entry = Entry { word: word.clone(), cat, target, note, layer: li, line: span };
                match index.get(&word).copied() {
                    None => {
                        index.insert(word, entries.len());
                        entries.push(entry);
                    }
                    Some(old) => {
                        let o = &entries[old];
                        let same = o.cat == entry.cat && o.target == entry.target;
                        if same {
                            push(
                                Diagnostic::note(format!("「{word}」重复收录（和第 {} 层的定义相同）", o.layer))
                                    .with_code("N3004")
                                    .with_primary(span, ""),
                            );
                        } else if o.layer == 0 && li > 0 {
                            push(
                                Diagnostic::warning(format!("「{word}」和核心词典的定义冲突，保留核心词典的"))
                                    .with_code("W3004")
                                    .with_primary(span, format!("这里定义成 {}", describe(&entry)))
                                    .with_note(format!("核心词典里是 {}", describe(o))),
                            );
                        } else {
                            push(
                                Diagnostic::warning(format!("「{word}」被重新定义，采用新的定义"))
                                    .with_code("W3004")
                                    .with_primary(span, format!("新的：{}", describe(&entry)))
                                    .with_note(format!("原来是 {}（第 {} 层）", describe(o), o.layer)),
                            );
                            entries[old] = entry;
                        }
                    }
                }
            }
        }
        let ac = AhoCorasick::build(entries.iter().enumerate().map(|(i, e)| (i, e.word.as_str())));
        let mut lex = Lexicon { layers, entries, index, chains: vec![], diags, ac };
        lex.resolve_chains();
        lex
    }

    /// 计算每个词条的降级链，检查成环、过长、终点仍是黑话。
    fn resolve_chains(&mut self) {
        let n = self.entries.len();
        let mut chains: Vec<Vec<String>> = Vec::with_capacity(n);
        let mut in_cycle = vec![false; n];
        for i in 0..n {
            let mut chain = vec![self.entries[i].word.clone()];
            let mut path = vec![i];
            let mut cur = i;
            let mut cycle = None;
            while let Some(t) = self.entries[cur].target.clone() {
                if !self.entries[cur].cat.needs_target() && !matches!(self.entries[cur].cat, Cat::Func(Func::Link)) {
                    break;
                }
                chain.push(t.clone());
                match self.index.get(&t) {
                    Some(&next) if self.entries[next].cat.needs_target() => {
                        if let Some(pos) = path.iter().position(|&p| p == next) {
                            cycle = Some(path[pos..].to_vec());
                            break;
                        }
                        path.push(next);
                        cur = next;
                    }
                    _ => break,
                }
            }
            if let Some(cyc) = cycle {
                // 映射到自己的已经报过 W3013
                if cyc.len() > 1 && cyc.iter().all(|&c| !in_cycle[c]) {
                    let words: Vec<String> = cyc.iter().map(|&c| self.entries[c].word.clone()).collect();
                    let first = &self.entries[cyc[0]];
                    let mut d = Diagnostic::error(format!("上位词绕成了一个圈：{} → {}", words.join(" → "), words[0]))
                        .with_code("E3007")
                        .with_primary(first.line, "")
                        .with_note("降级时会一直绕下去，永远到不了朴素的说法；圈上的词都不再降级")
                        .with_help("把其中一个词的映射改成不在圈上的朴素说法");
                    for &c in &cyc[1..] {
                        if self.entries[c].layer == first.layer {
                            d = d.with_secondary(self.entries[c].line, "");
                        }
                    }
                    self.diags.push(LexDiag { layer: first.layer, diag: d });
                }
                for &c in &cyc {
                    in_cycle[c] = true;
                }
                chains.push(vec![self.entries[i].word.clone()]);
                continue;
            }
            if chain.len() > MAX_CHAIN + 1 {
                let e = &self.entries[i];
                self.diags.push(LexDiag {
                    layer: e.layer,
                    diag: Diagnostic::note(format!(
                        "「{}」的降级链有 {} 步：{}",
                        e.word,
                        chain.len() - 1,
                        chain.join(" → ")
                    ))
                    .with_code("N3009")
                    .with_primary(e.line, "")
                    .with_note("链太长往往说明中间某一步的映射不够朴素，可以直接映射到最后的说法"),
                });
            }
            chains.push(chain);
        }
        // 终点检查：最后的说法里不应该还有黑话
        for i in 0..n {
            let e = &self.entries[i];
            if !e.cat.needs_target() || in_cycle[i] || chains[i].len() < 2 {
                continue;
            }
            let last = chains[i].last().unwrap().clone();
            // 用编译器自己的分词器切一遍最后的说法（直接找子串会误报："提高质量"里有"高质量"）
            let seg = crate::segment::segment(&last, Span::new(0, last.len()), String::new(), false, self);
            let bad: Vec<String> = seg
                .tokens
                .iter()
                .filter(|t| t.cat(self).is_jargon() && t.text != last)
                .map(|t| t.text.clone())
                .collect();
            if !bad.is_empty() {
                let e = &self.entries[i];
                self.diags.push(LexDiag {
                    layer: e.layer,
                    diag: Diagnostic::warning(format!(
                        "「{}」最后变成「{last}」，里面还有黑话「{}」",
                        e.word,
                        bad.join("」「")
                    ))
                    .with_code("W3008")
                    .with_primary(e.line, "")
                    .with_note("翻译出来的大白话里会残留黑话")
                    .with_help("换一个完全朴素的说法"),
                });
            }
            let wl = e.word.chars().count();
            let tl = last.chars().count();
            if tl > 8 && tl > wl * 3 {
                self.diags.push(LexDiag {
                    layer: e.layer,
                    diag: Diagnostic::note(format!("「{}」→「{last}」变长了很多", e.word))
                        .with_code("N3011")
                        .with_primary(e.line, "")
                        .with_note("大白话应该更短；太长的解释放在备注里更合适"),
                });
            }
        }
        self.chains = chains;
    }

    pub fn get(&self, word: &str) -> Option<usize> {
        self.index.get(word).copied()
    }

    /// 降级链。
    pub fn chain(&self, entry: usize) -> &[String] {
        &self.chains[entry]
    }

    pub fn count_by_layer(&self, layer: usize) -> usize {
        self.entries.iter().filter(|e| e.layer == layer).count()
    }

    pub fn errors(&self) -> usize {
        self.diags.iter().filter(|d| d.diag.severity == bsc_core::Severity::Error).count()
    }
}

fn describe(e: &Entry) -> String {
    match &e.target {
        Some(t) => format!("{}（→ {t}）", e.cat.name()),
        None => e.cat.name().to_owned(),
    }
}

/// 随程序打包的词典层。
pub fn builtin_layers() -> Vec<Layer> {
    vec![
        Layer { name: "core.txt（核心词典）".to_owned(), text: include_str!("../lexicon/core.txt").to_owned() },
        Layer {
            name: "ai-001.txt（AI 扩充第 1 批）".to_owned(),
            text: include_str!("../lexicon/ai-001.txt").to_owned(),
        },
    ]
}
