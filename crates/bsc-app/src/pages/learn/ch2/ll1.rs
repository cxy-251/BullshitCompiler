//! 2.5 LL(1) 分析表。

use std::collections::BTreeSet;

use bsc_grammar::first_follow::{self, Kind};
use bsc_grammar::ll1::{self, Action, Table};
use bsc_grammar::{Grammar, Sym};
use eframe::egui::{self, RichText, Ui};

use crate::grammar_view::{GrammarInput, production_list};
use crate::theme::{Palette, mono};
use crate::widgets::{CalloutKind, NodeStyle, Stepper, callout, card, prose, prose_sized, quiz, scrollable_tree};

const PRESETS: &[(&str, &str, &str)] = &[
    (
        "表达式（已消除左递归）",
        "E -> T E'\nE' -> + T E' | ε\nT -> F T'\nT' -> * F T' | ε\nF -> ( E ) | id",
        "id + id * id",
    ),
    ("左递归（有冲突）", "E -> E + T | T\nT -> id", "id + id"),
    ("括号配对", "S -> ( S ) S | ε", "( ( ) ) ( )"),
    (
        "悬空 else（有冲突）",
        "S -> if E then S Else | other\nElse -> else S | ε\nE -> b",
        "if b then if b then other else other",
    ),
];

pub struct Lesson {
    input: GrammarInput,
    ff: Stepper,
    run: Stepper,
}

impl Default for Lesson {
    fn default() -> Self {
        let (_, g, s) = PRESETS[0];
        Self { input: GrammarInput::new(g, s), ff: Stepper::default(), run: Stepper::default() }
    }
}

impl Lesson {
    pub fn ui(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        ui.heading("2.5  LL(1) 分析表");
        ui.label(RichText::new("进阶 · 约 35 分钟 · 需要先学 2.1、2.3").color(p.muted));

        ui.add_space(8.0);
        ui.label(RichText::new("一句话").size(20.0).strong());
        ui.label(
            "自顶向下分析（比如递归下降）每一步都要回答一个问题：\"要展开非终结符 A 了，A 有好几条产生式，用哪条？\"\
             LL(1) 的回答是：只偷看下一个记号，查一张事先算好的表。这张表就是 LL(1) 分析表。",
        );
        callout(ui, CalloutKind::Analogy, "岔路口的路牌", |ui| {
            ui.label(
                "走到岔路口（非终结符有好几条产生式），看一眼前方的第一个景物（下一个记号），查路牌就知道该走哪条路。\
                 路牌要事先做好：每条路走下去\"第一眼会看到什么\"（FIRST 集）；如果某条路可以\"什么都不走\"（推出空串），\
                 那就得看\"走完这段之后会看到什么\"（FOLLOW 集）。",
            );
        });
        prose(ui, "LL(1) 这个名字：第一个 L 是从左往右读输入，第二个 L 是产生最左推导，1 是只向前看 1 个记号。");

        ui.add_space(8.0);
        ui.label(RichText::new("FIRST 与 FOLLOW").size(20.0).strong());
        card(ui, |ui| {
            egui::Grid::new("ff_defs").num_columns(2).striped(true).spacing([18.0, 8.0]).show(ui, |ui| {
                for (name, def) in [
                    ("可空", "非终结符 A 能不能推出空串 ε。"),
                    ("FIRST(α)", "从 α 推出的串里，可能出现在**第一个**位置的终结符。比如 FIRST(`F`) = { `(` `id` }。"),
                    (
                        "FOLLOW(A)",
                        "在所有句型里，可能**紧跟在 A 后面**的终结符。输入结束 `$` 也算，它跟在开始符号后面。",
                    ),
                ] {
                    ui.label(RichText::new(name).color(p.accent).strong());
                    prose(ui, def);
                    ui.end_row();
                }
            });
        });
        prose(
            ui,
            "三者都用\"不动点迭代\"计算：先都当成空集，反复扫描所有产生式、按规则往集合里加东西，直到某一轮什么都没加。\
             下面可以单步观察每一次\"加东西\"和它的理由。",
        );

        let (gc, sc) = self.input.editors(ui, PRESETS);
        if gc {
            self.ff.reset();
        }
        if gc || sc {
            self.run.reset();
        }
        if !self.input.show_errors(ui) {
            return;
        }
        let Ok(g) = &self.input.grammar else { return };
        let g = g.clone();

        ui.add_space(8.0);
        ui.label(RichText::new("单步计算 FIRST 与 FOLLOW").size(20.0).strong());
        let sets = first_follow::compute(&g);
        self.first_follow(ui, &g, &sets);

        ui.add_space(8.0);
        ui.label(RichText::new("填分析表").size(20.0).strong());
        card(ui, |ui| {
            for line in [
                "对每条产生式 A → α：",
                "    FIRST(α) 里的每个终结符 a：在 M[A, a] 填 A → α",
                "    如果 α 能推出 ε：FOLLOW(A) 里的每个终结符 b，在 M[A, b] 也填 A → α",
            ] {
                ui.label(RichText::new(line).font(mono(14.0)));
            }
        });
        let table = Table::build(&g, &sets);
        table_view(ui, &g, &table);
        let conflicts = table.conflicts();
        if conflicts.is_empty() {
            ui.label(RichText::new("没有冲突：每一格最多一条产生式，这是一个 LL(1) 文法。").color(p.ok));
        } else {
            ui.label(
                RichText::new(format!(
                    "有 {} 个格子填了不止一条产生式（红色）——只看一个记号没法决定走哪条路，这个文法不是 LL(1) 的。",
                    conflicts.len()
                ))
                .color(p.error),
            );
            prose(
                ui,
                "常见原因一：**左递归**。`E -> E + T | T` 的两条产生式都以 `T` 能打头的记号开头，永远分不开。\
                 解决办法是改写成 `E -> T E'`、`E' -> + T E' | ε`（第一个例子就是这样改写过的）。\
                 原因二：**公共前缀**，`A -> a b | a c` 要提取左因子，改成 `A -> a A'`、`A' -> b | c`。\
                 原因三：文法本身有歧义，比如\"悬空 else\"。",
            );
        }

        ui.add_space(8.0);
        ui.label(RichText::new("用分析表分析句子").size(20.0).strong());
        prose(
            ui,
            "用一个栈代替递归：一开始栈里是 `$` 和开始符号。栈顶是终结符，就和下一个输入比对、一起消掉；\
             栈顶是非终结符，就查表，把产生式右部**倒着**压进栈（这样最左边的符号在栈顶）。栈和输入同时只剩 `$` 时成功。",
        );
        self.parsing(ui, &g, &table);

        ui.add_space(12.0);
        ui.label(RichText::new("小测验").size(20.0).strong());
        quiz(
            ui,
            "ch2-ll1-1",
            "为什么 `E' -> ε` 这条产生式要填在 FOLLOW(E') 的那些列里？",
            &["规定如此", "选择\"什么都不产生\"时，下一个记号其实属于 E' 后面的东西", "为了避免冲突"],
            1,
            "E' 推出空串，相当于直接跳过它，那么接下来看到的记号必然是跟在 E' 后面的——也就是 FOLLOW(E') 里的。",
        );
        quiz(
            ui,
            "ch2-ll1-2",
            "左递归的文法为什么不能直接用 LL(1)（或递归下降）分析？",
            &["它有歧义", "展开 E 又得到 E 打头，不读任何记号就无限展开下去", "它的 FOLLOW 集太大"],
            1,
            "`E -> E + T`：要展开 E，先要展开右部第一个 E，而那又是 E……没有读入任何记号，死循环。表里则表现为冲突。",
        );

        ui.add_space(12.0);
        callout(ui, CalloutKind::Deeper, "递归下降就是手写的 LL(1)", |ui| {
            ui.label(
                "2.3 课的递归下降分析器，每个函数开头用 if/match 看下一个记号决定走哪个分支——那正是在查 LL(1) 表，只不过表\"写在了代码里\"。\
                 表驱动的写法把\"文法\"和\"分析程序\"彻底分开：换一个文法只需要换一张表，程序不用改。ANTLR 等工具就是这么生成分析器的\
                 （ANTLR 用的是更强的 LL(*)，可以向前看任意多个记号）。",
            );
            ui.label(
                "LL(k) 向前看 k 个记号，k 越大能处理的文法越多，但表会指数级变大。LL 文法的局限在于它必须\"一开始就猜对\"用哪条产生式；\
                 下一课的 LR 分析可以\"先读、后决定\"，能处理的文法多得多（包括左递归）。",
            );
        });
        ui.add_space(24.0);
    }

    fn first_follow(&mut self, ui: &mut Ui, g: &Grammar, sets: &first_follow::Sets) {
        let p = Palette::of(ui);
        self.ff.ui(ui, sets.steps.len());
        let k = self.ff.pos;
        let current = k.checked_sub(1).map(|i| &sets.steps[i]);

        // 回放前 k 步
        let n = g.num_symbols();
        let mut nullable = vec![false; n];
        let mut first: Vec<BTreeSet<Sym>> = vec![BTreeSet::new(); n];
        let mut follow: Vec<BTreeSet<Sym>> = vec![BTreeSet::new(); n];
        for st in &sets.steps[..k] {
            match st.kind {
                Kind::Nullable => nullable[st.target] = true,
                Kind::First => first[st.target].extend(st.added.iter().copied()),
                Kind::Follow => follow[st.target].extend(st.added.iter().copied()),
            }
        }

        let explain = match current {
            None => "开始时所有集合都是空的。".to_owned(),
            Some(st) => {
                let phase = match st.kind {
                    Kind::Nullable | Kind::First => "FIRST 阶段",
                    Kind::Follow => "FOLLOW 阶段",
                };
                let round = if st.round == 0 { "初始化".to_owned() } else { format!("第 {} 轮", st.round) };
                format!("[{phase} · {round}] {}", st.reason)
            }
        };
        prose_sized(ui, &explain, 17.0);
        if k == sets.steps.len() {
            ui.label(
                RichText::new(format!(
                    "计算完成。FIRST 用了 {} 轮、FOLLOW 用了 {} 轮——每个阶段的最后一轮什么都没加，说明到达了不动点。",
                    sets.first_rounds, sets.follow_rounds
                ))
                .color(p.ok),
            );
        }

        let cols = ui.available_width() > 820.0;
        let table = |ui: &mut Ui| {
            card(ui, |ui| {
                egui::Grid::new("ff_table").striped(true).spacing([18.0, 6.0]).show(ui, |ui| {
                    for h in ["非终结符", "可空", "FIRST", "FOLLOW"] {
                        ui.label(RichText::new(h).strong());
                    }
                    ui.end_row();
                    for nt in g.nonterminals() {
                        let target = current.filter(|st| st.target == nt);
                        ui.label(RichText::new(g.name(nt)).font(mono(15.0)).color(p.keyword));
                        let mut nl = RichText::new(if nullable[nt] { "是" } else { "" });
                        if target.is_some_and(|st| st.kind == Kind::Nullable) {
                            nl = nl.background_color(p.focus_bg);
                        }
                        ui.label(nl);
                        for (set, kind) in [(&first[nt], Kind::First), (&follow[nt], Kind::Follow)] {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 6.0;
                                let added: &[Sym] = match target {
                                    Some(st) if st.kind == kind => &st.added,
                                    _ => &[],
                                };
                                for &s in set {
                                    let mut rt = RichText::new(g.name(s)).font(mono(15.0));
                                    if added.contains(&s) {
                                        rt = rt.background_color(p.focus_bg).strong();
                                    }
                                    ui.label(rt);
                                }
                                if kind == Kind::First && nullable[nt] {
                                    ui.label(RichText::new("ε").font(mono(15.0)).color(p.muted));
                                }
                            });
                        }
                        ui.end_row();
                    }
                });
            });
        };
        let highlight: Vec<usize> = current.and_then(|st| st.prod).into_iter().collect();
        if cols {
            ui.columns(2, |c| {
                table(&mut c[0]);
                production_list(&mut c[1], g, &highlight);
            });
        } else {
            table(ui);
            production_list(ui, g, &highlight);
        }
    }

    fn parsing(&mut self, ui: &mut Ui, g: &Grammar, table: &Table) {
        let p = Palette::of(ui);
        let toks = self.input.syms();
        let (steps, tree) = ll1::parse(g, table, &toks);
        self.run.ui(ui, steps.len());
        let k = self.run.pos;

        let input_text = |pos: usize| -> String {
            let mut parts: Vec<String> = (pos..toks.len()).map(|i| self.input.token_text(i).to_owned()).collect();
            parts.push("$".to_owned());
            parts.join(" ")
        };
        card(ui, |ui| {
            egui::ScrollArea::horizontal().id_salt("ll1_run").show(ui, |ui| {
                egui::Grid::new("ll1_run_grid").striped(true).spacing([18.0, 4.0]).show(ui, |ui| {
                    for h in ["栈（右边是栈顶）", "剩余输入", "动作"] {
                        ui.label(RichText::new(h).strong());
                    }
                    ui.end_row();
                    for (i, st) in steps[..k].iter().enumerate() {
                        let current = i + 1 == k;
                        let hl = |rt: RichText| if current { rt.background_color(p.focus_bg) } else { rt };
                        ui.label(hl(RichText::new(g.seq_text(&st.stack)).font(mono(14.0))));
                        ui.label(hl(RichText::new(input_text(st.pos)).font(mono(14.0))));
                        let act = match &st.action {
                            Action::Match(t) => format!("匹配 {}", g.name(*t)),
                            Action::Expand(prod) => format!("展开 {}", g.prod_text(*prod)),
                            Action::Accept => "接受".to_owned(),
                            Action::Error { top, lookahead } => {
                                format!("出错：栈顶 {}、输入 {} 无法继续", g.name(*top), g.name(*lookahead))
                            }
                        };
                        let color = match st.action {
                            Action::Accept => p.ok,
                            Action::Error { .. } => p.error,
                            _ => p.text,
                        };
                        ui.label(hl(RichText::new(act).color(color)));
                        ui.end_row();
                    }
                });
            });
        });
        if let Some(st) = k.checked_sub(1).map(|i| &steps[i]) {
            let why = match &st.action {
                Action::Match(t) => format!("栈顶是终结符 `{}`，和下一个输入一样，一起消掉。", g.name(*t)),
                Action::Expand(prod) => {
                    let lhs = g.productions[*prod].lhs;
                    let la = toks.get(st.pos).copied().unwrap_or(g.eof);
                    format!(
                        "栈顶是非终结符 `{}`，下一个输入是 `{}`。查表 M[{}, {}] = `{}`，弹出 `{}`、把右部倒着压栈。",
                        g.name(lhs),
                        g.name(la),
                        g.name(lhs),
                        g.name(la),
                        g.prod_text(*prod),
                        g.name(lhs)
                    )
                }
                Action::Accept => "栈和输入都只剩 `$`：分析成功！".to_owned(),
                Action::Error { top, lookahead } if g.is_terminal(*top) => {
                    format!("栈顶是终结符 `{}`，输入却是 `{}`，对不上：语法错误。", g.name(*top), g.name(*lookahead))
                }
                Action::Error { top, lookahead } => {
                    format!(
                        "M[{}, {}] 是空的：没有产生式能以 `{}` 开头，语法错误。",
                        g.name(*top),
                        g.name(*lookahead),
                        g.name(*lookahead)
                    )
                }
            };
            prose_sized(ui, &why, 17.0);
        }
        if k == steps.len()
            && let Some(tree) = &tree
        {
            ui.label(RichText::new("得到的语法树（展开的顺序就是这棵树的前序遍历）").strong());
            let nodes = self.input.tree_nodes(g, tree);
            scrollable_tree(ui, "ll1_tree", &nodes, |_| NodeStyle::normal(&p));
        }
    }
}

fn table_view(ui: &mut Ui, g: &Grammar, table: &Table) {
    let p = Palette::of(ui);
    let terms: Vec<Sym> = g.terminals().chain([g.eof]).collect();
    card(ui, |ui| {
        egui::ScrollArea::horizontal().id_salt("ll1_table").show(ui, |ui| {
            egui::Grid::new("ll1_table_grid").striped(true).spacing([14.0, 6.0]).show(ui, |ui| {
                ui.label("");
                for &t in &terms {
                    ui.label(RichText::new(g.name(t)).font(mono(15.0)).color(p.number).strong());
                }
                ui.end_row();
                for nt in g.nonterminals() {
                    ui.label(RichText::new(g.name(nt)).font(mono(15.0)).color(p.keyword).strong());
                    for &t in &terms {
                        let cell = table.get(nt, t);
                        let text: Vec<String> = cell.iter().map(|&pr| g.prod_text(pr)).collect();
                        let mut rt = RichText::new(text.join("\n")).font(mono(13.0));
                        if cell.len() > 1 {
                            rt = rt.background_color(p.error_bg).color(p.error);
                        }
                        ui.label(rt);
                    }
                    ui.end_row();
                }
            });
        });
    });
}
