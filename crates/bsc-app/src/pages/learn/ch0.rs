//! 第 0 课：编译器是什么。

use bsc_calc::{Compilation, Stage};
use eframe::egui::{Color32, RichText, Stroke, TextEdit, Ui};

use crate::calc_view::{StageSteppers, stage_view};
use crate::theme::{Palette, mono};
use crate::widgets::{CalloutKind, callout, card, chip, chip_button, quiz};

const STAGES: [Stage; 4] = [Stage::Lex, Stage::Parse, Stage::Codegen, Stage::Run];

const PRESETS: &[(&str, &str)] = &[
    ("1 + 2 * 3", "先乘除后加减"),
    ("(1 + 2) * 3", "括号改变顺序"),
    ("8 - 3 - 2", "从左往右算"),
    ("-(4 - 6) * 5", "负号"),
    ("10 / (3 - 3)", "运行时错误"),
    ("2 * (3 + ", "语法错误"),
    ("1 + 2 × 3", "词法错误"),
];

pub struct Lesson {
    input: String,
    comp: Compilation,
    stage: Stage,
    steppers: StageSteppers,
}

impl Default for Lesson {
    fn default() -> Self {
        let input = PRESETS[0].0.to_owned();
        Self { comp: Compilation::new(&input), input, stage: Stage::Lex, steppers: StageSteppers::default() }
    }
}

impl Lesson {
    fn recompile(&mut self) {
        self.comp = Compilation::new(&self.input);
        self.steppers.reset();
        // 如果某个阶段出错，直接跳到那个阶段，让学习者先看到错误。
        if let Some((stage, _)) = self.comp.error() {
            self.stage = stage;
            self.steppers.get(stage).seek_end();
        }
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);

        ui.heading("0.1  编译器是什么");
        ui.label(RichText::new("入门 · 约 15 分钟 · 不需要任何编程基础").color(p.muted));
        ui.add_space(8.0);

        // ---- 1. 一句话
        ui.label(RichText::new("一句话").size(20.0).strong());
        ui.label(
            "编译器是一个翻译程序：它把人写的、机器看不懂的代码，翻译成机器能一步步执行的指令。\
             你写的每一行 Rust、C、Java，都要先经过编译器（或它的近亲——解释器）才能运行。",
        );
        callout(ui, CalloutKind::Analogy, "菜谱与机器人厨师", |ui| {
            ui.label(
                "你写了一份菜谱：\"把土豆切丝，大火炒两分钟\"。厨房里的机器人只懂最基本的动作：\
                 \"抓取(土豆)\"\"下刀(间距 2 毫米)\"\"设置火力(大)\"\"等待(120 秒)\"。\
                 编译器就是那个把菜谱逐句翻成动作清单的翻译官。",
            );
            ui.label(
                "翻译官不会一口气完成：先认出菜谱里的词（\"土豆\"\"切丝\"），再弄清句子结构（谁、怎么处理、处理多久），\
                 最后才写出动作清单。编译器也是这样分成好几个阶段，一个接一个，像流水线。",
            );
        });

        // ---- 2. 流水线
        ui.add_space(12.0);
        ui.label(RichText::new("编译流水线").size(20.0).strong());
        ui.label("下面是本课用到的\"计算器语言\"编译器的完整流水线。点击任意一个阶段，就能在下面单步观察它在做什么。");
        self.pipeline(ui);
        ui.label(
            RichText::new(
                "真实的编译器在\"语法分析\"和\"代码生成\"之间还有语义分析、中间表示、优化等阶段，后面的章节会一一讲到。",
            )
            .color(p.muted)
            .small(),
        );

        // ---- 3. 动手
        ui.add_space(12.0);
        ui.label(RichText::new("亲手试一试").size(20.0).strong());
        ui.label(
            "输入一个算式（只支持整数和 + - * / 以及括号），或者点一个例子。下面的每一步都来自编译器真实的运行过程。",
        );
        let changed = card(ui, |ui| {
            let mut changed = false;
            ui.horizontal(|ui| {
                ui.label("算式：");
                let edit = TextEdit::singleline(&mut self.input).font(mono(20.0)).desired_width(f32::INFINITY);
                changed |= ui.add(edit).changed();
            });
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("例子：").color(p.muted));
                for (src, note) in PRESETS {
                    if ui.button(format!("{src}   ·  {note}")).clicked() {
                        self.input = (*src).to_owned();
                        changed = true;
                    }
                }
            });
            changed
        });
        if changed {
            self.recompile();
        }

        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            for s in STAGES {
                let failed = self.comp.error().is_some_and(|(e, _)| e == s);
                let mut text = RichText::new(s.name());
                if failed {
                    text = text.color(p.error);
                }
                ui.selectable_value(&mut self.stage, s, text);
            }
        });
        ui.separator();
        stage_view(ui, &self.comp, self.stage, self.steppers.get(self.stage));
        ui.add_space(4.0);
        ui.label(RichText::new(stage_summary(self.stage)).color(p.muted));

        // ---- 4. 小测验
        ui.add_space(12.0);
        ui.label(RichText::new("小测验").size(20.0).strong());
        quiz(
            ui,
            "ch0-assoc",
            "`8 - 3 - 2` 的结果是多少？（可以在上面输入它，看看语法树长什么样）",
            &["3，因为先算 8 - 3", "7，因为先算 3 - 2"],
            0,
            "减法是\"左结合\"的：语法分析器用循环从左往右读，先把 8 - 3 组成一棵子树，再把它和 2 组合。\
             语法树的形状决定了计算顺序。",
        );
        quiz(
            ui,
            "ch0-postorder",
            "`2 + 3 * 4` 生成的最后一条指令是什么？",
            &["push 4", "mul", "add"],
            2,
            "代码生成按后序遍历：先处理孩子，最后处理自己。加法节点是树根，所以它的 add 指令最后输出。",
        );
        quiz(
            ui,
            "ch0-stage",
            "`1 / 0` 这个错误，是在哪个阶段发现的？",
            &["词法分析", "语法分析", "执行"],
            2,
            "`1 / 0` 的每个字符都认识，句子结构也完全正确，只有真正做除法时才发现除数是 0。\
             这种错误叫\"运行时错误\"。一些编译器会在编译时提前发现常量除以零，这属于第 5 章\"优化\"里常量折叠的范畴。",
        );

        // ---- 5. 深入一点
        ui.add_space(12.0);
        callout(ui, CalloutKind::Deeper, "编译器、解释器和虚拟机", |ui| {
            ui.label(
                "本课的计算器把算式编译成\"栈式虚拟机\"的指令，再由虚拟机执行。Java（JVM）、Python（CPython）、\
                 浏览器里的 WebAssembly 都是这个模式。",
            );
            ui.label(
                "C、Rust 这样的语言则一路编译到 CPU 能直接执行的机器码，不需要虚拟机。\
                 还有一种更简单的做法：不生成指令，直接在语法树上边走边算，这叫\"树遍历解释器\"。",
            );
        });
        callout(ui, CalloutKind::KeyPoint, "为什么要分阶段", |ui| {
            ui.label(
                "每个阶段的输入和输出都是一种明确的数据结构：字符串 → 记号序列 → 语法树 → 指令。\
                 这样每个阶段都可以单独理解、单独测试、单独替换。比如想让编译器支持新的 CPU，只需要换掉最后的代码生成阶段。",
            );
            ui.label(
                "业界常把编译器分成三段：前端（理解源码：词法、语法、语义）、中端（优化）、后端（生成目标代码）。",
            );
        });

        ui.add_space(12.0);
        ui.label(RichText::new("下一课：第 1 章 词法分析——让机器认字，从 1.1「手写一个扫描器」开始。").color(p.muted));
        ui.add_space(24.0);
    }

    fn pipeline(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        let data = ["源码", "记号序列", "语法树", "指令", "结果"];
        let failed = self.comp.error().map(|(s, _)| s);
        ui.horizontal_wrapped(|ui| {
            for (i, item) in data.iter().enumerate() {
                chip(ui, RichText::new(*item).color(p.muted), Color32::TRANSPARENT, Stroke::new(1.0, p.card_stroke));
                if let Some(&stage) = STAGES.get(i) {
                    ui.label(RichText::new("→").color(p.muted));
                    let selected = self.stage == stage;
                    let fill = if selected { p.focus_bg } else { p.card_bg };
                    let mut text = RichText::new(stage.name()).strong();
                    if failed == Some(stage) {
                        text = text.color(p.error);
                    }
                    let clicked =
                        chip_button(ui, text, fill, Stroke::new(if selected { 2.0 } else { 1.0 }, p.accent)).clicked();
                    if clicked {
                        self.stage = stage;
                    }
                    ui.label(RichText::new("→").color(p.muted));
                }
            }
        });
    }
}

fn stage_summary(stage: Stage) -> &'static str {
    match stage {
        Stage::Lex => {
            "词法分析：输入是一串字符，输出是一串记号 (token)。它只关心\"这是什么词\"，不关心词和词之间的关系。"
        }
        Stage::Parse => "语法分析：输入是记号序列，输出是语法树。树的形状决定了谁先算、谁后算。",
        Stage::Codegen => "代码生成：输入是语法树，输出是一条条指令。每条指令都记着自己来自树上的哪个节点。",
        Stage::Run => "执行：虚拟机按顺序执行指令，用一个栈存放中间结果。",
    }
}
