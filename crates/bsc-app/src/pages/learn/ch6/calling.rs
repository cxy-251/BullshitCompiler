//! 6.4 函数调用约定：栈帧、参数传递、保存寄存器。

use std::collections::BTreeMap;

use bsc_minilang::regalloc::{AsmFunc, color, finalize};
use bsc_minilang::rv::{IOp, MInst, Reg, select};
use bsc_minilang::sim::{self, STACK_TOP};
use eframe::egui::{self, Color32, RichText, Ui};

use super::{FIB, SQUARE, SUM, cfgs, listing};
use crate::pages::learn::ch3::editor;
use crate::pages::learn::ch4::IrFront;
use crate::theme::{Palette, mono};
use crate::widgets::{CalloutKind, Stepper, callout, card, prose, prose_sized, quiz};

const TRACE_LIMIT: usize = 3000;

const CONVENTION: &[(&str, &str, &str)] = &[
    ("a0–a7", "参数 / 返回值", "调用前把实参放进 a0、a1……；返回值放在 a0"),
    ("ra", "返回地址", "call 把\"回来的位置\"写进 ra，ret 跳回 ra"),
    ("sp", "栈指针", "指向当前栈顶；栈从高地址往低地址长"),
    ("s1–s11", "被调用者保存", "函数要用就得先存到自己的栈帧里，返回前恢复——调用者可以放心地把值留在这里"),
    ("t0–t6", "调用者保存", "随便用，但调用别的函数后可能被改掉"),
    ("zero", "常数 0", "读出来永远是 0，写进去没用"),
];

pub struct Lesson {
    src: String,
    ir: IrFront,
    asm: Vec<AsmFunc>,
    run: Option<sim::Run>,
    stepper: Stepper,
}

impl Default for Lesson {
    fn default() -> Self {
        let mut l =
            Self { src: FIB.1.to_owned(), ir: IrFront::new(""), asm: vec![], run: None, stepper: Stepper::default() };
        l.rebuild();
        l
    }
}

/// 回放到第 k 步时的机器状态。
struct State {
    regs: BTreeMap<Reg, i64>,
    mem: BTreeMap<i64, i64>,
    /// 调用栈：(函数, 栈帧起点 sp)。
    frames: Vec<(usize, Option<i64>)>,
    output: Vec<i64>,
    changed: Vec<Reg>,
    stored: Option<i64>,
}

fn replay(asm: &[AsmFunc], r: &sim::Run, k: usize) -> State {
    let main = asm.iter().position(|f| f.name == "main").unwrap_or(0);
    let mut st = State {
        regs: BTreeMap::from([(Reg::Sp, STACK_TOP), (Reg::Ra, -1)]),
        mem: BTreeMap::new(),
        frames: vec![(main, None)],
        output: vec![],
        changed: vec![],
        stored: None,
    };
    for (i, s) in r.steps[..k].iter().enumerate() {
        let last = i + 1 == k;
        for &(reg, v) in &s.writes {
            st.regs.insert(reg, v);
        }
        if last {
            st.changed = s.writes.iter().map(|w| w.0).collect();
            st.stored = s.store.map(|x| x.0);
        }
        if let Some((a, v)) = s.store {
            st.mem.insert(a, v);
        }
        if let Some(v) = s.output {
            st.output.push(v);
        }
        // 序言里 sp 变小：记下这一帧的起点
        if let MInst::I { op: IOp::Addi, rd: Reg::Sp, imm, .. } = r.lines[s.pc].inst
            && imm < 0
            && let Some(top) = st.frames.last_mut()
        {
            top.1 = Some(st.regs[&Reg::Sp]);
        }
        match s.call {
            Some(Some(f)) => st.frames.push((f, None)),
            Some(None) => {
                st.frames.pop();
            }
            None => {}
        }
    }
    st
}

impl Lesson {
    fn rebuild(&mut self) {
        self.ir = IrFront::new(&self.src);
        self.asm = cfgs(&self.ir)
            .iter()
            .map(|c| {
                let (m, _) = select(c, true);
                finalize(&m, &color(&m, 11).alloc)
            })
            .collect();
        self.run = (!self.asm.is_empty()).then(|| sim::run(&self.asm, TRACE_LIMIT));
        self.stepper.reset();
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        ui.heading("6.4  函数调用约定");
        ui.label(RichText::new("进阶 · 约 35 分钟").color(p.muted));

        ui.add_space(8.0);
        ui.label(RichText::new("一句话").size(20.0).strong());
        prose(
            ui,
            "函数 f 调用 g 时，双方得事先约好：实参放在哪？返回值放在哪？g 能随便改哪些寄存器、哪些必须原样还回去？返回后跳回哪里？\
             这套约定叫**调用约定**（ABI 的一部分）。只要遵守它，不同编译器、不同语言编译出来的函数就能互相调用——\
             Rust 能调用 C 的库，靠的就是这个。",
        );
        callout(ui, CalloutKind::Analogy, "借用会议室的规矩", |ui| {
            prose(
                ui,
                "公司规定：白板（s 寄存器）用完必须擦成原样再走；便签纸（t 寄存器）随便用、用完不管。\
                 所以你出门办事前，重要的东西写在白板上就放心了；写在便签纸上的，回来可能已经被人扔了。\
                 每次办事都在走廊尽头领一个储物柜（**栈帧**），走的时候还回去。",
            );
        });

        ui.add_space(8.0);
        ui.label(RichText::new("RISC-V 的约定（这里用到的部分）").size(20.0).strong());
        card(ui, |ui| {
            egui::Grid::new("abi").num_columns(3).striped(true).spacing([20.0, 6.0]).show(ui, |ui| {
                for (r, role, rule) in CONVENTION {
                    ui.label(RichText::new(*r).font(mono(14.0)).strong());
                    ui.label(*role);
                    ui.label(*rule);
                    ui.end_row();
                }
            });
        });
        prose(
            ui,
            "每个函数开头的**序言**（prologue）把 sp 减小，开辟自己的栈帧，把 ra 和要用到的 s 寄存器存进去；\
             结尾的**尾声**（epilogue）恢复它们，把 sp 加回去，`ret`。只有自己还要调用别人的函数才需要保存 ra（call 会覆盖它）。",
        );

        ui.add_space(8.0);
        ui.label(RichText::new("在模拟器里单步执行").size(20.0).strong());
        prose(
            ui,
            "下面是完整编译流水线（词法 → 语法 → 语义 → 三地址码 → 指令选择 → 图着色寄存器分配）生成的 RISC-V 汇编，\
             在内置的模拟器里一条条执行。右边是寄存器和栈内存：看 `fib` 递归时栈帧怎样一层层长出来又退回去。",
        );
        if editor(ui, &mut self.src, &[FIB, SQUARE, SUM], 6, true) {
            self.rebuild();
        }
        if self.ir.check(ui) {
            self.stepping(ui);
        }

        ui.add_space(12.0);
        ui.label(RichText::new("小测验").size(20.0).strong());
        quiz(
            ui,
            "ch6-cc-1",
            "函数 f 把一个值放在 t0 里，然后 call g。回来以后 t0 里还是原来的值吗？",
            &["一定是", "不一定：t 寄存器是调用者保存的，g 可以随便改", "一定不是"],
            1,
            "要跨过调用保存的值，要么放在 s 寄存器里（g 用到会自己保存、恢复），要么 f 自己在调用前存到栈上。",
        );
        quiz(
            ui,
            "ch6-cc-2",
            "为什么 main 调用 fib 前后，sp 的值是一样的？",
            &["fib 的序言减小了 sp，尾声又加了回去", "sp 是只读的", "巧合"],
            0,
            "每个函数都要把栈恢复成进来时的样子，这是调用约定的一部分。否则调用者找不到自己栈帧里的东西了。",
        );

        ui.add_space(12.0);
        callout(ui, CalloutKind::Deeper, "真实世界的调用约定", |ui| {
            prose(
                ui,
                "x86-64 上 Linux 用 System V ABI（前 6 个整数参数放在 rdi、rsi、rdx、rcx、r8、r9），Windows 用另一套；\
                 参数太多时多出来的放在栈上；结构体怎么传另有复杂的规则。尾调用优化能让\"最后一步是调用\"的函数复用当前栈帧，\
                 递归再深也不会栈溢出——函数式语言非常依赖它。",
            );
        });
        ui.add_space(24.0);
    }

    fn stepping(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        let Some(r) = &self.run else { return };
        self.stepper.ui(ui, r.steps.len());
        let k = self.stepper.pos;
        let st = replay(&self.asm, r, k);
        // 下一条要执行的指令
        let pc = if k < r.steps.len() { Some(r.steps[k].pc) } else { None };

        let explain = match pc {
            None => match &r.error {
                Some(e) => format!("出错：{e}"),
                None => format!("main 返回，程序结束。共执行 {} 条指令。", r.executed),
            },
            Some(pc) => {
                let line = &r.lines[pc];
                let f = &self.asm[line.func];
                let what = match &line.inst {
                    MInst::I { op: IOp::Addi, rd: Reg::Sp, imm, .. } if *imm < 0 => {
                        format!("序言：sp 减 {}，开辟 {} 的栈帧。", -imm, f.name)
                    }
                    MInst::I { op: IOp::Addi, rd: Reg::Sp, .. } => "尾声：sp 加回去，释放栈帧。".to_owned(),
                    MInst::Sd { rs: Reg::Ra, .. } => {
                        "把返回地址 ra 存进栈帧：这个函数还要调用别人，call 会覆盖 ra。".to_owned()
                    }
                    MInst::Ld { rd: Reg::Ra, .. } => "从栈帧里恢复 ra，待会儿 ret 才能回到正确的位置。".to_owned(),
                    MInst::Sd { rs: Reg::S(n), base: Reg::Sp, .. } if line_in_prologue(f, pc, r) => {
                        format!("保存 s{n}：它是被调用者保存的寄存器，本函数要用它，得先把调用者的值存起来。")
                    }
                    MInst::Ld { rd: Reg::S(n), .. } if line_in_epilogue(f, pc, r) => {
                        format!("恢复 s{n}：还给调用者原来的值。")
                    }
                    MInst::Call { func } if func == "print" => "调用内置函数 print：打印 a0。".to_owned(),
                    MInst::Call { func } => format!("call {func}：把返回地址写进 ra，跳到 {func}。"),
                    MInst::Ret => "ret：跳回 ra 指向的位置（调用者 call 的下一条）。".to_owned(),
                    MInst::Mv { rd: Reg::A(n), .. } => format!("把实参 / 返回值放进 a{n}。"),
                    MInst::Mv { rs: Reg::A(n), .. } => format!("从 a{n} 取出参数 / 返回值。"),
                    MInst::Ld { .. } => "从栈上读回一个溢出的值。".to_owned(),
                    MInst::Sd { .. } => "把值写到栈上。".to_owned(),
                    _ => "普通运算。".to_owned(),
                };
                format!("即将执行 {}：`{}`。{what}", f.name, line.inst.text(&[]))
            }
        };
        prose_sized(ui, &explain, 17.0);

        let code = |ui: &mut Ui| {
            // 只显示当前函数的代码
            let func = pc.map_or(0, |pc| r.lines[pc].func);
            let mut lines: Vec<(String, Option<Color32>)> = Vec::new();
            for (i, l) in r.lines.iter().enumerate().filter(|(_, l)| l.func == func) {
                if let Some(lb) = &l.label {
                    lines.push((format!("{lb}:"), None));
                }
                let bg = (pc == Some(i)).then_some(p.focus_bg);
                lines.push((format!("    {}", l.inst.text(&[])), bg));
            }
            listing(ui, &format!("fn {} 的汇编", self.asm[func].name), &lines);
        };
        let machine = |ui: &mut Ui| {
            ui.label(RichText::new("寄存器（只列出用到过的）").strong());
            card(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for (reg, v) in &st.regs {
                        let mut t = RichText::new(format!("{}={}", reg.name(&[]), fmt_val(*reg, *v))).font(mono(14.0));
                        if st.changed.contains(reg) {
                            t = t.background_color(p.focus_bg);
                        }
                        ui.label(t);
                    }
                });
            });
            ui.label(RichText::new("栈（地址从高到低；sp 以下还没用到）").strong());
            card(ui, |ui| {
                ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                    ui.spacing_mut().item_spacing.y = 1.0;
                    let sp = st.regs.get(&Reg::Sp).copied().unwrap_or(STACK_TOP);
                    let mut addr = STACK_TOP - 8;
                    while addr >= sp {
                        let owner = frame_label(&self.asm, &st.frames, addr);
                        let v = st.mem.get(&addr).map_or("—".to_owned(), |v| v.to_string());
                        let mut t = RichText::new(format!("{addr:#06x}  {v:>6}  {owner}")).font(mono(13.0));
                        if st.stored == Some(addr) {
                            t = t.background_color(p.focus_bg);
                        }
                        ui.label(t);
                        addr -= 8;
                    }
                    ui.label(RichText::new(format!("{sp:#06x}  ← sp")).font(mono(13.0)).color(p.accent));
                });
            });
            let frames: Vec<String> = st.frames.iter().map(|(f, _)| self.asm[*f].name.clone()).collect();
            ui.label(RichText::new(format!("调用链：{}", frames.join(" → "))).font(mono(14.0)));
            let out: Vec<String> = st.output.iter().map(|v| v.to_string()).collect();
            ui.label(RichText::new(format!("输出：{}", out.join(" "))).font(mono(14.0)));
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
        if k == r.steps.len() && r.executed > r.steps.len() {
            ui.label(
                RichText::new(format!("（只回放了前 {} 步，程序一共执行了 {} 条指令）", r.steps.len(), r.executed))
                    .color(p.muted),
            );
        }
    }
}

fn fmt_val(r: Reg, v: i64) -> String {
    match r {
        Reg::Sp => format!("{v:#x}"),
        Reg::Ra if v < 0 => "(结束)".to_owned(),
        Reg::Ra => format!("#{v}"),
        _ => v.to_string(),
    }
}

fn line_in_prologue(f: &AsmFunc, pc: usize, r: &sim::Run) -> bool {
    // 序言是函数的第一块：从函数第一条指令往后数，不超过序言的长度
    let start = r.lines.iter().position(|l| l.label.as_deref() == Some(f.name.as_str())).unwrap_or(0);
    pc >= start && pc < start + f.blocks[0].insts.len()
}

fn line_in_epilogue(f: &AsmFunc, pc: usize, r: &sim::Run) -> bool {
    let exit = format!("{}_exit", f.name);
    r.lines.iter().position(|l| l.label.as_deref() == Some(exit.as_str())).is_some_and(|s| pc >= s)
}

/// 栈上某个地址是哪个栈帧里的什么东西。
fn frame_label(asm: &[AsmFunc], frames: &[(usize, Option<i64>)], addr: i64) -> String {
    for (f, sp) in frames.iter().rev() {
        let Some(sp) = sp else { continue };
        let fr = &asm[*f].frame;
        if addr < *sp || addr >= sp + fr.size {
            continue;
        }
        let off = addr - sp;
        let name = &asm[*f].name;
        if fr.ra == Some(off) {
            return format!("{name} 的帧：保存的 ra");
        }
        if let Some((r, _)) = fr.saved.iter().find(|(_, o)| *o == off) {
            return format!("{name} 的帧：保存的 {}", r.name(&[]));
        }
        if let Some(s) = fr.slots.iter().position(|&o| o == off) {
            return format!("{name} 的帧：溢出槽 {s}");
        }
        return format!("{name} 的帧：（对齐填充）");
    }
    String::new()
}
