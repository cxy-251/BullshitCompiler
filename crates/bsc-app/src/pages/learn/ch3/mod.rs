//! 第 3 章 · 语义分析：检查意思对不对。

pub mod infer;
pub mod scope;
pub mod typeck;

use bsc_core::{Diagnostic, Span};
use bsc_minilang::lexer::{Token, lex};
use bsc_minilang::parser::{ParseOutput, parse_program};
use bsc_minilang::sema::{Analysis, analyze};
use eframe::egui::{self, RichText, TextEdit, Ui};

use crate::pages::learn::ch1::scanner::class_color;
use crate::theme::{Palette, mono};
use crate::widgets::{card, diagnostic_view};

/// 第 3 章各课的状态。
#[derive(Default)]
pub struct Chapter {
    pub scope: scope::Lesson,
    pub typeck: typeck::Lesson,
    pub infer: infer::Lesson,
}

/// 3.1、3.2 共用：mini-lang 前端（词法 → 语法 → 语义）跑一遍的结果。
pub struct Front {
    pub src: String,
    pub tokens: Vec<Token>,
    pub parse: ParseOutput,
    /// 词法、语法都没有错误时才做语义分析。
    pub sema: Option<Analysis>,
    pub syntax_errors: Vec<Diagnostic>,
}

impl Front {
    pub fn new(src: &str) -> Self {
        let lexed = lex(src);
        let parse = parse_program(src, &lexed.tokens);
        let mut syntax_errors = lexed.errors;
        syntax_errors.extend(parse.errors.iter().cloned());
        let sema = syntax_errors.is_empty().then(|| analyze(src, &parse.ast, parse.root));
        Self { src: src.to_owned(), tokens: lexed.tokens, parse, sema, syntax_errors }
    }

    /// 源码里记号的着色。
    pub fn colors(&self, p: &Palette) -> Vec<(Span, eframe::egui::Color32)> {
        self.tokens.iter().map(|t| (t.span, class_color(p, t.kind.class()))).collect()
    }

    /// 节点范围里名字第一次出现的位置（调用、赋值节点只高亮名字那一段）。
    pub fn name_span(&self, node_span: Span, name: &str) -> Span {
        match node_span.text(&self.src).find(name) {
            Some(i) => Span::new(node_span.start + i, node_span.start + i + name.len()),
            None => node_span,
        }
    }
}

/// 例子按钮 + 代码编辑框。返回源码是否被改动。
///
/// `folded`：编辑框默认收起（程序较长时，下面单步演示的源码视图已经显示了全部代码，不必重复）。
pub fn editor(ui: &mut Ui, src: &mut String, presets: &[(&str, &str)], rows: usize, folded: bool) -> bool {
    let p = Palette::of(ui);
    card(ui, |ui| {
        let mut changed = false;
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("例子：").color(p.muted));
            for (name, code) in presets {
                if ui.button(*name).clicked() {
                    *src = (*code).to_owned();
                    changed = true;
                }
            }
        });
        let mut edit = |ui: &mut Ui| {
            ui.add(TextEdit::multiline(src).font(mono(16.0)).desired_rows(rows).desired_width(f32::INFINITY)).changed()
        };
        if folded {
            changed |= egui::CollapsingHeader::new("自己动手改代码")
                .id_salt(("ch3_editor", presets[0].0))
                .show(ui, |ui| edit(ui))
                .body_returned
                .unwrap_or(false);
        } else {
            changed |= edit(ui);
        }
        changed
    })
}

/// 有语法错误时，语义分析不做，先把语法错误列出来。
pub fn syntax_errors(ui: &mut Ui, front: &Front) {
    let p = Palette::of(ui);
    ui.label(RichText::new("程序还有词法或语法错误，先改正它们才能做语义分析：").color(p.error).strong());
    for d in &front.syntax_errors {
        diagnostic_view(ui, &front.src, d);
    }
}
