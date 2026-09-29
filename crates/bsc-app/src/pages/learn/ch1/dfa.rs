//! 1.4 DFA 与子集构造。

use bsc_automata::dfa::SubsetStep;
use bsc_core::Span;
use eframe::egui::{self, RichText, TextEdit, Ui};

use crate::automata_view::{RegexPipeline, dfa_graph, dfa_name, nfa_graph, regex_input, set_text, show_pipeline_error};
use crate::theme::{Palette, mono};
use crate::widgets::{
    CalloutKind, GEdgeStyle, GNodeStyle, Mark, Stepper, callout, card, graph_view, prose, prose_sized, quiz,
    source_view,
};

const PRESETS: &[(&str, &str)] =
    &[("(a|b)*abb", "龙书经典例子"), ("a|ab", ""), ("[a-z]+|if", "字符类重叠"), ("(a|b)*a(a|b)(a|b)", "状态会变多")];

pub struct Lesson {
    src: String,
    pipe: RegexPipeline,
    build: Stepper,
    input: String,
    run: Stepper,
}

impl Default for Lesson {
    fn default() -> Self {
        let src = PRESETS[0].0.to_owned();
        Self {
            pipe: RegexPipeline::new(&src),
            src,
            build: Stepper::default(),
            input: "aabb".to_owned(),
            run: Stepper::default(),
        }
    }
}

impl Lesson {
    pub fn ui(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        ui.heading("1.4  DFA 与子集构造");
        ui.label(RichText::new("进阶 · 约 30 分钟 · 需要先学 1.3").color(p.muted));

        ui.add_space(8.0);
        ui.label(RichText::new("一句话").size(20.0).strong());
        ui.label(
            "NFA 每读一个字符都要更新一整个状态集合，太慢了。DFA（确定有限自动机）规定：每个状态、每个字符最多只有一条出边，\
             也没有 ε 边。于是运行时只需记住\"当前在哪一个状态\"，每读一个字符查一次表就行——真实的词法分析器跑的都是 DFA。",
        );
        callout(ui, CalloutKind::Analogy, "把分身们的位置拍成照片", |ui| {
            ui.label(
                "模拟 NFA 时，每读一个字符，分身们所在的位置就是一个状态集合，比如 {0, 1, 2, 4, 7}。\
                 子集构造的想法是：把每一种可能出现的\"集合\"事先算出来，给它起个名字（A、B、C……），当成 DFA 的一个状态。\
                 就像给分身们的站位拍照，一张照片就是一个 DFA 状态。运行时不用再管分身，只要知道现在是哪张照片。",
            );
        });

        ui.add_space(8.0);
        ui.label(RichText::new("算法").size(20.0).strong());
        card(ui, |ui| {
            for line in [
                "A = ε闭包({NFA 的起点})，放进待处理列表",
                "取出一个待处理的 DFA 状态 X，对字母表里的每一类字符 c：",
                "    move：从 X 里的状态出发，沿 c 边走一步能到的 NFA 状态",
                "    再求这些状态的 ε 闭包，得到集合 U",
                "    如果 U 是第一次出现，给它起个新名字，放进待处理列表",
                "    记下转移 X --c--> U",
                "直到没有待处理的状态。含有 NFA 接受状态的集合就是 DFA 的接受状态。",
            ] {
                ui.label(RichText::new(line).font(mono(14.0)));
            }
        });
        prose(
            ui,
            "\"字母表里的每一类字符\"：NFA 的边上可能出现 `[a-z]` 和 `i` 这样互相重叠的字符集。算法先把字母表切成两两不相交的几类\
             （这里是 `i` 和 `[a-hj-z]`），同一类里的字符走向完全相同，只需算一次。",
        );

        ui.add_space(8.0);
        ui.label(RichText::new("单步观察子集构造").size(20.0).strong());
        if regex_input(ui, &mut self.src, PRESETS) {
            self.pipe = RegexPipeline::new(&self.src);
            self.build.reset();
            self.run.reset();
        }
        if show_pipeline_error(ui, &self.pipe) {
            self.construction(ui);
            ui.add_space(12.0);
            ui.label(RichText::new("让 DFA 跑起来").size(20.0).strong());
            ui.label("对比一下上一课的 NFA：现在每一步只在一个状态上，查一次表就知道下一步去哪。");
            self.running(ui);
        }

        ui.add_space(12.0);
        ui.label(RichText::new("小测验").size(20.0).strong());
        quiz(
            ui,
            "ch1-dfa-1",
            "DFA 的一个状态，读同一个字符，最多有几条出边？",
            &["0 条", "1 条", "任意多条"],
            1,
            "\"确定\"的意思就是下一步去哪是唯一确定的。也可能是 0 条——那表示读到这个字符就失败。",
        );
        quiz(
            ui,
            "ch1-dfa-2",
            "子集构造中，什么样的 DFA 状态是接受状态？",
            &["集合里全是 NFA 的接受状态", "集合里至少有一个 NFA 的接受状态", "集合里只有一个状态"],
            1,
            "DFA 状态代表\"分身们的站位\"。只要有一个分身站在终点，就算成功。",
        );
        quiz(
            ui,
            "ch1-dfa-3",
            "为什么词法分析器用 DFA 而不是直接模拟 NFA？",
            &["DFA 能匹配更多种字符串", "DFA 每个字符只查一次表，运行更快", "NFA 无法表示字符类"],
            1,
            "NFA 和 DFA 能识别的语言完全一样（子集构造就是证明）。区别在于速度：DFA 把\"算集合\"的工作提前到了生成阶段。",
        );

        ui.add_space(12.0);
        callout(ui, CalloutKind::Deeper, "状态爆炸", |ui| {
            prose(
                ui,
                "有 n 个状态的 NFA，可能的子集有 2ⁿ 个。最坏情况下 DFA 真的会有指数多个状态：\
                 \"倒数第 k 个字符是 a\"——`(a|b)*a(a|b)…(a|b)`（后面 k-1 个 `(a|b)`）——对应的最小 DFA 恰好有 2ᵏ 个状态，\
                 因为它必须记住最近 k 个字符。试试在上面输入 `(a|b)*a(a|b)(a|b)`，再多加几个 `(a|b)` 看看。",
            );
            ui.label(
                "实践中这种情况很少见。有些正则引擎（比如 RE2）采用\"惰性\"的办法：运行时用到哪个集合才去算、并缓存起来，\
                 既不用提前付出指数代价，又能享受 DFA 的速度。",
            );
        });
        ui.add_space(24.0);
    }

    fn construction(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        let (Some(c), Some((dfa, steps))) = (&self.pipe.nfa, &self.pipe.dfa) else { return };
        self.build.ui(ui, steps.len());
        let k = self.build.pos;
        let current = k.checked_sub(1).map(|i| &steps[i]);

        // 回放前 k 步：已发现的 DFA 状态、已算出的转移。
        let mut discovered = vec![false; dfa.states.len()];
        let mut known: Vec<Vec<Option<Option<usize>>>> = vec![vec![None; dfa.alphabet.len()]; dfa.states.len()];
        for st in &steps[..k] {
            match st {
                SubsetStep::Start { .. } => discovered[0] = true,
                SubsetStep::Explore { from, atom, to, .. } => {
                    known[*from][*atom] = Some(*to);
                    if let Some(t) = to {
                        discovered[*t] = true;
                    }
                }
            }
        }

        let explain = match current {
            None => "从 NFA 的起点开始。".to_owned(),
            Some(SubsetStep::Start { closure }) => format!(
                "起点：A = ε闭包({{{}}}) = {}。{}",
                c.nfa.start,
                set_text(closure),
                if dfa.states[0].accept.is_some() { "它含有接受状态，所以 A 是接受状态。" } else { "" }
            ),
            Some(SubsetStep::Explore { from, atom, moved, closure, to, is_new }) => {
                let head = format!(
                    "处理 {} 在 `{}` 上：move = {}，ε闭包 = {}",
                    dfa_name(*from),
                    dfa.alphabet[*atom].label(),
                    set_text(moved),
                    set_text(closure)
                );
                match to {
                    None => format!(
                        "{head}。空集——{} 读 `{}` 没有路可走，不加这条边。",
                        dfa_name(*from),
                        dfa.alphabet[*atom].label()
                    ),
                    Some(t) if *is_new => format!(
                        "{head}。这是新出现的集合，命名为 {}{}。",
                        dfa_name(*t),
                        if dfa.states[*t].accept.is_some() { "（含接受状态，是接受状态）" } else { "" }
                    ),
                    Some(t) => format!("{head}。这个集合就是已有的 {}。", dfa_name(*t)),
                }
            }
        };
        prose_sized(ui, &explain, 17.0);

        // NFA：当前 DFA 状态对应的集合 + move + 闭包
        let (from_set, moved, closure) = match current {
            Some(SubsetStep::Explore { from, moved, closure, .. }) => {
                (Some(&dfa.states[*from].nfa_set), Some(moved), Some(closure))
            }
            Some(SubsetStep::Start { closure }) => (None, None, Some(closure)),
            None => (None, None, None),
        };
        ui.label(RichText::new("NFA").strong());
        graph_view(
            ui,
            "dfa_nfa",
            &nfa_graph(&c.nfa),
            |s| {
                if moved.is_some_and(|m| m.contains(&s)) {
                    GNodeStyle::focus(&p)
                } else if closure.is_some_and(|m| m.contains(&s)) {
                    GNodeStyle::filled(&p, p.focus_bg)
                } else if from_set.is_some_and(|m| m.contains(&s)) {
                    GNodeStyle::filled(&p, p.done_bg)
                } else {
                    GNodeStyle::normal(&p)
                }
            },
            |_| GEdgeStyle::normal(&p),
        );
        ui.label(
            RichText::new("蓝色：出发的集合；粗边框：move 到达的状态；黄色：加上 ε 闭包后的结果集合。")
                .small()
                .color(p.muted),
        );

        // 表格
        ui.label(RichText::new("子集构造表").strong());
        let cur_cell = match current {
            Some(SubsetStep::Explore { from, atom, .. }) => Some((*from, *atom)),
            _ => None,
        };
        card(ui, |ui| {
            egui::ScrollArea::horizontal().id_salt("subset_table").show(ui, |ui| {
                egui::Grid::new("subset_grid").striped(true).spacing([16.0, 6.0]).show(ui, |ui| {
                    ui.label(RichText::new("DFA 状态").strong());
                    ui.label(RichText::new("对应的 NFA 状态集合").strong());
                    for a in &dfa.alphabet {
                        ui.label(RichText::new(format!("读 {}", a.label())).strong());
                    }
                    ui.end_row();
                    for (s, st) in dfa.states.iter().enumerate() {
                        if !discovered[s] {
                            continue;
                        }
                        let mut name = RichText::new(dfa_name(s)).font(mono(15.0)).strong();
                        if st.accept.is_some() {
                            name = name.color(p.ok);
                        }
                        ui.label(name);
                        ui.label(RichText::new(set_text(&st.nfa_set)).font(mono(14.0)));
                        for (a, cell) in known[s].iter().enumerate() {
                            let text = match cell {
                                None => String::new(),
                                Some(None) => "—".to_owned(),
                                Some(Some(t)) => dfa_name(*t),
                            };
                            let mut rt = RichText::new(text).font(mono(15.0));
                            if cur_cell == Some((s, a)) {
                                rt = rt.background_color(p.focus_bg);
                            }
                            ui.label(rt);
                        }
                        ui.end_row();
                    }
                });
            });
            ui.label(RichText::new("绿色的是接受状态；\"—\"表示没有这条边。").small().color(p.muted));
        });

        // DFA 图
        ui.label(RichText::new(format!("DFA（最终共 {} 个状态）", dfa.states.len())).strong());
        let g = dfa_graph(dfa, true);
        let merged = dfa.merged_edges();
        graph_view(
            ui,
            "dfa_build",
            &g,
            |s| {
                if !discovered[s] {
                    GNodeStyle::hidden()
                } else if cur_cell.is_some_and(|(f, _)| f == s) {
                    GNodeStyle::filled(&p, p.done_bg)
                } else if matches!(current, Some(SubsetStep::Explore { to: Some(t), .. }) if *t == s)
                    || matches!(current, Some(SubsetStep::Start { .. }))
                {
                    GNodeStyle::focus(&p)
                } else {
                    GNodeStyle::normal(&p)
                }
            },
            |e| {
                let (from, to, atoms) = &merged[e];
                let shown = atoms.iter().any(|&a| known[*from][a] == Some(Some(*to)));
                if !shown {
                    GEdgeStyle::hidden()
                } else if cur_cell.is_some_and(|(f, a)| f == *from && atoms.contains(&a)) {
                    GEdgeStyle::focus(&p)
                } else {
                    GEdgeStyle::normal(&p)
                }
            },
        );
        if k == steps.len() {
            ui.label(
                RichText::new(format!(
                    "构造完成：NFA 的 {} 个状态变成了 DFA 的 {} 个状态。",
                    c.nfa.num_states,
                    dfa.states.len()
                ))
                .color(p.ok),
            );
        }
    }

    fn running(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        let Some((dfa, _)) = &self.pipe.dfa else { return };
        let changed = card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("输入：");
                ui.add(TextEdit::singleline(&mut self.input).font(mono(20.0)).desired_width(f32::INFINITY)).changed()
            })
            .inner
        });
        if changed {
            self.run.reset();
        }
        let path = dfa.run(&self.input);
        let chars: Vec<(usize, char)> = self.input.char_indices().collect();
        let stuck = path.len() < chars.len() + 1;
        let len = path.len() - 1 + usize::from(stuck);
        self.run.ui(ui, len);
        let k = self.run.pos;
        let state = path[k.min(path.len() - 1)];

        let mut marks = Vec::new();
        if k > 0 {
            let (i, ch) = chars[k - 1];
            marks.push(Mark { span: Span::new(0, i), bg: p.done_bg });
            marks.push(Mark { span: Span::new(i, i + ch.len_utf8()), bg: p.focus_bg });
        } else {
            marks.push(Mark { span: Span::empty_at(0), bg: p.focus_bg });
        }
        card(ui, |ui| source_view(ui, &self.input, &[], &marks, 22.0));

        let explain = if k == 0 {
            format!("从起点 {} 出发。", dfa_name(dfa.start))
        } else if stuck && k == len {
            let ch = chars[k - 1].1;
            format!("在 {} 读 `{ch}`：表里没有这一格——无路可走，匹配失败。", dfa_name(state))
        } else {
            format!("读 `{}`：查表 {} → {}。", chars[k - 1].1, dfa_name(path[k - 1]), dfa_name(state))
        };
        prose_sized(ui, &explain, 17.0);

        let g = dfa_graph(dfa, false);
        let merged = dfa.merged_edges();
        let last_edge = (k > 0 && !(stuck && k == len)).then(|| (path[k - 1], state));
        graph_view(
            ui,
            "dfa_run",
            &g,
            |s| if s == state { GNodeStyle::focus(&p) } else { GNodeStyle::normal(&p) },
            |e| {
                if last_edge == Some((merged[e].0, merged[e].1)) {
                    GEdgeStyle::focus(&p)
                } else {
                    GEdgeStyle::normal(&p)
                }
            },
        );
        if k == len {
            let ok = !stuck && dfa.states[state].accept.is_some();
            let text = if ok {
                RichText::new("读完了，停在接受状态上：匹配成功！").color(p.ok)
            } else if stuck {
                RichText::new("中途无路可走：匹配失败。").color(p.error)
            } else {
                RichText::new("读完了，但停在非接受状态：匹配失败。").color(p.error)
            };
            ui.label(text.size(17.0).strong());
        }
    }
}
