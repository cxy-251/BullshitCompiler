//! 5.5 稀疏条件常量传播（SCCP）。

use bsc_minilang::cfg::Cfg;
use bsc_minilang::ir::Inst;
use bsc_minilang::sccp::{Sccp, Step, Val, sccp};
use bsc_minilang::ssa;
use eframe::egui::{Color32, RichText, Stroke, Ui};

use super::{AREA, NEVER, SUM, run_check};
use crate::pages::learn::ch3::editor;
use crate::pages::learn::ch4::{IrFront, block_card, blocks_grid, cfg_graph, func_picker};
use crate::theme::{Palette, mono};
use crate::widgets::{
    CalloutKind, GEdgeStyle, GNodeStyle, Stepper, callout, card, chip, graph_view, prose, prose_sized, quiz,
};

pub struct Lesson {
    src: String,
    ir: IrFront,
    func: usize,
    /// 每个函数的 SSA 形式。
    ssas: Vec<Cfg>,
    cond: Option<Sccp>,
    plain: Option<Sccp>,
    stepper: Stepper,
}

impl Default for Lesson {
    fn default() -> Self {
        let mut l = Self {
            src: NEVER.1.to_owned(),
            ir: IrFront::new(""),
            func: 0,
            ssas: vec![],
            cond: None,
            plain: None,
            stepper: Stepper::default(),
        };
        l.rebuild();
        l
    }
}

fn val_style(p: &Palette, v: Val) -> (Color32, Stroke) {
    match v {
        Val::Top => (p.card_bg, Stroke::new(1.0, p.card_stroke)),
        Val::Const(_) => (p.ok.gamma_multiply(0.2), Stroke::new(1.0, p.ok)),
        Val::Bottom => (p.error_bg, Stroke::new(1.0, p.error)),
    }
}

impl Lesson {
    fn rebuild(&mut self) {
        self.ir = IrFront::new(&self.src);
        self.func = self.ir.default_func();
        self.ssas = self.ir.cfgs.iter().map(|c| ssa::build(&c.compact()).after).collect();
        self.select();
    }

    fn select(&mut self) {
        self.cond = self.ssas.get(self.func).map(|c| sccp(c, true));
        self.plain = self.ssas.get(self.func).map(|c| sccp(c, false));
        self.stepper.reset();
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        ui.heading("5.5  稀疏条件常量传播");
        ui.label(RichText::new("进阶 · 约 40 分钟").color(p.muted));

        ui.add_space(8.0);
        ui.label(RichText::new("一句话").size(20.0).strong());
        prose(
            ui,
            "5.1 课的常量传播只在一个基本块内部做。跨块的常量传播要回答：\"x 在所有可能的执行路径上是不是同一个常量？\"\
             **SCCP**（Wegman & Zadeck, 1991）在 SSA 形式上回答这个问题，而且同时追踪**哪些代码可能被执行**：\
             条件是常量的分支，另一边永远不会执行，那边的赋值就不该影响结果。",
        );
        callout(ui, CalloutKind::Analogy, "乐观的侦探", |ui| {
            prose(
                ui,
                "普通的侦探一开始就怀疑所有人（所有分支都可能走），SCCP 是乐观的侦探：先假设每个变量都可能是常量（⊤），\
                 只有真的看到两个不同的值时才放弃（⊥）；先假设所有代码都走不到，只有真的有路进来时才去看它。\
                 乐观的假设如果最后没被推翻，就成了证明。",
            );
        });

        ui.add_space(8.0);
        ui.label(RichText::new("算法").size(20.0).strong());
        card(ui, |ui| {
            prose(
                ui,
                "每个 SSA 变量的值是 5.4 课的常量格：⊤（还不知道）→ 常量 → ⊥（不是常量），只能往下走。维护两个工作表：",
            );
            prose(
                ui,
                "• **控制流工作表**：新变成\"可执行\"的边。块第一次被执行到时，计算其中所有指令；以后每多一条可执行的入边，重新计算它的 φ。",
            );
            prose(ui, "• **SSA 工作表**：值刚刚下降的变量。只重新计算**用到它**的指令——这就是\"稀疏\"。");
            prose(
                ui,
                "φ 只汇合来自**可执行边**的值。条件跳转的条件是常量时，只把会走的那条边标为可执行；是 ⊥ 时两边都可执行；是 ⊤ 时先都不走。",
            );
        });

        ui.add_space(8.0);
        ui.label(RichText::new("单步观察").size(20.0).strong());
        if editor(ui, &mut self.src, &[NEVER, SUM, AREA], 6, true) {
            self.rebuild();
        }
        if self.ir.check(ui) {
            if func_picker(ui, &self.ir, &mut self.func) {
                self.select();
            }
            self.stepping(ui);
        }

        ui.add_space(8.0);
        callout(ui, CalloutKind::KeyPoint, "为什么\"条件\"很重要", |ui| {
            prose(
                ui,
                "\"走不到的分支\"例子里，`x = 2` 只在 `x != 1` 时执行，而 x 一开始就是 1。普通的常量传播认为两边都可能走，\
                 循环头的 φ 汇合了 1 和 2，只好判为 ⊥。SCCP 乐观地先不走那个分支：只要 x 是 1，条件就是假，分支就永远走不到，x 就一直是 1——\
                 一个自洽的结论。页面最后对比了两种做法的结果。",
            );
        });

        ui.add_space(12.0);
        ui.label(RichText::new("小测验").size(20.0).strong());
        quiz(
            ui,
            "ch5-sccp-1",
            "SCCP 的\"稀疏\"指的是？",
            &["变量的值变了，只重新计算用到它的指令，而不是整个函数", "只分析一部分函数", "只处理稀疏矩阵"],
            0,
            "SSA 让\"谁用到了这个变量\"一目了然，所以只需沿着定义-使用关系传播变化。",
        );
        quiz(
            ui,
            "ch5-sccp-2",
            "条件跳转的条件还是 ⊤ 时，SCCP 怎么处理？",
            &["两边都标为可执行", "暂时哪边都不标，等条件的值确定了再说", "报错"],
            1,
            "⊤ 表示还没有任何值流到这里，乐观地先不走；以后条件变成常量或 ⊥ 时，会重新计算这条跳转。",
        );

        ui.add_space(12.0);
        callout(ui, CalloutKind::Deeper, "组合优化的力量", |ui| {
            prose(
                ui,
                "SCCP 同时做了常量传播和不可达代码消除，而且两者互相帮助，效果比\"先做一个再做另一个\"反复多少遍都好——\
                 这是\"组合分析\"比\"分开分析\"更强的经典例子。LLVM 有 SCCP 和它的跨函数版本 IPSCCP；\
                 GCC 的 CCP pass 也基于同样的思路。",
            );
        });
        ui.add_space(24.0);
    }

    fn stepping(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        let (Some(s), Some(plain)) = (&self.cond, &self.plain) else { return };
        let cfg = &s.input;
        self.stepper.ui(ui, s.steps.len());
        let k = self.stepper.pos;
        let last = k.checked_sub(1).map(|i| &s.steps[i]);

        let mut values = vec![Val::Top; cfg.func.vars.len()];
        let mut exec = vec![false; cfg.blocks.len()];
        let mut edges: Vec<(usize, usize)> = Vec::new();
        for st in &s.steps[..k] {
            match st {
                Step::Edge { from, to, .. } => {
                    exec[*to] = true;
                    if let Some(f) = from {
                        edges.push((*f, *to));
                    }
                }
                Step::Eval { block, inst, after, .. } => {
                    if let Some(d) = cfg.blocks[*block].insts[*inst].def() {
                        values[d] = *after;
                    }
                }
                Step::Branch { .. } => {}
            }
        }
        let name = |b: usize| cfg.blocks[b].name.clone();
        let explain = match last {
            None => "开始：所有变量都是 ⊤，所有块都不可执行。控制流工作表里只有\"函数入口\"。".to_owned(),
            Some(Step::Edge { from: None, to, .. }) => {
                format!("从入口开始：{} 可执行。第一次执行到它，计算其中所有指令。", name(*to))
            }
            Some(Step::Edge { from: Some(f), to, first }) => {
                let tail = if *first {
                    format!("{} 第一次被执行到，计算它的所有指令。", name(*to))
                } else {
                    format!("{} 以前就执行过，只需重新计算它的 φ（多了一条可能的来路）。", name(*to))
                };
                format!("边 {} → {} 变成可执行。{tail}", name(*f), name(*to))
            }
            Some(Step::Eval { block, inst, before, after }) => {
                let i = &cfg.blocks[*block].insts[*inst];
                let phi = if matches!(i, Inst::Phi { .. }) { "（φ 只汇合来自可执行边的值）" } else { "" };
                let tail = if before == after {
                    "没变。".to_owned()
                } else {
                    "下降了 → 用到它的指令放进 SSA 工作表，稍后重新计算。".to_owned()
                };
                format!("计算 `{}`{phi}：{} → {}，{tail}", cfg.inst_text(i), before.show(), after.show())
            }
            Some(Step::Branch { cond, targets, .. }) => {
                let t: Vec<String> = targets.iter().map(|&b| name(b)).collect();
                match cond {
                    Val::Top => "条件跳转：条件还是 ⊤，暂时哪边都不走。".to_owned(),
                    Val::Const(c) => {
                        format!("条件跳转：条件是常量 {c}，只有去 {} 的边可执行，另一边走不到。", t.join(""))
                    }
                    Val::Bottom => format!("条件跳转：条件是 ⊥，两边（{}）都可能走。", t.join("、")),
                }
            }
        };
        prose_sized(ui, &explain, 17.0);

        let (focus_block, focus_inst) = match last {
            Some(Step::Eval { block, inst, .. } | Step::Branch { block, inst, .. }) => (Some(*block), Some(*inst)),
            Some(Step::Edge { to, .. }) => (Some(*to), None),
            None => (None, None),
        };
        let g = cfg_graph(cfg, |_| None);
        let edge_list: Vec<(usize, usize)> = g.edges.iter().map(|e| (e.from, e.to)).collect();
        card(ui, |ui| {
            graph_view(
                ui,
                "sccp_graph",
                &g,
                |b| {
                    if focus_block == Some(b) {
                        GNodeStyle::focus(&p)
                    } else if exec[b] {
                        GNodeStyle::filled(&p, p.ok.gamma_multiply(0.2))
                    } else {
                        GNodeStyle { visible: true, fill: p.card_bg, stroke: p.card_stroke, emphasized: false }
                    }
                },
                |e| {
                    if edges.contains(&edge_list[e]) {
                        GEdgeStyle { visible: true, color: p.ok, emphasized: true }
                    } else {
                        GEdgeStyle { visible: true, color: p.card_stroke, emphasized: false }
                    }
                },
            );
            ui.label(
                RichText::new("绿色的块和边：已经确定可能被执行。灰色的：目前还认为走不到。").small().color(p.muted),
            );
        });

        ui.label(RichText::new("每个 SSA 变量当前的值").strong());
        let changed_var = match last {
            Some(Step::Eval { block, inst, .. }) => cfg.blocks[*block].insts[*inst].def(),
            _ => None,
        };
        card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                for (v, var) in cfg.func.vars.iter().enumerate() {
                    if var.ssa.is_none() && cfg.blocks.iter().flat_map(|b| &b.insts).all(|i| i.def() != Some(v)) {
                        continue; // 被重命名掉的原变量不再出现
                    }
                    let (mut fill, mut stroke) = val_style(&p, values[v]);
                    if changed_var == Some(v) {
                        fill = p.focus_bg;
                        stroke = Stroke::new(2.0, p.accent);
                    }
                    chip(
                        ui,
                        RichText::new(format!("{} = {}", var.name, values[v].show())).font(mono(14.0)),
                        fill,
                        stroke,
                    );
                }
            });
        });

        blocks_grid(ui, cfg, |ui, b| {
            let lines: Vec<(String, Option<Color32>)> = cfg.blocks[b]
                .insts
                .iter()
                .enumerate()
                .map(|(i, inst)| {
                    let bg = (focus_block == Some(b) && focus_inst == Some(i)).then_some(p.focus_bg);
                    (cfg.inst_text(inst), bg)
                })
                .collect();
            block_card(ui, cfg, b, false, &lines);
            if !exec[b] && k == s.steps.len() {
                ui.label(RichText::new(format!("{} 永远不会执行", name(b))).small().color(p.error));
            }
            ui.add_space(4.0);
        });

        if k == s.steps.len() {
            ui.add_space(6.0);
            ui.label(RichText::new("对比：不追踪可执行边的普通常量传播").strong());
            card(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for (v, var) in cfg.func.vars.iter().enumerate() {
                        if s.values[v] != plain.values[v] {
                            let (fill, stroke) = val_style(&p, plain.values[v]);
                            let t = format!(
                                "{}：SCCP 得 {}，普通做法得 {}",
                                var.name,
                                s.values[v].show(),
                                plain.values[v].show()
                            );
                            chip(ui, RichText::new(t).font(mono(14.0)), fill, stroke);
                        }
                    }
                });
                let diff = cfg.func.vars.iter().enumerate().any(|(v, _)| s.values[v] != plain.values[v]);
                if !diff {
                    ui.label(RichText::new("这个函数里两种做法的结论一样。").color(p.muted));
                }
            });
            ui.label(RichText::new("用 SCCP 的结论改写后的程序：常量代入、删掉常量的定义和走不到的块").strong());
            let out = &s.output;
            blocks_grid(ui, out, |ui, b| {
                let lines: Vec<(String, Option<Color32>)> =
                    out.blocks[b].insts.iter().map(|i| (out.inst_text(i), None)).collect();
                block_card(ui, out, b, false, &lines);
                ui.add_space(4.0);
            });
            let after: Vec<Cfg> = self.ssas.iter().map(|c| sccp(c, true).output).collect();
            run_check(ui, &self.ssas, &after);
        }
    }
}
