//! 第 1 章各课共用的部分：正则流水线、把自动机转换成状态图、正则输入框。

use bsc_automata::StateSet;
use bsc_automata::dfa::{Dfa, SubsetStep, subset_construction};
use bsc_automata::minimize::{Minimization, minimize};
use bsc_automata::nfa::{Construction, Nfa, thompson};
use bsc_automata::regex::{Regex, RegexId};
use bsc_core::Diagnostic;
use eframe::egui::{RichText, TextEdit, Ui};

use crate::theme::{Palette, mono};
use crate::widgets::{GEdge, GNode, Graph, TreeNode, card, diagnostic_view};

/// NFA 状态数超过这个值就不再做子集构造（图也没法看了）。
pub const MAX_NFA_STATES: usize = 120;

/// 一条正则经过的整条流水线：解析 → NFA → DFA → 最小 DFA。前面失败时后面为空。
pub struct RegexPipeline {
    pub src: String,
    pub regex: Result<Regex, Diagnostic>,
    pub nfa: Option<Construction>,
    pub dfa: Option<(Dfa, Vec<SubsetStep>)>,
    pub min: Option<Minimization>,
}

impl RegexPipeline {
    pub fn new(src: &str) -> Self {
        let regex = Regex::parse(src);
        let nfa = regex.as_ref().ok().map(thompson);
        let dfa = nfa.as_ref().filter(|c| c.nfa.num_states <= MAX_NFA_STATES).map(|c| subset_construction(&c.nfa));
        let min = dfa.as_ref().map(|(d, _)| minimize(d));
        Self { src: src.to_owned(), regex, nfa, dfa, min }
    }

    /// 用最小 DFA 判断整串是否匹配。
    pub fn matches(&self, s: &str) -> Option<bool> {
        match (&self.min, &self.regex) {
            (Some(m), _) => Some(m.dfa.accepts_str(s)),
            (None, Ok(r)) => Some(r.matches(s)),
            _ => None,
        }
    }
}

/// DFA 状态的名字：A, B, C, …（龙书的习惯），超过 26 个后加数字。
pub fn dfa_name(i: usize) -> String {
    let letter = (b'A' + (i % 26) as u8) as char;
    if i < 26 { letter.to_string() } else { format!("{letter}{}", i / 26) }
}

pub fn set_text(set: &StateSet) -> String {
    bsc_automata::fmt_set(set)
}

pub fn nfa_graph(nfa: &Nfa) -> Graph {
    Graph {
        nodes: (0..nfa.num_states)
            .map(|s| GNode { label: s.to_string(), accepting: nfa.is_accepting(s), note: None })
            .collect(),
        edges: nfa.edges.iter().map(|e| GEdge { from: e.from, to: e.to, label: e.label.text() }).collect(),
        start: Some(nfa.start),
        show_start: true,
    }
}

/// `with_sets`：在状态下方写出它对应的 NFA 状态集合。
pub fn dfa_graph(dfa: &Dfa, with_sets: bool) -> Graph {
    Graph {
        nodes: dfa
            .states
            .iter()
            .enumerate()
            .map(|(i, s)| GNode {
                label: dfa_name(i),
                accepting: s.accept.is_some(),
                note: with_sets.then(|| set_text(&s.nfa_set)),
            })
            .collect(),
        edges: dfa
            .merged_edges()
            .into_iter()
            .map(|(from, to, atoms)| GEdge { from, to, label: dfa.atoms_label(&atoms) })
            .collect(),
        start: Some(dfa.start),
        show_start: true,
    }
}

/// 最小 DFA：状态名用数字，下方写出它由原 DFA 的哪些状态合并而来。
pub fn min_graph(m: &Minimization) -> Graph {
    let mut g = dfa_graph(&m.dfa, false);
    for (i, node) in g.nodes.iter_mut().enumerate() {
        node.label = i.to_string();
        let names: Vec<String> = m.members[i].iter().map(|&s| dfa_name(s)).collect();
        node.note = Some(format!("{{{}}}", names.join(", ")));
    }
    g
}

pub fn regex_tree(r: &Regex) -> Vec<TreeNode> {
    (0..r.nodes.len())
        .map(|i| {
            let id = RegexId(i as u32);
            TreeNode { label: r.label(id), children: r.children(id).into_iter().map(RegexId::index).collect() }
        })
        .collect()
}

/// 正则输入框 + 例子按钮。返回输入是否改变。
pub fn regex_input(ui: &mut Ui, src: &mut String, presets: &[(&str, &str)]) -> bool {
    let p = Palette::of(ui);
    card(ui, |ui| {
        let mut changed = false;
        ui.horizontal(|ui| {
            ui.label("正则：");
            changed |= ui.add(TextEdit::singleline(src).font(mono(20.0)).desired_width(f32::INFINITY)).changed();
        });
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("例子：").color(p.muted));
            for (re, note) in presets {
                let text = if note.is_empty() { (*re).to_owned() } else { format!("{re}   ·  {note}") };
                if ui.button(RichText::new(text).font(mono(14.0))).clicked() {
                    *src = (*re).to_owned();
                    changed = true;
                }
            }
        });
        changed
    })
}

/// 流水线前面的阶段出错时显示错误，返回是否可以继续显示后面的内容。
pub fn show_pipeline_error(ui: &mut Ui, pipe: &RegexPipeline) -> bool {
    let p = Palette::of(ui);
    match &pipe.regex {
        Err(d) => {
            ui.label("这条正则写得有问题：");
            diagnostic_view(ui, &pipe.src, d);
            false
        }
        Ok(_) if pipe.dfa.is_none() => {
            ui.label(
                RichText::new(format!(
                    "这条正则生成的 NFA 超过 {MAX_NFA_STATES} 个状态，图已经没法看了。换一条短一点的正则试试。"
                ))
                .color(p.error),
            );
            false
        }
        Ok(_) => true,
    }
}
