//! 5.4 格与不动点。

use bsc_minilang::cfg::Cfg;
use bsc_minilang::dataflow::{Solution, liveness, solve};
use bsc_minilang::interp;
use bsc_minilang::sccp::{Val, sccp};
use bsc_minilang::ssa;
use eframe::egui::{Align2, CornerRadius, FontId, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Ui, Vec2};

use super::{NEVER, SUM};
use crate::pages::learn::ch3::editor;
use crate::pages::learn::ch4::{IrFront, func_picker};
use crate::theme::{Palette, mono};
use crate::widgets::{CalloutKind, callout, card, chip_button, prose, quiz, source_view};

const JOIN: &str = "fn main() {\n    print(g(1));\n    print(g(0));\n}\n\nfn g(c: int) -> int {\n    let mut a = 0;\n    let mut b = 0;\n    if c > 0 {\n        a = 2;\n        b = 3;\n    } else {\n        a = 3;\n        b = 2;\n    }\n    let x = a + b;\n    return x;\n}";

pub struct Lesson {
    src: String,
    ir: IrFront,
    func: usize,
    cfg: Option<Cfg>,
    sol: Option<Solution>,
    block: usize,
    /// 分配律反例：SCCP 的结论和实际运行结果。
    join: IrFront,
    join_vals: Vec<(String, Val)>,
    join_run: Result<Vec<i64>, String>,
}

impl Default for Lesson {
    fn default() -> Self {
        let join = IrFront::new(JOIN);
        let cfgs: Vec<Cfg> = join.cfgs.iter().map(Cfg::compact).collect();
        let ssas: Vec<Cfg> = cfgs.iter().map(|c| ssa::build(c).after).collect();
        let g = sccp(&ssas[1], true);
        let mut join_vals: Vec<(String, Val)> = ssas[1]
            .func
            .vars
            .iter()
            .enumerate()
            .filter(|(_, v)| !v.temp && (v.ssa.is_some() || v.name == "x"))
            .map(|(i, v)| (v.name.clone(), g.values[i]))
            .collect();
        join_vals.sort_by(|a, b| a.0.cmp(&b.0));
        let join_run = interp::run(&ssas);
        let mut l = Self {
            src: SUM.1.to_owned(),
            ir: IrFront::new(""),
            func: 0,
            cfg: None,
            sol: None,
            block: 1,
            join,
            join_vals,
            join_run,
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
        self.sol = self.cfg.as_ref().map(|c| solve(&c.succs(), &liveness(c)));
        self.block = 1.min(self.cfg.as_ref().map_or(0, |c| c.blocks.len() - 1));
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        ui.heading("5.4  格与不动点");
        ui.label(RichText::new("进阶 · 约 40 分钟").color(p.muted));

        ui.add_space(8.0);
        ui.label(RichText::new("为什么需要理论").size(20.0).strong());
        prose(
            ui,
            "5.2 课的工作表算法留下了几个问题：它**一定会停**吗？停下来时的答案**唯一**吗（换个取块的顺序会不会不同）？\
             这个答案**有多准**？数据流分析的理论基础——**格**与**不动点**——一次回答这三个问题。",
        );

        ui.add_space(8.0);
        ui.label(RichText::new("格：信息的\"多少\"").size(20.0).strong());
        prose(
            ui,
            "分析中每个点上的信息（比如活跃变量的集合）之间有\"包含\"关系：{i} ⊆ {i, s}。这种\"有的能比大小、有的不能\"的关系叫**偏序**\
             （{i} 和 {s} 就比不了）。如果任意两个元素都有一个\"最小的共同上界\"（这里就是并集），这个偏序就是一个**格**。\
             把元素画成点、小的在下大的在上、相邻的连线，得到**哈斯图**。",
        );
        prose(
            ui,
            "下图是真实的活跃变量分析在所选函数上用到的格（所有可能活跃的变量组成的集合，的全部子集）。\
             选一个块，高亮的是工作表算法运行过程中 IN(块) 先后取过的值——它只会**往上走**。",
        );
        if editor(ui, &mut self.src, &[SUM, NEVER], 6, true) {
            self.rebuild();
        }
        if self.ir.check(ui) {
            if func_picker(ui, &self.ir, &mut self.func) {
                self.select();
            }
            self.hasse(ui);
        }

        ui.add_space(8.0);
        ui.label(RichText::new("单调函数与不动点").size(20.0).strong());
        card(ui, |ui| {
            prose(
                ui,
                "• 每个块的传递函数 `f(x) = gen ∪ (x − kill)` 是**单调**的：输入变大，输出不会变小。汇合（并集）也是单调的。",
            );
            prose(
                ui,
                "• 整个分析就是解方程组 `X = F(X)`，X 是所有块的信息，F 把所有传递函数和汇合合在一起。满足 `X = F(X)` 的 X 叫**不动点**。",
            );
            prose(
                ui,
                "• **Kleene 不动点定理**：从格的最底部 ⊥ 出发，反复计算 ⊥, F(⊥), F(F(⊥)), …，这串值只会往上走；\
                 如果格的**高度有限**（从底到顶的链长度有限），它一定在有限步后停在**最小不动点**上。",
            );
            prose(
                ui,
                "• 工作表算法只是换了个计算顺序，结果仍是这个最小不动点，所以答案**与取块的顺序无关**。\
                 高度决定了最多迭代多少次：n 个变量的子集格高度是 n，每个块最多变 n 次。",
            );
        });

        ui.add_space(8.0);
        ui.label(RichText::new("常量传播的格").size(20.0).strong());
        prose(
            ui,
            "常量传播（5.5 课）给每个变量一个值：⊤（还没见到任何赋值）、某个具体的常量、⊥（不是常量）。\
             这个格很\"扁\"：常量有无穷多个，但高度只有 2——每个变量最多变两次（⊤ → 常量 → ⊥），所以算法一定会停。",
        );
        card(ui, |ui| flat_lattice(ui, &p));

        ui.add_space(8.0);
        ui.label(RichText::new("答案有多准：MOP 与 MFP").size(20.0).strong());
        prose(
            ui,
            "最理想的答案是：把从入口到某点的**每一条路径**分别算一遍，再合并——叫 **MOP**（meet over all paths）。\
             路径可能有无穷多条，没法直接算。不动点算法是在每个汇合点**先合并、再继续算**，得到的叫 **MFP**（maximal fixed point）。",
        );
        prose(
            ui,
            "如果传递函数满足**分配律** `f(x ∪ y) = f(x) ∪ f(y)`，先合并后算和先算后合并一样，MFP = MOP，没有损失。\
             gen/kill 形式的函数都满足分配律，所以活跃变量、到达定值的结果是最精确的。常量传播**不满足**分配律，下面是一个真实的例子：",
        );
        card(ui, |ui| {
            source_view(ui, JOIN, &self.join.front.colors(&p), &[], 15.0);
        });
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("5.5 课的 SCCP 在函数 g 上的结论：").strong());
            for (name, v) in &self.join_vals {
                let (fill, stroke) = match v {
                    Val::Const(_) => (p.ok.gamma_multiply(0.2), Stroke::new(1.0, p.ok)),
                    _ => (p.error_bg, Stroke::new(1.0, p.error)),
                };
                chip_button(ui, RichText::new(format!("{name} = {}", v.show())).font(mono(14.0)), fill, stroke);
            }
        });
        let run = match &self.join_run {
            Ok(v) => format!("{v:?}"),
            Err(e) => e.clone(),
        };
        prose(
            ui,
            &format!(
                "实际运行打印 {run}：不管走哪条路，a + b 都是 5。可是在汇合点，a 合并成 {{2, 3}} → ⊥，b 也是 ⊥，\
                 ⊥ + ⊥ 只能是 ⊥。如果按路径分别算（MOP），两条路上 x 都是 5，结论是\"x 是常量 5\"。\
                 **先合并**丢掉了\"a 和 b 此消彼长\"的关联——这就是不满足分配律带来的精度损失。"
            ),
        );

        ui.add_space(12.0);
        ui.label(RichText::new("小测验").size(20.0).strong());
        quiz(
            ui,
            "ch5-lat-1",
            "为什么活跃变量分析一定会停？",
            &[
                "传递函数单调，集合只会变大，而变量个数有限（格的高度有限）",
                "因为工作表越来越短",
                "因为程序里没有死循环",
            ],
            0,
            "每个块的信息只增不减，最多增加到\"全部变量\"。工作表可能变长，但每次放回去都意味着某个集合变大了，这只能发生有限次。",
        );
        quiz(
            ui,
            "ch5-lat-2",
            "对于满足分配律的分析，MFP 和 MOP 的关系是？",
            &["MFP 更准", "两者相等", "MOP 算不出来，所以无法比较"],
            1,
            "分配律保证\"先合并再算\"和\"先算再合并\"结果一样，所以不动点算法没有精度损失。",
        );

        ui.add_space(12.0);
        callout(ui, CalloutKind::Deeper, "抽象解释", |ui| {
            prose(
                ui,
                "Cousot 夫妇 1977 年提出的**抽象解释**把这一切统一起来：程序分析就是在一个\"抽象\"的格上运行程序。\
                 集合格、常量格之外，还有区间格（x ∈ [0, 10]）、多面体格（x + y ≤ 5）……高度无限的格（比如区间）要用\"加宽\"算子强行加速收敛。\
                 Astrée 用这套方法证明了空客 A380 飞控软件没有运行时错误。",
            );
        });
        ui.add_space(24.0);
    }

    /// 活跃变量的子集格（哈斯图），高亮所选块 IN 的变化过程。
    fn hasse(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        let (Some(cfg), Some(sol)) = (&self.cfg, &self.sol) else { return };
        let mut universe: Vec<usize> = Vec::new();
        for s in sol.inn.iter().chain(&sol.out) {
            for &v in s {
                if !universe.contains(&v) {
                    universe.push(v);
                }
            }
        }
        universe.sort_unstable();
        let clipped = universe.len() > 4;
        universe.truncate(4);
        let n = universe.len();

        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("看哪个块的 IN：").color(p.muted));
            for b in 0..cfg.blocks.len() {
                let (fill, stroke) = if b == self.block {
                    (p.focus_bg, Stroke::new(2.0, p.accent))
                } else {
                    (p.card_bg, Stroke::new(1.0, p.card_stroke))
                };
                if chip_button(ui, RichText::new(&cfg.blocks[b].name).font(mono(14.0)), fill, stroke).clicked() {
                    self.block = b;
                }
            }
        });
        let mask = |s: &[usize]| {
            universe.iter().enumerate().filter(|(_, v)| s.contains(v)).fold(0usize, |m, (i, _)| m | (1 << i))
        };
        // IN(块) 先后取过的值
        let mut chain = vec![0usize];
        for s in &sol.steps {
            if s.block == self.block {
                let m = mask(&s.result);
                if *chain.last().unwrap() != m {
                    chain.push(m);
                }
            }
        }
        let label = |m: usize| {
            let names: Vec<String> =
                (0..n).filter(|i| m & (1 << i) != 0).map(|i| cfg.func.vars[universe[i]].name.clone()).collect();
            if names.is_empty() { "∅".to_owned() } else { format!("{{{}}}", names.join(",")) }
        };

        card(ui, |ui| {
            if n == 0 {
                ui.label(RichText::new("这个函数里没有跨块活跃的变量，格只有一个点 ∅。").color(p.muted));
                return;
            }
            let layers: Vec<Vec<usize>> =
                (0..=n).map(|k| (0..1usize << n).filter(|m| m.count_ones() as usize == k).collect()).collect();
            let widest = layers.iter().map(Vec::len).max().unwrap_or(1) as f32;
            let (w, h) = ((widest * 96.0).max(300.0), (n as f32 + 1.0) * 64.0);
            let (resp, painter) =
                ui.allocate_painter(Vec2::new(ui.available_width().min(w.max(300.0)), h), Sense::hover());
            let rect = resp.rect;
            let pos = |m: usize| {
                let k = m.count_ones() as usize;
                let row = &layers[k];
                let i = row.iter().position(|&x| x == m).unwrap() as f32;
                let x = rect.center().x + (i - (row.len() as f32 - 1.0) / 2.0) * (rect.width() / widest);
                let y = rect.bottom() - 30.0 - k as f32 * 64.0;
                Pos2::new(x, y)
            };
            for m in 0..1usize << n {
                for i in 0..n {
                    if m & (1 << i) == 0 {
                        painter.line_segment([pos(m), pos(m | (1 << i))], Stroke::new(1.0, p.card_stroke));
                    }
                }
            }
            for w in chain.windows(2) {
                painter.line_segment([pos(w[0]), pos(w[1])], Stroke::new(3.0, p.accent));
            }
            for m in 0..1usize << n {
                let text = label(m);
                let galley = painter.layout_no_wrap(text, mono(13.0), p.text);
                let r = Rect::from_center_size(pos(m), galley.size() + Vec2::new(12.0, 8.0));
                let on = chain.contains(&m);
                let fill = if chain.last() == Some(&m) {
                    p.focus_bg
                } else if on {
                    p.done_bg
                } else {
                    p.node_fill
                };
                painter.rect_filled(r, CornerRadius::same(6), fill);
                painter.rect_stroke(
                    r,
                    CornerRadius::same(6),
                    Stroke::new(if on { 2.0 } else { 1.0 }, if on { p.accent } else { p.node_stroke }),
                    StrokeKind::Inside,
                );
                painter.galley(r.center() - galley.size() / 2.0, galley, p.text);
            }
        });
        let steps: Vec<String> = chain.iter().map(|&m| label(m)).collect();
        prose(
            ui,
            &format!(
                "IN({}) 依次是：{}。每一步都是往上走（集合变大），最后停在最终答案上。{}",
                cfg.blocks[self.block].name,
                steps.join(" → "),
                if clipped { "（变量超过 4 个，图里只画了前 4 个。）" } else { "" }
            ),
        );
    }
}

/// 常量传播的"扁平"格。
fn flat_lattice(ui: &mut Ui, p: &Palette) {
    let (resp, painter) = ui.allocate_painter(Vec2::new(ui.available_width().min(520.0), 170.0), Sense::hover());
    let r = resp.rect;
    let top = Pos2::new(r.center().x, r.top() + 22.0);
    let bottom = Pos2::new(r.center().x, r.bottom() - 22.0);
    let items = ["…", "-2", "-1", "0", "1", "2", "…"];
    let y = r.center().y;
    for (i, t) in items.iter().enumerate() {
        let x = r.center().x + (i as f32 - 3.0) * 62.0;
        let c = Pos2::new(x, y);
        if *t != "…" {
            painter.line_segment([top, c], Stroke::new(1.0, p.node_stroke));
            painter.line_segment([c, bottom], Stroke::new(1.0, p.node_stroke));
            painter.circle_filled(c, 16.0, p.node_fill);
            painter.circle_stroke(c, 16.0, Stroke::new(1.2, p.node_stroke));
        }
        painter.text(c, Align2::CENTER_CENTER, *t, mono(14.0), p.text);
    }
    for (c, t, note) in [(top, "⊤", "还没见到赋值（乐观）"), (bottom, "⊥", "不是常量")] {
        painter.circle_filled(c, 16.0, p.focus_bg);
        painter.circle_stroke(c, 16.0, Stroke::new(1.5, p.accent));
        painter.text(c, Align2::CENTER_CENTER, t, FontId::proportional(18.0), p.text);
        painter.text(c + Vec2::new(26.0, 0.0), Align2::LEFT_CENTER, note, FontId::proportional(13.0), p.muted);
    }
}
