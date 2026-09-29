//! 5.1 常量折叠与代数化简。

use bsc_minilang::cfg::Cfg;
use bsc_minilang::opt::{Folded, fold};
use eframe::egui::{self, Color32, RichText, Ui};

use super::{AREA, SUM, WEEK, run_check};
use crate::pages::learn::ch3::editor;
use crate::pages::learn::ch4::{IrFront, block_card, blocks_grid, func_picker};
use crate::theme::{Palette, mono};
use crate::widgets::{CalloutKind, Stepper, callout, card, prose, prose_sized, quiz};

const RULES: &[(&str, &str, &str)] = &[
    ("常量传播", "x = 3 之后，同一块里用到 x 的地方换成 3", "y = x + 1  →  y = 3 + 1"),
    ("复制传播", "x = y 之后，同一块里用到 x 的地方换成 y", "z = x * 2  →  z = y * 2"),
    ("常量折叠", "两个操作数都是常量，编译时直接算出结果", "t1 = 3 * 4  →  t1 = 12"),
    ("代数化简", "利用恒等式：x + 0 = x，x * 1 = x，x * 0 = 0，x - x = 0", "c = t1 + 0  →  c = t1"),
    ("分支折叠", "条件是常量的条件跳转变成无条件跳转", "if 1 goto B1 else goto B2  →  goto B1"),
];

pub struct Lesson {
    src: String,
    ir: IrFront,
    func: usize,
    folded: Vec<Folded>,
    stepper: Stepper,
}

impl Default for Lesson {
    fn default() -> Self {
        let mut l =
            Self { src: AREA.1.to_owned(), ir: IrFront::new(""), func: 0, folded: vec![], stepper: Stepper::default() };
        l.rebuild();
        l
    }
}

impl Lesson {
    fn rebuild(&mut self) {
        self.ir = IrFront::new(&self.src);
        self.func = self.ir.default_func();
        self.folded = self.ir.cfgs.iter().map(|c| fold(&c.compact())).collect();
        self.stepper.reset();
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        ui.heading("5.1  常量折叠与代数化简");
        ui.label(RichText::new("入门 · 约 20 分钟").color(p.muted));

        ui.add_space(8.0);
        ui.label(RichText::new("一句话").size(20.0).strong());
        prose(
            ui,
            "**编译时能算出来的，就不要留到运行时再算。** 程序员常常为了可读性写 `60 * 60 * 24`，而不是 `86400`；\
             降级成三地址码时也会产生 `x + 0` 这类多余的计算。优化器在编译时把它们算掉、化简掉，\
             生成的程序更短、更快，而且**行为完全不变**——这是所有优化的铁律。",
        );
        callout(ui, CalloutKind::Analogy, "提前备好的菜", |ui| {
            prose(
                ui,
                "餐馆知道每天中午都要用切好的葱花，就在开门前切好，而不是每来一位客人现切一次。\
                 编译就是\"开门前\"，运行才是\"来客人\"——能提前做的事，就在编译时做掉。",
            );
        });

        ui.add_space(8.0);
        ui.label(RichText::new("五条规则").size(20.0).strong());
        card(ui, |ui| {
            egui::Grid::new("fold_rules").num_columns(3).striped(true).spacing([20.0, 6.0]).show(ui, |ui| {
                for (name, what, eg) in RULES {
                    ui.label(RichText::new(*name).strong());
                    ui.label(*what);
                    ui.label(RichText::new(*eg).font(mono(13.0)));
                    ui.end_row();
                }
            });
        });
        prose(
            ui,
            "规则之间会**连锁反应**：常量传播让操作数变成常量，常量折叠算出新的常量，新的常量又可以接着传播下去。\
             这里的常量传播、复制传播只在一个基本块内部做（\"局部\"）；跨块的版本要用数据流分析，5.5 课讲。",
        );

        ui.add_space(8.0);
        ui.label(RichText::new("单步观察").size(20.0).strong());
        if editor(ui, &mut self.src, &[AREA, WEEK, SUM], 6, true) {
            self.rebuild();
        }
        if self.ir.check(ui) {
            if func_picker(ui, &self.ir, &mut self.func) {
                self.stepper.reset();
            }
            self.stepping(ui);
        }

        ui.add_space(8.0);
        callout(ui, CalloutKind::KeyPoint, "折叠出来的\"垃圾\"", |ui| {
            prose(
                ui,
                "折叠完以后，`pi = 3`、`two = 2` 这些赋值还在，可它们的值已经被传播到各处，再也没人读它们了。\
                 分支折叠后，`return 0` 所在的块永远走不到，直接删除。剩下没人读的赋值，交给 5.3 课的死代码消除。\
                 优化器就是这样由许多小的 pass 组成，一个 pass 为下一个创造机会。",
            );
        });
        callout(ui, CalloutKind::KeyPoint, "不能随便折叠的情况", |ui| {
            prose(
                ui,
                "`10 / 0` 不能在编译时算成某个数——它在运行时应该报错，折叠掉就改变了程序的行为，所以这里保留不动。\
                 浮点数更麻烦：`x * 0.0` 不一定等于 `0.0`（x 可能是无穷大或 NaN），`(a + b) + c` 也不一定等于 `a + (b + c)`。\
                 这就是 C 编译器的 `-ffast-math` 选项要单独打开的原因。",
            );
        });

        ui.add_space(12.0);
        ui.label(RichText::new("小测验").size(20.0).strong());
        quiz(
            ui,
            "ch5-fold-1",
            "`let a = 4; let b = a * 2 + 1;` 经过常量传播和折叠，b 的赋值变成什么？",
            &["b = a * 2 + 1（不变）", "b = 9", "b = 8 + 1"],
            1,
            "a 传播成 4，`4 * 2` 折叠成 8，8 再传播进下一条，`8 + 1` 折叠成 9。",
        );
        quiz(
            ui,
            "ch5-fold-2",
            "下面哪个化简是**错误**的？",
            &["x * 1 → x", "x - x → 0", "x / x → 1"],
            2,
            "x 可能是 0，`0 / 0` 运行时要报错，化简成 1 就把错误吞掉了。",
        );

        ui.add_space(12.0);
        callout(ui, CalloutKind::Deeper, "真实编译器怎么做", |ui| {
            prose(
                ui,
                "LLVM 的 InstCombine 是一个巨大的\"窥孔\"化简器，有上千条规则，而且每条规则都要证明正确——\
                 Alive 项目就是用 SMT 求解器自动验证这些规则的，找出过不少真实的 bug。\
                 另一个思路是 e-graph（等式饱和）：把所有等价的写法同时存下来，最后再挑最好的一个，避免规则之间\"先用哪条\"的顺序问题。",
            );
        });
        ui.add_space(24.0);
    }

    fn stepping(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        let Some(f) = self.folded.get(self.func) else { return };
        let has_removal = !f.removed_blocks.is_empty();
        let len = f.changes.len() + usize::from(has_removal);
        self.stepper.ui(ui, len);
        let k = self.stepper.pos;
        let removed_shown = has_removal && k == len;
        let explain = if k == 0 {
            "从第一块开始，逐条检查能不能化简。".to_owned()
        } else if k <= f.changes.len() {
            let c = &f.changes[k - 1];
            format!("{}：`{}` → `{}`", c.rule, c.before, c.after.as_deref().unwrap_or("（删除）"))
        } else {
            let names: Vec<String> = f.removed_blocks.iter().map(|&b| f.input.blocks[b].name.clone()).collect();
            format!("分支折叠后，{} 从入口再也走不到了，整块删掉。", names.join("、"))
        };
        prose_sized(ui, &explain, 17.0);

        let input: &Cfg = &f.input;
        blocks_grid(ui, input, |ui, b| {
            let lines: Vec<(String, Option<Color32>)> = input.blocks[b]
                .insts
                .iter()
                .enumerate()
                .map(|(i, inst)| match f.changes.iter().position(|c| c.block == b && c.inst == i).filter(|&j| j < k) {
                    Some(j) => {
                        let bg = if j + 1 == k { p.focus_bg } else { p.ok.gamma_multiply(0.18) };
                        (f.changes[j].after.clone().unwrap_or_default(), Some(bg))
                    }
                    None => (input.inst_text(inst), None),
                })
                .collect();
            let dead = removed_shown && f.removed_blocks.contains(&b);
            block_card(ui, input, b, dead, &lines);
            if dead {
                ui.label(RichText::new(format!("{} 不可达，删除", input.blocks[b].name)).small().color(p.error));
            }
            ui.add_space(4.0);
        });
        if k == len {
            let before: Vec<Cfg> = self.folded.iter().map(|f| f.input.clone()).collect();
            let after: Vec<Cfg> = self.folded.iter().map(|f| f.output.clone()).collect();
            run_check(ui, &before, &after);
        }
    }
}
