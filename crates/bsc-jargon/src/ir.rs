//! 中间表示：语义角色。
//!
//! 语法树记录的是"句子怎么写"，优化需要的是"句子说了什么"：谁（主语）、做什么（动词）、对什么（宾语）、
//! 靠什么（手段 / 途径）、为了什么（目的）。所以把语法树翻译成一组**语义块**，每块有一个角色和几个槽位；
//! 每个词记着自己的当前说法和是否被删掉。优化 Pass 只改这些标记，生成时再按角色拼回中文。

use bsc_core::Span;
use bsc_grammar::Tree;
use bsc_grammar::grammar::Grammar;

use crate::lexicon::{Cat, Func, Lexicon};
use crate::segment::Token;

#[derive(Clone, Debug)]
pub struct Word {
    pub text: String,
    pub span: Span,
    pub entry: Option<usize>,
    pub cat: Cat,
    /// 被 Pass 改写后的说法；`None` 表示原样。
    pub now: Option<String>,
    pub dead: bool,
}

impl Word {
    pub fn current(&self) -> &str {
        self.now.as_deref().unwrap_or(&self.text)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// 以 X 为 Y
    Means,
    /// 围绕 X
    Focus,
    /// 通过 X / 在 X
    Via,
    /// 为了 X
    Purpose,
    /// 动词 + 宾语（以及把 / 让 的变体）
    Action,
    /// 进而 / 从而 ……
    Link,
}

impl Role {
    pub fn name(self) -> &'static str {
        match self {
            Role::Means => "手段",
            Role::Focus => "焦点",
            Role::Via => "途径",
            Role::Purpose => "目的",
            Role::Action => "动作",
            Role::Link => "连接",
        }
    }
}

/// 句式改写：生成时不再按原词序拼接，而是换一个朴素的句式。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Template {
    /// 以 X 为抓手 → 从 X 入手
    StartFrom,
    /// 以 X 为引擎 → 靠 X 推动
    DrivenBy,
    /// 以 X 为 Y → 把 X 当作 Y
    TreatAs,
    /// 形成 X 闭环 → X 有完整流程（参数：obj2 从这里起是虚指名词）
    Has(usize),
}

#[derive(Clone, Debug)]
pub struct Part {
    pub role: Role,
    /// 这个块的所有词（按原文顺序）。
    pub words: Vec<usize>,
    /// 标记词：以 / 为 / 围绕 / 通过 / 把 / 让 ……
    pub markers: Vec<usize>,
    /// 动词（谓词里的"动"）。
    pub verbs: Vec<usize>,
    /// 第一个名词组：手段的 X、焦点 / 途径 / 目的的对象、把 / 让 后面的对象、话题。
    pub obj: Vec<usize>,
    /// 第二个名词组：手段的 Y、动作的宾语（有第一个名词组时）。
    pub obj2: Vec<usize>,
    pub template: Option<Template>,
    /// 排比合并进来的其他分句的宾语。
    pub appended: Vec<String>,
    pub dead: bool,
}

#[derive(Clone, Debug)]
pub struct ClauseIr {
    pub words: Vec<Word>,
    /// 前置的主语、情态词。
    pub pre: Vec<usize>,
    pub parts: Vec<Part>,
    /// 句式分析失败：原样保留。
    pub failed: bool,
    /// 被合并进别的分句（公共子表达式消除）。
    pub merged: bool,
    /// 分句后面的标点。
    pub punct: String,
    pub ends_sentence: bool,
}

impl ClauseIr {
    pub fn alive(&self, w: usize) -> bool {
        !self.words[w].dead
    }

    /// 一组词里还活着的词的当前说法。
    pub fn text_of(&self, ws: &[usize]) -> String {
        ws.iter().filter(|&&w| self.alive(w)).map(|&w| self.words[w].current()).collect()
    }

    pub fn has_body(&self) -> bool {
        !self.merged && self.parts.iter().any(|p| !p.dead)
    }
}

pub fn words_of(tokens: &[Token], lex: &Lexicon) -> Vec<Word> {
    tokens
        .iter()
        .map(|t| Word { text: t.text.clone(), span: t.span, entry: t.entry, cat: t.cat(lex), now: None, dead: false })
        .collect()
}

/// 把语法树翻译成语义块。
pub fn lower_tree(g: &Grammar, tree: &Tree, words: Vec<Word>, punct: String, ends_sentence: bool) -> ClauseIr {
    let mut ir = ClauseIr { words, pre: vec![], parts: vec![], failed: false, merged: false, punct, ends_sentence };
    visit(g, tree, &mut ir);
    ir
}

fn leaves(t: &Tree, out: &mut Vec<usize>) {
    if t.prod.is_none() {
        out.push(t.start);
    }
    for c in &t.children {
        leaves(c, out);
    }
}

fn leaves_of(t: &Tree) -> Vec<usize> {
    let mut v = Vec::new();
    leaves(t, &mut v);
    v
}

fn visit(g: &Grammar, t: &Tree, ir: &mut ClauseIr) {
    match g.name(t.sym) {
        "前置" => ir.pre = leaves_of(t),
        "块" if t.children.len() == 2 => {
            // 块 -> 修饰 块：修饰语归到后面那个块里
            visit(g, &t.children[1], ir);
            ir.parts.last_mut().unwrap().words.insert(0, t.children[0].start);
        }
        "块" => {
            let inner = &t.children[0];
            let role = match g.name(inner.sym) {
                "手段" => Role::Means,
                "焦点" => Role::Focus,
                "途径" => Role::Via,
                "目的" => Role::Purpose,
                "动作" => Role::Action,
                _ => Role::Link,
            };
            let mut part = Part {
                role,
                words: leaves_of(inner),
                markers: vec![],
                verbs: vec![],
                obj: vec![],
                obj2: vec![],
                template: None,
                appended: vec![],
                dead: false,
            };
            for c in &inner.children {
                match g.name(c.sym) {
                    "名组" if part.obj.is_empty() && !(role == Role::Action && is_second_np(g, inner, c)) => {
                        part.obj = leaves_of(c)
                    }
                    "名组" => part.obj2 = leaves_of(c),
                    "谓词" => part.verbs = leaves_of(c).into_iter().filter(|&w| is_verb(ir.words[w].cat)).collect(),
                    _ if c.prod.is_none() => part.markers.push(c.start),
                    _ => {}
                }
            }
            ir.parts.push(part);
        }
        _ => {
            for c in &t.children {
                visit(g, c, ir);
            }
        }
    }
}

/// 动作块 "谓词 名组" 里的名组是宾语，放到 obj2（obj 留给话题 / 把 / 让 的对象）。
fn is_second_np(g: &Grammar, action: &Tree, np: &Tree) -> bool {
    let first = &action.children[0];
    !std::ptr::eq(first, np) && g.name(first.sym) == "谓词"
}

pub fn is_verb(c: Cat) -> bool {
    matches!(c, Cat::Hollow | Cat::Filler | Cat::Verb | Cat::FixedVerb)
}

pub fn is_glue(c: Cat) -> bool {
    matches!(c, Cat::Func(Func::De | Func::BingLie))
}
