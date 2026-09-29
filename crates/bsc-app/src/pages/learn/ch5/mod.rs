//! 第 5 章 · 优化：让程序更快更小。

pub mod dataflow;
pub mod dce;
pub mod fold;
pub mod lattice;
pub mod sccp;

use bsc_minilang::cfg::Cfg;
use bsc_minilang::interp;
use eframe::egui::{RichText, Ui};

use crate::theme::Palette;

/// 第 5 章各课的状态。
#[derive(Default)]
pub struct Chapter {
    pub fold: fold::Lesson,
    pub dataflow: dataflow::Lesson,
    pub dce: dce::Lesson,
    pub lattice: lattice::Lesson,
    pub sccp: sccp::Lesson,
}

pub const AREA: (&str, &str) = (
    "常量与死代码",
    "fn main() {\n    print(area(5));\n}\n\nfn area(r: int) -> int {\n    let pi = 3;\n    let two = 2;\n    let d = r * two;\n    let unused = d * 100;\n    let c = pi * d + 0;\n    if two > 1 {\n        return c * 1;\n    }\n    return 0;\n}",
);

pub const WEEK: (&str, &str) = (
    "编译时就能算完",
    "fn main() {\n    let day = 60 * 60 * 24;\n    let week = day * 7;\n    let debug = false;\n    if debug {\n        print(0 - 1);\n    }\n    print(week);\n}",
);

pub const SUM: (&str, &str) = (
    "求和循环",
    "fn main() {\n    print(sum(10));\n}\n\nfn sum(n: int) -> int {\n    let mut s = 0;\n    let mut i = 1;\n    while i <= n {\n        s = s + i;\n        i = i + 1;\n    }\n    return s;\n}",
);

pub const NEVER: (&str, &str) = (
    "走不到的分支",
    "fn main() {\n    print(f(10));\n}\n\nfn f(n: int) -> int {\n    let mut x = 1;\n    let mut i = 0;\n    while i < n {\n        if x != 1 {\n            x = 2;\n        }\n        i = i + 1;\n    }\n    return x;\n}",
);

/// 用解释器分别运行优化前后的整个程序，对照打印结果。
pub fn run_check(ui: &mut Ui, before: &[Cfg], after: &[Cfg]) {
    let p = Palette::of(ui);
    if !before.iter().any(|c| c.func.name == "main") {
        return;
    }
    let show = |r: &Result<Vec<i64>, String>| match r {
        Ok(v) => format!("{v:?}"),
        Err(e) => format!("出错：{e}"),
    };
    let (a, b) = (interp::run(before), interp::run(after));
    let count = |cs: &[Cfg]| cs.iter().flat_map(|c| &c.blocks).map(|b| b.insts.len()).sum::<usize>();
    let same = a == b;
    let text = format!(
        "用解释器分别运行优化前后的整个程序：打印 {} 和 {}{}　指令数 {} → {}。",
        show(&a),
        show(&b),
        if same { "，结果一致。" } else { "，不一致！" },
        count(before),
        count(after)
    );
    ui.label(RichText::new(text).color(if same { p.ok } else { p.error }).strong());
}
