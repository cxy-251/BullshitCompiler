//! 5.3 死代码消除。

use bsc_minilang::cfg::Cfg;
use bsc_minilang::dataflow::live_after_each;
use bsc_minilang::opt::{Dce, dce, fold};
use eframe::egui::{Color32, RichText, Ui};

use super::{AREA, SUM, WEEK, run_check};
use crate::pages::learn::ch3::editor;
use crate::pages::learn::ch4::{IrFront, block_card, blocks_grid, func_picker};
use crate::theme::Palette;
use crate::widgets::{CalloutKind, Stepper, callout, prose, prose_sized, quiz};

pub struct Lesson {
    src: String,
    ir: IrFront,
    func: usize,
    fold_first: bool,
    inputs: Vec<Cfg>,
    dces: Vec<Dce>,
    stepper: Stepper,
}

impl Default for Lesson {
    fn default() -> Self {
        let mut l = Self {
            src: AREA.1.to_owned(),
            ir: IrFront::new(""),
            func: 0,
            fold_first: true,
            inputs: vec![],
            dces: vec![],
            stepper: Stepper::default(),
        };
        l.rebuild();
        l
    }
}

impl Lesson {
    fn rebuild(&mut self) {
        self.ir = IrFront::new(&self.src);
        self.func = self.ir.default_func();
        self.recompute();
    }

    fn recompute(&mut self) {
        self.inputs = self
            .ir
            .cfgs
            .iter()
            .map(|c| if self.fold_first { fold(&c.compact()).output } else { c.compact() })
            .collect();
        self.dces = self.inputs.iter().map(dce).collect();
        self.stepper.reset();
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        ui.heading("5.3  死代码消除");
        ui.label(RichText::new("入门 · 约 20 分钟").color(p.muted));

        ui.add_space(8.0);
        ui.label(RichText::new("一句话").size(20.0).strong());
        prose(
            ui,
            "算出来却没人用的值，算了也白算。如果一条指令赋值的变量在它之后**不再活跃**（5.2 课的活跃变量分析），\
             而且这条指令**没有副作用**（不打印、不调用函数、不写内存），就可以把它整条删掉。这就是**死代码消除**（DCE）。",
        );
        callout(ui, CalloutKind::Analogy, "没人看的报表", |ui| {
            prose(
                ui,
                "公司里有人每周都做一份报表，一问才发现根本没人看——那就别做了。但如果做报表的过程中顺便给客户发了邮件（副作用），\
                 就不能简单地不做。还有：不做这份报表以后，给它提供数据的那份报表也没人看了，也可以不做——所以要一轮一轮地查。",
            );
        });

        ui.add_space(8.0);
        ui.label(RichText::new("单步观察").size(20.0).strong());
        prose(
            ui,
            "每一轮先做活跃变量分析，每条指令右边写着\"执行完这条之后还活跃的变量\"。赋值的变量不在这个集合里，这条就是死的。",
        );
        if editor(ui, &mut self.src, &[AREA, WEEK, SUM], 6, true) {
            self.rebuild();
        }
        if self.ir.check(ui) {
            let mut changed = func_picker(ui, &self.ir, &mut self.func);
            changed |= ui.checkbox(&mut self.fold_first, "先做 5.1 的常量折叠（会产生更多死代码）").changed();
            if changed {
                self.recompute();
            }
            self.stepping(ui);
        }

        ui.add_space(8.0);
        callout(ui, CalloutKind::KeyPoint, "为什么函数调用不能删", |ui| {
            prose(
                ui,
                "`t1 = call f(x)` 的结果没人用，但 f 里面可能打印了东西、改了全局状态——删掉就改变了程序的行为。\
                 能不能删要看 f 是否\"纯\"（没有副作用），这需要跨函数的分析。C/C++ 编译器会给函数标上 `pure`、`const` 之类的属性来记录这件事。",
            );
        });
        callout(ui, CalloutKind::KeyPoint, "优化是一条流水线", |ui| {
            prose(
                ui,
                "勾掉\"先做常量折叠\"再看：能删的东西少多了。常量折叠把值传播出去，让原来的赋值变成死代码；\
                 死代码消除删掉它们，程序变短，又可能让别的优化更容易。真实编译器会把几十个 pass 排成流水线，有的 pass 还会跑好几遍。",
            );
        });

        ui.add_space(12.0);
        ui.label(RichText::new("小测验").size(20.0).strong());
        quiz(
            ui,
            "ch5-dce-1",
            "`a = 1; b = a + 2; c = b * 3; print(a);`，哪些赋值是死代码？",
            &["只有 c = b * 3", "c = b * 3 和 b = a + 2（删掉 c 之后 b 也没人用了）", "全部"],
            1,
            "第一轮删掉 c；第二轮发现 b 也不再活跃，删掉 b；a 被 print 用到，保留。",
        );
        quiz(
            ui,
            "ch5-dce-2",
            "为什么死代码消除要一轮一轮地做？",
            &["删掉一条指令，它用到的变量可能也跟着变成死的", "每轮只能删一条", "为了让动画好看"],
            0,
            "死代码会\"传染\"：给死代码提供输入的代码也是死的。也可以用\"标记-清除\"一次做完：先标记所有有用的，没标记的全删。",
        );

        ui.add_space(12.0);
        callout(ui, CalloutKind::Deeper, "更激进的死代码消除", |ui| {
            prose(
                ui,
                "在 SSA 上通常反过来做：先认定 return、打印、写内存这类指令\"有用\"，再沿着定义-使用链把它们依赖的指令都标记为有用，\
                 没被标记的统统删掉（Aggressive DCE）。它还能删掉整个没用的循环，以及永远不会影响结果的分支。\
                 链接时优化（LTO）更进一步，能删掉整个程序里从没被调用的函数。",
            );
        });
        ui.add_space(24.0);
    }

    fn stepping(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        let Some(d) = self.dces.get(self.func) else { return };
        // 步骤：每轮先"做活跃变量分析"，再逐条删除
        let mut steps: Vec<(usize, Option<usize>)> = Vec::new();
        for (r, round) in d.rounds.iter().enumerate() {
            steps.push((r, None));
            steps.extend((0..round.removed.len()).map(|j| (r, Some(j))));
        }
        self.stepper.ui(ui, steps.len());
        let k = self.stepper.pos;
        let Some(&(r, cur)) = k.checked_sub(1).and_then(|i| steps.get(i)) else {
            prose_sized(ui, "输入的程序如下。按\"下一步\"开始第一轮。", 17.0);
            show_blocks(ui, &self.inputs[self.func], None, &[], None, &p);
            return;
        };
        let round = &d.rounds[r];
        let explain = match cur {
            None if round.removed.is_empty() => {
                format!("第 {} 轮：重新做活跃变量分析，没有找到可以删的指令——死代码消除完成。", r + 1)
            }
            None => format!(
                "第 {} 轮：先做活跃变量分析（5.2 课），得到每条指令之后还活跃的变量。这一轮能删 {} 条。",
                r + 1,
                round.removed.len()
            ),
            Some(j) => {
                let c = &round.removed[j];
                format!("`{}`：{}，也不是函数调用 → 删除。", c.before, c.rule)
            }
        };
        prose_sized(ui, &explain, 17.0);
        let shown: Vec<(usize, usize)> = match cur {
            Some(j) => round.removed[..=j].iter().map(|c| (c.block, c.inst)).collect(),
            None => vec![],
        };
        let focus = cur.map(|j| (round.removed[j].block, round.removed[j].inst));
        show_blocks(ui, &round.cfg, Some(&round.live.out), &shown, focus, &p);
        if k == steps.len() {
            run_check(ui, &self.inputs, &self.dces.iter().map(|d| d.output.clone()).collect::<Vec<_>>());
        }
    }
}

/// 每个块的指令，右边写着执行完之后还活跃的变量。`removed` 里的指令标成要删除。
fn show_blocks(
    ui: &mut Ui,
    cfg: &Cfg,
    live_out: Option<&Vec<Vec<usize>>>,
    removed: &[(usize, usize)],
    focus: Option<(usize, usize)>,
    p: &Palette,
) {
    blocks_grid(ui, cfg, |ui, b| {
        let after = live_out.map(|lo| live_after_each(cfg, b, &lo[b]));
        let lines: Vec<(String, Option<Color32>)> = cfg.blocks[b]
            .insts
            .iter()
            .enumerate()
            .map(|(i, inst)| {
                let mut text = cfg.inst_text(inst);
                if let Some(a) = &after {
                    let names: Vec<String> = a[i].iter().map(|&v| cfg.func.vars[v].name.clone()).collect();
                    text = format!("{text:<20} 活跃 {{{}}}", names.join(","));
                }
                let bg = if focus == Some((b, i)) {
                    Some(p.error.gamma_multiply(0.35))
                } else if removed.contains(&(b, i)) {
                    Some(p.error_bg)
                } else {
                    None
                };
                (text, bg)
            })
            .collect();
        block_card(ui, cfg, b, false, &lines);
        ui.add_space(4.0);
    });
}
