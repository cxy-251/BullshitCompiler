//! 1.3 NFA：非确定有限自动机（Thompson 构造与模拟）。

use std::collections::BTreeSet;

use bsc_automata::nfa::{Construction, thompson};
use bsc_automata::regex::{Regex, RegexKind};
use bsc_core::Span;
use eframe::egui::{RichText, TextEdit, Ui};

use crate::automata_view::{RegexPipeline, nfa_graph, regex_input, regex_tree, set_text, show_pipeline_error};
use crate::theme::{Palette, mono};
use crate::widgets::{
    CalloutKind, GEdgeStyle, GNodeStyle, Mark, NodeStyle, Stepper, callout, card, graph_view, prose, prose_sized, quiz,
    scrollable_tree, source_view,
};

const PRESETS: &[(&str, &str)] =
    &[("(a|b)*abb", "龙书经典例子"), ("ab", "连接"), ("a|b", "选择"), ("a*", "重复"), ("[0-9]+", ""), ("ab?c", "")];

pub struct Lesson {
    src: String,
    pipe: RegexPipeline,
    build: Stepper,
    input: String,
    sim: Stepper,
    /// 四条构造规则的示意图，由真实的 Thompson 构造生成。
    rules: Vec<(&'static str, &'static str, Construction)>,
}

impl Default for Lesson {
    fn default() -> Self {
        let src = PRESETS[0].0.to_owned();
        let rules = [
            ("单个字符 a", "a", "两个状态，一条写着 a 的边。"),
            ("连接 ab", "ab", "把 a 的出口用 ε 边接到 b 的入口。"),
            ("选择 a|b", "a|b", "新入口分岔到两条路，两条路再汇合到新出口。"),
            ("重复 a*", "a*", "可以直接跳过（0 次），也可以走完一遍再绕回开头。"),
        ]
        .into_iter()
        .map(|(title, re, note)| (title, note, thompson(&Regex::parse(re).expect("内置正则"))))
        .collect();
        Self {
            pipe: RegexPipeline::new(&src),
            src,
            build: Stepper::default(),
            input: "babb".to_owned(),
            sim: Stepper::default(),
            rules,
        }
    }
}

impl Lesson {
    pub fn ui(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        ui.heading("1.3  NFA：非确定有限自动机");
        ui.label(RichText::new("入门 · 约 25 分钟").color(p.muted));

        ui.add_space(8.0);
        ui.label(RichText::new("一句话").size(20.0).strong());
        ui.label(
            "正则表达式是\"描述\"，而计算机需要一台能照着执行的\"机器\"。有限自动机就是这样的机器：\
             一张由圆圈（状态）和箭头（转移）组成的图。从\"开始\"出发，每读一个字符就沿着写着这个字符的箭头走一步；\
             读完时如果停在双圈（接受状态）上，就说明匹配成功。",
        );
        callout(ui, CalloutKind::Analogy, "会分身的探险家", |ui| {
            ui.label(
                "NFA 里的\"非确定\"是说：同一个地方、同一个字符，可能有好几条路可走；还有写着 ε 的路，不读字符也能走过去。\
                 想象一个会分身术的探险家：遇到岔路就分身，每条路都派一个分身去走；走进死胡同的分身消失。\
                 只要最后有任何一个分身站在终点，就算成功。",
            );
            ui.label("所以运行 NFA 时，要记住的不是\"在哪个状态\"，而是\"所有分身分别在哪些状态\"——一个状态集合。");
        });

        ui.add_space(8.0);
        ui.label(RichText::new("从正则造出 NFA：Thompson 构造法").size(20.0).strong());
        ui.label(
            "1968 年 Ken Thompson 给出了一个极其简单的方法：沿着正则的语法树从下往上，每个节点造一小块 NFA（一个\"片段\"，\
             有一个入口、一个出口），再按下面四条规则把孩子的片段拼起来。下面的图都是程序真实构造出来的：",
        );
        let rule_cols = if ui.available_width() > 820.0 { 2 } else { 1 };
        let rules = &self.rules;
        ui.columns(rule_cols, |cols| {
            for (i, (title, note, c)) in rules.iter().enumerate() {
                let ui = &mut cols[i % rule_cols];
                card(ui, |ui| {
                    ui.label(RichText::new(*title).strong());
                    ui.label(RichText::new(*note).color(p.muted));
                    let g = nfa_graph(&c.nfa);
                    graph_view(ui, &format!("rule{i}"), &g, |_| GNodeStyle::normal(&p), |_| GEdgeStyle::normal(&p));
                });
            }
        });
        prose(ui, "`+` 和 `?` 是 `*` 的变体：`a+` 去掉\"直接跳过\"那条 ε 边，`a?` 去掉\"绕回开头\"那条。");

        ui.add_space(8.0);
        ui.label(RichText::new("单步观察构造过程").size(20.0).strong());
        if regex_input(ui, &mut self.src, PRESETS) {
            self.pipe = RegexPipeline::new(&self.src);
            self.build.reset();
            self.sim.reset();
        }
        if show_pipeline_error(ui, &self.pipe) {
            self.construction(ui);

            ui.add_space(12.0);
            ui.label(RichText::new("让 NFA 跑起来").size(20.0).strong());
            ui.label("输入一个字符串，一个字符一个字符地看\"分身们\"都在哪。");
            self.simulation(ui);
        }

        ui.add_space(12.0);
        ui.label(RichText::new("小测验").size(20.0).strong());
        quiz(
            ui,
            "ch1-nfa-1",
            "箭头上写着 ε 是什么意思？",
            &["读入字符 ε", "不读任何字符就能走过去", "这条路不能走"],
            1,
            "ε 表示空串。沿 ε 边走不消耗输入，所以 NFA 在读下一个字符之前，会先把 ε 能到达的状态都算上（叫\"ε 闭包\"）。",
        );
        quiz(
            ui,
            "ch1-nfa-2",
            "NFA 读完输入后，当前状态集合是 {3, 7, 9}，其中只有 9 是接受状态。结果是？",
            &["匹配成功", "匹配失败", "无法判断"],
            0,
            "只要有一个分身站在接受状态上就算成功。",
        );
        quiz(
            ui,
            "ch1-nfa-3",
            "模拟 NFA 时，状态集合在中途变成了空集。接下来会怎样？",
            &["继续读，后面可能还会恢复", "一定匹配失败", "从头再来"],
            1,
            "空集说明所有分身都走进了死胡同。之后无论读什么，都不可能再有状态，结果一定是失败。",
        );

        ui.add_space(12.0);
        callout(ui, CalloutKind::Deeper, "Thompson NFA 的性质", |ui| {
            ui.label(
                "Thompson 构造出的 NFA 很\"规整\"：状态数不超过正则长度的 2 倍；只有一个接受状态；\
                 每个状态最多两条出边，要么是一条字符边，要么是最多两条 ε 边。",
            );
            ui.label(
                "用\"状态集合\"模拟 NFA，每读一个字符最多处理所有状态一遍，所以匹配长度为 n 的文本、状态数为 m 的 NFA，\
                 时间是 O(n·m)。很多编程语言自带的正则引擎用的是\"回溯\"，遇到 `(a*)*b` 这样的正则可能慢到指数级；\
                 而 Thompson 的方法永远是线性的。Go 语言的 regexp 和 Rust 的 regex 库都采用这种思路。",
            );
        });
        ui.add_space(24.0);
    }

    fn construction(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        let (Ok(regex), Some(c)) = (&self.pipe.regex, &self.pipe.nfa) else { return };
        self.build.ui(ui, c.steps.len());
        let k = self.build.pos;
        let current = k.checked_sub(1).map(|i| &c.steps[i]);

        let mut created_at = vec![usize::MAX; c.nfa.num_states];
        let mut edge_at = vec![usize::MAX; c.nfa.edges.len()];
        let mut done_nodes = BTreeSet::new();
        for (i, st) in c.steps[..k].iter().enumerate() {
            for &s in &st.new_states {
                created_at[s] = i;
            }
            for &e in &st.new_edges {
                edge_at[e] = i;
            }
            if let Some(n) = st.node {
                done_nodes.insert(n.index());
            }
        }

        // 正则源码上高亮当前节点
        let marks: Vec<Mark> = current
            .and_then(|st| st.node)
            .map(|n| Mark { span: regex.node(n).span, bg: p.focus_bg })
            .into_iter()
            .collect();
        card(ui, |ui| source_view(ui, &self.pipe.src, &[], &marks, 22.0));

        let explain = match current.and_then(|st| st.node) {
            None => "构造从语法树的叶子开始，自底向上：每个节点都要等它的孩子造好片段之后才能处理。".to_owned(),
            Some(n) => {
                let text = regex.node(n).span.text(&regex.source);
                let rule = match &regex.node(n).kind {
                    RegexKind::Set(s) => format!("字符 `{}`：新建两个状态，中间一条写着 {} 的边", text, s.label()),
                    RegexKind::Empty => "ε：两个状态之间一条 ε 边".to_owned(),
                    RegexKind::Concat(..) => format!("连接 `{text}`：把左边片段的出口用 ε 边接到右边片段的入口"),
                    RegexKind::Alt(..) => {
                        format!("选择 `{text}`：新建入口和出口，入口分岔到两个分支，两个分支再汇合到出口")
                    }
                    RegexKind::Star(_) => {
                        format!("重复 `{text}`：新建入口和出口；可以直接跳到出口（0 次），也可以走完片段后绕回开头再来")
                    }
                    RegexKind::Plus(_) => format!("重复 `{text}`：新建入口和出口；片段至少走一遍，走完可以绕回开头"),
                    RegexKind::Optional(_) => format!("可选 `{text}`：新建入口和出口；可以走片段，也可以直接跳到出口"),
                };
                let st = current.unwrap();
                format!("{rule}。这个片段的入口是 {}，出口是 {}。", st.start, st.accept)
            }
        };
        prose_sized(ui, &explain, 17.0);

        let tree = |ui: &mut Ui| {
            ui.label(RichText::new("语法树（处理过的节点变蓝）").strong());
            scrollable_tree(ui, "nfa_regex_tree", &regex_tree(regex), |i| {
                let mut s = NodeStyle::normal(&p);
                if done_nodes.contains(&i) {
                    s.fill = p.done_bg;
                }
                if current.and_then(|st| st.node).is_some_and(|n| n.index() == i) {
                    s.fill = p.focus_bg;
                    s.stroke = p.accent;
                    s.emphasized = true;
                }
                s
            });
        };
        let graph = |ui: &mut Ui| {
            ui.label(RichText::new(format!("NFA（共 {} 个状态）", c.nfa.num_states)).strong());
            let mut g = nfa_graph(&c.nfa);
            let finished = k == c.steps.len();
            for (s, node) in g.nodes.iter_mut().enumerate() {
                if !finished {
                    node.accepting = false;
                }
                if let Some(st) = current {
                    if s == st.start {
                        node.note = Some("入口".to_owned());
                    } else if s == st.accept {
                        node.note = Some("出口".to_owned());
                    }
                }
            }
            g.show_start = finished;
            let last = k.checked_sub(1);
            graph_view(
                ui,
                "nfa_build",
                &g,
                |s| match created_at[s] {
                    usize::MAX => GNodeStyle::hidden(),
                    i if Some(i) == last => GNodeStyle::focus(&p),
                    _ => GNodeStyle::normal(&p),
                },
                |e| match edge_at[e] {
                    usize::MAX => GEdgeStyle::hidden(),
                    i if Some(i) == last => GEdgeStyle::focus(&p),
                    _ => GEdgeStyle::normal(&p),
                },
            );
        };
        tree(ui);
        graph(ui);
        if k == c.steps.len() {
            ui.label(RichText::new("构造完成！起点和终点标出来了，下面让它跑起来。").color(p.ok));
        }
    }

    fn simulation(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        let Some(c) = &self.pipe.nfa else { return };
        let changed = card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("输入：");
                ui.add(TextEdit::singleline(&mut self.input).font(mono(20.0)).desired_width(f32::INFINITY)).changed()
            })
            .inner
        });
        if changed {
            self.sim.reset();
        }
        let steps = c.nfa.simulate(&self.input);
        self.sim.ui(ui, steps.len() - 1);
        let k = self.sim.pos;
        let step = &steps[k];

        // 输入串：已读的部分变蓝，刚读的字符高亮
        let mut marks = Vec::new();
        if let Some((i, ch)) = step.ch {
            marks.push(Mark { span: Span::new(0, i), bg: p.done_bg });
            marks.push(Mark { span: Span::new(i, i + ch.len_utf8()), bg: p.focus_bg });
        } else {
            marks.push(Mark { span: Span::empty_at(0), bg: p.focus_bg });
        }
        card(ui, |ui| source_view(ui, &self.input, &[], &marks, 22.0));

        let finished = k == steps.len() - 1;
        let read_all = steps.len() == self.input.chars().count() + 1;
        let explain = match step.ch {
            None => format!(
                "开始：分身站在起点 {}。不读字符，先沿 ε 边能走到的地方都派上分身：{}。",
                c.nfa.start,
                set_text(&step.current)
            ),
            Some((_, ch)) if step.current.is_empty() => {
                format!("读入 `{ch}`：没有任何一个状态有写着 `{ch}` 的出边——所有分身都走进了死胡同。")
            }
            Some((_, ch)) => format!(
                "读入 `{ch}`：从 {} 沿着能接受 `{ch}` 的边走一步，到达 {}；再补上 ε 闭包，得到 {}。",
                set_text(&steps[k - 1].current),
                set_text(&step.moved),
                set_text(&step.current)
            ),
        };
        prose_sized(ui, &explain, 17.0);

        let g = nfa_graph(&c.nfa);
        graph_view(
            ui,
            "nfa_sim",
            &g,
            |s| {
                if step.moved.contains(&s) && step.ch.is_some() {
                    GNodeStyle::focus(&p)
                } else if step.current.contains(&s) {
                    GNodeStyle::filled(&p, p.focus_bg)
                } else {
                    GNodeStyle::normal(&p)
                }
            },
            |_| GEdgeStyle::normal(&p),
        );
        ui.label(
            RichText::new("黄色：当前有分身的状态；粗边框：刚沿字符边走到的状态（其余是 ε 闭包带来的）。")
                .small()
                .color(p.muted),
        );

        if finished {
            let accepted = read_all && c.nfa.accept_tag(&step.current).is_some();
            let text = if accepted {
                RichText::new("读完了，有分身站在接受状态上：匹配成功！").color(p.ok)
            } else if !read_all {
                RichText::new("还没读完，状态集合就空了：匹配失败。").color(p.error)
            } else {
                RichText::new("读完了，但没有分身站在接受状态上：匹配失败。").color(p.error)
            };
            ui.label(text.size(17.0).strong());
        }
    }
}
