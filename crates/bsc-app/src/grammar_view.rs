//! 第 2 章共用：文法编辑器、句子输入、产生式列表、语法树转换。

use bsc_core::{Diagnostic, Span};
use bsc_grammar::{Grammar, Sym, Tree};
use eframe::egui::{self, RichText, TextEdit, Ui};

use crate::theme::{Palette, mono};
use crate::widgets::{TreeNode, card, diagnostic_view};

/// 文法 + 一个输入句子，以及切分结果。
pub struct GrammarInput {
    pub grammar_src: String,
    pub grammar: Result<Grammar, Diagnostic>,
    pub sentence: String,
    pub tokens: Result<Vec<(Sym, Span)>, Diagnostic>,
}

impl GrammarInput {
    pub fn new(grammar_src: &str, sentence: &str) -> Self {
        let grammar = Grammar::parse(grammar_src);
        let tokens = tokenize(&grammar, sentence);
        Self { grammar_src: grammar_src.to_owned(), grammar, sentence: sentence.to_owned(), tokens }
    }

    pub fn reparse(&mut self) {
        self.grammar = Grammar::parse(&self.grammar_src);
        self.retokenize();
    }

    pub fn retokenize(&mut self) {
        self.tokens = tokenize(&self.grammar, &self.sentence);
    }

    pub fn syms(&self) -> Vec<Sym> {
        self.tokens.as_ref().map(|t| t.iter().map(|&(s, _)| s).collect()).unwrap_or_default()
    }

    /// 第 i 个记号在句子里的原文。
    pub fn token_text(&self, i: usize) -> &str {
        match &self.tokens {
            Ok(t) => t.get(i).map_or("", |&(_, sp)| sp.text(&self.sentence)),
            Err(_) => "",
        }
    }

    /// 文法编辑器 + 句子输入。返回 (文法是否改变, 句子是否改变)。
    pub fn editors(&mut self, ui: &mut Ui, grammar_presets: &[(&str, &str, &str)]) -> (bool, bool) {
        let p = Palette::of(ui);
        let mut g_changed = false;
        let mut s_changed = false;
        card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("例子：").color(p.muted));
                for (name, g, s) in grammar_presets {
                    if ui.button(*name).clicked() {
                        self.grammar_src = (*g).to_owned();
                        self.sentence = (*s).to_owned();
                        g_changed = true;
                        s_changed = true;
                    }
                }
            });
            ui.label(
                RichText::new("文法（箭头左边的是非终结符，其余是终结符；第一条规则的左部是开始符号）")
                    .small()
                    .color(p.muted),
            );
            g_changed |= ui
                .add(
                    TextEdit::multiline(&mut self.grammar_src)
                        .font(mono(16.0))
                        .desired_rows(3)
                        .desired_width(f32::INFINITY),
                )
                .changed();
            ui.horizontal(|ui| {
                ui.label("句子：");
                s_changed |= ui
                    .add(TextEdit::singleline(&mut self.sentence).font(mono(18.0)).desired_width(f32::INFINITY))
                    .changed();
            });
            ui.label(
                RichText::new(
                    "句子里的数字会自动当作终结符 num、其他名字当作 id（如果文法里有的话）；中文词之间请用空格隔开。",
                )
                .small()
                .color(p.muted),
            );
        });
        if g_changed {
            self.reparse();
        } else if s_changed {
            self.retokenize();
        }
        (g_changed, s_changed)
    }

    /// 显示文法或句子的错误。返回是否一切正常。
    pub fn show_errors(&self, ui: &mut Ui) -> bool {
        match (&self.grammar, &self.tokens) {
            (Err(d), _) => {
                ui.label("文法写得有问题：");
                diagnostic_view(ui, &self.grammar_src, d);
                false
            }
            (Ok(_), Err(d)) => {
                ui.label("句子里有文法不认识的符号：");
                diagnostic_view(ui, &self.sentence, d);
                false
            }
            _ => true,
        }
    }

    /// 语法树 → 画树用的节点。终结符叶子显示句子里的原文。
    pub fn tree_nodes(&self, g: &Grammar, tree: &Tree) -> Vec<TreeNode> {
        tree.flatten()
            .into_iter()
            .map(|(t, kids)| {
                let label = if g.is_terminal(t.sym) {
                    let text = self.token_text(t.start);
                    if text.is_empty() || text == g.name(t.sym) { g.name(t.sym).to_owned() } else { text.to_owned() }
                } else if t.children.is_empty() {
                    format!("{} (ε)", g.name(t.sym))
                } else {
                    g.name(t.sym).to_owned()
                };
                TreeNode { label, children: kids }
            })
            .collect()
    }
}

fn tokenize(g: &Result<Grammar, Diagnostic>, s: &str) -> Result<Vec<(Sym, Span)>, Diagnostic> {
    match g {
        Ok(g) => g.tokenize(s),
        Err(_) => Ok(vec![]),
    }
}

/// 列出所有产生式，带编号；`highlight` 里的产生式高亮。
pub fn production_list(ui: &mut Ui, g: &Grammar, highlight: &[usize]) {
    let p = Palette::of(ui);
    card(ui, |ui| {
        ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
            for i in 0..g.productions.len() {
                let mut t = RichText::new(format!("({i}) {}", g.prod_text(i))).font(mono(15.0));
                if highlight.contains(&i) {
                    t = t.background_color(p.focus_bg).strong();
                }
                ui.label(t);
            }
        });
    });
}
