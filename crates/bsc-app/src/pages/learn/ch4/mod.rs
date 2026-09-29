//! 第 4 章 · 中间表示：编译器的内部语言。

pub mod cfg;
pub mod dom;
pub mod ssa;
pub mod tac;

use bsc_minilang::cfg::{Cfg, EdgeKind, build};
use bsc_minilang::ir::{Lowered, lower};
use eframe::egui::{RichText, Stroke, Ui};

use super::ch3::Front;
use crate::theme::{Palette, mono};
use crate::widgets::{GEdge, GNode, Graph, chip_button};

/// 第 4 章各课的状态。
#[derive(Default)]
pub struct Chapter {
    pub tac: tac::Lesson,
    pub cfg: cfg::Lesson,
    pub dom: dom::Lesson,
    pub ssa: ssa::Lesson,
}

pub const PRESETS: &[(&str, &str)] = &[
    (
        "求和循环",
        "fn main() {\n    print(sum(10));\n}\n\nfn sum(n: int) -> int {\n    let mut s = 0;\n    let mut i = 1;\n    while i <= n {\n        s = s + i;\n        i = i + 1;\n    }\n    return s;\n}",
    ),
    (
        "三个数的最大值",
        "fn main() {\n    print(max3(4, 9, 7));\n}\n\nfn max3(a: int, b: int, c: int) -> int {\n    let mut m = a;\n    if b > m {\n        m = b;\n    }\n    if c > m {\n        m = c;\n    }\n    return m;\n}",
    ),
    (
        "循环 + 短路",
        "fn main() {\n    print(count(20));\n}\n\nfn count(n: int) -> int {\n    let mut c = 0;\n    let mut i = 1;\n    while i <= n {\n        if i % 3 == 0 || i % 5 == 0 {\n            c = c + 1;\n        }\n        i = i + 1;\n    }\n    return c;\n}",
    ),
    (
        "不可达代码",
        "fn main() {\n    print(pick(5));\n}\n\nfn pick(x: int) -> int {\n    if x > 0 {\n        return 1;\n    } else {\n        return 2;\n    }\n    print(x);\n}",
    ),
];

/// mini-lang 前端 + 降级成三地址码 + 每个函数的控制流图。
pub struct IrFront {
    pub front: Front,
    pub low: Option<Lowered>,
    /// 每个函数的控制流图（含不可达块）。
    pub cfgs: Vec<Cfg>,
}

impl IrFront {
    pub fn new(src: &str) -> Self {
        let front = Front::new(src);
        let low = match &front.sema {
            Some(a) if a.errors.is_empty() => Some(lower(src, &front.parse.ast, front.parse.root, a)),
            _ => None,
        };
        let cfgs = low.as_ref().map(|l| l.funcs.iter().map(build).collect()).unwrap_or_default();
        Self { front, low, cfgs }
    }

    /// 默认展示指令最多的那个函数（通常是最有意思的）。
    pub fn default_func(&self) -> usize {
        self.low.as_ref().and_then(|l| (0..l.funcs.len()).max_by_key(|&i| l.funcs[i].code.len())).unwrap_or(0)
    }

    /// 有错误时把错误列出来，返回是否可以继续。
    pub fn check(&self, ui: &mut Ui) -> bool {
        let p = Palette::of(ui);
        if !self.front.syntax_errors.is_empty() {
            super::ch3::syntax_errors(ui, &self.front);
            return false;
        }
        if let Some(a) = &self.front.sema
            && !a.errors.is_empty()
        {
            ui.label(RichText::new("程序还有语义错误（第 3 章），先改正它们：").color(p.error).strong());
            for d in &a.errors {
                crate::widgets::diagnostic_view(ui, &self.front.src, d);
            }
            return false;
        }
        true
    }
}

/// 选择函数的一排按钮。返回是否换了函数。
pub fn func_picker(ui: &mut Ui, front: &IrFront, current: &mut usize) -> bool {
    let p = Palette::of(ui);
    let Some(low) = &front.low else { return false };
    if low.funcs.len() < 2 {
        return false;
    }
    let mut changed = false;
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("看哪个函数：").color(p.muted));
        for (i, f) in low.funcs.iter().enumerate() {
            let (fill, stroke) = if i == *current {
                (p.focus_bg, Stroke::new(2.0, p.accent))
            } else {
                (p.card_bg, Stroke::new(1.0, p.card_stroke))
            };
            if chip_button(ui, RichText::new(format!("fn {}", f.name)).font(mono(14.0)), fill, stroke).clicked()
                && i != *current
            {
                *current = i;
                changed = true;
            }
        }
    });
    changed
}

/// 控制流图画成状态图：每个圆圈是一个基本块。`notes` 是画在块下方的小字。
pub fn cfg_graph(cfg: &Cfg, notes: impl Fn(usize) -> Option<String>) -> Graph {
    let mut edges = Vec::new();
    for (b, block) in cfg.blocks.iter().enumerate() {
        for (&s, k) in block.succs.iter().zip(&block.kinds) {
            let label = match k {
                EdgeKind::True => "是",
                EdgeKind::False => "否",
                EdgeKind::Fall | EdgeKind::Jump => "",
            };
            edges.push(GEdge { from: b, to: s, label: label.to_owned() });
        }
    }
    Graph {
        nodes: cfg
            .blocks
            .iter()
            .enumerate()
            .map(|(i, b)| GNode { label: b.name.clone(), accepting: false, note: notes(i) })
            .collect(),
        edges,
        start: Some(0),
        show_start: true,
    }
}

/// 一个基本块的指令清单（控制流图里的写法），`lines` 可以替换每条指令的文字。
pub fn block_card(ui: &mut Ui, cfg: &Cfg, b: usize, focus: bool, lines: &[(String, Option<eframe::egui::Color32>)]) {
    let p = Palette::of(ui);
    let frame = eframe::egui::Frame::new()
        .fill(if focus { p.focus_bg } else { p.card_bg })
        .stroke(Stroke::new(if focus { 2.0 } else { 1.0 }, if focus { p.accent } else { p.card_stroke }))
        .corner_radius(eframe::egui::CornerRadius::same(8))
        .inner_margin(eframe::egui::Margin::same(8));
    frame.show(ui, |ui| {
        ui.set_width(ui.available_width());
        // 两栏布局是两端对齐的，会吃掉行首空格，这里换成普通的左对齐
        ui.with_layout(eframe::egui::Layout::top_down(eframe::egui::Align::Min), |ui| block_body(ui, cfg, b, lines));
    });
}

fn block_body(ui: &mut Ui, cfg: &Cfg, b: usize, lines: &[(String, Option<eframe::egui::Color32>)]) {
    let p = Palette::of(ui);
    let block = &cfg.blocks[b];
    {
        let preds: Vec<String> = block.preds.iter().map(|&x| cfg.blocks[x].name.clone()).collect();
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(&block.name).font(mono(15.0)).strong().color(p.accent));
            if !preds.is_empty() {
                ui.label(RichText::new(format!("前驱：{}", preds.join("、"))).small().color(p.muted));
            }
        });
        if lines.is_empty() {
            ui.label(RichText::new("（空块）").font(mono(14.0)).color(p.muted));
        }
        for (text, bg) in lines {
            let mut t = RichText::new(format!("  {text}")).font(mono(14.0));
            if let Some(bg) = bg {
                t = t.background_color(*bg);
            }
            ui.label(t);
        }
        if block.insts.last().is_none_or(|i| !i.is_terminator())
            && let Some(&s) = block.succs.first()
        {
            ui.label(RichText::new(format!("  （顺序执行到 {}）", cfg.blocks[s].name)).font(mono(13.0)).color(p.muted));
        }
    }
}

/// 按两栏排列所有块。
pub fn blocks_grid(ui: &mut Ui, cfg: &Cfg, mut each: impl FnMut(&mut Ui, usize)) {
    let n = cfg.blocks.len();
    if ui.available_width() > 700.0 {
        let half = n.div_ceil(2);
        ui.columns(2, |c| {
            for b in 0..half {
                each(&mut c[0], b);
            }
            for b in half..n {
                each(&mut c[1], b);
            }
        });
    } else {
        for b in 0..n {
            each(ui, b);
        }
    }
}
