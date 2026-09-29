//! 4.1 三地址码。

use bsc_core::Span;
use bsc_minilang::ir::Inst;
use eframe::egui::{self, RichText, Sense, Ui};

use super::{IrFront, PRESETS};
use crate::pages::learn::ch3::editor;
use crate::theme::{Palette, mono};
use crate::widgets::{CalloutKind, Mark, Stepper, callout, card, prose, prose_sized, quiz, source_view};

const FLAT: (&str, &str) = (
    "表达式拍平",
    "fn main() {\n    print(frame(3, 4));\n}\n\nfn frame(w: int, h: int) -> int {\n    let s = (w + 2) * (h + 2) - w * h;\n    return s;\n}",
);

/// 三地址码的指令形式。
const FORMS: &[(&str, &str)] = &[
    ("x = y op z", "一次二元运算，比如 t1 = a + b、t2 = i <= n"),
    ("x = op y", "一次一元运算：取负、取反"),
    ("x = y", "复制"),
    ("x = call f(a, b)", "调用函数，返回值放进 x（没有返回值就不写 x =）"),
    ("x = param k", "取第 k 个参数"),
    ("L:", "标签，标记一个位置，供跳转使用"),
    ("goto L", "无条件跳转"),
    ("if x goto L1 else goto L2", "条件跳转：x 非 0 去 L1，否则去 L2"),
    ("return x", "返回"),
];

pub struct Lesson {
    src: String,
    ir: IrFront,
    stepper: Stepper,
    /// 鼠标停在哪条指令上（上一帧），对应的源码高亮。
    hovered: Option<Span>,
}

impl Default for Lesson {
    fn default() -> Self {
        let mut l = Self { src: FLAT.1.to_owned(), ir: IrFront::new(""), stepper: Stepper::default(), hovered: None };
        l.rebuild();
        l
    }
}

impl Lesson {
    fn rebuild(&mut self) {
        self.ir = IrFront::new(&self.src);
        self.stepper.reset();
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        ui.heading("4.1  三地址码");
        ui.label(RichText::new("入门 · 约 25 分钟").color(p.muted));

        ui.add_space(8.0);
        ui.label(RichText::new("一句话").size(20.0).strong());
        prose(
            ui,
            "语法树适合检查\"意思对不对\"，却不适合做优化和生成机器码：它是嵌套的，而 CPU 一次只做一件小事。\
             编译器因此会先把程序翻译成一种**中间表示**（IR）。最经典的是**三地址码**：每条指令最多一个运算，\
             最多涉及三个\"地址\"（两个操作数、一个结果）。嵌套的表达式被拆成一串用**临时变量**（t1、t2……）\
             串起来的小步骤，`if`、`while` 变成**标签**和**跳转**。",
        );
        callout(ui, CalloutKind::Analogy, "把菜谱拆成一步一步", |ui| {
            prose(
                ui,
                "\"把两个鸡蛋打散后和炒熟的番茄混合再加盐\"是一句嵌套的话。给新手的菜谱会写成：\
                 ①打蛋，放进碗 A；②炒番茄，放进盘 B；③把 A 和 B 混合，放进锅 C；④往 C 里加盐。\
                 每一步只做一件事，中间结果放进有名字的容器——碗 A、盘 B 就是临时变量。",
            );
        });

        ui.add_space(8.0);
        ui.label(RichText::new("指令一览").size(20.0).strong());
        card(ui, |ui| {
            egui::Grid::new("tac_forms").num_columns(2).striped(true).spacing([24.0, 6.0]).show(ui, |ui| {
                for (form, what) in FORMS {
                    ui.label(RichText::new(*form).font(mono(14.0)));
                    ui.label(*what);
                    ui.end_row();
                }
            });
        });

        ui.add_space(8.0);
        ui.label(RichText::new("单步观察：从语法树生成三地址码").size(20.0).strong());
        prose(
            ui,
            "编译器按语法树自底向上生成指令：子表达式先算好，结果放进临时变量，再用它们算上一层。\
             每一步生成一条指令，上面的源码里高亮的是产生这条指令的那一段。播放完以后，把鼠标移到指令上，也能看到它来自哪里。",
        );
        let mut presets = vec![FLAT];
        presets.extend_from_slice(&PRESETS[..3]);
        if editor(ui, &mut self.src, &presets, 6, true) {
            self.rebuild();
        }
        if self.ir.check(ui) {
            self.stepping(ui);
        }

        ui.add_space(8.0);
        callout(ui, CalloutKind::KeyPoint, "&& 和 || 变成了跳转", |ui| {
            prose(
                ui,
                "选\"循环 + 短路\"例子，看 `i % 3 == 0 || i % 5 == 0` 生成了什么：左边为真时，右边**根本不算**，直接跳过。\
                 这就是短路求值，它在三地址码里只能用条件跳转表达——`||` 不是一条普通的运算指令。",
            );
        });
        callout(ui, CalloutKind::KeyPoint, "能少用一个临时变量就少用一个", |ui| {
            prose(
                ui,
                "`let s = (w + 2) * (h + 2) - w * h;` 最后一步的结果直接写进 `s`，而不是先放进 t5 再 `s = t5`。\
                 生成代码时把\"结果要放到哪里\"传给子表达式，就能省掉这条多余的复制。\
                 省不掉的多余指令，第 5 章的优化会继续清理。",
            );
        });

        ui.add_space(12.0);
        ui.label(RichText::new("小测验").size(20.0).strong());
        quiz(
            ui,
            "ch4-tac-1",
            "`x = a * b + c * d` 至少需要几个临时变量？",
            &["0 个", "2 个：t1 = a * b，t2 = c * d，最后 x = t1 + t2", "3 个"],
            1,
            "两个乘积都得先算出来、找地方放，加法的结果可以直接写进 x。",
        );
        quiz(
            ui,
            "ch4-tac-2",
            "`while` 循环在三地址码里对应什么？",
            &["一条专门的 while 指令", "检查条件的标签 + 条件跳转 + 循环体末尾跳回去的 goto", "把循环体复制很多遍"],
            1,
            "三地址码里没有结构化的控制语句，只有标签和跳转。所有的 if、while、for、break 最后都变成这两样。",
        );

        ui.add_space(12.0);
        callout(ui, CalloutKind::Deeper, "真实编译器的中间表示", |ui| {
            prose(
                ui,
                "三地址码是一类 IR 的统称。GCC 的 GIMPLE 就是三地址码；LLVM IR 是三地址码 + SSA（4.4 课）+ 类型；\
                 JVM 字节码和 WebAssembly 则是**栈式**的（第 0 课计算器的风格），操作数隐含在栈上。\
                 很多编译器会用好几层 IR：rustc 从 AST 到 HIR、MIR，再到 LLVM IR，每层去掉一些高层信息、加上一些底层细节。",
            );
        });
        ui.add_space(24.0);
    }

    fn stepping(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        let Some(low) = &self.ir.low else { return };
        self.stepper.ui(ui, low.events.len());
        let k = self.stepper.pos;
        let last = k.checked_sub(1).map(|i| &low.events[i]);

        let mut marks = Vec::new();
        if let Some(e) = last {
            marks.push(Mark { span: self.ir.front.parse.ast.node(e.node).span, bg: p.focus_bg });
        }
        if let Some(h) = self.hovered {
            marks.push(Mark { span: h, bg: p.warn_bg });
        }
        card(ui, |ui| {
            source_view(ui, &self.ir.front.src, &self.ir.front.colors(&p), &marks, 16.0);
        });
        let explain = match last {
            None => "从第一个函数开始。".to_owned(),
            Some(e) => format!("`{}`：{}", low.funcs[e.func].linear_text(e.inst), e.note),
        };
        prose_sized(ui, &explain, 17.0);

        // 每个函数已经生成的指令
        let mut hovered = None;
        for (fi, f) in low.funcs.iter().enumerate() {
            let done: Vec<usize> = low.events[..k].iter().filter(|e| e.func == fi).map(|e| e.inst).collect();
            if done.is_empty() {
                continue;
            }
            ui.label(RichText::new(format!("fn {}", f.name)).font(mono(15.0)).strong());
            card(ui, |ui| {
                ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                    for &i in &done {
                        let is_label = matches!(f.code[i], Inst::Label(_));
                        let indent = if is_label { "" } else { "    " };
                        let mut t = RichText::new(format!("{indent}{}", f.linear_text(i))).font(mono(15.0));
                        if is_label {
                            t = t.color(p.keyword);
                        }
                        if last.is_some_and(|e| e.func == fi && e.inst == i) {
                            t = t.background_color(p.focus_bg);
                        }
                        let r = ui.add(egui::Label::new(t).sense(Sense::hover()));
                        if r.hovered() {
                            hovered = Some(f.origin[i]);
                        }
                    }
                });
            });
        }
        if hovered != self.hovered {
            self.hovered = hovered;
            ui.ctx().request_repaint();
        }
        if k == low.events.len() {
            let n: usize = low.funcs.iter().map(|f| f.code.len()).sum();
            let temps: usize = low.funcs.iter().map(|f| f.vars.iter().filter(|v| v.temp).count()).sum();
            ui.label(RichText::new(format!("完成：共 {n} 条指令，用了 {temps} 个临时变量。")).color(p.ok).strong());
        }
    }
}
