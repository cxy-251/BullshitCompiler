//! 「实验台」：mini-lang 的全流程浏览器（类似 Compiler Explorer）。
//!
//! 左边写程序，右边一次看到编译器每个阶段的产物：记号、语法树、语义分析、三地址码、控制流图、SSA、
//! 优化后的代码、RISC-V 汇编，以及几种执行方式的结果。鼠标指向任意一个记号、语法树节点或指令，
//! 源码里对应的部分就会高亮。课程是"一次学一个阶段"，实验台是"一眼看完整个编译器"。

use bsc_core::{Span, line_col};
use bsc_minilang::bytecode;
use bsc_minilang::cfg::Cfg;
use bsc_minilang::interp;
use bsc_minilang::opt::{dce, fold};
use bsc_minilang::regalloc::{AsmFunc, color, finalize, linear_scan};
use bsc_minilang::rv::select;
use bsc_minilang::sccp::sccp;
use bsc_minilang::{sim, ssa};
use eframe::egui::{
    self, Color32, RichText, ScrollArea, Sense, Slider, Stroke, TextEdit, TextFormat, Ui, text::LayoutJob,
};

use crate::pages::learn::ch4::{IrFront, cfg_graph, func_picker};
use crate::theme::{Palette, mono};
use crate::widgets::{
    GEdgeStyle, GNodeStyle, NodeStyle, TreeNode, card, chip, diagnostic_view, graph_view, scrollable_tree,
};

const PRESETS: &[(&str, &str)] = &[
    (
        "求和",
        "fn main() {\n    print(sum(10));\n}\n\nfn sum(n: int) -> int {\n    let mut s = 0;\n    let mut i = 1;\n    while i <= n {\n        s = s + i;\n        i = i + 1;\n    }\n    return s;\n}",
    ),
    (
        "斐波那契",
        "fn main() {\n    let mut i = 0;\n    while i < 10 {\n        print(fib(i));\n        i = i + 1;\n    }\n}\n\nfn fib(n: int) -> int {\n    if n < 2 {\n        return n;\n    }\n    return fib(n - 1) + fib(n - 2);\n}",
    ),
    (
        "能优化掉很多",
        "fn main() {\n    let day = 60 * 60 * 24;\n    let debug = false;\n    if debug {\n        print(0 - 1);\n    }\n    print(day * 7);\n    print(f(3));\n}\n\nfn f(n: int) -> int {\n    let mut x = 1;\n    let mut i = 0;\n    while i < n {\n        if x != 1 {\n            x = 2;\n        }\n        i = i + 1;\n    }\n    return x * 10 + 0;\n}",
    ),
    (
        "最大公约数",
        "fn main() {\n    print(gcd(48, 18));\n    print(gcd(17, 5));\n}\n\nfn gcd(a: int, b: int) -> int {\n    let mut x = a;\n    let mut y = b;\n    while y != 0 {\n        let t = x % y;\n        x = y;\n        y = t;\n    }\n    return x;\n}",
    ),
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage {
    Tokens,
    Ast,
    Sema,
    Tac,
    Cfg,
    Ssa,
    Opt,
    Asm,
    Run,
}

impl Stage {
    const ALL: [Stage; 9] = [
        Stage::Tokens,
        Stage::Ast,
        Stage::Sema,
        Stage::Tac,
        Stage::Cfg,
        Stage::Ssa,
        Stage::Opt,
        Stage::Asm,
        Stage::Run,
    ];

    fn title(self) -> &'static str {
        match self {
            Stage::Tokens => "记号",
            Stage::Ast => "语法树",
            Stage::Sema => "语义",
            Stage::Tac => "三地址码",
            Stage::Cfg => "控制流图",
            Stage::Ssa => "SSA",
            Stage::Opt => "优化",
            Stage::Asm => "RISC-V",
            Stage::Run => "运行",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum OptPass {
    FoldDce,
    Sccp,
}

type RunResult = Result<Vec<i64>, String>;

/// 整条流水线的产物（程序有错时只有前面几步）。
struct Pipeline {
    ir: IrFront,
    /// 每个函数的控制流图（已删除不可达块）。
    cfgs: Vec<Cfg>,
    ssas: Vec<Cfg>,
    folded: Vec<Cfg>,
    sccps: Vec<Cfg>,
    asm: Vec<AsmFunc>,
    /// (执行方式, 输出, 执行的指令条数)
    runs: Vec<(&'static str, RunResult, Option<usize>)>,
}

impl Pipeline {
    fn new(src: &str, scan: bool, k: usize) -> Self {
        let ir = IrFront::new(src);
        let cfgs: Vec<Cfg> = ir.cfgs.iter().map(Cfg::compact).collect();
        let ssas: Vec<Cfg> = cfgs.iter().map(|c| ssa::build(c).after).collect();
        let folded: Vec<Cfg> = cfgs.iter().map(|c| dce(&fold(c).output).output).collect();
        let sccps: Vec<Cfg> = ssas.iter().map(|c| sccp(c, true).output).collect();
        let asm: Vec<AsmFunc> = cfgs
            .iter()
            .map(|c| {
                let (m, _) = select(c, true);
                let a = if scan { linear_scan(&m, k).alloc } else { color(&m, k).alloc };
                finalize(&m, &a)
            })
            .collect();
        let mut runs = Vec::new();
        if cfgs.iter().any(|c| c.func.name == "main") {
            runs.push(("IR 解释器（三地址码）", interp::run(&cfgs), None));
            runs.push(("IR 解释器（折叠 + 死代码消除之后）", interp::run(&folded), None));
            runs.push(("IR 解释器（SSA + SCCP 之后）", interp::run(&sccps), None));
            if let Some(a) = &ir.front.sema {
                let r = bytecode::run(&bytecode::compile(&ir.front.parse.ast, ir.front.parse.root, a), 0);
                runs.push(("栈式虚拟机（字节码）", r.error.map_or(Ok(r.output), Err), Some(r.steps)));
            }
            let r = sim::run(&asm, 0);
            runs.push(("RISC-V 模拟器", r.error.map_or(Ok(r.output), Err), Some(r.executed)));
        }
        Self { ir, cfgs, ssas, folded, sccps, asm, runs }
    }

    fn ok(&self) -> bool {
        self.ir.low.is_some()
    }
}

pub struct LabPage {
    pub input: String,
    pipe: Pipeline,
    stage: Stage,
    func: usize,
    pass: OptPass,
    scan: bool,
    k: usize,
    /// 上一帧鼠标指向的源码区间（先画源码、后画各栏，所以用上一帧的结果）。
    hover: Option<Span>,
}

impl Default for LabPage {
    fn default() -> Self {
        Self::with_input(PRESETS[0].1.to_owned())
    }
}

impl LabPage {
    pub fn with_input(input: String) -> Self {
        let mut l = Self {
            pipe: Pipeline::new(&input, false, 11),
            input,
            stage: Stage::Tac,
            func: 0,
            pass: OptPass::FoldDce,
            scan: false,
            k: 11,
            hover: None,
        };
        l.func = l.pipe.ir.default_func();
        l
    }

    fn recompute(&mut self) {
        self.pipe = Pipeline::new(&self.input, self.scan, self.k);
        if self.func >= self.pipe.cfgs.len() {
            self.func = self.pipe.ir.default_func();
        }
        self.hover = None;
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let wide = ui.available_width() > 1000.0;
        let mut hover = None;
        if wide {
            egui::Panel::left("lab_source").default_size(460.0).size_range(320.0..=720.0).show(ui, |ui| {
                ScrollArea::vertical().id_salt("lab_src_scroll").show(ui, |ui| self.source_panel(ui));
            });
            egui::CentralPanel::default().show(ui, |ui| {
                ScrollArea::vertical().id_salt("lab_stage_scroll").show(ui, |ui| hover = self.stages(ui));
            });
        } else {
            egui::CentralPanel::default().show(ui, |ui| {
                ScrollArea::vertical().id_salt("lab_scroll").show(ui, |ui| {
                    self.source_panel(ui);
                    hover = self.stages(ui);
                });
            });
        }
        if hover != self.hover {
            self.hover = hover;
            ui.ctx().request_repaint();
        }
    }

    fn source_panel(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        ui.heading("实验台");
        ui.label(
            RichText::new("写一段 mini-lang 程序，右边同时显示编译器每个阶段的产物。鼠标指向记号、语法树节点或指令，这里会高亮对应的源码。")
                .color(p.muted),
        );
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("例子：").color(p.muted));
            for (name, code) in PRESETS {
                if ui.button(*name).clicked() {
                    self.input = (*code).to_owned();
                    self.recompute();
                }
            }
        });
        // 编辑框自带语法着色，鼠标指向的产物对应的源码加底色
        let colors = self.pipe.ir.front.colors(&p);
        let hover = self.hover;
        let mut layouter = |ui: &Ui, text: &dyn egui::TextBuffer, wrap: f32| {
            let mut job = highlight(text.as_str(), &colors, hover, &p);
            job.wrap.max_width = wrap;
            ui.fonts_mut(|f| f.layout_job(job))
        };
        let edit = TextEdit::multiline(&mut self.input)
            .font(mono(15.0))
            .desired_rows(18)
            .desired_width(f32::INFINITY)
            .layouter(&mut layouter);
        if ui.add(edit).changed() {
            self.recompute();
        }
        let front = &self.pipe.ir.front;
        for d in &front.syntax_errors {
            diagnostic_view(ui, &front.src, d);
        }
        if let Some(a) = &front.sema {
            for d in &a.errors {
                diagnostic_view(ui, &front.src, d);
            }
        }
        if self.pipe.ok() {
            self.summary(ui);
        }
    }

    /// 流水线各阶段的规模一览。
    fn summary(&self, ui: &mut Ui) {
        let p = Palette::of(ui);
        let count = |cs: &[Cfg]| cs.iter().flat_map(|c| &c.blocks).map(|b| b.insts.len()).sum::<usize>();
        let asm: usize = self.pipe.asm.iter().flat_map(|f| &f.blocks).map(|b| b.insts.len()).sum();
        let front = &self.pipe.ir.front;
        let rows = [
            ("记号", front.tokens.len().saturating_sub(1)),
            ("语法树节点", front.parse.ast.nodes.len()),
            ("三地址码", count(&self.pipe.cfgs)),
            ("折叠 + 死代码消除后", count(&self.pipe.folded)),
            ("SSA + SCCP 后", count(&self.pipe.sccps)),
            ("RISC-V 指令", asm),
        ];
        ui.add_space(6.0);
        card(ui, |ui| {
            ui.label(RichText::new("流水线一览").strong());
            egui::Grid::new("lab_summary").num_columns(2).spacing([20.0, 4.0]).show(ui, |ui| {
                for (name, n) in rows {
                    ui.label(name);
                    ui.label(RichText::new(n.to_string()).font(mono(14.0)));
                    ui.end_row();
                }
            });
            if let Some((_, r, _)) = self.pipe.runs.first() {
                let out = match r {
                    Ok(v) => format!("{v:?}"),
                    Err(e) => format!("出错：{e}"),
                };
                ui.label(RichText::new(format!("输出：{out}")).font(mono(14.0)).color(p.ok));
            }
        });
    }

    fn stages(&mut self, ui: &mut Ui) -> Option<Span> {
        let p = Palette::of(ui);
        ui.horizontal_wrapped(|ui| {
            for s in Stage::ALL {
                ui.selectable_value(&mut self.stage, s, RichText::new(s.title()).size(16.0));
            }
        });
        ui.separator();
        let needs_ir = !matches!(self.stage, Stage::Tokens | Stage::Ast | Stage::Sema);
        if needs_ir && !self.pipe.ok() {
            ui.label(RichText::new("程序还有错误（见左边），后面的阶段没法进行。").color(p.error));
            return None;
        }
        if matches!(self.stage, Stage::Cfg | Stage::Ssa | Stage::Opt) {
            func_picker(ui, &self.pipe.ir, &mut self.func);
        }
        match self.stage {
            Stage::Tokens => self.tokens(ui),
            Stage::Ast => self.ast(ui),
            Stage::Sema => self.sema(ui),
            Stage::Tac => self.tac(ui),
            Stage::Cfg => self.cfg(ui),
            Stage::Ssa => {
                ui.label(RichText::new("每个变量只赋值一次；汇合处用 φ 选择来路（第 4.4 课）。").color(p.muted));
                blocks(ui, &self.pipe.ssas[self.func])
            }
            Stage::Opt => self.opt(ui),
            Stage::Asm => {
                self.asm(ui);
                None
            }
            Stage::Run => {
                self.run(ui);
                None
            }
        }
    }

    fn tokens(&self, ui: &mut Ui) -> Option<Span> {
        let p = Palette::of(ui);
        let front = &self.pipe.ir.front;
        let colors = front.colors(&p);
        let mut hover = None;
        ui.label(RichText::new("词法分析把字符流切成记号（第 1 章）。鼠标指向记号可以看它的种类。").color(p.muted));
        card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                for (t, (_, c)) in front.tokens.iter().zip(&colors) {
                    let text = t.span.text(&front.src);
                    if text.is_empty() {
                        continue;
                    }
                    let r = chip(
                        ui,
                        RichText::new(text).font(mono(14.0)).color(*c),
                        p.card_bg,
                        Stroke::new(1.0, p.card_stroke),
                    );
                    if r.hovered() {
                        hover = Some(t.span);
                    }
                    r.on_hover_text(t.kind.name());
                }
            });
        });
        hover
    }

    fn ast(&self, ui: &mut Ui) -> Option<Span> {
        let p = Palette::of(ui);
        let front = &self.pipe.ir.front;
        let ast = &front.parse.ast;
        ui.label(RichText::new("语法分析的结果（第 2 章）。树很宽时可以左右拖动。").color(p.muted));
        let nodes: Vec<TreeNode> = (0..ast.nodes.len())
            .map(|i| {
                let id = bsc_minilang::ast::NodeId(i as u32);
                TreeNode { label: ast.label(id), children: ast.node(id).children.iter().map(|c| c.index()).collect() }
            })
            .collect();
        let mut hover = None;
        card(ui, |ui| {
            hover = scrollable_tree(ui, "lab_ast", &nodes, |_| NodeStyle::normal(&p)).map(|i| ast.nodes[i].span);
        });
        hover
    }

    fn sema(&self, ui: &mut Ui) -> Option<Span> {
        let p = Palette::of(ui);
        let front = &self.pipe.ir.front;
        let Some(a) = &front.sema else {
            ui.label(RichText::new("还有语法错误，没有做语义分析。").color(p.error));
            return None;
        };
        let mut hover = None;
        ui.label(RichText::new("作用域与符号表（第 3 章）。").color(p.muted));
        card(ui, |ui| {
            for s in &a.scopes {
                let syms: Vec<String> = s.symbols.iter().map(|&id| a.symbols[id].describe()).collect();
                if syms.is_empty() {
                    continue;
                }
                ui.label(RichText::new(s.title()).strong());
                ui.label(RichText::new(format!("    {}", syms.join("    "))).font(mono(13.0)));
            }
        });
        ui.label(RichText::new("每个名字的使用 → 它的声明（鼠标指向可以在源码里看到位置）").strong());
        card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                for (i, r) in a.resolved.iter().enumerate() {
                    let Some(sym) = r else { continue };
                    let s = &a.symbols[*sym];
                    let use_span = front.parse.ast.nodes[i].span;
                    let at = line_col(&front.src, use_span.start).line;
                    let to = if s.decl.is_some() {
                        format!("第 {} 行", line_col(&front.src, s.span.start).line)
                    } else {
                        "内置".to_owned()
                    };
                    let text = format!("第 {at} 行 {} → {to}", s.name);
                    let resp =
                        chip(ui, RichText::new(text).font(mono(13.0)), p.card_bg, Stroke::new(1.0, p.card_stroke));
                    if resp.hovered() {
                        hover = Some(use_span);
                    }
                }
            });
        });
        hover
    }

    fn tac(&self, ui: &mut Ui) -> Option<Span> {
        let p = Palette::of(ui);
        let Some(low) = &self.pipe.ir.low else { return None };
        ui.label(RichText::new("降级成三地址码（第 4.1 课）。").color(p.muted));
        let mut hover = None;
        for f in &low.funcs {
            ui.label(RichText::new(format!("fn {}", f.name)).font(mono(15.0)).strong());
            let lines: Vec<(String, Option<Span>, bool)> = (0..f.code.len())
                .map(|i| {
                    let label = matches!(f.code[i], bsc_minilang::ir::Inst::Label(_));
                    (f.linear_text(i), Some(f.origin[i]), label)
                })
                .collect();
            hover = hover.or(code_lines(ui, &lines));
        }
        hover
    }

    fn cfg(&self, ui: &mut Ui) -> Option<Span> {
        let p = Palette::of(ui);
        let cfg = &self.pipe.cfgs[self.func];
        ui.label(RichText::new("基本块与控制流图（第 4.2 课），不可达的块已删除。").color(p.muted));
        let g = cfg_graph(cfg, |_| None);
        card(ui, |ui| {
            graph_view(ui, "lab_cfg", &g, |_| GNodeStyle::normal(&p), |_| GEdgeStyle::normal(&p));
        });
        blocks(ui, cfg)
    }

    fn opt(&mut self, ui: &mut Ui) -> Option<Span> {
        let p = Palette::of(ui);
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("优化：").color(p.muted));
            ui.selectable_value(&mut self.pass, OptPass::FoldDce, "常量折叠 + 死代码消除（5.1、5.3 课）");
            ui.selectable_value(&mut self.pass, OptPass::Sccp, "SSA + 稀疏条件常量传播（5.5 课）");
        });
        let (before, after) = match self.pass {
            OptPass::FoldDce => (&self.pipe.cfgs[self.func], &self.pipe.folded[self.func]),
            OptPass::Sccp => (&self.pipe.ssas[self.func], &self.pipe.sccps[self.func]),
        };
        let count = |c: &Cfg| c.blocks.iter().map(|b| b.insts.len()).sum::<usize>();
        ui.label(RichText::new(format!("这个函数：{} 条 → {} 条", count(before), count(after))).color(p.ok).strong());
        blocks(ui, after)
    }

    fn asm(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        let mut changed = false;
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("寄存器分配：").color(p.muted));
            changed |= ui.selectable_value(&mut self.scan, false, "图着色").changed();
            changed |= ui.selectable_value(&mut self.scan, true, "线性扫描").changed();
            ui.separator();
            ui.label(RichText::new("可用寄存器 K：").color(p.muted));
            changed |= ui.add(Slider::new(&mut self.k, 1..=11)).changed();
        });
        if changed {
            self.recompute();
        }
        ui.label(
            RichText::new("指令选择 → 寄存器分配 → 加上序言和尾声之后的 RISC-V 汇编（第 6 章）。K 调小可以看到溢出到栈上的 ld / sd。")
                .color(p.muted),
        );
        for f in &self.pipe.asm {
            let lines: Vec<(String, Option<Span>, bool)> = f
                .blocks
                .iter()
                .flat_map(|b| {
                    std::iter::once((format!("{}:", b.label), None, true))
                        .chain(b.insts.iter().map(|i| (i.text(&[]), None, false)))
                })
                .collect();
            ui.label(RichText::new(format!("fn {}（栈帧 {} 字节）", f.name, f.frame.size)).font(mono(15.0)).strong());
            code_lines(ui, &lines);
        }
    }

    fn run(&self, ui: &mut Ui) {
        let p = Palette::of(ui);
        if self.pipe.runs.is_empty() {
            ui.label(RichText::new("程序里没有 main 函数，没法运行。").color(p.muted));
            return;
        }
        ui.label(
            RichText::new(
                "同一个程序，用五种方式执行。它们来自流水线的不同阶段，结果应该完全一样——这也是编译器测试的常用方法。",
            )
            .color(p.muted),
        );
        card(ui, |ui| {
            egui::Grid::new("lab_runs").num_columns(3).striped(true).spacing([24.0, 6.0]).show(ui, |ui| {
                for h in ["执行方式", "输出", "执行的指令数"] {
                    ui.label(RichText::new(h).strong());
                }
                ui.end_row();
                for (name, r, steps) in &self.pipe.runs {
                    ui.label(*name);
                    match r {
                        Ok(v) => ui.label(RichText::new(format!("{v:?}")).font(mono(14.0))),
                        Err(e) => ui.label(RichText::new(e).color(p.error)),
                    };
                    ui.label(steps.map_or("—".to_owned(), |s| s.to_string()));
                    ui.end_row();
                }
            });
        });
        let first = &self.pipe.runs[0].1;
        let same = self.pipe.runs.iter().all(|(_, r, _)| r == first);
        let (text, c) =
            if same { ("五种方式的输出完全一致。", p.ok) } else { ("输出不一致！", p.error) };
        ui.label(RichText::new(text).color(c).strong());
    }
}

/// 按块列出指令，指向某条可以看到它来自哪段源码。
fn blocks(ui: &mut Ui, cfg: &Cfg) -> Option<Span> {
    let mut lines = Vec::new();
    for b in &cfg.blocks {
        let preds: Vec<String> = b.preds.iter().map(|&x| cfg.blocks[x].name.clone()).collect();
        let head = if preds.is_empty() {
            format!("{}:", b.name)
        } else {
            format!("{}:        ; 前驱 {}", b.name, preds.join(", "))
        };
        lines.push((head, None, true));
        for (inst, sp) in b.insts.iter().zip(&b.origin) {
            lines.push((cfg.inst_text(inst), Some(*sp), false));
        }
    }
    code_lines(ui, &lines)
}

/// 一段代码清单：(文字, 来源, 是否标签行)。返回鼠标指向的那行的来源。
fn code_lines(ui: &mut Ui, lines: &[(String, Option<Span>, bool)]) -> Option<Span> {
    let p = Palette::of(ui);
    let mut hover = None;
    card(ui, |ui| {
        ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
            ui.spacing_mut().item_spacing.y = 1.0;
            for (text, span, label) in lines {
                let t = if *label {
                    RichText::new(text).font(mono(14.0)).color(p.keyword)
                } else {
                    RichText::new(format!("    {text}")).font(mono(14.0))
                };
                let r = ui.add(egui::Label::new(t).sense(Sense::hover()));
                if r.hovered()
                    && let Some(s) = span
                {
                    hover = Some(*s);
                    ui.painter().rect_filled(r.rect, 2.0, p.focus_bg.gamma_multiply(0.5));
                }
            }
        });
    });
    hover
}

/// 编辑框的着色：记号按种类上色，鼠标指向的产物对应的源码加底色。
fn highlight(src: &str, colors: &[(Span, Color32)], hover: Option<Span>, p: &Palette) -> LayoutJob {
    let mut cuts = vec![0, src.len()];
    for (s, _) in colors {
        cuts.extend([s.start, s.end]);
    }
    if let Some(h) = hover {
        cuts.extend([h.start, h.end]);
    }
    cuts.retain(|&c| c <= src.len() && src.is_char_boundary(c));
    cuts.sort_unstable();
    cuts.dedup();
    let mut job = LayoutJob::default();
    for w in cuts.windows(2) {
        let (a, b) = (w[0], w[1]);
        if a == b {
            continue;
        }
        let seg = Span::new(a, b);
        let fg = colors.iter().find(|(s, _)| s.contains(seg)).map_or(p.text, |(_, c)| *c);
        let bg = hover.filter(|h| h.contains(seg)).map_or(Color32::TRANSPARENT, |_| p.focus_bg);
        job.append(
            &src[a..b],
            0.0,
            TextFormat { font_id: mono(15.0), color: fg, background: bg, ..Default::default() },
        );
    }
    job
}
