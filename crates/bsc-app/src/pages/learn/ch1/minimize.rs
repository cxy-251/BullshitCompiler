//! 1.5 DFA 最小化。

use bsc_automata::dfa::Dfa;
use bsc_automata::minimize::{MinStep, Minimization};
use eframe::egui::{CornerRadius, Frame, Margin, RichText, Ui};

use crate::automata_view::{RegexPipeline, dfa_graph, dfa_name, min_graph, regex_input, show_pipeline_error};
use crate::theme::{Palette, mono};
use crate::widgets::{
    CalloutKind, GEdge, GEdgeStyle, GNode, GNodeStyle, Graph, Stepper, callout, card, graph_view, prose, prose_sized,
    quiz,
};

const PRESETS: &[(&str, &str)] =
    &[("(a|b)*abb", "龙书例 3.40"), ("a*|a*b", ""), ("(ab|b)*", ""), ("ab|cb", "需要死状态"), ("(a|b)*a(a|b)", "")];

const DEAD: &str = "死";

pub struct Lesson {
    src: String,
    pipe: RegexPipeline,
    stepper: Stepper,
}

impl Default for Lesson {
    fn default() -> Self {
        let src = PRESETS[0].0.to_owned();
        Self { pipe: RegexPipeline::new(&src), src, stepper: Stepper::default() }
    }
}

fn name(dfa: &Dfa, s: usize) -> String {
    if s == dfa.states.len() { DEAD.to_owned() } else { dfa_name(s) }
}

fn block_text(dfa: &Dfa, block: &[usize]) -> String {
    let names: Vec<String> = block.iter().map(|&s| name(dfa, s)).collect();
    format!("{{{}}}", names.join(", "))
}

/// 原 DFA 的状态图；需要死状态时把它和指向它的边也画出来。
fn graph_with_dead(dfa: &Dfa, m: &Minimization) -> Graph {
    let mut g = dfa_graph(dfa, false);
    if let Some(dead) = m.dead {
        g.nodes.push(GNode { label: DEAD.to_owned(), accepting: false, note: None });
        for (s, st) in dfa.states.iter().enumerate() {
            let missing: Vec<usize> = (0..dfa.alphabet.len()).filter(|&a| st.trans[a].is_none()).collect();
            if !missing.is_empty() {
                g.edges.push(GEdge { from: s, to: dead, label: dfa.atoms_label(&missing) });
            }
        }
        g.edges.push(GEdge { from: dead, to: dead, label: "任意".to_owned() });
    }
    g
}

impl Lesson {
    pub fn ui(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        ui.heading("1.5  DFA 最小化");
        ui.label(RichText::new("进阶 · 约 25 分钟 · 需要先学 1.4").color(p.muted));

        ui.add_space(8.0);
        ui.label(RichText::new("一句话").size(20.0).strong());
        ui.label(
            "子集构造得到的 DFA 往往有多余的状态：两个状态\"表现完全一样\"，却被当成了两个。最小化就是找出这样的状态并合并，\
             得到状态数最少的 DFA——生成的词法分析器表更小、更省内存。",
        );
        callout(ui, CalloutKind::KeyPoint, "什么叫\"表现完全一样\"", |ui| {
            ui.label(
                "从状态 p 和状态 q 分别出发，喂给它们任何一个后续字符串，如果两边总是同时接受、或同时拒绝，\
                 那 p 和 q 就是等价的，可以合并成一个状态。",
            );
        });
        callout(ui, CalloutKind::Analogy, "先当成一家人，再找不同", |ui| {
            ui.label(
                "\"任何后续字符串\"有无穷多个，没法一个个试。于是反过来想：先假设能合并的都合并（只分\"接受\"和\"不接受\"两组），\
                 然后找反例——如果同一组里的两个状态，读同一个字符后去了不同的组，那它们肯定不一样，拆开。\
                 一直拆到拆不动为止，剩下的每一组就是一个状态。",
            );
        });

        ui.add_space(8.0);
        ui.label(RichText::new("算法（划分细化）").size(20.0).strong());
        card(ui, |ui| {
            for line in [
                "补一个\"死状态\"：所有缺失的边都指向它，它读什么都回到自己，永远不接受",
                "初始划分：{接受状态}、{非接受状态}",
                "重复：找一个组 G 和一类字符 c，G 里的状态读 c 后落到了不同的组",
                "      → 按\"落到哪个组\"把 G 拆开",
                "直到每个组都\"意见一致\"。每组合并成一个状态，最后删掉死状态所在的组",
            ] {
                ui.label(RichText::new(line).font(mono(14.0)));
            }
        });

        ui.add_space(8.0);
        ui.label(RichText::new("单步观察").size(20.0).strong());
        if regex_input(ui, &mut self.src, PRESETS) {
            self.pipe = RegexPipeline::new(&self.src);
            self.stepper.reset();
        }
        if show_pipeline_error(ui, &self.pipe) {
            self.refinement(ui);
        }

        ui.add_space(12.0);
        ui.label(RichText::new("小测验").size(20.0).strong());
        quiz(
            ui,
            "ch1-min-1",
            "初始划分为什么要把接受状态和非接受状态分开？",
            &["为了让算法跑得更快", "读空串时它们一个接受一个拒绝，已经\"表现不同\"了", "这是规定，没有原因"],
            1,
            "\"任何后续字符串\"也包括空串。接受状态读完空串就接受，非接受状态不接受，所以它们一定不等价。",
        );
        quiz(
            ui,
            "ch1-min-2",
            "同一组里的 A 和 B，读 a 后都去了第 1 组，读 b 后 A 去第 1 组、B 去第 2 组。应该怎么办？",
            &["把 A 和 B 拆开", "保持在同一组", "把第 1 组和第 2 组合并"],
            0,
            "读 b 之后它们落到了\"已知不同\"的两组里，从那里出发一定能找到让它们表现不同的后续字符串。",
        );

        ui.add_space(12.0);
        callout(ui, CalloutKind::Deeper, "最小 DFA 是唯一的", |ui| {
            ui.label(
                "Myhill–Nerode 定理告诉我们：对一个正则语言，状态数最少的 DFA 在\"改名字\"的意义下是唯一的。\
                 所以不管正则怎么写，只要描述的是同一个语言，最小化后得到的 DFA 都一样——这也给了我们一个判断两条正则是否等价的办法。",
            );
            ui.label(
                "这里演示的是 Moore 算法：每次找一个能拆的组就拆，最坏 O(n²·k)。Hopcroft 算法（1971）是它的高效版本：\
                 维护一个\"待用的划分者\"工作表，每次拆分后只把较小的那一半放回工作表，把复杂度降到 O(n·k·log n)。",
            );
        });
        ui.add_space(24.0);
    }

    fn refinement(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        let (Some((dfa, _)), Some(m)) = (&self.pipe.dfa, &self.pipe.min) else { return };
        self.stepper.ui(ui, m.steps.len() - 1);
        let k = self.stepper.pos;
        let step = &m.steps[k];
        let blocks = step.blocks();
        let total = dfa.states.len() + usize::from(m.dead.is_some());
        let mut block_of = vec![0; total];
        for (bi, b) in blocks.iter().enumerate() {
            for &s in b {
                block_of[s] = bi;
            }
        }

        let explain = match step {
            MinStep::Init { blocks } => {
                let dead = if m.dead.is_some() {
                    "先补上死状态（有些状态缺边）。"
                } else {
                    "这个 DFA 没有缺边，不需要死状态。"
                };
                format!("{dead}初始划分：按能否接受分成 {} 组。", blocks.len())
            }
            MinStep::Split { block, atom, targets, blocks } => {
                let prev = &m.steps[k - 1].blocks()[*block];
                let moves: Vec<String> =
                    targets.iter().map(|(s, t)| format!("{}→组{}", name(dfa, *s), t + 1)).collect();
                let parts: Vec<String> =
                    blocks.iter().filter(|b| b.iter().all(|s| prev.contains(s))).map(|b| block_text(dfa, b)).collect();
                format!(
                    "组 {} {} 里的状态读 `{}` 后：{}。它们落到了不同的组，所以拆成 {}。",
                    block + 1,
                    block_text(dfa, prev),
                    dfa.alphabet[*atom].label(),
                    moves.join("，"),
                    parts.join(" 和 ")
                )
            }
            MinStep::Done { .. } => format!(
                "再也找不到能拆的组了。每组合并成一个状态，去掉死状态：{} 个状态 → {} 个状态。",
                dfa.states.len(),
                m.dfa.states.len()
            ),
        };
        prose_sized(ui, &explain, 17.0);

        // 当前划分，每组一个颜色
        let split_block = match step {
            MinStep::Split { targets, .. } => Some(targets.iter().map(|(s, _)| *s).collect::<Vec<_>>()),
            _ => None,
        };
        ui.horizontal_wrapped(|ui| {
            for (bi, b) in blocks.iter().enumerate() {
                Frame::new()
                    .fill(p.category(bi))
                    .corner_radius(CornerRadius::same(6))
                    .inner_margin(Margin::symmetric(8, 3))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(format!("组{} {}", bi + 1, block_text(dfa, b)))
                                .font(mono(14.0))
                                .color(p.text),
                        );
                    });
            }
        });

        let g = graph_with_dead(dfa, m);
        graph_view(
            ui,
            "min_dfa",
            &g,
            |s| {
                let mut st = GNodeStyle::filled(&p, p.category(block_of[s]));
                if split_block.as_ref().is_some_and(|b| b.contains(&s)) {
                    st.stroke = p.accent;
                    st.emphasized = true;
                }
                st
            },
            |_| GEdgeStyle::normal(&p),
        );

        if matches!(step, MinStep::Done { .. }) {
            ui.label(RichText::new("最小 DFA（状态下方是它由哪些原状态合并而来）").strong());
            graph_view(ui, "min_result", &min_graph(m), |_| GNodeStyle::normal(&p), |_| GEdgeStyle::normal(&p));
        } else {
            prose(ui, "点到最后一步，可以看到合并后的最小 DFA。");
        }
    }
}
