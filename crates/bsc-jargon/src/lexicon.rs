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
    /// 缩略语：压缩过的说法，背后有具体内容（放管服、三农）。展开，不删。
    Abbrev,
    /// 套话模板：带槽位的句式（坚持{X}不动摇）。不参与分词，加载时变成句式文法的规则。
    Pattern,
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
            "缩略" => Cat::Abbrev,
            "模板" => Cat::Pattern,
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
            Cat::Abbrev => "缩略",
            Cat::Pattern => "模板",
            Cat::Func(f) => f.name(),
        }
    }

    /// 是不是"黑话"（需要被优化掉的词）。
    pub fn is_jargon(self) -> bool {
        matches!(
            self,
            Cat::Modifier | Cat::Hollow | Cat::Filler | Cat::Vague | Cat::Fixed | Cat::FixedVerb | Cat::Abbrev
        )
    }

    /// 需要写映射的类别。
    fn needs_target(self) -> bool {
        matches!(
            self,
            Cat::Hollow | Cat::Filler | Cat::Vague | Cat::Fixed | Cat::FixedVerb | Cat::Abbrev | Cat::Pattern
        )
    }

    /// 句式文法里的终结符。
    pub fn terminal(self) -> &'static str {
        match self {
            Cat::Modifier => "修饰",
            Cat::Hollow | Cat::Filler | Cat::Verb | Cat::FixedVerb => "动",
            Cat::Vague | Cat::Fixed | Cat::Content | Cat::Abbrev | Cat::Pattern => "名",
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

/// 模板的一段：字面文字，或者槽位（0 = {X}，1 = {Y}）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Piece {
    Lit(String),
    Slot(usize),
}

/// 一条套话模板：坚持{X}不动摇 → 一直坚持{X}。
#[derive(Clone, Debug)]
pub struct Pattern {
    /// 模板词条。
    pub entry: usize,
    pub src: Vec<Piece>,
    pub dst: Vec<Piece>,
}

/// 解析模板文字。`src` 为真时是模板本身（字面部分只能是文字，至少一个槽位和一段字面文字，槽位不重复）；
/// 否则是映射（字面部分可以有标点）。
pub fn parse_pattern(text: &str, src: bool) -> Result<Vec<Piece>, String> {
    let mut out = Vec::new();
    let mut rest = text;
    while !rest.is_empty() {
        if let Some(r) = rest.strip_prefix('{') {
            let Some(end) = r.find('}') else { return Err("「{」没有配对的「}」".to_owned()) };
            let slot = match &r[..end] {
                "X" => 0,
                "Y" => 1,
                other => return Err(format!("槽位只能写成 {{X}} 或 {{Y}}，不能是 {{{other}}}")),
            };
            if matches!(out.last(), Some(Piece::Slot(_))) {
                return Err("两个槽位不能挨着，中间要有字".to_owned());
            }
            if src && out.contains(&Piece::Slot(slot)) {
                return Err("同一个槽位在模板里只能出现一次".to_owned());
            }
            out.push(Piece::Slot(slot));
            rest = &r[end + 1..];
        } else {
            let end = rest.find('{').unwrap_or(rest.len());
            let lit = &rest[..end];
            if lit.contains('}') {
                return Err("「}」前面没有「{」".to_owned());
            }
            if src && !lit.chars().all(char::is_alphanumeric) {
                return Err(format!("模板的字面部分「{lit}」只能是文字，不能有空格和标点"));
            }
            out.push(Piece::Lit(lit.to_owned()));
            rest = &rest[end..];
        }
    }
    if src && !(out.iter().any(|p| matches!(p, Piece::Slot(_))) && out.iter().any(|p| matches!(p, Piece::Lit(_)))) {
        return Err("模板至少要有一个槽位和一段字面文字".to_owned());
    }
    Ok(out)
}

/// 模板词条的检查：(错误码, 消息)。
fn check_pattern(word: &str, target: Option<&str>) -> Result<(), (&'static str, String)> {
    let src = parse_pattern(word, true).map_err(|m| ("E3014", m))?;
    let Some(target) = target else { return Ok(()) };
    let dst = parse_pattern(target, false).map_err(|m| ("E3014", format!("映射：{m}")))?;
    let slots = |ps: &[Piece]| {
        let mut v: Vec<usize> =
            ps.iter().filter_map(|p| if let Piece::Slot(s) = p { Some(*s) } else { None }).collect();
        v.sort_unstable();
        v
    };
    if slots(&src) != slots(&dst) {
        return Err(("E3015", format!("映射「{target}」里的槽位和模板对不上")));
    }
    Ok(())
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
    /// 套话模板（只收录检查通过、写了映射的）。
    pub patterns: Vec<Pattern>,
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
                            .with_note(
                                "类别只能是：修饰、空动、空转、虚名、固定、固定动、实动、内容、缩略、模板，或者功能词",
                            )
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
                let has_slot = word.contains(['{', '}']);
                if has_slot != (cat == Cat::Pattern) {
                    let msg = if has_slot {
                        format!("「{word}」写了槽位，但类别不是模板")
                    } else {
                        format!("模板「{word}」里没有槽位")
                    };
                    push(Diagnostic::error(msg).with_code("E3014").with_primary(span, "").with_help(
                        "模板写成 `坚持{X}不动摇 | 模板 | 一直坚持{X}`，{X}、{Y} 是槽位，里面是讲话的实际内容",
                    ));
                    continue;
                }
                if cat == Cat::Pattern
                    && let Err((code, msg)) = check_pattern(&word, target.as_deref())
                {
                    push(
                        Diagnostic::error(msg)
                            .with_code(code)
                            .with_primary(span, "")
                            .with_note(
                                "模板里的每个槽位在映射里都要正好出现一次：槽位里是讲话的实际内容，不能丢，也不能重复",
                            )
                            .with_help("这个词条没有收录"),
                    );
                    continue;
                }
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
        let patterns = build_patterns(&mut entries, &mut index, &mut diags);
        let ac = AhoCorasick::build(
            entries.iter().enumerate().filter(|(_, e)| e.cat != Cat::Pattern).map(|(i, e)| (i, e.word.as_str())),
        );
        let mut lex = Lexicon { layers, entries, index, chains: vec![], diags, ac, patterns };
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
            // 用编译器自己的分词器切一遍最后的说法（直接找子串会误报："提高质量"里有"高质量"）；模板只看字面部分
            let check =
                if e.cat == Cat::Pattern { last.replace("{X}", "，").replace("{Y}", "，") } else { last.clone() };
            let seg = crate::segment::segment(&check, Span::new(0, check.len()), String::new(), false, self);
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
            if tl > 8 && tl > wl * 3 && !matches!(e.cat, Cat::Abbrev | Cat::Pattern) {
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

/// 收集检查通过的模板。模板的字面部分要能被分词器切成一个词，所以没收录的字面部分登记成"内容"类的词条；
/// 单字的字面部分必须本来就是词典里的词（E3016），否则会把普通词切碎。
fn build_patterns(
    entries: &mut Vec<Entry>,
    index: &mut HashMap<String, usize>,
    diags: &mut Vec<LexDiag>,
) -> Vec<Pattern> {
    let mut patterns = Vec::new();
    let ids: Vec<usize> = (0..entries.len()).filter(|&i| entries[i].cat == Cat::Pattern).collect();
    for i in ids {
        let e = &entries[i];
        let (Ok(src), Some(Ok(dst))) =
            (parse_pattern(&e.word, true), e.target.as_deref().map(|t| parse_pattern(t, false)))
        else {
            continue;
        };
        let (word, layer, line) = (e.word.clone(), e.layer, e.line);
        let lits: Vec<String> =
            src.iter().filter_map(|p| if let Piece::Lit(l) = p { Some(l.clone()) } else { None }).collect();
        let single: Vec<&String> = lits.iter().filter(|l| !index.contains_key(*l) && l.chars().count() == 1).collect();
        if !single.is_empty() {
            let chars: Vec<&str> = single.iter().map(|l| l.as_str()).collect();
            diags.push(LexDiag {
                layer,
                diag: Diagnostic::error(format!("模板「{word}」里的单字「{}」不是词典里的词", chars.join("」「")))
                    .with_code("E3016")
                    .with_primary(line, "")
                    .with_note("模板的字面部分要能被分词器认出来；为了它把单个字收进词典，会把很多普通词切碎")
                    .with_help("把字面部分写成两个字以上的词，或者换一种写法；这个模板没有收录"),
            });
            continue;
        }
        for lit in lits {
            if !index.contains_key(&lit) {
                index.insert(lit.clone(), entries.len());
                entries.push(Entry {
                    word: lit,
                    cat: Cat::Content,
                    target: None,
                    note: Some(format!("模板「{word}」的字面部分")),
                    layer,
                    line,
                });
            }
        }
        patterns.push(Pattern { entry: i, src, dst });
    }
    patterns
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
