//! 6.3 寄存器分配。

use bsc_minilang::cfg::Cfg;
use bsc_minilang::interp;
use bsc_minilang::regalloc::{
    AsmFunc, ColorStep, Coloring, LinearScan, Loc, POOL, ScanStep, color, finalize, linear_scan,
};
use bsc_minilang::rv::{MFunc, Reg, select};
use bsc_minilang::sim;
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, RichText, Sense, Slider, Stroke, Ui, Vec2};

use super::{MANY, SUM, cfgs, listing};
use crate::pages::learn::ch3::editor;
use crate::pages::learn::ch4::{IrFront, func_picker};
use crate::theme::{Palette, mono};
use crate::widgets::{CalloutKind, Stepper, callout, card, prose, prose_sized, quiz};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Algo {
    Scan,
    Color,
}

type RunResult = Result<Vec<i64>, String>;

pub struct Lesson {
    src: String,
    ir: IrFront,
    func: usize,
    cfgs: Vec<Cfg>,
    k: usize,
    algo: Algo,
    m: Option<MFunc>,
    ls: Option<LinearScan>,
    col: Option<Coloring>,
    asm: Option<AsmFunc>,
    /// 整个程序：解释器的输出、分配后在模拟器里的输出。
    check: Option<(RunResult, RunResult)>,
    stepper: Stepper,
}

impl Default for Lesson {
    fn default() -> Self {
        let mut l = Self {
            src: MANY.1.to_owned(),
            ir: IrFront::new(""),
            func: 0,
            cfgs: vec![],
            k: 3,
            algo: Algo::Scan,
            m: None,
            ls: None,
            col: None,
            asm: None,
            check: None,
            stepper: Stepper::default(),
        };
        l.rebuild();
        l
    }
}

fn reg_index(r: Reg) -> usize {
    POOL.iter().position(|p| *p == r).unwrap_or(0)
}

impl Lesson {
    fn rebuild(&mut self) {
        self.ir = IrFront::new(&self.src);
        self.cfgs = cfgs(&self.ir);
        self.func = self.ir.default_func();
        self.recompute();
    }

    fn recompute(&mut self) {
        let alloc_all = |algo: Algo, k: usize, c: &Cfg| {
            let (m, _) = select(c, true);
            let a = match algo {
                Algo::Scan => linear_scan(&m, k).alloc,
                Algo::Color => color(&m, k).alloc,
            };
            finalize(&m, &a)
        };
        self.m = self.cfgs.get(self.func).map(|c| select(c, true).0);
        self.ls = self.m.as_ref().map(|m| linear_scan(m, self.k));
        self.col = self.m.as_ref().map(|m| color(m, self.k));
        self.asm = self.m.as_ref().map(|m| {
            let a = match self.algo {
                Algo::Scan => &self.ls.as_ref().unwrap().alloc,
                Algo::Color => &self.col.as_ref().unwrap().alloc,
            };
            finalize(m, a)
        });
        self.check = (self.cfgs.iter().any(|c| c.func.name == "main")).then(|| {
            let asm: Vec<AsmFunc> = self.cfgs.iter().map(|c| alloc_all(self.algo, self.k, c)).collect();
            let r = sim::run(&asm, 0);
            (interp::run(&self.cfgs), r.error.map_or(Ok(r.output), Err))
        });
        self.stepper.reset();
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        ui.heading("6.3  寄存器分配");
        ui.label(RichText::new("进阶 · 约 45 分钟").color(p.muted));

        ui.add_space(8.0);
        ui.label(RichText::new("一句话").size(20.0).strong());
        prose(
            ui,
            "寄存器是 CPU 里最快的存储，但数量很少（RISC-V 有 32 个，能随便用的更少）。指令选择时假设了无限多的**虚拟寄存器**，\
             现在要把它们装进有限的物理寄存器。关键观察：**同时活跃**的两个值不能放进同一个寄存器，不同时活跃的可以轮流用。\
             实在放不下，就把某些值**溢出**到内存（栈）里，用的时候再读回来——内存比寄存器慢得多，所以要尽量少溢出。",
        );
        callout(ui, CalloutKind::Analogy, "会议室排期", |ui| {
            prose(
                ui,
                "每个值是一场会，活跃区间是开会的时间段，寄存器是会议室。时间不重叠的会可以用同一间会议室。\
                 会议室不够时，只好让某场会去楼下咖啡馆开（溢出到内存）——该让哪场会去？让还要开很久的那场，腾出的会议室最有用。",
            );
        });

        ui.add_space(8.0);
        ui.label(RichText::new("两种算法").size(20.0).strong());
        card(ui, |ui| {
            prose(
                ui,
                "**线性扫描**：把每个虚拟寄存器的活跃范围画成一条区间，按起点从左到右扫。先回收已经结束的区间占用的寄存器；\
                 有空闲寄存器就分配，没有就溢出**结束得最晚**的那个区间。一遍扫完，很快——Java、V8 等 JIT 编译器常用。",
            );
            prose(
                ui,
                "**图着色**：同时活跃的两个虚拟寄存器之间连一条边，得到**干涉图**；给图着色（K 种颜色 = K 个寄存器），相邻的点颜色不同。\
                 度数小于 K 的点一定能着上色（邻居最多用掉 K−1 种），先把它拿掉压栈（**简化**），图就变小了；\
                 如果剩下的点度数都 ≥ K，挑一个**可能溢出**的也压栈。最后倒着出栈、逐个着色。GCC、LLVM 的主力分配器都是图着色的变种。",
            );
        });

        ui.add_space(8.0);
        ui.label(RichText::new("单步观察").size(20.0).strong());
        if editor(ui, &mut self.src, &[MANY, SUM], 6, true) {
            self.rebuild();
        }
        if self.ir.check(ui) {
            let mut changed = func_picker(ui, &self.ir, &mut self.func);
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("算法：").color(p.muted));
                changed |= ui.selectable_value(&mut self.algo, Algo::Scan, "线性扫描").changed();
                changed |= ui.selectable_value(&mut self.algo, Algo::Color, "图着色").changed();
                ui.separator();
                ui.label(RichText::new("可用的寄存器个数 K：").color(p.muted));
                changed |= ui.add(Slider::new(&mut self.k, 1..=6)).changed();
            });
            if changed {
                self.recompute();
            }
            match self.algo {
                Algo::Scan => self.scan_view(ui),
                Algo::Color => self.color_view(ui),
            }
            self.result(ui);
        }

        ui.add_space(8.0);
        callout(ui, CalloutKind::KeyPoint, "分给谁：调用约定的影响", |ui| {
            prose(
                ui,
                "这里只从 s1–s11 里挑寄存器。它们是**被调用者保存**的：别的函数如果要用，得先存起来、返回前恢复，\
                 所以我们的值跨过函数调用也不会被破坏（代价是函数开头、结尾要多几条保存/恢复指令，6.4 课详细讲）。\
                 t5、t6 留给溢出代码临时中转：溢出的值先 `ld` 进 t5，算完再 `sd` 回去。",
            );
        });

        ui.add_space(12.0);
        ui.label(RichText::new("小测验").size(20.0).strong());
        quiz(
            ui,
            "ch6-ra-1",
            "图着色时，为什么度数小于 K 的点可以放心地先拿掉？",
            &["它的邻居最多占用 K−1 种颜色，放回来时一定还剩一种", "它不重要", "它一定会被溢出"],
            0,
            "这是 Chaitin 算法的核心观察。拿掉它以后，其他点的度数也跟着变小，可能又出现新的\"好拿掉\"的点。",
        );
        quiz(
            ui,
            "ch6-ra-2",
            "线性扫描寄存器不够时，为什么溢出\"结束得最晚\"的区间？",
            &["它占着寄存器的时间最长，腾出来对后面的区间最有用", "它最不重要", "随便选一个也一样"],
            0,
            "这是一个贪心策略，和操作系统里\"最久以后才用到的页面先换出去\"（Belady 算法）是同一个道理。",
        );

        ui.add_space(12.0);
        callout(ui, CalloutKind::Deeper, "更多技巧", |ui| {
            prose(
                ui,
                "**合并**（coalescing）：`mv a, b` 的两端如果不冲突，就分到同一个寄存器，这条 mv 就能删掉（这里的实现在建干涉图时跳过了 mv 两端之间的边，\
                 结果里\"删掉了几条自复制\"就是这么来的）。**活跃范围拆分**：一个值只在一部分区间溢出，其余时间还在寄存器里。\
                 SSA 形式的干涉图是**弦图**，可以在多项式时间内最优着色——这是近年\"在 SSA 上直接分配寄存器\"研究的出发点。",
            );
        });
        ui.add_space(24.0);
    }

    fn code_listing(&self, ui: &mut Ui, focus: Option<usize>) {
        let p = Palette::of(ui);
        let Some(m) = &self.m else { return };
        let mut lines = Vec::new();
        let mut idx = 0;
        for b in &m.blocks {
            lines.push((format!("{}:", b.label), None));
            for i in &b.insts {
                let bg = (focus == Some(idx)).then_some(p.focus_bg);
                lines.push((format!("{idx:>3}  {}", i.text(&m.vregs)), bg));
                idx += 1;
            }
        }
        listing(ui, "指令选择的结果（左边是指令编号）", &lines);
    }

    fn scan_view(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        let (Some(m), Some(ls)) = (&self.m, &self.ls) else { return };
        self.stepper.ui(ui, ls.steps.len());
        let k = self.stepper.pos;
        let mut assigned: Vec<Option<Reg>> = vec![None; m.vregs.len()];
        let mut spilled = vec![false; m.vregs.len()];
        let mut visited = vec![false; m.vregs.len()];
        let mut expired = vec![false; m.vregs.len()];
        for s in &ls.steps[..k] {
            match s {
                ScanStep::Visit(iv) => visited[iv.vreg as usize] = true,
                ScanStep::Expire { vreg, .. } => expired[*vreg as usize] = true,
                ScanStep::Assign { vreg, reg } => assigned[*vreg as usize] = Some(*reg),
                ScanStep::SpillSelf { vreg } => spilled[*vreg as usize] = true,
                ScanStep::SpillOther { vreg, victim, reg } => {
                    spilled[*victim as usize] = true;
                    assigned[*victim as usize] = None;
                    assigned[*vreg as usize] = Some(*reg);
                }
            }
        }
        let v = |x: u32| format!("%{}", m.vregs[x as usize]);
        let rn = |r: Reg| r.name(&[]);
        let last = k.checked_sub(1).map(|i| &ls.steps[i]);
        let explain = match last {
            None => format!("先算出每个虚拟寄存器的活跃区间，按起点排好。可用寄存器：{} 个。", self.k),
            Some(ScanStep::Visit(iv)) => {
                format!("扫描到 {}：活跃区间是第 {} 到第 {} 条指令。", v(iv.vreg), iv.start, iv.end)
            }
            Some(ScanStep::Expire { vreg, reg }) => {
                format!("{} 的区间在当前位置之前已经结束，它占的 {} 空出来了。", v(*vreg), rn(*reg))
            }
            Some(ScanStep::Assign { vreg, reg }) => format!("有空闲寄存器：{} 分到 {}。", v(*vreg), rn(*reg)),
            Some(ScanStep::SpillSelf { vreg }) => {
                format!("寄存器全被占着，而且 {} 自己结束得最晚：把它溢出到栈上。", v(*vreg))
            }
            Some(ScanStep::SpillOther { vreg, victim, reg }) => format!(
                "寄存器全被占着。正在占用的区间里 {} 结束得最晚（比 {} 还晚）：把它溢出到栈上，腾出的 {} 给 {}。",
                v(*victim),
                v(*vreg),
                rn(*reg),
                v(*vreg)
            ),
        };
        prose_sized(ui, &explain, 17.0);
        let pos = ls.steps[..k].iter().rev().find_map(|s| match s {
            ScanStep::Visit(iv) => Some(iv.start),
            _ => None,
        });
        let cur = ls.steps[..k].iter().rev().find_map(|s| match s {
            ScanStep::Visit(iv) => Some(iv.vreg),
            _ => None,
        });

        // 区间图
        ui.label(RichText::new("活跃区间（颜色 = 分到的寄存器，红色 = 溢出）").strong());
        card(ui, |ui| {
            let n = ls.intervals.iter().map(|i| i.end + 1).max().unwrap_or(1);
            let left = 70.0;
            let row = 24.0;
            let w = ui.available_width();
            let dx = ((w - left - 10.0) / n as f32).clamp(6.0, 26.0);
            let h = ls.intervals.len() as f32 * row + 24.0;
            let (resp, painter) = ui.allocate_painter(Vec2::new(w, h), Sense::hover());
            let o = resp.rect.min;
            for t in (0..n).step_by(5) {
                let x = o.x + left + t as f32 * dx;
                painter.text(
                    Pos2::new(x, o.y + h - 10.0),
                    Align2::LEFT_CENTER,
                    t.to_string(),
                    FontId::proportional(11.0),
                    p.muted,
                );
            }
            for (r, iv) in ls.intervals.iter().enumerate() {
                let y = o.y + r as f32 * row;
                let vi = iv.vreg as usize;
                painter.text(Pos2::new(o.x + 4.0, y + row / 2.0), Align2::LEFT_CENTER, v(iv.vreg), mono(13.0), p.text);
                let rect = Rect::from_min_max(
                    Pos2::new(o.x + left + iv.start as f32 * dx, y + 4.0),
                    Pos2::new(o.x + left + (iv.end + 1) as f32 * dx - 2.0, y + row - 4.0),
                );
                let fill = if spilled[vi] {
                    p.error_bg
                } else if let Some(reg) = assigned[vi] {
                    p.category(reg_index(reg)).gamma_multiply(if expired[vi] { 0.35 } else { 0.8 })
                } else if visited[vi] {
                    p.focus_bg
                } else {
                    p.card_stroke.gamma_multiply(0.5)
                };
                painter.rect_filled(rect, 4.0, fill);
                if cur == Some(iv.vreg) {
                    painter.rect_stroke(rect, 4.0, Stroke::new(2.0, p.accent), egui::StrokeKind::Outside);
                }
                if spilled[vi] {
                    painter.rect_stroke(rect, 4.0, Stroke::new(1.0, p.error), egui::StrokeKind::Inside);
                }
                let label = if spilled[vi] { "栈".to_owned() } else { assigned[vi].map_or(String::new(), rn) };
                painter.text(rect.left_center() + Vec2::new(4.0, 0.0), Align2::LEFT_CENTER, label, mono(12.0), p.text);
            }
            if let Some(t) = pos {
                let x = o.x + left + t as f32 * dx;
                painter.line_segment([Pos2::new(x, o.y), Pos2::new(x, o.y + h - 18.0)], Stroke::new(1.5, p.accent));
            }
        });
        self.code_listing(ui, pos);
    }

    fn color_view(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        let (Some(m), Some(col)) = (&self.m, &self.col) else { return };
        self.stepper.ui(ui, col.steps.len());
        let k = self.stepper.pos;
        let mut stack: Vec<u32> = Vec::new();
        let mut potential: Vec<u32> = Vec::new();
        let mut colors: Vec<Option<Option<Reg>>> = vec![None; m.vregs.len()];
        for s in &col.steps[..k] {
            match s {
                ColorStep::Simplify { vreg, .. } => stack.push(*vreg),
                ColorStep::Potential { vreg, .. } => {
                    stack.push(*vreg);
                    potential.push(*vreg);
                }
                ColorStep::Select { vreg, reg } => {
                    stack.pop();
                    colors[*vreg as usize] = Some(*reg);
                }
            }
        }
        let v = |x: u32| format!("%{}", m.vregs[x as usize]);
        let last = k.checked_sub(1).map(|i| &col.steps[i]);
        let explain = match last {
            None => format!(
                "干涉图有 {} 个点、{} 条边：连线的两个虚拟寄存器同时活跃，不能共用寄存器。K = {}。",
                col.nodes.len(),
                col.edges.len(),
                self.k
            ),
            Some(ColorStep::Simplify { vreg, degree }) => {
                format!("简化：{} 在剩下的图里度数是 {degree} < K，一定能着色，拿掉压栈。", v(*vreg))
            }
            Some(ColorStep::Potential { vreg, degree }) => format!(
                "剩下的点度数都 ≥ K。挑度数最大的 {}（度数 {degree}）作为\"可能溢出\"压栈——乐观地希望放回去时邻居颜色有重复。",
                v(*vreg)
            ),
            Some(ColorStep::Select { vreg, reg: Some(r) }) => {
                format!("出栈 {}：邻居没用 {}，给它着这个颜色。", v(*vreg), r.name(&[]))
            }
            Some(ColorStep::Select { vreg, reg: None }) => {
                format!("出栈 {}：K 种颜色都被邻居用了，只能真的溢出到栈上。", v(*vreg))
            }
        };
        prose_sized(ui, &explain, 17.0);
        let focus = match last {
            Some(
                ColorStep::Simplify { vreg, .. } | ColorStep::Potential { vreg, .. } | ColorStep::Select { vreg, .. },
            ) => Some(*vreg),
            None => None,
        };

        ui.label(RichText::new("干涉图（颜色 = 分到的寄存器，虚线框 = 已压栈，红色 = 溢出）").strong());
        card(ui, |ui| {
            let n = col.nodes.len().max(1);
            let w = ui.available_width().min(640.0);
            let radius = (w / 2.0 - 50.0).min(40.0 + n as f32 * 14.0);
            let (resp, painter) = ui.allocate_painter(Vec2::new(w, radius * 2.0 + 70.0), Sense::hover());
            let c = resp.rect.center();
            let at = |i: usize| {
                let a = i as f32 / n as f32 * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
                c + Vec2::new(a.cos(), a.sin()) * radius
            };
            let idx = |x: u32| col.nodes.iter().position(|&y| y == x).unwrap();
            for &(a, b) in &col.edges {
                let gone = |x: u32| stack.contains(&x);
                let color = if gone(a) || gone(b) { p.card_stroke } else { p.node_stroke };
                painter.line_segment([at(idx(a)), at(idx(b))], Stroke::new(1.2, color));
            }
            for (i, &x) in col.nodes.iter().enumerate() {
                let pos = at(i);
                let (fill, stroke) = match colors[x as usize] {
                    Some(Some(r)) => (p.category(reg_index(r)).gamma_multiply(0.8), Stroke::new(1.5, p.node_stroke)),
                    Some(None) => (p.error_bg, Stroke::new(1.5, p.error)),
                    None if stack.contains(&x) => (p.card_bg, Stroke::new(1.0, p.muted)),
                    None => (p.node_fill, Stroke::new(1.5, p.node_stroke)),
                };
                painter.circle_filled(pos, 22.0, fill);
                let s = if focus == Some(x) { Stroke::new(3.0, p.accent) } else { stroke };
                painter.circle_stroke(pos, 22.0, s);
                painter.text(pos - Vec2::new(0.0, 5.0), Align2::CENTER_CENTER, v(x), mono(12.0), p.text);
                if let Some(Some(r)) = colors[x as usize] {
                    painter.text(pos + Vec2::new(0.0, 9.0), Align2::CENTER_CENTER, r.name(&[]), mono(11.0), p.text);
                }
            }
        });
        let st: Vec<String> = stack.iter().map(|&x| v(x)).collect();
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("栈（右边是栈顶）：").strong());
            ui.label(RichText::new(if st.is_empty() { "（空）".to_owned() } else { st.join("  ") }).font(mono(14.0)));
        });
        self.code_listing(ui, None);
    }

    fn result(&self, ui: &mut Ui) {
        let p = Palette::of(ui);
        let (Some(asm), Some(m)) = (&self.asm, &self.m) else { return };
        let alloc = match self.algo {
            Algo::Scan => &self.ls.as_ref().unwrap().alloc,
            Algo::Color => &self.col.as_ref().unwrap().alloc,
        };
        let map: Vec<String> = alloc
            .loc
            .iter()
            .enumerate()
            .filter_map(|(i, l)| {
                l.map(|l| {
                    let to = match l {
                        Loc::Reg(r) => r.name(&[]),
                        Loc::Slot(s) => format!("栈槽{s}"),
                    };
                    format!("%{} → {to}", m.vregs[i])
                })
            })
            .collect();
        ui.add_space(6.0);
        ui.label(RichText::new("分配结果").strong());
        card(ui, |ui| {
            ui.label(RichText::new(map.join("    ")).font(mono(13.0)));
            ui.label(
                RichText::new(format!(
                    "溢出 {} 个；删掉了 {} 条\"自己复制给自己\"的 mv；栈帧 {} 字节。",
                    alloc.slots, asm.removed_moves, asm.frame.size
                ))
                .color(p.muted),
            );
        });
        let lines: Vec<(String, Option<Color32>)> = asm
            .blocks
            .iter()
            .flat_map(|b| {
                std::iter::once((format!("{}:", b.label), None))
                    .chain(b.insts.iter().map(|i| (format!("    {}", i.text(&[])), None)))
            })
            .collect();
        listing(ui, "分配之后的汇编（加上了保存/恢复现场的代码，溢出的值用 ld/sd 读写）", &lines);
        if let Some((a, b)) = &self.check {
            let show = |r: &RunResult| match r {
                Ok(v) => format!("{v:?}"),
                Err(e) => e.clone(),
            };
            let ok = a == b;
            let t = format!(
                "整个程序：IR 解释器打印 {}，把分配好的汇编放进 RISC-V 模拟器运行打印 {}{}",
                show(a),
                show(b),
                if ok { "，一致。" } else { "，不一致！" }
            );
            ui.label(RichText::new(t).color(if ok { p.ok } else { p.error }).strong());
        }
    }
}
