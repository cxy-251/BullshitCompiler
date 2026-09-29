//! 4.4 SSA 形式。

use bsc_minilang::cfg::Cfg;
use bsc_minilang::interp;
use bsc_minilang::ir::{Inst, VarId};
use bsc_minilang::ssa::{Ssa, Step, build};
use eframe::egui::{Color32, RichText, Ui};

use super::{IrFront, PRESETS, block_card, blocks_grid, cfg_graph, func_picker};
use crate::pages::learn::ch3::editor;
use crate::theme::{Palette, mono};
use crate::widgets::{
    CalloutKind, GEdgeStyle, GNodeStyle, Stepper, callout, card, graph_view, prose, prose_sized, quiz,
};

type RunResult = Result<Vec<i64>, String>;

pub struct Lesson {
    src: String,
    ir: IrFront,
    func: usize,
    ssa: Option<Ssa>,
    /// 整个程序转换前、后分别运行的结果。
    runs: Option<(RunResult, RunResult)>,
    stepper: Stepper,
}

impl Default for Lesson {
    fn default() -> Self {
        let mut l = Self {
            src: PRESETS[0].1.to_owned(),
            ir: IrFront::new(""),
            func: 0,
            ssa: None,
            runs: None,
            stepper: Stepper::default(),
        };
        l.rebuild();
        l
    }
}

/// 回放到第 k 步时的状态。
struct State {
    globals: Option<Vec<VarId>>,
    /// 每个块已经插入的 φ 个数。
    phis: Vec<usize>,
    rewritten: Vec<Vec<bool>>,
    filled: Vec<(usize, usize, usize)>,
    stacks: Vec<Vec<VarId>>,
    current: Option<usize>,
    renaming: bool,
}

fn replay(ssa: &Ssa, steps: &[Step]) -> State {
    let n = ssa.after.blocks.len();
    let mut st = State {
        globals: None,
        phis: vec![0; n],
        rewritten: ssa.after.blocks.iter().map(|b| vec![false; b.insts.len()]).collect(),
        filled: vec![],
        stacks: vec![Vec::new(); ssa.before.func.vars.len()],
        current: None,
        renaming: false,
    };
    for s in steps {
        match s {
            Step::Globals(g) => st.globals = Some(g.clone()),
            Step::DefSites { .. } => {}
            Step::Phi { block, .. } => st.phis[*block] += 1,
            Step::Enter(b) => {
                st.current = Some(*b);
                st.renaming = true;
            }
            Step::Rewrite { block, inst } => st.rewritten[*block][*inst] = true,
            Step::Push { var, new } => st.stacks[*var].push(*new),
            Step::PhiArg { block, inst, arg } => st.filled.push((*block, *inst, *arg)),
            Step::Exit { popped, .. } => {
                st.current = None;
                for v in popped {
                    st.stacks[*v].pop();
                }
            }
        }
    }
    st
}

impl Lesson {
    fn rebuild(&mut self) {
        self.ir = IrFront::new(&self.src);
        self.func = self.ir.default_func();
        let before: Vec<Cfg> = self.ir.cfgs.iter().map(Cfg::compact).collect();
        self.runs = (self.ir.low.is_some() && before.iter().any(|c| c.func.name == "main")).then(|| {
            let after: Vec<Cfg> = before.iter().map(|c| build(c).after).collect();
            (interp::run(&before), interp::run(&after))
        });
        self.select();
    }

    fn select(&mut self) {
        self.ssa = self.ir.cfgs.get(self.func).map(|c| build(&c.compact()));
        self.stepper.reset();
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        ui.heading("4.4  SSA 形式");
        ui.label(RichText::new("进阶 · 约 40 分钟").color(p.muted));

        ui.add_space(8.0);
        ui.label(RichText::new("一句话").size(20.0).strong());
        prose(
            ui,
            "**SSA**（静态单赋值）要求程序里每个变量**只被赋值一次**。做法是每赋值一次就换一个新名字：`i.1`、`i.2`……\
             这样一来，每次\"使用\"都唯一对应一个\"定义\"，优化器问\"这个 i 的值是从哪来的\"时不用再做任何分析。\
             LLVM、GCC、V8、HotSpot、Go 编译器的优化都建立在 SSA 上。",
        );
        callout(ui, CalloutKind::Analogy, "文件的版本号", |ui| {
            prose(
                ui,
                "改文档时不覆盖原文件，而是另存为\"报告-v1\"\"报告-v2\"：任何人提到\"报告-v1\"，指的永远是同一份内容。\
                 麻烦在于两条路线汇合时：循环头的 `i` 可能是刚进循环时的 `i.1`，也可能是转了一圈回来的 `i.3`。\
                 SSA 的办法是在汇合处放一个 **φ 函数**：`i.2 = φ(B0: i.1, B6: i.3)`——从 B0 来就取 `i.1`，从 B6 来就取 `i.3`。",
            );
        });

        ui.add_space(8.0);
        ui.label(RichText::new("构造分两步").size(20.0).strong());
        card(ui, |ui| {
            prose(
                ui,
                "**① 插 φ**：变量 x 在块 b 里被赋值，就在 b 的**支配边界** DF(b)（4.3 课）上的每个块开头放一个 x 的 φ。\
                 φ 本身也是一次赋值，所以新放了 φ 的块也要照此处理，直到不再有新的 φ。",
            );
            prose(
                ui,
                "**② 重命名**：沿**支配树**深度优先遍历。每个变量有一个\"当前版本\"栈：遇到赋值，起个新名字压栈；\
                 遇到使用，换成栈顶的名字；一个块处理完，给它后继块里的 φ 填上对应的参数；离开时弹出本块压入的版本。",
            );
        });

        ui.add_space(8.0);
        ui.label(RichText::new("单步观察").size(20.0).strong());
        if editor(ui, &mut self.src, PRESETS, 6, true) {
            self.rebuild();
        }
        if self.ir.check(ui) {
            if func_picker(ui, &self.ir, &mut self.func) {
                self.select();
            }
            self.stepping(ui);
        }

        ui.add_space(8.0);
        callout(ui, CalloutKind::KeyPoint, "看到\"未定义\"不要慌", |ui| {
            prose(
                ui,
                "选\"循环 + 短路\"例子，可能会看到 `t2.1 = φ(B0: 未定义, …)`：沿 B0 这条路走来时 t2 还没被赋值过。\
                 这个 φ 其实没人用——它是\"半剪枝 SSA\"多插的。想只插真正需要的 φ（剪枝 SSA），\
                 得先知道变量在汇合点是否还**活着**，这要用到第 5 章的活跃变量分析。多插的 φ 不影响正确性，死代码消除会把它删掉。",
            );
        });
        callout(ui, CalloutKind::KeyPoint, "为什么 SSA 让优化变简单", |ui| {
            prose(
                ui,
                "在 SSA 里，`x.3 = 5` 之后，只要看到 `x.3`，它就是 5——不用担心中途被改掉（因为 x.3 不会被再次赋值）。\
                 常量传播、死代码消除（一个版本没人用，它的定义就能删）、公共子表达式消除都变成了在\"定义-使用\"链上的简单遍历。",
            );
        });

        ui.add_space(12.0);
        ui.label(RichText::new("小测验").size(20.0).strong());
        quiz(
            ui,
            "ch4-ssa-1",
            "`x = 1; if c { x = 2; } print(x);` 转成 SSA 后，print 的参数是什么？",
            &["x.1", "x.2", "一个 φ：x.3 = φ(x.1, x.2)"],
            2,
            "print 之前是汇合点：从 then 分支来 x 是 2，直接走过来 x 是 1。只能用 φ 按来路选择。",
        );
        quiz(
            ui,
            "ch4-ssa-2",
            "变量 x 只在 B2 里被赋值，φ 应该插在哪？",
            &["每个块的开头", "B2 的支配边界 DF(B2) 上的块", "B2 的所有后继"],
            1,
            "B2 支配的块里，x 的值只能来自 B2，不需要 φ；恰好在支配边界上，才可能和其他路径的值汇合。",
        );

        ui.add_space(12.0);
        callout(ui, CalloutKind::Deeper, "SSA 的来龙去脉", |ui| {
            prose(
                ui,
                "这里用的是 Cytron 等人 1991 年的经典算法。LLVM 的前端先把局部变量放在内存里，再由 mem2reg 这个 pass 用同样的方法\
                 转成 SSA。Braun 等人 2013 年提出了不需要支配树、边生成代码边构造 SSA 的简单算法。\
                 CPU 没有 φ 指令，所以代码生成前要**退出 SSA**：把 φ 换成前驱块末尾的复制指令——\
                 看似简单，其实藏着\"丢失复制\"\"交换\"等陷阱，要小心处理并行复制。",
            );
        });
        ui.add_space(24.0);
    }

    fn stepping(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        let Some(ssa) = &self.ssa else { return };
        self.stepper.ui(ui, ssa.steps.len());
        let k = self.stepper.pos;
        let last = k.checked_sub(1).map(|i| &ssa.steps[i]);
        let st = replay(ssa, &ssa.steps[..k]);
        let (before, after) = (&ssa.before, &ssa.after);
        let name = |b: usize| before.blocks[b].name.clone();
        let var = |v: VarId| before.func.vars[v].name.clone();
        let vars = |vs: &[VarId]| vs.iter().map(|&v| var(v)).collect::<Vec<_>>().join("、");

        let explain = match last {
            None => "第一步：找出需要考虑 φ 的变量。".to_owned(),
            Some(Step::Globals(g)) => {
                if g.is_empty() {
                    "没有跨块使用的变量，不需要任何 φ。".to_owned()
                } else {
                    format!(
                        "跨块使用的变量（在某个块里先被用到、后被赋值）：{}。只有它们可能需要 φ；\
                         只在一个块内部先赋值后使用的临时变量不需要。",
                        vars(g)
                    )
                }
            }
            Some(Step::DefSites { var: v, blocks }) => {
                let bs: Vec<String> = blocks.iter().map(|&b| name(b)).collect();
                let df: Vec<String> = blocks
                    .iter()
                    .map(|&b| {
                        let d: Vec<String> = ssa.dom.df[b].iter().map(|&x| name(x)).collect();
                        format!("DF({}) = {{{}}}", name(b), d.join(", "))
                    })
                    .collect();
                format!("{} 在 {} 里被赋值。查支配边界：{}。", var(*v), bs.join("、"), df.join("，"))
            }
            Some(Step::Phi { var: v, block, from }) => format!(
                "{} ∈ DF({})：在 {} 开头插入 {} 的 φ。φ 也算一次赋值，所以还要再看 DF({})。",
                name(*block),
                name(*from),
                name(*block),
                var(*v),
                name(*block)
            ),
            Some(Step::Enter(b)) => format!("第二步：重命名。沿支配树深度优先，进入 {}。", name(*b)),
            Some(Step::Push { var: v, new }) => format!(
                "{} 被赋值，起新名字 **{}** 压入 {} 的栈。之后用到 {} 的地方都换成它。",
                var(*v),
                after.func.vars[*new].name,
                var(*v),
                var(*v)
            ),
            Some(Step::Rewrite { block, inst }) => {
                let phis = ssa.phi_vars[*block].len();
                let new = after.inst_text(&after.blocks[*block].insts[*inst]);
                if *inst < phis {
                    format!("φ 的结果也是一次赋值：`{new}`（参数等前驱块处理完再填）。")
                } else {
                    let old = before.inst_text(&before.blocks[*block].insts[*inst - phis]);
                    if old == new {
                        format!("`{old}` 不涉及要改名的变量，保持不变。")
                    } else {
                        format!("`{old}` 改写成 `{new}`：用到的变量换成栈顶版本，赋值的变量用新版本。")
                    }
                }
            }
            Some(Step::PhiArg { block, inst, arg }) => {
                let Inst::Phi { args, .. } = &after.blocks[*block].insts[*inst] else { unreachable!() };
                let (pred, val) = args[*arg];
                format!(
                    "{} 是 {} 的前驱：{} 里 {} 的 φ，来自 {} 的参数填当前栈顶 {}。",
                    name(pred),
                    name(*block),
                    name(*block),
                    var(ssa.phi_vars[*block][*inst]),
                    name(pred),
                    after.func.operand(val)
                )
            }
            Some(Step::Exit { block, popped }) => {
                if popped.is_empty() {
                    format!("离开 {}（它没有压入新版本）。", name(*block))
                } else {
                    format!("离开 {}，弹出它压入的 {} 的版本，回到进入它之前的状态。", name(*block), vars(popped))
                }
            }
        };
        prose_sized(ui, &explain, 17.0);

        // 控制流图 + 版本栈
        let phi_note = |b: usize| {
            let n = st.phis[b];
            (n > 0).then(|| format!("φ: {}", vars(&ssa.phi_vars[b][..n])))
        };
        let g = cfg_graph(before, phi_note);
        let focus_block = match last {
            Some(Step::Phi { block, .. } | Step::PhiArg { block, .. }) => Some(*block),
            _ => st.current,
        };
        let graph = |ui: &mut Ui| {
            ui.label(RichText::new("控制流图（块下方是已插入的 φ）").strong());
            card(ui, |ui| {
                graph_view(
                    ui,
                    "ssa_graph",
                    &g,
                    |b| if focus_block == Some(b) { GNodeStyle::focus(&p) } else { GNodeStyle::normal(&p) },
                    |_| GEdgeStyle::normal(&p),
                );
            });
        };
        let stacks = |ui: &mut Ui| {
            ui.label(RichText::new("每个变量的版本栈（右边是栈顶）").strong());
            card(ui, |ui| {
                if !st.renaming {
                    ui.label(RichText::new("（重命名阶段才用到）").color(p.muted));
                    return;
                }
                for &v in &ssa.renamed {
                    let items: Vec<String> = st.stacks[v].iter().map(|&x| after.func.vars[x].name.clone()).collect();
                    let text = if items.is_empty() { "（空）".to_owned() } else { items.join("  ") };
                    ui.label(RichText::new(format!("{:>4} │ {text}", var(v))).font(mono(15.0)));
                }
                let kept: Vec<String> = (0..before.func.vars.len())
                    .filter(|v| !ssa.renamed.contains(v))
                    .map(|v| before.func.vars[v].name.clone())
                    .collect();
                if !kept.is_empty() {
                    ui.label(
                        RichText::new(format!(
                            "只赋值一次、不需要 φ 的 {} 本来就满足 SSA，保持原名。",
                            kept.join("、")
                        ))
                        .small()
                        .color(p.muted),
                    );
                }
            });
        };
        graph(ui);
        stacks(ui);

        // 每个块的指令：改写过的显示 SSA 形式，没改写的显示原样
        let focus_inst = match last {
            Some(Step::Rewrite { block, inst } | Step::PhiArg { block, inst, .. }) => Some((*block, *inst)),
            Some(Step::Phi { block, .. }) => Some((*block, st.phis[*block] - 1)),
            _ => None,
        };
        ui.label(RichText::new(format!("fn {} 的各个基本块", before.func.name)).strong());
        blocks_grid(ui, after, |ui, b| {
            let total_phis = ssa.phi_vars[b].len();
            let mut lines: Vec<(String, Option<Color32>)> = Vec::new();
            for (i, inst) in after.blocks[b].insts.iter().enumerate() {
                let text = if i < total_phis {
                    if i >= st.phis[b] {
                        continue;
                    }
                    let Inst::Phi { dst, args } = inst else { continue };
                    let d =
                        if st.rewritten[b][i] { after.func.vars[*dst].name.clone() } else { var(ssa.phi_vars[b][i]) };
                    let a: Vec<String> = args
                        .iter()
                        .enumerate()
                        .map(|(j, (pb, o))| {
                            let val =
                                if st.filled.contains(&(b, i, j)) { after.func.operand(*o) } else { "?".to_owned() };
                            format!("{}: {val}", name(*pb))
                        })
                        .collect();
                    format!("{d} = φ({})", a.join(", "))
                } else if st.rewritten[b][i] {
                    after.inst_text(inst)
                } else {
                    before.inst_text(&before.blocks[b].insts[i - total_phis])
                };
                let bg = if focus_inst == Some((b, i)) {
                    Some(p.focus_bg)
                } else if i < total_phis {
                    Some(p.done_bg)
                } else {
                    None
                };
                lines.push((text, bg));
            }
            block_card(ui, after, b, st.current == Some(b), &lines);
            ui.add_space(4.0);
        });

        if k == ssa.steps.len()
            && let Some((a, b)) = &self.runs
        {
            let show = |r: &RunResult| match r {
                Ok(v) => format!("{v:?}"),
                Err(e) => format!("出错：{e}"),
            };
            let same = a == b;
            let text = format!(
                "用解释器分别运行转换前和转换后的整个程序：打印出 {} 和 {}{}",
                show(a),
                show(b),
                if same { "，结果一致。" } else { "，不一致！" }
            );
            ui.label(RichText::new(text).color(if same { p.ok } else { p.error }).strong());
        }
    }
}
