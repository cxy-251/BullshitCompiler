//! 6.2 指令选择。

use bsc_minilang::cfg::Cfg;
use bsc_minilang::rv::{MFunc, Step, select};
use eframe::egui::{self, Color32, RichText, Ui};

use super::{FIB, SUM, cfgs, listing};
use crate::pages::learn::ch3::editor;
use crate::pages::learn::ch4::{IrFront, func_picker};
use crate::theme::{Palette, mono};
use crate::widgets::{CalloutKind, Stepper, callout, card, prose, prose_sized, quiz};

/// 指令模式（"瓦片"）一览。
const TILES: &[(&str, &str, &str)] = &[
    ("x = y + 常量（小）", "addi x, y, 常量", "1 条：常量直接写进指令里（立即数，-2048…2047）"),
    ("x = y + z", "add x, y, z", "1 条"),
    ("x = y < 常量", "slti x, y, 常量", "1 条"),
    ("x = y <= z", "slt x, z, y；xori x, x, 1", "2 条：RISC-V 只有\"小于\"，\"小于等于\"= 非\"大于\""),
    ("x = y == z", "sub x, y, z；seqz x, x", "2 条：相减，再看是不是 0"),
    ("t = y < z；if t goto A else B", "blt y, z, A（B 紧跟在后面时省掉 j B）", "1～2 条：两条 IR 合成一条比较并跳转"),
    ("常量 0 作操作数", "直接用 zero 寄存器", "0 条：zero 寄存器永远是 0"),
    ("其他常量作操作数", "li 临时, 常量", "多 1 条"),
    ("goto 紧挨着的下一块", "（不生成）", "0 条"),
];

pub struct Lesson {
    src: String,
    ir: IrFront,
    func: usize,
    cfgs: Vec<Cfg>,
    smart: bool,
    sel: Option<(MFunc, Vec<Step>)>,
    counts: (usize, usize),
    stepper: Stepper,
}

impl Default for Lesson {
    fn default() -> Self {
        let mut l = Self {
            src: SUM.1.to_owned(),
            ir: IrFront::new(""),
            func: 0,
            cfgs: vec![],
            smart: true,
            sel: None,
            counts: (0, 0),
            stepper: Stepper::default(),
        };
        l.rebuild();
        l
    }
}

fn count(m: &MFunc) -> usize {
    m.blocks.iter().map(|b| b.insts.len()).sum()
}

impl Lesson {
    fn rebuild(&mut self) {
        self.ir = IrFront::new(&self.src);
        self.cfgs = cfgs(&self.ir);
        self.func = self.ir.default_func();
        self.select();
    }

    fn select(&mut self) {
        self.sel = self.cfgs.get(self.func).map(|c| select(c, self.smart));
        self.counts =
            self.cfgs.get(self.func).map_or((0, 0), |c| (count(&select(c, false).0), count(&select(c, true).0)));
        self.stepper.reset();
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        ui.heading("6.2  指令选择");
        ui.label(RichText::new("进阶 · 约 30 分钟").color(p.muted));

        ui.add_space(8.0);
        ui.label(RichText::new("一句话").size(20.0).strong());
        prose(
            ui,
            "从这一课起，目标换成真实的 CPU 指令集 **RISC-V**（一种开放、精简的指令集，教学和工业界都在用）。\
             **指令选择**把三地址码翻译成机器指令。同一件事往往有好几种翻译法：`i = i + 1` 可以翻成\
             `li t, 1` + `add i, i, t` 两条，也可以直接用带立即数的 `addi i, i, 1` 一条。\
             指令选择就是为每段 IR 挑最省的翻译。",
        );
        callout(ui, CalloutKind::Analogy, "用瓷砖铺地", |ui| {
            prose(
                ui,
                "把 IR 想成一块要铺满的地面，每种机器指令是一种形状的瓷砖：小瓷砖只能盖住一条 IR，大瓷砖能一次盖住好几条\
                 （比如\"比较 + 跳转\"）。指令选择就是用最少、最便宜的瓷砖把地面铺满。每次都先试能盖住最多的那块，\
                 叫**最大吞噬**（maximal munch）。",
            );
        });

        ui.add_space(8.0);
        ui.label(RichText::new("这里用到的模式").size(20.0).strong());
        card(ui, |ui| {
            egui::Grid::new("tiles").num_columns(3).striped(true).spacing([20.0, 6.0]).show(ui, |ui| {
                for h in ["IR 的样子", "机器指令", "代价"] {
                    ui.label(RichText::new(h).strong());
                }
                ui.end_row();
                for (ir, asm, cost) in TILES {
                    ui.label(RichText::new(*ir).font(mono(13.0)));
                    ui.label(RichText::new(*asm).font(mono(13.0)));
                    ui.label(*cost);
                    ui.end_row();
                }
            });
        });
        prose(
            ui,
            "机器指令里写成 `%s`、`%i` 的是**虚拟寄存器**：这时假设寄存器要多少有多少，每个 IR 变量一个。\
             真正的 RISC-V 只有 32 个寄存器，怎么分配是下一课的事。",
        );

        ui.add_space(8.0);
        ui.label(RichText::new("单步观察").size(20.0).strong());
        if editor(ui, &mut self.src, &[SUM, FIB], 6, true) {
            self.rebuild();
        }
        if self.ir.check(ui) {
            let mut changed = func_picker(ui, &self.ir, &mut self.func);
            ui.horizontal(|ui| {
                ui.label(RichText::new("翻译方式：").color(p.muted));
                changed |= ui.selectable_value(&mut self.smart, true, "模式匹配").changed();
                changed |= ui.selectable_value(&mut self.smart, false, "朴素逐条展开").changed();
                ui.label(
                    RichText::new(format!("这个函数：朴素 {} 条，模式匹配 {} 条", self.counts.0, self.counts.1))
                        .color(p.muted),
                );
            });
            if changed {
                self.select();
            }
            self.stepping(ui);
        }

        ui.add_space(12.0);
        ui.label(RichText::new("小测验").size(20.0).strong());
        quiz(
            ui,
            "ch6-isel-1",
            "`x = y - 5` 最少翻译成几条 RISC-V 指令？",
            &["1 条：addi x, y, -5", "2 条：li t, 5；sub x, y, t", "3 条"],
            0,
            "RISC-V 没有 subi，但减一个常量就是加它的相反数，addi 就够了。",
        );
        quiz(
            ui,
            "ch6-isel-2",
            "为什么 `t = i < n; if t goto A else B` 能合成一条 blt？",
            &["t 只给这个跳转用，不需要真的把比较结果存下来", "因为 blt 更快", "任何比较都能这样合并"],
            0,
            "如果 t 后面还有别的地方要用，就必须把 0/1 算出来存好，不能合并。编译器会先检查 t 被用了几次。",
        );

        ui.add_space(12.0);
        callout(ui, CalloutKind::Deeper, "树覆盖与指令选择器生成器", |ui| {
            prose(
                ui,
                "教科书上的指令选择在**表达式树**上做：每种指令是一棵小树（瓦片），用瓦片覆盖整棵表达式树。\
                 最大吞噬是贪心的；动态规划（Aho、Ganapathi、Tjiang 的 twig，以及 BURS）能找到代价最小的覆盖。\
                 LLVM 的 SelectionDAG 在有向无环图上做模式匹配，模式由 TableGen 描述文件自动生成；新的 GlobalISel 则直接在机器 IR 上逐条选择。",
            );
        });
        ui.add_space(24.0);
    }

    fn stepping(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        let (Some(cfg), Some((m, steps))) = (self.cfgs.get(self.func), &self.sel) else { return };
        self.stepper.ui(ui, steps.len());
        let k = self.stepper.pos;
        let cur = k.checked_sub(1).map(|i| &steps[i]);

        let explain = match cur {
            None => "从第一块的第一条 IR 开始，每次挑一个能覆盖当前 IR 的模式。".to_owned(),
            Some(s) => {
                let ir: Vec<String> =
                    s.ir.iter().map(|&i| format!("`{}`", cfg.inst_text(&cfg.blocks[s.block].insts[i]))).collect();
                let out: Vec<String> =
                    m.blocks[s.block].insts[s.out.clone()].iter().map(|i| format!("`{}`", i.text(&m.vregs))).collect();
                format!(
                    "{} → 模式「{}」→ {}",
                    ir.join(" + "),
                    s.pattern,
                    if out.is_empty() { "不生成指令".to_owned() } else { out.join("、") }
                )
            }
        };
        prose_sized(ui, &explain, 17.0);

        // 左边：IR；右边：已经生成的机器指令
        let mut ir_lines = Vec::new();
        for (b, block) in cfg.blocks.iter().enumerate() {
            ir_lines.push((format!("{}:", block.name), None));
            for (i, inst) in block.insts.iter().enumerate() {
                let done = steps[..k].iter().any(|s| s.block == b && s.ir.contains(&i));
                let bg = if cur.is_some_and(|s| s.block == b && s.ir.contains(&i)) {
                    Some(p.focus_bg)
                } else if done {
                    Some(p.done_bg)
                } else {
                    None
                };
                ir_lines.push((format!("    {}", cfg.inst_text(inst)), bg));
            }
        }
        let mut asm_lines: Vec<(String, Option<Color32>)> = Vec::new();
        for (b, block) in m.blocks.iter().enumerate() {
            let emitted: usize = steps[..k].iter().filter(|s| s.block == b).map(|s| s.out.end).max().unwrap_or(0);
            if !steps[..k].iter().any(|s| s.block == b) {
                continue;
            }
            asm_lines.push((format!("{}:", block.label), None));
            for (i, inst) in block.insts[..emitted].iter().enumerate() {
                let bg = cur.filter(|s| s.block == b && s.out.contains(&i)).map(|_| p.focus_bg);
                asm_lines.push((format!("    {}", inst.text(&m.vregs)), bg));
            }
        }
        if ui.available_width() > 760.0 {
            ui.columns(2, |c| {
                listing(&mut c[0], "三地址码（输入）", &ir_lines);
                listing(&mut c[1], "RISC-V 汇编（虚拟寄存器）", &asm_lines);
            });
        } else {
            listing(ui, "三地址码（输入）", &ir_lines);
            listing(ui, "RISC-V 汇编（虚拟寄存器）", &asm_lines);
        }
        if k == steps.len() {
            let text = format!(
                "完成：{} 条 IR 指令 → {} 条机器指令。切换\"朴素逐条展开\"对比一下。",
                cfg.blocks.iter().map(|b| b.insts.len()).sum::<usize>(),
                count(m)
            );
            ui.label(RichText::new(text).color(p.ok).strong());
        }
    }
}
