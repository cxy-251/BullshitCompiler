//! 4.2 基本块与控制流图。

use bsc_minilang::cfg::{EdgeKind, LeaderWhy, Step};
use bsc_minilang::ir::Inst;
use eframe::egui::{self, RichText, Ui};

use super::{IrFront, PRESETS, block_card, blocks_grid, cfg_graph, func_picker};
use crate::pages::learn::ch3::editor;
use crate::theme::{Palette, mono};
use crate::widgets::{
    CalloutKind, GEdgeStyle, GNodeStyle, Stepper, callout, card, graph_view, prose, prose_sized, quiz,
};

pub struct Lesson {
    src: String,
    ir: IrFront,
    func: usize,
    stepper: Stepper,
}

impl Default for Lesson {
    fn default() -> Self {
        let mut l = Self { src: PRESETS[2].1.to_owned(), ir: IrFront::new(""), func: 0, stepper: Stepper::default() };
        l.rebuild();
        l
    }
}

impl Lesson {
    fn rebuild(&mut self) {
        self.ir = IrFront::new(&self.src);
        self.func = self.ir.default_func();
        self.stepper.reset();
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        ui.heading("4.2  基本块与控制流图");
        ui.label(RichText::new("入门 · 约 25 分钟").color(p.muted));

        ui.add_space(8.0);
        ui.label(RichText::new("一句话").size(20.0).strong());
        prose(
            ui,
            "三地址码是一长串指令，中间夹着跳转。把它切成一段一段的**基本块**：每块只能从第一条指令进入、从最后一条指令离开，\
             中间不会跳进来也不会跳出去——只要进了块，块里的指令就一定按顺序全部执行。\
             再按跳转关系把块连起来，就得到**控制流图**（CFG）：结点是基本块，边表示\"执行完这块，可能接着执行那块\"。\
             后面几乎所有的分析和优化，都在控制流图上进行。",
        );
        callout(ui, CalloutKind::Analogy, "地铁线路图", |ui| {
            prose(
                ui,
                "一段基本块像两个换乘站之间的一段线路：上了车就只能一站一站往前坐，中途没有岔路。\
                 换乘站就是块的边界，线路图只关心\"从哪段能换到哪段\"——这就是控制流图。",
            );
        });

        ui.add_space(8.0);
        ui.label(RichText::new("怎么切：找首指令").size(20.0).strong());
        card(ui, |ui| {
            prose(ui, "每个基本块的第一条指令叫**首指令**（leader）。以下三种指令是首指令：");
            prose(ui, "1. 函数的第一条指令；");
            prose(ui, "2. 跳转的目标，也就是标签 `L:` 所在的位置（可能从别处跳进来）；");
            prose(ui, "3. 紧跟在跳转或 `return` 之后的指令（前一条一定会离开，这里要么被跳进来，要么根本到不了）。");
            prose(ui, "从一个首指令开始，到下一个首指令之前，就是一个基本块。");
        });

        ui.add_space(8.0);
        ui.label(RichText::new("单步观察").size(20.0).strong());
        if editor(ui, &mut self.src, PRESETS, 6, true) {
            self.rebuild();
        }
        if self.ir.check(ui) {
            if func_picker(ui, &self.ir, &mut self.func) {
                self.stepper.reset();
            }
            self.stepping(ui);
        }

        ui.add_space(8.0);
        callout(ui, CalloutKind::KeyPoint, "循环就是图里的环", |ui| {
            prose(
                ui,
                "`while` 循环体末尾的 `goto` 跳回检查条件的块，图里就出现了一条往回指的边（回边）和一个环。\
                 编译器正是靠在控制流图里找环来识别循环的，而不是去看源码里写没写 `while`——\
                 所以不管源码用 while、for 还是 goto，优化器看到的都是同一种结构。",
            );
        });
        callout(ui, CalloutKind::KeyPoint, "不可达代码", |ui| {
            prose(
                ui,
                "选\"不可达代码\"例子：`if` 和 `else` 都 `return` 了，后面的 `print(x)` 永远不会执行。\
                 在控制流图里，从入口出发沿着边走不到它所在的块，直接删掉。很多编译器会对这种代码给出警告（rustc 的 unreachable code）。",
            );
        });

        ui.add_space(12.0);
        ui.label(RichText::new("小测验").size(20.0).strong());
        quiz(
            ui,
            "ch4-cfg-1",
            "如果一个基本块的中间有一条 `goto`，会怎么样？",
            &["不可能：goto 只能是块的最后一条指令", "块会从 goto 那里跳走，后面的指令不执行", "goto 会被忽略"],
            0,
            "跳转之后的指令是首指令（规则 3），所以跳转永远在块的末尾。这正是基本块\"进了就全部执行\"的保证。",
        );
        quiz(
            ui,
            "ch4-cfg-2",
            "`if c { A } else { B }` 之后的汇合点在控制流图里有几个前驱？",
            &["1 个", "2 个：then 分支的末尾和 else 分支的末尾", "3 个"],
            1,
            "两个分支执行完都会走到汇合点（除非某个分支 return 了）。有多个前驱的块叫汇合点，4.4 课的 φ 函数就放在这种地方。",
        );

        ui.add_space(12.0);
        callout(ui, CalloutKind::Deeper, "比基本块更大的单位", |ui| {
            prose(
                ui,
                "基本块内部的优化叫**局部优化**，跨块的叫**全局优化**（这里\"全局\"指整个函数）。\
                 介于两者之间的还有：**扩展基本块**（一棵只在根部有多个前驱的块树）、**trace / 超级块**（沿最常走的路径连成一串，\
                 用于指令调度）。JIT 编译器（如 JavaScript 引擎）常常只编译实际跑到的热路径。",
            );
        });
        ui.add_space(24.0);
    }

    fn stepping(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        let Some(low) = &self.ir.low else { return };
        let Some(cfg) = self.ir.cfgs.get(self.func) else { return };
        let f = &low.funcs[self.func];
        self.stepper.ui(ui, cfg.steps.len());
        let k = self.stepper.pos;
        let done = &cfg.steps[..k];
        let last = done.last();

        let mut leader = vec![None; f.code.len()];
        let mut formed = vec![false; cfg.blocks.len()];
        let mut edges: Vec<(usize, usize)> = Vec::new();
        let mut dead = vec![false; cfg.blocks.len()];
        for s in done {
            match s {
                Step::Leader { inst, why } => leader[*inst] = Some(*why),
                Step::Block(b) => formed[*b] = true,
                Step::Edge { from, to, .. } => edges.push((*from, *to)),
                Step::Unreachable(b) => dead[*b] = true,
                Step::EntryBlock => formed[0] = true,
            }
        }
        let name = |b: usize| cfg.blocks[b].name.clone();
        let explain = match last {
            None => "从头扫描三地址码，找出首指令。".to_owned(),
            Some(Step::Leader { inst, why }) => match why {
                LeaderWhy::First => format!("第 {inst} 条是函数的第一条指令 → 首指令（规则 1）。"),
                LeaderWhy::Target(l) => format!("第 {inst} 条是标签 `L{l}`，有跳转会跳到这里 → 首指令（规则 2）。"),
                LeaderWhy::AfterJump => format!("第 {inst} 条紧跟在跳转或 return 之后 → 首指令（规则 3）。"),
            },
            Some(Step::EntryBlock) => {
                "函数一开头就是循环的标签，以后会有边跳回这里。另加一个空的入口块 B0，保证入口没有前驱\
                                      （4.3、4.4 课的算法需要这一点）。"
                    .to_owned()
            }
            Some(Step::Block(b)) => {
                let r = &cfg.blocks[*b].range;
                format!("第 {} 到第 {} 条指令组成基本块 {}。", r.start, r.end.saturating_sub(1), name(*b))
            }
            Some(Step::Edge { from, to, kind }) => match kind {
                EdgeKind::Fall => format!("{} 末尾没有跳转，执行完顺序进入 {}。", name(*from), name(*to)),
                EdgeKind::Jump => format!("{} 以 goto 结尾，连一条边到 {}。", name(*from), name(*to)),
                EdgeKind::True => format!("{} 以条件跳转结尾：条件成立去 {}（标\"是\"的边）。", name(*from), name(*to)),
                EdgeKind::False => format!("条件不成立去 {}（标\"否\"的边）。", name(*to)),
            },
            Some(Step::Unreachable(b)) => {
                format!("从入口 B0 出发沿着边走，怎么也到不了 {}——它是不可达代码，可以删掉。", name(*b))
            }
        };
        prose_sized(ui, &explain, 17.0);

        let focus_inst = match last {
            Some(Step::Leader { inst, .. }) => Some(*inst),
            _ => None,
        };
        let focus_block = match last {
            Some(Step::Block(b) | Step::Unreachable(b)) => Some(*b),
            _ => None,
        };
        let listing = |ui: &mut Ui| {
            ui.label(RichText::new(format!("fn {} 的三地址码", f.name)).strong());
            card(ui, |ui| {
                ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                    // 每行是一个 horizontal，默认行高按按钮算，太松；代码清单要紧凑一些
                    ui.spacing_mut().interact_size.y = 16.0;
                    ui.spacing_mut().item_spacing.y = 2.0;
                    for (i, inst) in f.code.iter().enumerate() {
                        let block = cfg.blocks.iter().position(|b| b.range.contains(&i)).unwrap_or(0);
                        let is_label = matches!(inst, Inst::Label(_));
                        let indent = if is_label { "" } else { "    " };
                        let mut t = RichText::new(format!("{i:>2} {indent}{}", f.linear_text(i))).font(mono(14.0));
                        if formed[block] {
                            t = t.background_color(p.category(block).gamma_multiply(0.22));
                        }
                        if focus_inst == Some(i) {
                            t = t.background_color(p.focus_bg);
                        }
                        ui.horizontal(|ui| {
                            ui.label(t);
                            if let Some(why) = leader[i] {
                                let w = match why {
                                    LeaderWhy::First => "▶ 首指令：第一条",
                                    LeaderWhy::Target(_) => "▶ 首指令：跳转目标",
                                    LeaderWhy::AfterJump => "▶ 首指令：跳转之后",
                                };
                                ui.label(RichText::new(w).small().color(p.accent));
                            }
                        });
                    }
                });
            });
        };
        let graph = |ui: &mut Ui| {
            ui.label(RichText::new("控制流图").strong());
            let g = cfg_graph(cfg, |_| None);
            let edge_list: Vec<(usize, usize)> = g.edges.iter().map(|e| (e.from, e.to)).collect();
            let last_edge = match last {
                Some(Step::Edge { from, to, .. }) => Some((*from, *to)),
                _ => None,
            };
            card(ui, |ui| {
                graph_view(
                    ui,
                    "cfg_graph",
                    &g,
                    |b| {
                        if !formed[b] {
                            GNodeStyle::hidden()
                        } else if dead[b] {
                            GNodeStyle::filled(&p, p.error_bg)
                        } else if focus_block == Some(b) {
                            GNodeStyle::focus(&p)
                        } else {
                            GNodeStyle::filled(&p, p.category(b).gamma_multiply(0.3))
                        }
                    },
                    |e| {
                        if !edges.contains(&edge_list[e]) {
                            GEdgeStyle::hidden()
                        } else if last_edge == Some(edge_list[e]) {
                            GEdgeStyle::focus(&p)
                        } else {
                            GEdgeStyle::normal(&p)
                        }
                    },
                );
            });
        };
        // 图放在上面、占满整行：放在半栏里会被缩得太小
        graph(ui);
        listing(ui);

        if formed.iter().all(|&x| x) {
            ui.label(RichText::new("每个基本块的内容（跳转目标换成了块名）").strong());
            blocks_grid(ui, cfg, |ui, b| {
                let lines: Vec<_> = cfg.blocks[b].insts.iter().map(|i| (cfg.inst_text(i), None)).collect();
                block_card(ui, cfg, b, focus_block == Some(b), &lines);
                if dead[b] {
                    ui.label(RichText::new(format!("{} 不可达，会被删除", cfg.blocks[b].name)).small().color(p.error));
                }
                ui.add_space(4.0);
            });
        }
    }
}
