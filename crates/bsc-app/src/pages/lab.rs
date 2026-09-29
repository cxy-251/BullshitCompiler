//! 「实验台」：类似 Compiler Explorer 的多栏联动视图。
//!
//! 所有阶段的产物同时摆出来；鼠标指向任意一个记号、语法树节点或指令，
//! 源码里对应的部分就会高亮。

use bsc_calc::{Compilation, Span};
use eframe::egui::{self, RichText, ScrollArea, TextEdit, Ui};

use crate::calc_view::{ast_tree, instr_list, token_chip};
use crate::theme::{Palette, mono};
use crate::widgets::{Mark, NodeStyle, card, diagnostic_view, scrollable_tree, source_view};

pub struct LabPage {
    pub input: String,
    comp: Compilation,
    /// 上一帧鼠标指向的源码区间（先画源码、后画各栏，所以用上一帧的结果）。
    hover: Option<Span>,
}

impl Default for LabPage {
    fn default() -> Self {
        Self::with_input("(1 + 2) * 3 - 4 / 2".to_owned())
    }
}

impl LabPage {
    pub fn with_input(input: String) -> Self {
        Self { comp: Compilation::new(&input), input, hover: None }
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        egui::CentralPanel::default().show(ui, |ui| {
            ScrollArea::vertical().show(ui, |ui| {
                ui.heading("实验台");
                ui.label(
                    RichText::new("当前支持\"计算器语言\"。带变量、函数、循环的 mini-lang 会随着课程推进加入。鼠标指向下面的任意记号、语法树节点或指令，源码中对应的部分会高亮。")
                        .color(p.muted),
                );

                let edit = TextEdit::singleline(&mut self.input).font(mono(20.0)).desired_width(f32::INFINITY);
                if ui.add(edit).changed() {
                    self.comp = Compilation::new(&self.input);
                    self.hover = None;
                }

                let colors: Vec<_> = self
                    .comp
                    .tokens
                    .as_deref()
                    .unwrap_or(&[])
                    .iter()
                    .map(|t| (t.span, p.token(t.kind.class())))
                    .collect();
                let marks: Vec<Mark> = self.hover.iter().map(|&span| Mark { span, bg: p.focus_bg }).collect();
                card(ui, |ui| source_view(ui, &self.comp.source, &colors, &marks, 22.0));

                if let Some((stage, d)) = self.comp.error() {
                    ui.label(RichText::new(format!("{}阶段出错：", stage.name())).color(p.error));
                    diagnostic_view(ui, &self.comp.source, d);
                }

                let comp = &self.comp;
                ui.label(RichText::new("记号").strong());
                let mut hover = tokens_panel(ui, comp);
                if ui.available_width() > 800.0 {
                    ui.columns(2, |cols| {
                        cols[0].label(RichText::new("语法树").strong());
                        hover = hover.or(tree_panel(&mut cols[0], comp));
                        cols[1].label(RichText::new("指令").strong());
                        hover = hover.or(code_panel(&mut cols[1], comp));
                    });
                } else {
                    ui.label(RichText::new("语法树").strong());
                    hover = hover.or(tree_panel(ui, comp));
                    ui.label(RichText::new("指令").strong());
                    hover = hover.or(code_panel(ui, comp));
                }

                ui.label(RichText::new("结果").strong());
                match comp.value() {
                    Some(v) => ui.label(RichText::new(v.to_string()).font(mono(24.0)).color(p.ok)),
                    None => ui.label(RichText::new("（无）").color(p.muted)),
                };

                if hover != self.hover {
                    self.hover = hover;
                    ui.ctx().request_repaint();
                }
            });
        });
    }
}

fn tokens_panel(ui: &mut Ui, comp: &Compilation) -> Option<Span> {
    let mut hover = None;
    match &comp.tokens {
        Ok(tokens) => {
            ui.horizontal_wrapped(|ui| {
                for t in tokens {
                    if token_chip(ui, t, false).hovered() {
                        hover = Some(t.span);
                    }
                }
            });
        }
        Err(_) => {
            ui.label("（词法分析失败）");
        }
    }
    hover
}

fn tree_panel(ui: &mut Ui, comp: &Compilation) -> Option<Span> {
    let p = Palette::of(ui);
    let parse = comp.parse.as_ref()?;
    if parse.root.is_err() {
        ui.label(RichText::new("（语法分析失败，下面是出错前建好的部分）").color(p.muted));
    }
    let hovered = scrollable_tree(ui, "lab_tree", &ast_tree(&parse.ast), |_| NodeStyle::normal(&p));
    if let Ok(root) = parse.root {
        ui.label(RichText::new(parse.ast.sexpr(root)).font(mono(13.0)).color(p.muted));
    }
    hovered.map(|i| parse.ast.nodes[i].span)
}

fn code_panel(ui: &mut Ui, comp: &Compilation) -> Option<Span> {
    let (Some(code), Some(parse)) = (&comp.code, &comp.parse) else {
        ui.label("（没有生成指令）");
        return None;
    };
    let hovered = instr_list(ui, code, code.len(), None, None);
    ui.label(RichText::new(format!("共 {} 条指令", code.len())).small());
    hovered.map(|i| parse.ast.node(code[i].node).span)
}
