//! 2.1 文法与推导。

use bsc_grammar::derive::{generate, leftmost};
use bsc_grammar::earley;
use eframe::egui::{self, RichText, Ui};

use crate::grammar_view::{GrammarInput, production_list};
use crate::theme::{Palette, mono};
use crate::widgets::{CalloutKind, NodeStyle, Stepper, callout, card, chip, prose, prose_sized, quiz, scrollable_tree};

const PRESETS: &[(&str, &str, &str)] = &[
    (
        "中文造句",
        "句子 -> 主语 谓语 宾语\n主语 -> 我 | 老板 | 领导\n谓语 -> 喜欢 | 赋能 | 对齐\n宾语 -> 咖啡 | 抓手 | 闭环",
        "领导 赋能 抓手",
    ),
    ("算术表达式", "E -> E + T | T\nT -> T * F | F\nF -> ( E ) | num", "1 + 2 * 3"),
    ("括号配对", "S -> ( S ) S | ε", "( ( ) ) ( )"),
    ("赋值语句", "语句 -> id = 表达式 ;\n表达式 -> 表达式 + id | id | num", "x = y + z ;"),
];

pub struct Lesson {
    input: GrammarInput,
    stepper: Stepper,
    samples: Vec<String>,
    seed: u64,
}

impl Default for Lesson {
    fn default() -> Self {
        let (_, g, s) = PRESETS[0];
        Self { input: GrammarInput::new(g, s), stepper: Stepper::default(), samples: vec![], seed: 1 }
    }
}

impl Lesson {
    pub fn ui(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        ui.heading("2.1  文法与推导");
        ui.label(RichText::new("入门 · 约 20 分钟").color(p.muted));

        ui.add_space(8.0);
        ui.label(RichText::new("一句话").size(20.0).strong());
        ui.label(
            "词法分析把源码切成了一个个记号，但记号之间是什么关系——哪几个记号组成一个表达式、表达式又组成什么语句——\
             需要另一套规则来描述，这就是\"文法\"。语法分析器按文法把记号序列组织成一棵语法树。",
        );
        callout(ui, CalloutKind::Analogy, "造句规则", |ui| {
            ui.label(
                "小学学造句：\"句子 = 主语 + 谓语 + 宾语\"，\"主语可以是'我'或'老板'\"……几条规则就能造出很多句子，\
                 也能判断一句话合不合规则。编程语言的文法是一模一样的东西，只不过写得更严格。",
            );
        });

        ui.add_space(8.0);
        ui.label(RichText::new("四个概念").size(20.0).strong());
        card(ui, |ui| {
            egui::Grid::new("cfg_terms").num_columns(2).striped(true).spacing([18.0, 8.0]).show(ui, |ui| {
                for (term, meaning) in [
                    ("终结符", "最终出现在句子里的\"词\"，也就是词法分析产出的记号，比如 `num` `+` `咖啡`。"),
                    (
                        "非终结符",
                        "代表\"一类结构\"的名字，比如 `表达式` `主语`。它不会出现在最终的句子里，要被继续替换。",
                    ),
                    ("产生式", "替换规则 `A → α`：非终结符 A 可以换成符号串 α。同一个 A 可以有好几条，用 `|` 隔开。"),
                    ("开始符号", "一切从它开始。通常是第一条规则左边的那个非终结符，比如 `句子`。"),
                ] {
                    ui.label(RichText::new(term).color(p.accent).strong());
                    prose(ui, meaning);
                    ui.end_row();
                }
            });
        });
        prose(ui, "这种每条规则左边只有一个非终结符的文法叫\"上下文无关文法\"（CFG）：替换 A 时不用管它左右是什么。");

        ui.add_space(8.0);
        ui.label(RichText::new("推导：从开始符号一步步替换出句子").size(20.0).strong());
        prose(
            ui,
            "从开始符号出发，每一步选一个非终结符、用它的某条产生式替换，直到只剩终结符——这个过程叫\"推导\"。\
             每一步得到的符号串叫\"句型\"。如果总是替换**最左边**的非终结符，叫\"最左推导\"。\
             推导的过程画出来，就是一棵语法树：被替换的非终结符是父节点，替换成的符号是它的孩子。",
        );

        let (g_changed, s_changed) = self.input.editors(ui, PRESETS);
        if g_changed || s_changed {
            self.stepper.reset();
            self.samples.clear();
        }
        if self.input.show_errors(ui) {
            self.derivation(ui);
            ui.add_space(8.0);
            ui.label(RichText::new("反过来：用文法造句").size(20.0).strong());
            ui.label("随机选产生式替换，就能\"生成\"符合文法的句子。点几次看看：");
            self.generation(ui);
        }

        ui.add_space(12.0);
        ui.label(RichText::new("小测验").size(20.0).strong());
        quiz(
            ui,
            "ch2-cfg-1",
            "在\"中文造句\"文法里，下面哪个是非终结符？",
            &["咖啡", "谓语", "领导"],
            1,
            "`谓语` 出现在箭头左边，它还要被替换成 `喜欢`、`赋能` 这样的词；咖啡、领导是最终出现在句子里的终结符。",
        );
        quiz(
            ui,
            "ch2-cfg-2",
            "用文法 `S -> ( S ) S | ε` 能推导出 `( ( )`吗？",
            &["能", "不能"],
            1,
            "这个文法每产生一个 `(` 就同时产生一个配对的 `)`，所以括号总是配对的。`( ( )` 少一个右括号，推不出来。",
        );

        ui.add_space(12.0);
        callout(ui, CalloutKind::Deeper, "文法的\"能力等级\"", |ui| {
            prose(
                ui,
                "1.2 课说过，正则表达式描述不了\"括号配对\"。上下文无关文法可以——上面的 `S -> ( S ) S | ε` 就是。\
                 语言学家乔姆斯基把文法按能力分成四级：正则文法 ⊂ 上下文无关文法 ⊂ 上下文有关文法 ⊂ 无限制文法。\
                 编译器的词法分析用正则（第一级），语法分析用上下文无关文法（第二级）。",
            );
            ui.label(
                "一个文法能推导出的所有句子的集合，叫做它描述的\"语言\"。判断一个句子是否属于这个语言、并给出语法树，\
                 就是语法分析器的工作——本课的演示背后用的是 Earley 算法，它能处理任何上下文无关文法；\
                 后面几课会讲编译器里更常用、也更快的专门方法。",
            );
        });
        ui.add_space(24.0);
    }

    fn derivation(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        let Ok(g) = &self.input.grammar else { return };
        let toks = self.input.syms();
        let result = earley::parse(g, &toks, 1);
        let Some(tree) = result.trees.first() else {
            let at = result.fail_at.unwrap_or(0);
            let msg = if at >= toks.len() {
                "句子还没完整就结束了。".to_owned()
            } else {
                format!("推导不出这个句子：从第 {} 个记号 `{}` 开始就接不上了。", at + 1, self.input.token_text(at))
            };
            ui.label(RichText::new(format!("这个句子不符合文法。{msg}")).color(p.error));
            return;
        };

        let steps = leftmost(tree);
        self.stepper.ui(ui, steps.len() - 1);
        let k = self.stepper.pos;
        let flat = tree.flatten();
        let nodes = self.input.tree_nodes(g, tree);

        // 句型列表：显示到第 k 步，最新一步里刚换上的部分高亮
        card(ui, |ui| {
            ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                for (i, st) in steps[..=k].iter().enumerate() {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(if i == 0 { "   " } else { "⇒ " }).font(mono(16.0)).color(p.muted));
                        let next_at = steps.get(i + 1).filter(|_| i == k).map(|s| s.at);
                        for (j, &(sym, node)) in st.form.iter().enumerate() {
                            let terminal = g.is_terminal(sym);
                            let text = if terminal { nodes[node].label.clone() } else { g.name(sym).to_owned() };
                            let mut rt = RichText::new(text).font(mono(16.0));
                            rt = if terminal { rt.color(p.number) } else { rt.color(p.keyword) };
                            let fresh = i == k
                                && i > 0
                                && j >= st.at
                                && j < st.at + (st.form.len() + 1 - steps[i - 1].form.len());
                            if fresh {
                                rt = rt.background_color(p.focus_bg);
                            }
                            if next_at == Some(j) {
                                rt = rt.underline();
                            }
                            ui.label(rt);
                        }
                    });
                }
            });
        });
        let explain = match steps[k].expanded {
            None => format!("第 0 步：从开始符号 `{}` 出发。", g.name(g.start)),
            Some((_, prod)) => {
                format!("用产生式 `{}` 替换最左边的 `{}`。", g.prod_text(prod), g.name(g.productions[prod].lhs))
            }
        };
        prose_sized(ui, &explain, 17.0);
        if k + 1 < steps.len() {
            ui.label(RichText::new("下划线标出的是下一步要替换的非终结符。").small().color(p.muted));
        } else {
            ui.label(RichText::new("只剩终结符了——推导完成，得到的正是输入的句子。").color(p.ok));
        }

        // 语法树：已展开的节点和它们的孩子可见
        let mut visible = vec![false; flat.len()];
        visible[0] = true;
        for st in &steps[1..=k] {
            if let Some((node, _)) = st.expanded {
                for &c in &flat[node].1 {
                    visible[c] = true;
                }
            }
        }
        let current = steps[k].expanded.map(|(n, _)| n);
        let highlight: Vec<usize> = steps[k].expanded.map(|(_, prod)| prod).into_iter().collect();
        let cols = ui.available_width() > 820.0;
        let tree_ui = |ui: &mut Ui| {
            ui.label(RichText::new("语法树").strong());
            scrollable_tree(ui, "cfg_tree", &nodes, |i| {
                if !visible[i] {
                    return NodeStyle::hidden();
                }
                let mut s = NodeStyle::normal(&p);
                if current == Some(i) {
                    s.fill = p.focus_bg;
                    s.stroke = p.accent;
                    s.emphasized = true;
                }
                s
            });
        };
        let prods_ui = |ui: &mut Ui| {
            ui.label(RichText::new("产生式").strong());
            production_list(ui, g, &highlight);
        };
        if cols {
            ui.columns(2, |c| {
                tree_ui(&mut c[0]);
                prods_ui(&mut c[1]);
            });
        } else {
            tree_ui(ui);
            prods_ui(ui);
        }
    }

    fn generation(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        let Ok(g) = &self.input.grammar else { return };
        if ui.button("随机造 5 个句子").clicked() {
            self.samples.clear();
            for _ in 0..5 {
                self.seed += 1;
                if let Some(s) = generate(g, self.seed, 12) {
                    let text = if s.is_empty() { "（空串）".to_owned() } else { g.seq_text(&s) };
                    self.samples.push(text);
                }
            }
            if self.samples.is_empty() {
                self.samples.push("这个文法从开始符号推不出任何只含终结符的句子。".to_owned());
            }
        }
        ui.horizontal_wrapped(|ui| {
            for s in &self.samples {
                chip(ui, RichText::new(s).font(mono(15.0)), p.card_bg, egui::Stroke::new(1.0, p.card_stroke));
            }
        });
    }
}
