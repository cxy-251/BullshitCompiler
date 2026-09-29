//! 第 6 章 · 后端：生成机器指令。

pub mod calling;
pub mod isel;
pub mod regalloc;
pub mod vm;

use bsc_minilang::cfg::Cfg;
use eframe::egui::{self, Color32, RichText, Ui};

use crate::theme::{Palette, mono};
use crate::widgets::card;

/// 第 6 章各课的状态。
#[derive(Default)]
pub struct Chapter {
    pub vm: vm::Lesson,
    pub isel: isel::Lesson,
    pub regalloc: regalloc::Lesson,
    pub calling: calling::Lesson,
}

pub const SQUARE: (&str, &str) = (
    "平方加一",
    "fn main() {\n    let x = 3;\n    print(square(x) + 1);\n}\n\nfn square(n: int) -> int {\n    return n * n;\n}",
);

pub const SUM: (&str, &str) = (
    "求和循环",
    "fn main() {\n    print(sum(10));\n}\n\nfn sum(n: int) -> int {\n    let mut s = 0;\n    let mut i = 1;\n    while i <= n {\n        s = s + i;\n        i = i + 1;\n    }\n    return s;\n}",
);

pub const FIB: (&str, &str) = (
    "递归斐波那契",
    "fn main() {\n    print(fib(5));\n}\n\nfn fib(n: int) -> int {\n    if n < 2 {\n        return n;\n    }\n    return fib(n - 1) + fib(n - 2);\n}",
);

pub const MANY: (&str, &str) = (
    "变量很多",
    "fn main() {\n    print(mix(1, 2));\n}\n\nfn mix(a: int, b: int) -> int {\n    let c = a + b;\n    let d = a * b;\n    let e = c - d;\n    let f = c * d + e;\n    let g = f - a + b;\n    return c + d + e + f + g;\n}",
);

/// 所有函数的控制流图（已删除不可达块）。
pub fn cfgs(ir: &crate::pages::learn::ch4::IrFront) -> Vec<Cfg> {
    ir.cfgs.iter().map(Cfg::compact).collect()
}

/// 一段等宽代码清单，`hl` 给某些行加背景色。每行前面可以带一个小标记。
pub fn listing(ui: &mut Ui, title: &str, lines: &[(String, Option<Color32>)]) {
    let p = Palette::of(ui);
    ui.label(RichText::new(title).strong());
    card(ui, |ui| {
        ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            for (text, bg) in lines {
                let mut t = RichText::new(text).font(mono(14.0));
                if text.ends_with(':') {
                    t = t.color(p.keyword);
                }
                if let Some(bg) = bg {
                    t = t.background_color(*bg);
                }
                ui.label(t);
            }
        });
    });
}
