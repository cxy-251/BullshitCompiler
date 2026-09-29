//! 2.6 LR 分析。

use bsc_grammar::Sym;
use bsc_grammar::lr::{self, Automaton, BuildStep, LrAction, SlrTable};
use eframe::egui::{self, RichText, Ui};

use crate::grammar_view::GrammarInput;
use crate::theme::{Palette, mono};
use crate::widgets::{
    CalloutKind, GEdge, GEdgeStyle, GNode, GNodeStyle, Graph, NodeStyle, Stepper, callout, card, graph_view, prose,
    prose_sized, quiz, scrollable_tree,
};

const PRESETS: &[(&str, &str, &str)] = &[
    ("表达式（左递归也没问题）", "E -> E + T | T\nT -> T * F | F\nF -> ( E ) | id", "id * ( id + id )"),
    ("括号配对", "S -> ( S ) S | ε", "( ( ) ) ( )"),
    ("有歧义（有冲突）", "E -> E + E | E * E | id", "id + id * id"),
    (
        "悬空 else（有冲突）",
        "S -> if E then S | if E then S else S | other\nE -> b",
        "if b then if b then other else other",
    ),
];

pub struct Lesson {
    input: GrammarInput,
    build: Stepper,
    run: Stepper,
}

impl Default for Lesson {
    fn default() -> Self {
        let (_, g, s) = PRESETS[0];
        Self { input: GrammarInput::new(g, s), build: Stepper::default(), run: Stepper::default() }
    }
}

fn action_text(a: LrAction) -> String {
    match a {
        LrAction::Shift(s) => format!("s{s}"),
        LrAction::Reduce(p) => format!("r{p}"),
        LrAction::Accept => "acc".to_owned(),
    }
}

impl Lesson {
    pub fn ui(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        ui.heading("2.6  LR 分析");
        ui.label(RichText::new("进阶 · 约 40 分钟 · 需要先学 2.5").color(p.muted));

        ui.add_space(8.0);
        ui.label(RichText::new("一句话").size(20.0).strong());
        ui.label(
            "LL 分析自顶向下：先决定用哪条产生式，再去匹配。LR 分析反过来，自底向上：先把记号一个个读进来，\
             等栈顶攒够了某条产生式的整个右部，再把它们\"归约\"成左部的非终结符。因为是\"看完了再决定\"，LR 能处理的文法比 LL 多得多，\
             左递归也不成问题。yacc、bison 生成的分析器都是 LR 的。",
        );
        callout(ui, CalloutKind::Analogy, "拼积木", |ui| {
            ui.label(
                "一块块拿起积木（移进），手里的几块正好能拼成一个部件时，就把它们拼起来当成一整块（归约）；\
                 部件再和别的积木拼成更大的部件……最后拼成一个完整的作品（开始符号）。难点在于：手里的积木既可以现在拼，\
                 也可以再拿一块再说时，该怎么选？LR 用一个有限自动机来回答这个问题。",
            );
        });

        ui.add_space(8.0);
        ui.label(RichText::new("项目与自动机").size(20.0).strong());
        prose(
            ui,
            "**项目**是一条标了进度的产生式：`E → E • + T` 表示\"正在匹配 `E → E + T`，已经看到了 `E`，接下来期待 `+ T`\"。\
             点在最后（`T → F •`）就表示右部看全了，可以归约。自动机的每个状态是一组项目，表示\"此刻可能处在哪些进度上\"。",
        );
        card(ui, |ui| {
            for line in [
                "闭包：• 后面是非终结符 B，就把 B 的所有产生式 B → • γ 加进来，重复到不变",
                "goto(I, X)：把 I 里 • 后面是 X 的项目的 • 右移一格，再求闭包",
                "从 闭包({S' → • S}) 出发，对每个状态的每个符号求 goto，直到没有新状态",
            ] {
                ui.label(RichText::new(line).font(mono(14.0)));
            }
        });
        prose(ui, "`S'` 是新加的开始符号（\"增广文法\"），这样\"归约出 `S'` 并且输入结束\"就是唯一的成功时机。");

        let (gc, sc) = self.input.editors(ui, PRESETS);
        if gc {
            self.build.reset();
        }
        if gc || sc {
            self.run.reset();
        }
        if !self.input.show_errors(ui) {
            return;
        }
        let Ok(g) = &self.input.grammar else { return };
        let auto = Automaton::build(g);
        if auto.states.len() > 60 {
            ui.label(RichText::new("这个文法的自动机超过 60 个状态，图太大了，换个小一点的文法试试。").color(p.error));
            return;
        }

        ui.add_space(8.0);
        ui.label(RichText::new("单步构造 LR(0) 自动机").size(20.0).strong());
        self.automaton(ui, &auto);

        ui.add_space(8.0);
        ui.label(RichText::new("SLR(1) 分析表").size(20.0).strong());
        card(ui, |ui| {
            for line in [
                "状态 i 有项目 A → α • a β（a 是终结符）：ACTION[i, a] = 移进，转到 goto(i, a)，记作 s几",
                "状态 i 有项目 A → α •（看全了）：对 FOLLOW(A) 里的每个 b，ACTION[i, b] = 按 A → α 归约，记作 r几",
                "状态 i 有项目 S' → S •：ACTION[i, $] = 接受",
                "非终结符的转移 goto(i, A) 填进 GOTO 表",
            ] {
                ui.label(RichText::new(line).font(mono(14.0)));
            }
        });
        let table = SlrTable::build(&auto);
        table_view(ui, &auto, &table);
        let conflicts = table.conflicts();
        if conflicts.is_empty() {
            ui.label(RichText::new("没有冲突：这是一个 SLR(1) 文法。").color(p.ok));
        } else {
            ui.label(
                RichText::new(format!(
                    "有 {} 个冲突格（红色）：同一个状态、同一个输入，既可以移进又可以归约（或有两种归约）。",
                    conflicts.len()
                ))
                .color(p.error),
            );
            prose(
                ui,
                "有歧义的文法一定有冲突。实践中 yacc/bison 允许声明运算符的优先级和结合性来消解这类冲突，比如声明 `*` 比 `+` 优先、都是左结合，\
                 冲突格就按声明选择移进或归约——相当于把 2.2 课的\"分层\"交给工具去做。悬空 else 的冲突则默认选择移进，让 else 跟最近的 if 配对。",
            );
        }

        ui.add_space(8.0);
        ui.label(RichText::new("移进-归约分析").size(20.0).strong());
        prose(
            ui,
            "栈里交替放着状态和符号，一开始只有状态 0。每一步看栈顶状态和下一个输入，查 ACTION 表：移进就压入记号和新状态；\
             归约就弹出右部那么多个符号，压入左部，再查 GOTO 表得到新状态。",
        );
        self.parsing(ui, &auto, &table);

        ui.add_space(12.0);
        ui.label(RichText::new("小测验").size(20.0).strong());
        quiz(
            ui,
            "ch2-lr-1",
            "项目 `T → T • * F` 表示什么？",
            &["T 已经全部匹配完", "已经看到了 T，接下来期待 * F", "接下来期待 T * F"],
            1,
            "点左边是已经看到的部分，点右边是还期待的部分。",
        );
        quiz(
            ui,
            "ch2-lr-2",
            "左递归文法 `E -> E + T | T` 能用 LR 分析吗？",
            &["能，LR 不怕左递归", "不能，会无限循环", "必须先消除左递归"],
            0,
            "LR 是先读进来再归约，不需要\"先猜用哪条产生式\"，左递归反而让栈更浅。LL 才怕左递归。",
        );
        quiz(
            ui,
            "ch2-lr-3",
            "SLR 分析表里的\"移进-归约冲突\"是什么意思？",
            &["表太大了", "同一个状态遇到同一个输入时，既可以移进也可以归约", "文法里有 ε 产生式"],
            1,
            "这时分析器不知道是该\"再拿一块积木\"还是\"现在就拼\"。SLR 用 FOLLOW 集判断何时归约，比较粗糙，有些无歧义的文法也会出冲突。",
        );

        ui.add_space(12.0);
        callout(ui, CalloutKind::Deeper, "SLR、LR(1)、LALR", |ui| {
            prose(
                ui,
                "SLR 只要下一个记号在 FOLLOW(A) 里就归约，但 FOLLOW 是\"全局\"的：A 在别处后面能跟的记号，这里不一定能跟。\
                 LR(1) 让每个项目自带\"向前看记号\"（`[A → α •, a]`：只有下一个是 a 才归约），判断精确得多，代价是状态数暴增。\
                 LALR(1) 把 LR(1) 里核心相同的状态合并回去，状态数和 LR(0) 一样多，能力几乎和 LR(1) 一样——yacc、bison 用的就是它。",
            );
            ui.label(
                "能力排序：LR(0) ⊂ SLR(1) ⊂ LALR(1) ⊂ LR(1)，而所有 LL(1) 文法都是 LR(1) 文法。\
                 不过手写编译器（GCC、Clang、rustc、Go）几乎都用递归下降：代码直观、错误信息好控制，优先级用 Pratt 分析法处理（2.4 课）。",
            );
        });
        ui.add_space(24.0);
    }

    fn automaton(&mut self, ui: &mut Ui, auto: &Automaton) {
        let p = Palette::of(ui);
        let g = &auto.g;
        self.build.ui(ui, auto.steps.len());
        let k = self.build.pos;
        let current = k.checked_sub(1).map(|i| &auto.steps[i]);

        let mut discovered = vec![false; auto.states.len()];
        let mut edges_known = std::collections::BTreeSet::new();
        for st in &auto.steps[..k] {
            match st {
                BuildStep::Start => discovered[0] = true,
                BuildStep::Goto { from, sym, to, .. } => {
                    discovered[*to] = true;
                    edges_known.insert((*from, *sym));
                }
            }
        }
        let focus_state = match current {
            Some(BuildStep::Start) => Some(0),
            Some(BuildStep::Goto { to, .. }) => Some(*to),
            None => None,
        };
        let explain = match current {
            None => "从初始项目 S' → • S 开始。".to_owned(),
            Some(BuildStep::Start) => "状态 0 = 闭包({ S' → • S })：• 后面是 S，把 S 的产生式都加进来；它们的 • 后面又是别的非终结符，继续加……".to_owned(),
            Some(BuildStep::Goto { from, sym, to, is_new }) => {
                let tail = if *is_new {
                    format!("得到一个新的项目集，编号为状态 {to}。")
                } else {
                    format!("得到的项目集和已有的状态 {to} 一样，不用新建。")
                };
                format!("从状态 {from} 读 `{}`：把 • 后面是 `{}` 的项目都右移一格，再求闭包——{tail}", g.name(*sym), g.name(*sym))
            }
        };
        prose_sized(ui, &explain, 17.0);

        let graph = Graph {
            nodes: auto
                .states
                .iter()
                .enumerate()
                .map(|(i, items)| GNode {
                    label: i.to_string(),
                    accepting: items.iter().any(|it| it.dot == g.productions[it.prod].rhs.len()),
                    note: None,
                })
                .collect(),
            edges: auto
                .goto
                .iter()
                .map(|(&(from, sym), &to)| GEdge { from, to, label: g.name(sym).to_owned() })
                .collect(),
            start: Some(0),
            show_start: true,
        };
        let edge_keys: Vec<(usize, Sym)> = auto.goto.keys().copied().collect();
        let edge_targets: Vec<usize> = auto.goto.values().copied().collect();
        // 聚焦：鼠标指向的状态（上一帧记下的），否则是这一步涉及的状态。与它无关的边淡化，图才看得清。
        let hover_id = ui.id().with("lr_hover");
        let prev_hover: Option<usize> = ui.data(|d| d.get_temp(hover_id)).flatten();
        let focus = prev_hover.filter(|&s| discovered[s]).or(focus_state);
        let mut hovered = None;
        let graph_ui = |ui: &mut Ui, hovered: &mut Option<usize>| {
            ui.label(
                RichText::new(format!("LR(0) 自动机（最终 {} 个状态；双圈表示含有可归约的项目）", auto.states.len()))
                    .strong(),
            );
            *hovered = graph_view(
                ui,
                "lr_automaton",
                &graph,
                |s| {
                    if !discovered[s] {
                        GNodeStyle::hidden()
                    } else if focus_state == Some(s) {
                        GNodeStyle::focus(&p)
                    } else {
                        GNodeStyle::normal(&p)
                    }
                },
                |e| {
                    let key = edge_keys[e];
                    if !edges_known.contains(&key) {
                        GEdgeStyle::hidden()
                    } else if matches!(current, Some(BuildStep::Goto { from, sym, .. }) if (*from, *sym) == key) {
                        GEdgeStyle::focus(&p)
                    } else if focus.is_some_and(|f| key.0 != f && edge_targets[e] != f) {
                        GEdgeStyle { color: p.node_stroke.gamma_multiply(0.25), ..GEdgeStyle::normal(&p) }
                    } else {
                        GEdgeStyle::normal(&p)
                    }
                },
            );
        };
        graph_ui(ui, &mut hovered);
        if hovered != prev_hover {
            ui.data_mut(|d| d.insert_temp(hover_id, hovered));
            ui.ctx().request_repaint();
        }
        let shown = hovered.filter(|&s| discovered[s]).or(focus_state);
        if let Some(s) = shown {
            card(ui, |ui| {
                let how = if hovered.is_some() { "（鼠标指向的状态）" } else { "" };
                ui.label(RichText::new(format!("状态 {s} 的项目{how}：粗体是核心项目，其余是闭包加进来的")).strong());
                ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                    for (i, it) in auto.states[s].iter().enumerate() {
                        let mut rt = RichText::new(auto.item_text(*it)).font(mono(15.0));
                        rt = if i < auto.kernel[s] { rt.strong() } else { rt.color(p.muted) };
                        ui.label(rt);
                    }
                });
            });
        } else {
            ui.label(RichText::new("鼠标指向任意一个状态，可以看到它的全部项目。").small().color(p.muted));
        }
    }

    fn parsing(&mut self, ui: &mut Ui, auto: &Automaton, table: &SlrTable) {
        let p = Palette::of(ui);
        let g = &auto.g;
        let toks = self.input.syms();
        let (steps, tree) = lr::parse(auto, table, &toks);
        self.run.ui(ui, steps.len());
        let k = self.run.pos;
        let input_text = |pos: usize| -> String {
            let mut parts: Vec<String> = (pos..toks.len()).map(|i| self.input.token_text(i).to_owned()).collect();
            parts.push("$".to_owned());
            parts.join(" ")
        };
        card(ui, |ui| {
            egui::ScrollArea::horizontal().id_salt("lr_run").show(ui, |ui| {
                egui::Grid::new("lr_run_grid").striped(true).spacing([18.0, 4.0]).show(ui, |ui| {
                    for h in ["状态栈", "符号栈", "剩余输入", "动作"] {
                        ui.label(RichText::new(h).strong());
                    }
                    ui.end_row();
                    for (i, st) in steps[..k].iter().enumerate() {
                        let current = i + 1 == k;
                        let hl = |rt: RichText| if current { rt.background_color(p.focus_bg) } else { rt };
                        let states: Vec<String> = st.states.iter().map(ToString::to_string).collect();
                        ui.label(hl(RichText::new(states.join(" ")).font(mono(14.0))));
                        let syms: Vec<&str> = st.syms.iter().map(|&s| g.name(s)).collect();
                        ui.label(hl(RichText::new(syms.join(" ")).font(mono(14.0))));
                        ui.label(hl(RichText::new(input_text(st.pos)).font(mono(14.0))));
                        let (act, color) = match st.action {
                            Some(LrAction::Shift(s)) => (format!("移进，转到状态 {s}"), p.text),
                            Some(LrAction::Reduce(pr)) => (format!("按 ({pr}) {} 归约", g.prod_text(pr)), p.accent),
                            Some(LrAction::Accept) => ("接受".to_owned(), p.ok),
                            None => ("出错".to_owned(), p.error),
                        };
                        ui.label(hl(RichText::new(act).color(color)));
                        ui.end_row();
                    }
                });
            });
        });
        if let Some(st) = k.checked_sub(1).map(|i| &steps[i]) {
            let s = *st.states.last().unwrap();
            let la = toks.get(st.pos).copied().unwrap_or(g.eof);
            let why = match st.action {
                Some(LrAction::Shift(t)) => {
                    format!("状态 {s} 遇到 `{}`：表里是 s{t}，把它移进栈，进入状态 {t}。", g.name(la))
                }
                Some(LrAction::Reduce(pr)) => {
                    let n = g.productions[pr].rhs.len();
                    format!(
                        "状态 {s} 遇到 `{}`：表里是 r{pr}。栈顶的 {n} 个符号正是 `{}` 的右部，把它们弹出、换成 `{}`，再查 GOTO 表决定新状态。",
                        g.name(la),
                        g.prod_text(pr),
                        g.name(g.productions[pr].lhs)
                    )
                }
                Some(LrAction::Accept) => "归约出了开始符号且输入已经结束：分析成功！".to_owned(),
                None => format!("状态 {s} 遇到 `{}`：表里这一格是空的，语法错误。", g.name(la)),
            };
            prose_sized(ui, &why, 17.0);
        }
        if k == steps.len()
            && let Some(tree) = &tree
        {
            ui.label(RichText::new("得到的语法树（每次归约建一个节点，所以是自底向上长出来的）").strong());
            let nodes = self.input.tree_nodes(g, tree);
            scrollable_tree(ui, "lr_tree", &nodes, |_| NodeStyle::normal(&p));
        }
    }
}

fn table_view(ui: &mut Ui, auto: &Automaton, table: &SlrTable) {
    let p = Palette::of(ui);
    let g = &auto.g;
    let terms: Vec<Sym> = g.terminals().chain([g.eof]).collect();
    let nts: Vec<Sym> = g.nonterminals().filter(|&n| n != g.start).collect();
    card(ui, |ui| {
        egui::ScrollArea::horizontal().id_salt("slr_table").show(ui, |ui| {
            egui::Grid::new("slr_grid").striped(true).spacing([12.0, 4.0]).show(ui, |ui| {
                ui.label(RichText::new("状态").strong());
                for &t in &terms {
                    ui.label(RichText::new(g.name(t)).font(mono(14.0)).color(p.number).strong());
                }
                for &n in &nts {
                    ui.label(RichText::new(g.name(n)).font(mono(14.0)).color(p.keyword).strong());
                }
                ui.end_row();
                for s in 0..auto.states.len() {
                    ui.label(RichText::new(s.to_string()).font(mono(14.0)).strong());
                    for &t in &terms {
                        let cell = table.action.get(&(s, t)).map_or(&[][..], Vec::as_slice);
                        let text: Vec<String> = cell.iter().map(|&a| action_text(a)).collect();
                        let mut rt = RichText::new(text.join("/")).font(mono(14.0));
                        if cell.len() > 1 {
                            rt = rt.background_color(p.error_bg).color(p.error);
                        } else if matches!(cell.first(), Some(LrAction::Accept)) {
                            rt = rt.color(p.ok);
                        }
                        ui.label(rt);
                    }
                    for &n in &nts {
                        let text = table.goto.get(&(s, n)).map_or(String::new(), ToString::to_string);
                        ui.label(RichText::new(text).font(mono(14.0)).color(p.muted));
                    }
                    ui.end_row();
                }
            });
        });
        ui.label(
            RichText::new("s几 = 移进并转到该状态；r几 = 按第几条产生式归约；acc = 接受；右侧灰色数字是 GOTO 表。")
                .small()
                .color(p.muted),
        );
    });
    ui.label(RichText::new("产生式编号：").small().color(p.muted));
    ui.horizontal_wrapped(|ui| {
        for i in 0..g.productions.len() - 1 {
            ui.label(RichText::new(format!("({i}) {}", g.prod_text(i))).font(mono(13.0)));
        }
    });
}
