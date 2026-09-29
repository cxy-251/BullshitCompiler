//! 4.3 支配树。

use bsc_minilang::cfg::Cfg;
use bsc_minilang::dom::{Dominators, Step, dominators};
use eframe::egui::{self, RichText, Ui};

use super::{IrFront, PRESETS, cfg_graph, func_picker};
use crate::pages::learn::ch3::editor;
use crate::theme::{Palette, mono};
use crate::widgets::{
    CalloutKind, GEdgeStyle, GNodeStyle, NodeStyle, Stepper, TreeNode, callout, card, graph_view, prose, prose_sized,
    quiz, scrollable_tree,
};

pub struct Lesson {
    src: String,
    ir: IrFront,
    func: usize,
    /// 选中函数的控制流图（已删除不可达块）和支配关系。
    cfg: Option<Cfg>,
    dom: Option<Dominators>,
    stepper: Stepper,
}

impl Default for Lesson {
    fn default() -> Self {
        let mut l = Self {
            src: PRESETS[2].1.to_owned(),
            ir: IrFront::new(""),
            func: 0,
            cfg: None,
            dom: None,
            stepper: Stepper::default(),
        };
        l.rebuild();
        l
    }
}

/// 回放到第 k 步时的状态。
struct State {
    dom: Vec<Vec<usize>>,
    idom: Vec<Option<usize>>,
    df: Vec<Vec<usize>>,
}

fn replay(n: usize, steps: &[Step]) -> State {
    let all: Vec<usize> = (0..n).collect();
    let mut st = State {
        dom: (0..n).map(|i| if i == 0 { vec![0] } else { all.clone() }).collect(),
        idom: vec![None; n],
        df: vec![Vec::new(); n],
    };
    for s in steps {
        match s {
            Step::Update { node, after, .. } => st.dom[*node] = after.clone(),
            Step::Idom { node, idom } => st.idom[*node] = Some(*idom),
            Step::Df { join, runner, .. } => {
                if !st.df[*runner].contains(join) {
                    st.df[*runner].push(*join);
                }
            }
            Step::Round(_) | Step::Stable(_) => {}
        }
    }
    st
}

impl Lesson {
    fn rebuild(&mut self) {
        self.ir = IrFront::new(&self.src);
        self.func = self.ir.default_func();
        self.select();
    }

    fn select(&mut self) {
        self.cfg = self.ir.cfgs.get(self.func).map(Cfg::compact);
        self.dom = self.cfg.as_ref().map(|c| dominators(&c.succs()));
        self.stepper.reset();
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        ui.heading("4.3  支配树");
        ui.label(RichText::new("进阶 · 约 35 分钟").color(p.muted));

        ui.add_space(8.0);
        ui.label(RichText::new("一句话").size(20.0).strong());
        prose(
            ui,
            "在控制流图里，如果从入口到块 n 的**每一条**路径都要经过块 d，就说 d **支配** n。\
             入口支配所有块，每个块也支配它自己。支配关系回答的是\"执行到这里时，哪些代码**一定**已经执行过了\"——\
             这是找循环、构造 SSA、做很多优化的基础。",
        );
        callout(ui, CalloutKind::Analogy, "进城必经的关卡", |ui| {
            prose(
                ui,
                "从城外（入口）到某个院子（块 n）有很多条路。不管走哪条路都绕不开的关卡，就是 n 的支配者。\
                 离 n 最近的那个必经关卡叫**直接支配者**（idom）。每个块向它的直接支配者连一条线，就得到一棵树——**支配树**。",
            );
        });

        ui.add_space(8.0);
        ui.label(RichText::new("迭代求解").size(20.0).strong());
        prose(ui, "要经过 n 的所有前驱之一才能到 n，所以\"每条路都经过 d\"等价于\"到每个前驱的每条路都经过 d\"：");
        card(ui, |ui| {
            ui.label(RichText::new("Dom(入口) = {入口}").font(mono(16.0)));
            ui.label(
                RichText::new("Dom(n)    = {n} ∪ ( Dom(p1) ∩ Dom(p2) ∩ … )   p 取遍 n 的所有前驱").font(mono(16.0)),
            );
        });
        prose(
            ui,
            "有环时这是一个\"方程组\"，用**不动点迭代**解：除入口外，先把每个 Dom(n) 设成\"全部块\"，\
             然后反复套用公式更新，直到一整轮下来什么都没变。第 5 章的数据流分析全都是这个套路。",
        );

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
        callout(ui, CalloutKind::KeyPoint, "用支配关系找循环", |ui| {
            prose(
                ui,
                "如果有一条边 n → h，而 h 支配 n，那么这条边就是**回边**，h 是一个循环的入口（循环头）：\
                 要到 n 必须先经过 h，n 又跳回 h，这就绕成了圈。上面的演示最后列出了找到的回边。\
                 用这种方法找到的循环叫**自然循环**，循环不变量外提等优化都以它为单位。",
            );
        });
        callout(ui, CalloutKind::KeyPoint, "支配边界：势力范围的边界", |ui| {
            prose(
                ui,
                "d 的**支配边界** DF(d) 是这样的块 n：d 支配 n 的某个前驱，却不严格支配 n 本身。\
                 直观地说，从 d 出发一路都在 d 的\"势力范围\"里，走到 n 时，有别的路也能进来了。\
                 变量在 d 里赋值后，n 就是\"这个新值可能和别的值汇合\"的地方——4.4 课的 φ 函数正是放在这里。",
            );
        });

        ui.add_space(12.0);
        ui.label(RichText::new("小测验").size(20.0).strong());
        quiz(
            ui,
            "ch4-dom-1",
            "`if c { A } else { B }` 之后的汇合块 J，它的直接支配者是？",
            &["A", "B", "计算条件 c 的那个块"],
            2,
            "到 J 可以经过 A 也可以经过 B，所以 A、B 都不是必经的；但两条路都要先经过计算 c 的块。",
        );
        quiz(
            ui,
            "ch4-dom-2",
            "为什么迭代开始时要把 Dom(n) 设成\"全部块\"而不是空集？",
            &[
                "每次更新都是求交集，只会让集合变小；从最大的集合出发，才能收敛到正确答案（最大不动点）",
                "随便设什么都一样",
                "为了让第一轮算得更快",
            ],
            0,
            "如果从空集出发，交集永远是空，有环的地方会卡在错误的答案上。第 5 章会系统地讲\"从哪一端开始迭代\"。",
        );

        ui.add_space(12.0);
        callout(ui, CalloutKind::Deeper, "更快的算法", |ui| {
            prose(
                ui,
                "这里的集合迭代最直观，但集合运算代价高。Cooper、Harvey、Kennedy 在 2001 年的论文\
                 \"A Simple, Fast Dominance Algorithm\" 里改成直接迭代 idom、用支配树上\"找公共祖先\"代替求交，\
                 实际跑得很快；本课求支配边界用的就是这篇论文的方法。理论上最快的是 Lengauer–Tarjan 算法（近似线性），\
                 LLVM 用的是它的变种 Semi-NCA。把控制流图的边反过来求支配，得到**后支配**，用来分析\"哪些代码受哪个条件控制\"。",
            );
        });
        ui.add_space(24.0);
    }

    fn stepping(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        let (Some(cfg), Some(dom)) = (&self.cfg, &self.dom) else { return };
        let n = cfg.blocks.len();
        self.stepper.ui(ui, dom.steps.len());
        let k = self.stepper.pos;
        let last = k.checked_sub(1).map(|i| &dom.steps[i]);
        let st = replay(n, &dom.steps[..k]);
        let name = |b: usize| cfg.blocks[b].name.clone();
        let set = |s: &[usize]| {
            if s.len() == n && n > 1 {
                "全部".to_owned()
            } else {
                let v: Vec<String> = s.iter().map(|&b| name(b)).collect();
                format!("{{{}}}", v.join(", "))
            }
        };

        let explain = match last {
            None => {
                let order: Vec<String> = dom.rpo.iter().map(|&b| name(b)).collect();
                format!(
                    "初始：Dom(B0) = {{B0}}，其余块先设成\"全部\"。之后每一轮按逆后序 {} 依次更新。",
                    order.join("、")
                )
            }
            Some(Step::Round(r)) => format!("第 {r} 轮开始。"),
            Some(Step::Update { node, preds, before, after }) => {
                let ps: Vec<String> = preds.iter().map(|&q| format!("Dom({})", name(q))).collect();
                let changed = if before == after { "没变" } else { "变了" };
                format!(
                    "Dom({}) = {{{}}} ∪ ({}) = {}（{changed}）",
                    name(*node),
                    name(*node),
                    ps.join(" ∩ "),
                    set(after)
                )
            }
            Some(Step::Stable(r)) => {
                format!("第 {r} 轮什么都没变，迭代结束——到达了不动点。下面由 Dom 集合求直接支配者。")
            }
            Some(Step::Idom { node, idom }) => {
                let strict: Vec<usize> = st.dom[*node].iter().copied().filter(|&d| d != *node).collect();
                format!(
                    "{} 的严格支配者是 {}，其中离它最近的是 {}，所以 idom({}) = {}。",
                    name(*node),
                    set(&strict),
                    name(*idom),
                    name(*node),
                    name(*idom)
                )
            }
            Some(Step::Df { join, pred, runner }) => format!(
                "{} 是汇合点。从它的前驱 {} 出发沿支配树往上走（直到 {} 的直接支配者为止），经过 {}：把 {} 加进 DF({})。",
                name(*join),
                name(*pred),
                name(*join),
                name(*runner),
                name(*join),
                name(*runner)
            ),
        };
        prose_sized(ui, &explain, 17.0);

        // 图：每个块下方写着当前的 Dom 集合
        let short = |s: &[usize]| {
            if s.len() == n && n > 1 {
                "全部".to_owned()
            } else {
                let v: Vec<String> = s.iter().map(|&b| name(b).trim_start_matches('B').to_owned()).collect();
                v.join(",")
            }
        };
        let g = cfg_graph(cfg, |b| Some(short(&st.dom[b])));
        let (focus, others, hl_edges): (Option<usize>, Vec<usize>, Vec<(usize, usize)>) = match last {
            Some(Step::Update { node, preds, .. }) => {
                (Some(*node), preds.clone(), preds.iter().map(|&q| (q, *node)).collect())
            }
            Some(Step::Idom { node, idom }) => (Some(*node), vec![*idom], vec![]),
            Some(Step::Df { join, pred, runner }) => (Some(*join), vec![*runner], vec![(*pred, *join)]),
            _ => (None, vec![], vec![]),
        };
        let edge_list: Vec<(usize, usize)> = g.edges.iter().map(|e| (e.from, e.to)).collect();
        let graph = |ui: &mut Ui| {
            ui.label(RichText::new("控制流图（块下方是当前的 Dom 集合，只写编号）").strong());
            card(ui, |ui| {
                graph_view(
                    ui,
                    "dom_graph",
                    &g,
                    |b| {
                        if focus == Some(b) {
                            GNodeStyle::focus(&p)
                        } else if others.contains(&b) {
                            GNodeStyle::filled(&p, p.warn_bg)
                        } else {
                            GNodeStyle::normal(&p)
                        }
                    },
                    |e| if hl_edges.contains(&edge_list[e]) { GEdgeStyle::focus(&p) } else { GEdgeStyle::normal(&p) },
                );
            });
        };
        let tree = |ui: &mut Ui| {
            ui.label(RichText::new("支配树").strong());
            card(ui, |ui| {
                let visible: Vec<usize> = (0..n).filter(|&b| b == 0 || st.idom[b].is_some()).collect();
                if visible.len() < 2 {
                    ui.label(RichText::new("（求出直接支配者后，在这里长成一棵树）").color(p.muted));
                    return;
                }
                let local = |b: usize| visible.iter().position(|&x| x == b).unwrap();
                let nodes: Vec<TreeNode> = visible
                    .iter()
                    .map(|&b| TreeNode {
                        label: name(b),
                        children: visible.iter().filter(|&&c| st.idom[c] == Some(b)).map(|&c| local(c)).collect(),
                    })
                    .collect();
                scrollable_tree(ui, "dom_tree", &nodes, |i| {
                    let b = visible[i];
                    let mut s = NodeStyle::normal(&p);
                    if focus == Some(b) {
                        s.fill = p.focus_bg;
                        s.stroke = p.accent;
                        s.emphasized = true;
                    }
                    s
                });
            });
        };
        graph(ui);

        ui.label(RichText::new("结果表").strong());
        card(ui, |ui| {
            egui::Grid::new("dom_table").num_columns(4).striped(true).spacing([28.0, 6.0]).show(ui, |ui| {
                for h in ["块", "Dom（支配它的块）", "idom", "支配边界 DF"] {
                    ui.label(RichText::new(h).strong());
                }
                ui.end_row();
                for b in 0..n {
                    let mut t = RichText::new(name(b)).font(mono(15.0));
                    if focus == Some(b) {
                        t = t.background_color(p.focus_bg);
                    }
                    ui.label(t);
                    ui.label(RichText::new(set(&st.dom[b])).font(mono(15.0)));
                    ui.label(RichText::new(st.idom[b].map_or("—".to_owned(), name)).font(mono(15.0)));
                    let mut df = st.df[b].clone();
                    df.sort_unstable();
                    let df_text = if df.is_empty() { "—".to_owned() } else { set(&df) };
                    ui.label(RichText::new(df_text).font(mono(15.0)));
                    ui.end_row();
                }
            });
        });

        tree(ui);

        if k == dom.steps.len() {
            let mut back = Vec::new();
            for (a, block) in cfg.blocks.iter().enumerate() {
                for &h in &block.succs {
                    if dom.dom[a].contains(&h) {
                        back.push(format!("{} → {}", name(a), name(h)));
                    }
                }
            }
            let text = if back.is_empty() {
                "没有回边：这个函数里没有循环。".to_owned()
            } else {
                format!("找到回边 {}：箭头指向的块支配箭头出发的块，它就是循环头。", back.join("、"))
            };
            ui.label(RichText::new(text).color(p.ok).strong());
        }
    }
}
