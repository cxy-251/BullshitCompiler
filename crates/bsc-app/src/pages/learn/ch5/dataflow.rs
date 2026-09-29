//! 5.2 数据流分析：活跃变量、到达定值，工作表算法。

use bsc_minilang::cfg::Cfg;
use bsc_minilang::dataflow::{Def, Direction, Problem, Solution, liveness, reaching_definitions, solve};
use eframe::egui::{self, RichText, Stroke, Ui};

use super::{AREA, NEVER, SUM};
use crate::pages::learn::ch3::editor;
use crate::pages::learn::ch4::{IrFront, cfg_graph, func_picker};
use crate::theme::{Palette, mono};
use crate::widgets::{
    CalloutKind, GEdgeStyle, GNodeStyle, Stepper, callout, card, chip, graph_view, prose, prose_sized, quiz,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Analysis {
    Live,
    Reaching,
}

pub struct Lesson {
    src: String,
    ir: IrFront,
    func: usize,
    analysis: Analysis,
    cfg: Option<Cfg>,
    problem: Option<Problem>,
    defs: Vec<Def>,
    sol: Option<Solution>,
    stepper: Stepper,
}

impl Default for Lesson {
    fn default() -> Self {
        let mut l = Self {
            src: SUM.1.to_owned(),
            ir: IrFront::new(""),
            func: 0,
            analysis: Analysis::Live,
            cfg: None,
            problem: None,
            defs: vec![],
            sol: None,
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
        self.select();
    }

    fn select(&mut self) {
        self.cfg = self.ir.cfgs.get(self.func).map(Cfg::compact);
        (self.problem, self.defs) = match (&self.cfg, self.analysis) {
            (Some(c), Analysis::Live) => (Some(liveness(c)), vec![]),
            (Some(c), Analysis::Reaching) => {
                let (p, d) = reaching_definitions(c);
                (Some(p), d)
            }
            _ => (None, vec![]),
        };
        self.sol = match (&self.cfg, &self.problem) {
            (Some(c), Some(p)) => Some(solve(&c.succs(), p)),
            _ => None,
        };
        self.stepper.reset();
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        ui.heading("5.2  数据流分析");
        ui.label(RichText::new("入门 · 约 35 分钟").color(p.muted));

        ui.add_space(8.0);
        ui.label(RichText::new("一句话").size(20.0).strong());
        prose(
            ui,
            "优化之前得先回答一些关于程序的问题：\"这个变量之后还会被用到吗？\"\"执行到这里时，x 的值可能来自哪几次赋值？\"\
             **数据流分析**是回答这类问题的统一方法：给每个基本块算出\"进入时\"的信息 IN 和\"离开时\"的信息 OUT，\
             信息沿着控制流图的边流动，在汇合处合并，直到所有块都不再变化。",
        );
        callout(ui, CalloutKind::Analogy, "传话游戏", |ui| {
            prose(
                ui,
                "每个块是一个人，从邻居那里听到消息（汇合），加上自己知道的、去掉自己知道已经过时的（传递函数），再告诉下一个人。\
                 有环时消息会绕回来，所以要一直传，直到谁听到的消息都不再变——这时的结果就是答案。",
            );
        });

        ui.add_space(8.0);
        ui.label(RichText::new("两个经典分析").size(20.0).strong());
        card(ui, |ui| {
            prose(
                ui,
                "**活跃变量**（反向）：变量 x 在某处\"活跃\"，是指从这里出发，存在一条路径会在 x 被重新赋值之前读到它。\
                 信息从后往前流：`OUT(b) = ∪ IN(后继)`，`IN(b) = use(b) ∪ (OUT(b) − def(b))`。\
                 use 是块里\"先用后赋值\"的变量，def 是块里赋过值的变量。用途：死代码消除、寄存器分配。",
            );
            ui.add_space(4.0);
            prose(
                ui,
                "**到达定值**（正向）：一次赋值 d（\"定值\"）能\"到达\"某处，是指存在一条从 d 到这里的路径，中途没有别的赋值覆盖它。\
                 信息从前往后流：`IN(b) = ∪ OUT(前驱)`，`OUT(b) = gen(b) ∪ (IN(b) − kill(b))`。\
                 gen 是块里每个变量的最后一次赋值，kill 是被块里的赋值覆盖掉的其他定值。用途：常量传播、找未初始化的变量。",
            );
        });
        prose(
            ui,
            "两者的形状一模一样，只是方向、gen/kill 不同。所以编译器只写**一个**求解器（本课用的就是同一份代码），\
             换上不同的 gen/kill 就能做不同的分析。求解用**工作表算法**：先把所有块放进工作表；每次取出一个块重新计算，\
             结果变了，就把受影响的邻居放回工作表；工作表空了，就收敛了。",
        );

        ui.add_space(8.0);
        ui.label(RichText::new("单步观察").size(20.0).strong());
        if editor(ui, &mut self.src, &[SUM, AREA, NEVER], 6, true) {
            self.rebuild();
        }
        if self.ir.check(ui) {
            let mut changed = func_picker(ui, &self.ir, &mut self.func);
            ui.horizontal(|ui| {
                ui.label(RichText::new("分析：").color(p.muted));
                changed |= ui.selectable_value(&mut self.analysis, Analysis::Live, "活跃变量（反向）").changed();
                changed |= ui.selectable_value(&mut self.analysis, Analysis::Reaching, "到达定值（正向）").changed();
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
            "ch5-df-1",
            "活跃变量分析为什么是\"反向\"的？",
            &["因为\"之后会不会被用到\"取决于后面的代码，信息要从后往前传", "因为反向算得更快", "这是随便规定的"],
            0,
            "一个变量此刻活不活跃，要看它将来会不会被读——将来的代码在后继块里，所以从后继往前推。",
        );
        quiz(
            ui,
            "ch5-df-2",
            "工作表算法里，一个块的结果变了，为什么只把它的邻居放回工作表？",
            &["只有邻居的输入受它影响，其他块的输入没变，重算也不会变", "为了省内存", "其他块已经永远不会变了"],
            0,
            "每个块的结果只依赖它的输入，而输入只来自邻居。只重算\"可能受影响\"的块，比一轮一轮全部重算快得多。",
        );

        ui.add_space(12.0);
        callout(ui, CalloutKind::Deeper, "为什么一定会停下来？答案为什么是对的？", |ui| {
            prose(
                ui,
                "集合只会变大（并集只增不减），而变量个数有限，所以每个块最多变有限次，算法一定终止。\
                 至于\"算出来的是不是最好的答案\"、\"换个计算顺序结果会不会不同\"，要用格与不动点理论来回答——这是 5.4 课的内容。\
                 还有一类分析（比如可用表达式）用交集汇合、从\"全集\"开始往小里算，4.3 课的支配关系就是这一类。",
            );
        });
        ui.add_space(24.0);
    }

    fn stepping(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        let (Some(cfg), Some(prob), Some(sol)) = (&self.cfg, &self.problem, &self.sol) else { return };
        let n = cfg.blocks.len();
        let forward = prob.dir == Direction::Forward;
        self.stepper.ui(ui, sol.steps.len());
        let k = self.stepper.pos;
        let last = k.checked_sub(1).map(|i| &sol.steps[i]);

        let mut inn = vec![Vec::new(); n];
        let mut out = vec![Vec::new(); n];
        for s in &sol.steps[..k] {
            if forward {
                (inn[s.block], out[s.block]) = (s.meet.clone(), s.result.clone());
            } else {
                (out[s.block], inn[s.block]) = (s.meet.clone(), s.result.clone());
            }
        }
        let name = |b: usize| cfg.blocks[b].name.clone();
        let elem = |x: usize| {
            if forward { format!("d{}", x + 1) } else { cfg.func.vars[x].name.clone() }
        };
        let set = |s: &[usize]| format!("{{{}}}", s.iter().map(|&x| elem(x)).collect::<Vec<_>>().join(", "));

        let explain = match last {
            None => {
                let order: Vec<String> =
                    sol.steps.first().map_or(vec![], |s| s.worklist.iter().map(|&b| name(b)).collect());
                format!(
                    "开始：所有块的 IN、OUT 都是空集，工作表里按{}放着 {}。",
                    if forward { "逆后序" } else { "后序（从出口往回）" },
                    order.join("、")
                )
            }
            Some(s) => {
                let b = name(s.block);
                let from: Vec<String> = if forward {
                    cfg.blocks[s.block].preds.iter().map(|&x| format!("OUT({})", name(x))).collect()
                } else {
                    cfg.blocks[s.block].succs.iter().map(|&x| format!("IN({})", name(x))).collect()
                };
                let from = if from.is_empty() { "∅（没有邻居）".to_owned() } else { from.join(" ∪ ") };
                let (m, r) = if forward { ("IN", "OUT") } else { ("OUT", "IN") };
                let (g, kl) = if forward { ("gen", "kill") } else { ("use", "def") };
                let tail = if s.changed {
                    let neighbors = if forward { &cfg.blocks[s.block].succs } else { &cfg.blocks[s.block].preds };
                    if neighbors.is_empty() {
                        format!("变了，但它没有{}，不影响别人。", if forward { "后继" } else { "前驱" })
                    } else if s.pushed.is_empty() {
                        "变了，但受影响的块都已经在工作表里。".to_owned()
                    } else {
                        let ps: Vec<String> = s.pushed.iter().map(|&x| name(x)).collect();
                        format!("变了 → 把{} {} 放回工作表。", if forward { "后继" } else { "前驱" }, ps.join("、"))
                    }
                } else {
                    "没变，不影响别人。".to_owned()
                };
                format!(
                    "取出 {b}：{m}({b}) = {from} = {}；{r}({b}) = {g} ∪ ({m} − {kl}) = {}。{tail}",
                    set(&s.meet),
                    set(&s.result)
                )
            }
        };
        prose_sized(ui, &explain, 17.0);

        // 工作表
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("工作表：").strong());
            let wl: Vec<usize> = match sol.steps.get(k) {
                Some(s) => s.worklist.clone(),
                None => vec![],
            };
            if wl.is_empty() {
                ui.label(RichText::new("（空）——收敛了").color(p.ok));
            }
            for (i, &b) in wl.iter().enumerate() {
                let (fill, stroke) = if i == 0 {
                    (p.focus_bg, Stroke::new(2.0, p.accent))
                } else {
                    (p.card_bg, Stroke::new(1.0, p.card_stroke))
                };
                chip(ui, RichText::new(name(b)).font(mono(14.0)), fill, stroke);
            }
            if !wl.is_empty() {
                ui.label(RichText::new("← 下一个取最左边的").small().color(p.muted));
            }
        });

        let g = cfg_graph(cfg, |_| None);
        let cur = last.map(|s| s.block);
        let pushed = last.map_or(vec![], |s| s.pushed.clone());
        card(ui, |ui| {
            graph_view(
                ui,
                "df_graph",
                &g,
                |b| {
                    if cur == Some(b) {
                        GNodeStyle::focus(&p)
                    } else if pushed.contains(&b) {
                        GNodeStyle::filled(&p, p.warn_bg)
                    } else {
                        GNodeStyle::normal(&p)
                    }
                },
                |_| GEdgeStyle::normal(&p),
            );
        });

        if !forward {
            ui.label(RichText::new("每个块的 use / def 和当前的 IN / OUT").strong());
        } else {
            let list: Vec<String> = self
                .defs
                .iter()
                .enumerate()
                .map(|(i, d)| {
                    format!("d{}: {}（{}）", i + 1, cfg.inst_text(&cfg.blocks[d.block].insts[d.inst]), name(d.block))
                })
                .collect();
            ui.label(RichText::new("定值编号").strong());
            card(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for l in list {
                        chip(ui, RichText::new(l).font(mono(13.0)), p.card_bg, Stroke::new(1.0, p.card_stroke));
                    }
                });
            });
            ui.label(RichText::new("每个块的 gen / kill 和当前的 IN / OUT").strong());
        }
        card(ui, |ui| {
            egui::Grid::new("df_table").num_columns(5).striped(true).spacing([24.0, 6.0]).show(ui, |ui| {
                let (g, kl) = if forward { ("gen", "kill") } else { ("use", "def") };
                for h in ["块", g, kl, "IN", "OUT"] {
                    ui.label(RichText::new(h).strong());
                }
                ui.end_row();
                for b in 0..n {
                    let mut t = RichText::new(name(b)).font(mono(14.0));
                    if cur == Some(b) {
                        t = t.background_color(p.focus_bg);
                    }
                    ui.label(t);
                    ui.label(RichText::new(set(&prob.gen_[b])).font(mono(14.0)));
                    ui.label(RichText::new(set(&prob.kill[b])).font(mono(14.0)));
                    let hl = |is_result: bool| cur == Some(b) && last.is_some_and(|s| s.changed) && is_result;
                    let mut i_t = RichText::new(set(&inn[b])).font(mono(14.0));
                    let mut o_t = RichText::new(set(&out[b])).font(mono(14.0));
                    if hl(!forward) {
                        i_t = i_t.background_color(p.warn_bg);
                    }
                    if hl(forward) {
                        o_t = o_t.background_color(p.warn_bg);
                    }
                    ui.label(i_t);
                    ui.label(o_t);
                    ui.end_row();
                }
            });
        });
        if k == sol.steps.len() {
            ui.label(RichText::new(format!("工作表空了，共取出 {} 次，得到最终结果。", k)).color(p.ok).strong());
        }
    }
}
