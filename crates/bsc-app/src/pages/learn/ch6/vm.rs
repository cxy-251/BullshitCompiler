//! 6.1 栈式虚拟机。

use bsc_minilang::bytecode::{Op, Program, Run, compile, run};
use eframe::egui::{Color32, RichText, Ui};

use super::{FIB, SQUARE, SUM, listing};
use crate::pages::learn::ch3::editor;
use crate::pages::learn::ch4::IrFront;
use crate::theme::{Palette, mono};
use crate::widgets::{CalloutKind, Mark, Stepper, callout, card, prose, prose_sized, quiz, source_view, stack_view};

const TRACE_LIMIT: usize = 3000;

pub struct Lesson {
    src: String,
    ir: IrFront,
    prog: Option<Program>,
    run: Option<Run>,
    stepper: Stepper,
}

impl Default for Lesson {
    fn default() -> Self {
        let mut l =
            Self { src: SQUARE.1.to_owned(), ir: IrFront::new(""), prog: None, run: None, stepper: Stepper::default() };
        l.rebuild();
        l
    }
}

fn explain(op: &Op, p: &Program, slots: &[String]) -> String {
    let slot = |k: usize| slots.get(k).cloned().unwrap_or_else(|| format!("槽 {k}"));
    match op {
        Op::Push(n) => format!("把常量 {n} 压栈。"),
        Op::Load(k) => format!("把局部变量 {}（第 {k} 个槽）的值压栈。", slot(*k)),
        Op::Store(k) => format!("弹出栈顶，存进局部变量 {}（第 {k} 个槽）。", slot(*k)),
        Op::Bin(_) => "弹出两个数（先弹出的是右边的操作数），计算，结果压栈。".to_owned(),
        Op::Un(_) => "弹出一个数，计算，结果压栈。".to_owned(),
        Op::Jump(t) => format!("跳到第 {t} 条指令。"),
        Op::JumpIfFalse(t) => format!("弹出栈顶：是 0（假）就跳到第 {t} 条，否则顺序执行下一条。"),
        Op::Call(f, n) => format!(
            "调用 {}：新建一个调用帧，把栈顶的 {n} 个实参搬进新帧的参数槽，从它的第 0 条指令开始执行。",
            p.funcs[*f].name
        ),
        Op::Print => "弹出栈顶并打印。".to_owned(),
        Op::Ret => "弹出返回值，销毁当前帧，回到调用者，把返回值压到调用者的栈上。".to_owned(),
        Op::RetVoid => "销毁当前帧，回到调用者（没有返回值）。".to_owned(),
        Op::Pop => "丢掉栈顶（表达式的值没人用）。".to_owned(),
    }
}

impl Lesson {
    fn rebuild(&mut self) {
        self.ir = IrFront::new(&self.src);
        self.prog = match &self.ir.front.sema {
            Some(a) if a.errors.is_empty() => Some(compile(&self.ir.front.parse.ast, self.ir.front.parse.root, a)),
            _ => None,
        };
        self.run = self.prog.as_ref().map(|p| run(p, TRACE_LIMIT));
        self.stepper.reset();
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        ui.heading("6.1  栈式虚拟机");
        ui.label(RichText::new("入门 · 约 30 分钟").color(p.muted));

        ui.add_space(8.0);
        ui.label(RichText::new("一句话").size(20.0).strong());
        prose(
            ui,
            "编译器的最后一步是生成\"机器\"能执行的指令。这台机器可以是真实的 CPU（6.2 课起），\
             也可以是一台用软件实现的**虚拟机**。第 0 课的计算器已经用过一台最小的栈式虚拟机；\
             mini-lang 有变量、分支、函数，所以虚拟机还需要**局部变量槽**、**跳转**和**调用帧**。\
             Java 的 JVM、Python 的 CPython、浏览器里的 WebAssembly 都是这样的栈式虚拟机。",
        );
        callout(ui, CalloutKind::Analogy, "一摞盘子和一排抽屉", |ui| {
            prose(
                ui,
                "操作数栈像一摞盘子：算 `a * b` 时先把 a、b 叠上去，`mul` 拿走最上面两个、放回一个结果。\
                 局部变量像一排编了号的抽屉：`store 1` 把盘子上的值收进 1 号抽屉，`load 1` 再取出来。\
                 每调用一次函数，就搬来一整套新的抽屉（调用帧）；函数返回，这套抽屉就撤走。",
            );
        });

        ui.add_space(8.0);
        ui.label(RichText::new("单步执行").size(20.0).strong());
        prose(
            ui,
            "下面是真实的字节码编译器生成的指令，以及虚拟机逐条执行的过程。高亮的是**即将执行**的指令，\
             源码里高亮的是产生它的那一段。",
        );
        if editor(ui, &mut self.src, &[SQUARE, SUM, FIB], 6, true) {
            self.rebuild();
        }
        if self.ir.check(ui) {
            self.stepping(ui);
        }

        ui.add_space(8.0);
        callout(ui, CalloutKind::KeyPoint, "为什么用栈", |ui| {
            prose(
                ui,
                "栈式指令不用写操作数在哪：`add` 永远是\"栈顶两个数相加\"。所以指令很短（JVM 的 `iadd` 只占一个字节），\
                 编译器也好写——对语法树做一次后序遍历就生成完了。代价是指令条数多、搬来搬去的次数多。\
                 Lua 5 和 Android 的 Dalvik 改用**寄存器式**虚拟机：`add r1, r2, r3`，指令更长但条数更少，解释起来更快。",
            );
        });

        ui.add_space(12.0);
        ui.label(RichText::new("小测验").size(20.0).strong());
        quiz(
            ui,
            "ch6-vm-1",
            "`x = a - b` 编译成栈式指令，顺序是？",
            &["load a, load b, sub, store x", "load b, load a, sub, store x", "sub, load a, load b, store x"],
            0,
            "先把两个操作数按从左到右的顺序压栈，sub 弹出时先弹出的是 b（右边），再弹出 a，算 a - b。",
        );
        quiz(
            ui,
            "ch6-vm-2",
            "递归调用 fib(5) 时，同一时刻最多有几个 fib 的调用帧？",
            &["1 个，帧会复用", "和递归深度一样多：fib(5) → fib(4) → … → fib(1)，每层一个", "25 个"],
            1,
            "每次调用都新建一帧，返回时才销毁。单步执行\"递归斐波那契\"例子，可以看到帧一层层叠上去又退回来。",
        );

        ui.add_space(12.0);
        callout(ui, CalloutKind::Deeper, "解释器怎样变快", |ui| {
            prose(
                ui,
                "最朴素的解释器是一个大 `match`（C 里是 `switch`）循环。更快的写法有\"直接线程化\"（每条指令末尾直接跳到下一条的处理代码）、\
                 超级指令（把常见的指令组合合成一条）。再往上就是 **JIT**：把热点代码在运行时编译成真正的机器码——\
                 HotSpot、V8、LuaJIT 都这么做。WebAssembly 则通常在加载时就整体编译成机器码。",
            );
        });
        ui.add_space(24.0);
    }

    fn stepping(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        let (Some(prog), Some(r)) = (&self.prog, &self.run) else { return };
        if r.trace.is_empty() {
            return;
        }
        self.stepper.ui(ui, r.trace.len() - 1);
        let k = self.stepper.pos;
        let snap = &r.trace[k];
        let finished = k + 1 == r.trace.len() && snap.frames.is_empty();
        let cur = snap.frames.last().map(|f| (f.func, f.pc));

        let mut marks = Vec::new();
        if let Some((f, pc)) = cur
            && let Some(sp) = prog.funcs[f].spans.get(pc)
        {
            marks.push(Mark { span: *sp, bg: p.focus_bg });
        }
        card(ui, |ui| {
            source_view(ui, &self.ir.front.src, &self.ir.front.colors(&p), &marks, 16.0);
        });

        let text = match cur {
            Some((f, pc)) => {
                let op = &prog.funcs[f].ops[pc];
                format!("{}：`{}` —— {}", prog.funcs[f].name, op.text(prog), explain(op, prog, &prog.funcs[f].slots))
            }
            None if r.error.is_some() => format!("出错：{}", r.error.as_deref().unwrap_or("")),
            None => "main 返回，程序结束。".to_owned(),
        };
        prose_sized(ui, &text, 17.0);

        let code = |ui: &mut Ui| {
            for (fi, f) in prog.funcs.iter().enumerate() {
                // 调用链上其他帧停在哪里（它们在等 call 返回）
                let waiting: Vec<usize> =
                    snap.frames.iter().rev().skip(1).filter(|fr| fr.func == fi).map(|fr| fr.pc - 1).collect();
                let lines: Vec<(String, Option<Color32>)> = f
                    .ops
                    .iter()
                    .enumerate()
                    .map(|(i, op)| {
                        let bg = if cur == Some((fi, i)) {
                            Some(p.focus_bg)
                        } else if waiting.contains(&i) {
                            Some(p.done_bg)
                        } else {
                            None
                        };
                        (format!("{i:>3}  {}", op.text(prog)), bg)
                    })
                    .collect();
                listing(ui, &format!("fn {}（{} 个槽）", f.name, f.slots.len()), &lines);
            }
        };
        let machine = |ui: &mut Ui| {
            ui.label(RichText::new("操作数栈").strong());
            card(ui, |ui| stack_view(ui, &snap.stack, true));
            ui.label(RichText::new("调用帧（最上面是当前帧）").strong());
            card(ui, |ui| {
                if snap.frames.is_empty() {
                    ui.label(RichText::new("（空）").color(p.muted));
                }
                for (depth, fr) in snap.frames.iter().enumerate().rev() {
                    let f = &prog.funcs[fr.func];
                    let vars: Vec<String> = f
                        .slots
                        .iter()
                        .zip(&fr.locals)
                        .map(|(n, v)| format!("{n}={}", v.map_or("?".to_owned(), |x| x.to_string())))
                        .collect();
                    let mut t = RichText::new(format!("{}  [{}]", f.name, vars.join(", "))).font(mono(14.0));
                    if depth + 1 == snap.frames.len() {
                        t = t.background_color(p.focus_bg);
                    }
                    ui.label(t);
                }
            });
            let out: Vec<String> = r.output[..snap.output].iter().map(|v| v.to_string()).collect();
            ui.label(RichText::new(format!("输出：{}", out.join(" "))).font(mono(15.0)));
        };
        if ui.available_width() > 760.0 {
            ui.columns(2, |c| {
                code(&mut c[0]);
                machine(&mut c[1]);
            });
        } else {
            machine(ui);
            code(ui);
        }
        if finished || k + 1 == r.trace.len() {
            let text = if r.steps > r.trace.len() {
                format!("共执行 {} 条指令（这里只回放了前 {} 步）。", r.steps, r.trace.len() - 1)
            } else {
                format!("共执行 {} 条指令。", r.steps)
            };
            ui.label(RichText::new(text).color(p.ok).strong());
        }
    }
}
