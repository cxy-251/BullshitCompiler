//! 2.3、2.4 共用：回放 mini-lang 语法分析器记录的事件。

use bsc_core::Span;
use bsc_minilang::ast::{Ast, NodeId};
use bsc_minilang::lexer::Token;
use bsc_minilang::parser::{Event, ParseOutput, Rule};
use eframe::egui::{Color32, RichText, Ui};

use crate::pages::learn::ch1::scanner::class_color;
use crate::theme::{Palette, mono};
use crate::widgets::{Mark, NodeStyle, TreeNode, card, scrollable_tree, source_view};

/// 回放到第 k 个事件时的状态。
pub struct Replay {
    pub stack: Vec<Rule>,
    /// 下一个要看的记号。
    pub next_token: usize,
    pub built: Vec<bool>,
    pub last: Option<Event>,
}

pub fn replay(out: &ParseOutput, k: usize) -> Replay {
    let mut r = Replay { stack: vec![], next_token: 0, built: vec![false; out.ast.nodes.len()], last: None };
    for e in &out.events[..k] {
        match e {
            Event::Enter(rule) => r.stack.push(*rule),
            Event::Exit(_) => {
                r.stack.pop();
            }
            Event::Consume(i) => r.next_token = i + 1,
            Event::Build(n) => r.built[n.index()] = true,
            Event::Recover { to, .. } => r.next_token = *to,
            Event::Pratt { .. } | Event::Error(_) => {}
        }
    }
    r.last = k.checked_sub(1).map(|i| out.events[i].clone());
    r
}

/// 源码：已读过的记号按种类着色，下一个记号高亮；`extra` 是额外的高亮。
pub fn source(ui: &mut Ui, src: &str, tokens: &[Token], r: &Replay, extra: &[Mark]) {
    let p = Palette::of(ui);
    let colors: Vec<(Span, Color32)> =
        tokens[..r.next_token.min(tokens.len())].iter().map(|t| (t.span, class_color(&p, t.kind.class()))).collect();
    let mut marks = vec![Mark { span: tokens[r.next_token.min(tokens.len() - 1)].span, bg: p.done_bg }];
    marks.extend_from_slice(extra);
    card(ui, |ui| {
        source_view(ui, src, &colors, &marks, 17.0);
        ui.label(RichText::new("着色的是已经读过的记号，淡蓝底色的是下一个要看的记号。").small().color(p.muted));
    });
}

/// 调用栈：最下面是正在执行的函数。
pub fn call_stack(ui: &mut Ui, r: &Replay) {
    let p = Palette::of(ui);
    ui.label(RichText::new("调用栈（最下面是正在执行的函数）").strong());
    card(ui, |ui| {
        if r.stack.is_empty() {
            ui.label(RichText::new("（空）").color(p.muted));
        }
        for (depth, rule) in r.stack.iter().enumerate() {
            let current = depth + 1 == r.stack.len();
            let name = rule.name();
            let name = if name.contains('(') { name } else { format!("{name}()") };
            let mut t = RichText::new(name).font(mono(15.0));
            if current {
                t = t.background_color(p.focus_bg).strong();
            }
            ui.horizontal(|ui| {
                ui.add_space(depth as f32 * 14.0);
                ui.label(t);
                if current {
                    ui.label(RichText::new(rule.meaning()).small().color(p.muted));
                }
            });
        }
    });
}

pub fn ast_nodes(ast: &Ast) -> Vec<TreeNode> {
    (0..ast.nodes.len())
        .map(|i| {
            let id = NodeId(i as u32);
            TreeNode { label: ast.label(id), children: ast.node(id).children.iter().map(|c| c.index()).collect() }
        })
        .collect()
}

/// 已经建好的语法树节点（森林），刚建的那个高亮。
pub fn partial_tree(ui: &mut Ui, id: &str, ast: &Ast, r: &Replay) {
    let p = Palette::of(ui);
    let just = match r.last {
        Some(Event::Build(n)) => Some(n.index()),
        _ => None,
    };
    let nodes = ast_nodes(ast);
    scrollable_tree(ui, id, &nodes, |i| {
        if !r.built[i] {
            return NodeStyle::hidden();
        }
        let mut s = NodeStyle::normal(&p);
        if matches!(ast.nodes[i].kind, bsc_minilang::ast::NodeKind::Error) {
            s.fill = p.error_bg;
            s.stroke = p.error;
        }
        if just == Some(i) {
            s.fill = p.focus_bg;
            s.stroke = p.accent;
            s.emphasized = true;
        }
        s
    });
}
