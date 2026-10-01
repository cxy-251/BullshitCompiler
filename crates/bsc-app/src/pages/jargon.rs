//! 「黑话编译器」页：把故作高深的"领导讲话"编译成大白话。
//!
//! 左边是输入和译文，右边是编译器每个阶段的产物（全部来自 `bsc-jargon` 真正的运行结果）：
//! 逐个 Pass 的改动和理由、分词用的词图、句式语法树、语义角色 IR、词典，以及"扩充词典"——
//! 粘贴一批 AI 生成的词条，立即看到校验结果和它对已有译文的影响。

use bsc_core::{Diagnostic, Span, line_col};
use bsc_jargon::corpus::{self, Candidate};
use bsc_jargon::ir::{ClauseIr, Part, Template};
use bsc_jargon::lexicon::{Cat, Layer, Lexicon};
use bsc_jargon::segment::Clause;
use bsc_jargon::{Compilation, Compiler, EXAMPLE_BATCH, PRESETS, ai_prompt, grammar};
use eframe::egui::{
    self, Color32, CornerRadius, FontId, Rect, RichText, ScrollArea, Sense, Stroke, StrokeKind, TextEdit, TextFormat,
    Ui, Vec2, text::LayoutJob,
};

use crate::theme::{Palette, mono};
use crate::widgets::{
    CalloutKind, NodeStyle, TreeNode, callout, card, chip, chip_button, diagnostic_view, prose, prose_colored,
    scrollable_tree,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum View {
    Passes,
    Lattice,
    Parse,
    Ir,
    Lexicon,
    Extend,
    Why,
}

impl View {
    const ALL: [View; 7] = [View::Passes, View::Lattice, View::Parse, View::Ir, View::Lexicon, View::Extend, View::Why];

    fn title(self) -> &'static str {
        match self {
            View::Passes => "翻译过程",
            View::Lattice => "分词",
            View::Parse => "句式",
            View::Ir => "语义角色",
            View::Lexicon => "词典",
            View::Extend => "扩充词典",
            View::Why => "原理",
        }
    }
}

/// 扩充批次带来的译文变化：(示例名, 原来的译文, 现在的译文)。
type Regression = Vec<(String, String, String)>;

pub struct JargonPage {
    pub input: String,
    /// 用户粘贴的扩充词条。
    pub batch: String,
    base: Compiler,
    comp: Compiler,
    result: Compilation,
    regress: Regression,
    /// 语料分析得到的候选词表（只在输入或词典变化时重算）。
    candidates: Vec<Candidate>,
    /// 待办词表显示前多少个。
    candidate_limit: usize,
    /// 参考译文对照（只在词典变化时重算）。
    ref_cache: Vec<RefEntry>,
    view: View,
    clause: usize,
    lookup: String,
    ir_final: bool,
    /// 上一帧鼠标指向的原文区间。
    hover: Vec<Span>,
    /// 本地词典文件夹里的批次（只有原生版有）。
    files: Vec<Layer>,
    /// 拖进来的文件（原生版是完整路径）。
    source: Option<String>,
    /// 最近一次操作的提示（保存到哪里了之类）。
    notice: String,
    /// 读完的拖入文件：(文件名, 内容)。
    pending: Pending,
}

struct RefEntry {
    name: String,
    current: String,
    reference: String,
    /// 加批次前的译文（如果和当前不同）。
    before_batch: Option<String>,
}

type Pending = std::sync::Arc<std::sync::Mutex<Vec<(String, Option<String>)>>>;

impl Default for JargonPage {
    fn default() -> Self {
        Self::with_saved(PRESETS[0].1.to_owned(), String::new())
    }
}

impl JargonPage {
    pub fn with_saved(input: String, batch: String) -> Self {
        let files = local::load();
        let base = Compiler::with_layers(files.clone());
        let comp = Compiler::with_layers(with_batch(&files, &batch));
        let result = comp.compile(&input);
        let candidates = corpus::analyze(&comp.lex, &input);
        let mut page = Self {
            input,
            batch,
            base,
            comp,
            result,
            regress: vec![],
            candidates,
            candidate_limit: 50,
            ref_cache: vec![],
            view: View::Passes,
            clause: 0,
            lookup: "抓手".to_owned(),
            ir_final: true,
            hover: vec![],
            files,
            source: None,
            notice: String::new(),
            pending: Pending::default(),
        };
        page.regress = page.regression();
        page.ref_cache = page.build_ref_cache();
        page
    }

    fn recompile(&mut self) {
        self.result = self.comp.compile(&self.input);
        self.clause = self.clause.min(self.result.clauses.len().saturating_sub(1));
        self.hover.clear();
        self.candidates = corpus::analyze(&self.comp.lex, &self.input);
    }

    fn set_batch(&mut self) {
        self.comp = Compiler::with_layers(with_batch(&self.files, &self.batch));
        self.regress = self.regression();
        self.recompile();
        self.ref_cache = self.build_ref_cache();
    }

    fn build_ref_cache(&self) -> Vec<RefEntry> {
        let refs = corpus::parse_references(corpus::BUILTIN_REFERENCES);
        let has_batch = self.batch_layer().is_some();
        refs.into_iter()
            .map(|r| {
                let current = self.comp.compile(&r.source).output;
                let before_batch = if has_batch {
                    let base_out = self.base.compile(&r.source).output;
                    if base_out != current { Some(base_out) } else { None }
                } else {
                    None
                };
                RefEntry { name: r.name, current, reference: r.reference, before_batch }
            })
            .collect()
    }

    /// 本地词典文件夹变了：重新加载。
    fn reload_files(&mut self) {
        self.files = local::load();
        self.base = Compiler::with_layers(self.files.clone());
        self.set_batch();
    }

    /// 粘贴的批次在词典里是第几层。
    fn batch_layer(&self) -> Option<usize> {
        (!self.batch.trim().is_empty()).then(|| self.comp.lex.layers.len() - 1)
    }

    /// 拖进窗口的 txt 文件：读出来直接编译（Web 版是异步读取，读完后下一帧生效）。
    fn take_dropped(&mut self, ui: &Ui) {
        for f in ui.ctx().input(|i| i.raw.dropped_files.clone()) {
            local::read_dropped(f, self.pending.clone(), ui.ctx().clone());
        }
        let done: Vec<(String, Option<String>)> =
            self.pending.lock().map(|mut v| v.drain(..).collect()).unwrap_or_default();
        for (name, text) in done {
            match text {
                Some(t) => {
                    self.input = t.replace("\r\n", "\n");
                    self.notice = format!("已载入 {name}");
                    self.source = Some(name);
                    self.recompile();
                }
                None => self.notice = format!("读不了 {name}"),
            }
        }
    }

    /// 译文 + 每处改动的说明，保存成 txt 时用。
    fn report(&self) -> String {
        let r = &self.result;
        let mut s = r.output.clone();
        s.push_str(&format!(
            "\n\n———— 黑话编译器报告 ————\n原文 {} 字 → 译文 {} 字，水分 {:.0}%\n",
            r.stats.chars_in,
            r.stats.chars_out,
            r.stats.water() * 100.0
        ));
        for run in &r.passes {
            for ch in &run.changes {
                let after = if ch.after.is_empty() { "（删掉）" } else { &ch.after };
                s.push_str(&format!("[{}] {} → {}：{}\n", run.pass.name(), ch.before, after, ch.why));
            }
        }
        for d in &r.diags {
            s.push_str(&format!("[提醒] {}\n", d.message));
        }
        s
    }

    /// 加上扩充批次以后，哪些示例的译文变了。
    fn regression(&self) -> Regression {
        if self.batch.trim().is_empty() {
            return vec![];
        }
        let mut out = Vec::new();
        let mut inputs: Vec<(String, &str)> = PRESETS.iter().map(|(n, t)| ((*n).to_owned(), *t)).collect();
        if !PRESETS.iter().any(|(_, t)| *t == self.input) {
            inputs.push(("当前输入".to_owned(), &self.input));
        }
        for (name, text) in inputs {
            let a = self.base.compile(text).output;
            let b = self.comp.compile(text).output;
            if a != b {
                out.push((name, a, b));
            }
        }
        out
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        self.take_dropped(ui);
        let wide = ui.available_width() > 1000.0;
        let mut hover = Vec::new();
        if wide {
            egui::Panel::left("jargon_source").default_size(470.0).size_range(340.0..=720.0).show(ui, |ui| {
                ScrollArea::vertical().id_salt("jargon_src_scroll").show(ui, |ui| self.source_panel(ui));
            });
            egui::CentralPanel::default().show(ui, |ui| {
                ScrollArea::vertical().id_salt("jargon_view_scroll").show(ui, |ui| self.views(ui, &mut hover));
            });
        } else {
            egui::CentralPanel::default().show(ui, |ui| {
                ScrollArea::vertical().id_salt("jargon_scroll").show(ui, |ui| {
                    self.source_panel(ui);
                    self.views(ui, &mut hover);
                });
            });
        }
        if hover != self.hover {
            self.hover = hover;
            ui.ctx().request_repaint();
        }
    }

    // ------------------------------------------------------------------ 左栏：输入与译文

    fn source_panel(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        ui.heading("黑话编译器");
        ui.label(RichText::new("把故作高深的\"领导讲话\"编译成平凡质朴的大白话。").color(p.muted));
        let dragging = ui.ctx().input(|i| !i.raw.hovered_files.is_empty());
        let hint = if dragging {
            "松开鼠标，载入这个文件"
        } else {
            "可以直接粘贴自己的文字，或者把 txt 文件拖进窗口"
        };
        ui.label(RichText::new(hint).color(if dragging { p.accent } else { p.muted }));
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("例子：").color(p.muted));
            for (name, text) in PRESETS {
                if ui.selectable_label(self.input == *text, *name).clicked() {
                    self.input = (*text).to_owned();
                    self.recompile();
                }
            }
        });
        let colors: Vec<(Span, Color32, bool)> = self
            .result
            .clauses
            .iter()
            .flat_map(|c| &c.seg.tokens)
            .map(|t| {
                let cat = t.cat(&self.comp.lex);
                (t.span, cat_color(&p, cat), cat == Cat::Modifier)
            })
            .collect();
        let marks = self.hover.clone();
        let mut layouter = |ui: &Ui, text: &dyn egui::TextBuffer, wrap: f32| {
            let mut job = highlight(text.as_str(), &colors, &marks, &p);
            job.wrap.max_width = wrap;
            ui.fonts_mut(|f| f.layout_job(job))
        };
        let edit = TextEdit::multiline(&mut self.input)
            .font(FontId::proportional(18.0))
            .desired_rows(5)
            .desired_width(f32::INFINITY)
            .layouter(&mut layouter);
        if ui.add(edit).changed() {
            self.recompile();
        }
        legend(ui);

        ui.add_space(6.0);
        let r = &self.result;
        let (mut copy, mut save) = (false, false);
        let notice = self.notice.clone();
        card(ui, |ui| {
            ui.label(RichText::new("大白话").strong());
            ui.label(
                RichText::new(if r.output.is_empty() { "（全是水分，什么也没剩下）" } else { &r.output })
                    .size(20.0)
                    .color(p.ok),
            );
            ui.add_space(4.0);
            let s = &r.stats;
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new(format!(
                        "原文 {} 字 → 译文 {} 字，水分 {:.0}%",
                        s.chars_in,
                        s.chars_out,
                        s.water() * 100.0
                    ))
                    .strong(),
                );
                ui.label(
                    RichText::new(format!("· 分出 {} 个词，其中黑话 {} 个", s.words, s.jargon_words)).color(p.muted),
                );
            });
            ui.horizontal_wrapped(|ui| {
                for run in &r.passes {
                    let n = run.changes.len();
                    let fill = if n > 0 { p.done_bg } else { p.card_bg };
                    chip(ui, RichText::new(format!("{} {n}", run.pass.name())).size(13.0), fill, Stroke::NONE);
                }
            });
            if s.fallback > 0 {
                ui.label(
                    RichText::new(format!("{} 个分句的句式没认出来，只做了逐词处理（见下方提醒）。", s.fallback))
                        .size(13.0)
                        .color(p.paren),
                );
            }
            ui.horizontal_wrapped(|ui| {
                copy = ui.button("复制译文").clicked();
                save = local::AVAILABLE && ui.button("保存译文和改动说明").clicked();
            });
            if !notice.is_empty() {
                ui.label(RichText::new(&notice).size(13.0).color(p.ok));
            }
            if r.residual.is_empty() && !r.output.is_empty() {
                ui.label(RichText::new("自检通过：把译文重新分词，没有残留黑话。").size(13.0).color(p.muted));
            }
        });
        if copy {
            ui.ctx().copy_text(self.result.output.clone());
            self.notice = "译文已复制".to_owned();
        }
        if save {
            self.notice = local::save_output(self.source.as_deref(), &self.report());
        }
        let r = &self.result;
        for d in &r.diags {
            diagnostic_view(ui, &self.input, d);
        }
        if !self.candidates.is_empty() {
            ui.add_space(4.0);
            ui.label(
                RichText::new("没认出来的词（原样保留；如果其中有黑话，可以交给 AI 生成词条，见\"扩充词典\"）：")
                    .color(p.muted),
            );
            ui.horizontal_wrapped(|ui| {
                for c in self.candidates.iter().take(30) {
                    let (w, n) = (&c.text, c.count);
                    let text = if n > 1 { format!("{w} ×{n}") } else { w.clone() };
                    chip(ui, RichText::new(text).size(14.0), p.card_bg, Stroke::new(1.0, p.card_stroke));
                }
            });
        }
    }

    // ------------------------------------------------------------------ 右栏

    fn views(&mut self, ui: &mut Ui, hover: &mut Vec<Span>) {
        ui.horizontal_wrapped(|ui| {
            for v in View::ALL {
                ui.selectable_value(&mut self.view, v, RichText::new(v.title()).size(16.0));
            }
        });
        ui.separator();
        match self.view {
            View::Passes => self.passes(ui, hover),
            View::Lattice => self.lattice(ui, hover),
            View::Parse => self.parse(ui, hover),
            View::Ir => self.ir(ui, hover),
            View::Lexicon => self.lexicon(ui),
            View::Extend => self.extend(ui, hover),
            View::Why => why(ui),
        }
    }

    fn passes(&self, ui: &mut Ui, hover: &mut Vec<Span>) {
        let p = Palette::of(ui);
        let r = &self.result;
        prose(
            ui,
            "编译器先把讲话分析成**语义角色**（谁、做什么、靠什么、为了什么），再由四个优化 Pass 依次改写，\
             最后生成中文。鼠标指向一处改动，左边原文里对应的词会高亮。",
        );
        ui.add_space(4.0);
        card(ui, |ui| {
            ui.label(RichText::new("优化前").strong());
            ui.label(
                RichText::new("直接由语义角色 IR 生成——和原文一模一样，说明前端分析没有丢掉任何东西。")
                    .size(13.0)
                    .color(p.muted),
            );
            ui.label(RichText::new(&r.unoptimized).size(17.0));
        });
        for (i, run) in r.passes.iter().enumerate() {
            ui.add_space(6.0);
            card(ui, |ui| {
                ui.label(RichText::new(format!("{} {}", ["①", "②", "③", "④"][i], run.pass.name())).size(18.0).strong());
                prose_colored(ui, run.pass.what(), 14.0, p.muted);
                if run.changes.is_empty() {
                    ui.label(RichText::new("（这段话里没有需要这一步处理的地方）").color(p.muted));
                }
                for ch in &run.changes {
                    let resp = ui
                        .horizontal_wrapped(|ui| {
                            ui.label(RichText::new(&ch.before).size(16.0).color(p.muted).strikethrough());
                            ui.label(RichText::new("→").color(p.accent));
                            let after = if ch.after.is_empty() { "（整块删掉）" } else { &ch.after };
                            ui.label(RichText::new(after).size(16.0).color(p.ok));
                        })
                        .response;
                    let hovered = ui.rect_contains_pointer(resp.rect);
                    prose_colored(ui, &ch.why, 13.5, if hovered { p.accent } else { p.muted });
                    if hovered {
                        *hover = ch.spans.clone();
                    }
                }
                ui.add_space(2.0);
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new("这一步之后：").color(p.muted));
                    ui.label(RichText::new(&run.output).size(16.0));
                });
            });
        }
    }

    fn lattice(&self, ui: &mut Ui, hover: &mut Vec<Span>) {
        let p = Palette::of(ui);
        prose(
            ui,
            "中文没有空格，先要切词。**Aho-Corasick 自动机**（第 1 章 DFA 思想的应用）扫一遍，找出词典里所有能匹配上的词，\
             画成一张**词图**：每一条横条是一个候选词。从头到尾挑一条**代价最小**的路径（动态规划）：\
             词典里的词代价 10，未收录的字每个 12，所以会尽量用词典里的词、尽量用长词。被选中的词是实心的；\
             连在一起的未收录的字合并成一个普通词（虚线框），编译器不改它们。",
        );
        let lex = &self.comp.lex;
        for (ci, c) in self.result.clauses.iter().enumerate() {
            ui.add_space(6.0);
            card(ui, |ui| {
                let cost: u32 = c.seg.best.iter().map(|&k| c.seg.edges[k].cost).sum();
                let cands = c.seg.edges.iter().filter(|e| e.entry.is_some()).count();
                ui.label(
                    RichText::new(format!("分句 {}：{} 个候选词，最优切分代价 {cost}", ci + 1, cands)).color(p.muted),
                );
                ScrollArea::horizontal().id_salt(("lattice", ci)).show(ui, |ui| {
                    if let Some(h) = lattice_view(ui, lex, &c.seg) {
                        *hover = vec![h];
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    for t in &c.seg.tokens {
                        let cat = t.cat(lex);
                        let resp = chip(
                            ui,
                            RichText::new(format!("{} {}", t.text, cat.name())).size(14.0).color(cat_color(&p, cat)),
                            p.card_bg,
                            Stroke::new(1.0, p.card_stroke),
                        );
                        if resp.hovered() {
                            *hover = vec![t.span];
                        }
                    }
                });
            });
        }
        ui.add_space(8.0);
        ui.label(
            RichText::new(format!(
                "词典共 {} 条，Aho-Corasick 自动机有 {} 个状态。扫描的时间只和输入长度、匹配个数有关，和词典大小无关——词典膨胀到几万条也一样快。",
                lex.entries.len(),
                lex.ac.states()
            ))
            .color(p.muted),
        );
    }

    fn clause_picker(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("分句：").color(p.muted));
            for (i, c) in self.result.clauses.iter().enumerate() {
                let fill = if i == self.clause { p.focus_bg } else { p.card_bg };
                let color = if c.parse.tree.is_none() { p.error } else { p.text };
                if chip_button(
                    ui,
                    RichText::new(&c.seg.text).size(14.0).color(color),
                    fill,
                    Stroke::new(1.0, p.card_stroke),
                )
                .clicked()
                {
                    self.clause = i;
                }
            }
        });
    }

    fn parse(&mut self, ui: &mut Ui, hover: &mut Vec<Span>) {
        let p = Palette::of(ui);
        prose(
            ui,
            "黑话的句式很固定，写成一个上下文无关文法（第 2 章）。文法的终结符是词的**类别**（动、名、修饰、以、为……），\
             不是具体的词——所以词典怎么扩充，文法都不用改。这个文法有大量歧义，每条产生式带一个代价，\
             **加权 Earley 分析**直接找出总代价最小的那棵树（Earley + 最短路）。",
        );
        if self.result.clauses.is_empty() {
            return;
        }
        self.clause_picker(ui);
        let c = &self.result.clauses[self.clause];
        let g = &self.comp.grammar;
        ui.horizontal_wrapped(|ui| {
            for (t, &s) in c.seg.tokens.iter().zip(&c.syms) {
                let resp = chip(
                    ui,
                    RichText::new(format!("{}〔{}〕", t.text, g.name(s))).size(14.0),
                    p.card_bg,
                    Stroke::new(1.0, p.card_stroke),
                );
                if resp.hovered() {
                    *hover = vec![t.span];
                }
            }
        });
        match &c.parse.tree {
            Some(tree) => {
                ui.label(
                    RichText::new(format!("总代价 {}，Earley 一共产生了 {} 个项目。", c.parse.cost, c.parse.items))
                        .color(p.muted),
                );
                let flat = tree.flatten();
                let nodes: Vec<TreeNode> = flat
                    .iter()
                    .map(|(t, kids)| TreeNode {
                        label: if t.prod.is_none() {
                            c.seg.tokens[t.start].text.clone()
                        } else {
                            g.name(t.sym).to_owned()
                        },
                        children: kids.clone(),
                    })
                    .collect();
                let style = |i: usize| {
                    let t = flat[i].0;
                    let mut s = NodeStyle::normal(&p);
                    if t.prod.is_none() {
                        s.fill = p.done_bg;
                    } else if g.name(t.sym) == "块" {
                        s.fill = p.focus_bg;
                    }
                    s
                };
                if let Some(i) = scrollable_tree(ui, "jargon_tree", &nodes, style) {
                    let t = flat[i].0;
                    let toks = &c.seg.tokens[t.start..t.end];
                    *hover = toks.iter().map(|t| t.span).collect();
                }
            }
            None => {
                ui.label(RichText::new("这个分句的句式没认出来，只做了逐词处理，见左边的提醒。").color(p.paren));
            }
        }
        egui::CollapsingHeader::new("句式文法").id_salt("jargon_grammar").show(ui, |ui| {
            ui.label(RichText::new(grammar::clause_grammar(&self.comp.lex)).font(mono(14.0)));
            prose_colored(
                ui,
                "代价：每个`块`10（尽量少切块）；没有动词的块 +8、话题在前的块 +3（优先把名词当宾语）；\
                 修饰语当名词用、情态词放进谓词 +1（优先让修饰语修饰后面的动词）。\
                 最后几行是词典里的套话模板生成的规则，`套话`块只算 4，认得出就优先用模板。",
                14.0,
                p.muted,
            );
        });
    }

    fn ir(&mut self, ui: &mut Ui, hover: &mut Vec<Span>) {
        let p = Palette::of(ui);
        prose(
            ui,
            "语法树记录\"句子怎么写\"，优化需要的是\"句子说了什么\"。所以把语法树翻译成一组**语义块**：\
             每块有一个角色（手段、焦点、途径、目的、动作、连接）和几个槽位。优化 Pass 只改词的说法、删除标记和块的句式，\
             生成时再按角色拼回中文。",
        );
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.ir_final, false, "优化前");
            ui.selectable_value(&mut self.ir_final, true, "优化后");
        });
        let irs = if self.ir_final { &self.result.irs } else { &self.result.ir0 };
        for (ci, ir) in irs.iter().enumerate() {
            ui.add_space(4.0);
            card(ui, |ui| {
                let status = if ir.fallback {
                    " · 句式没认出来，只做了逐词处理"
                } else if ir.merged {
                    " · 已合并进前一个分句"
                } else if !ir.has_body() {
                    " · 整个分句都被删掉了"
                } else {
                    ""
                };
                ui.label(RichText::new(format!("分句 {}{status}", ci + 1)).color(p.muted));
                if !ir.pre.is_empty() {
                    ir_row(ui, ir, "前置", &[("主语 / 情态", &ir.pre)], None, false, hover);
                }
                for part in &ir.parts {
                    let slots = slots(part);
                    ir_row(ui, ir, part.role.name(), &slots, part.template.as_ref(), part.dead, hover);
                    if !part.appended.is_empty() {
                        ui.label(
                            RichText::new(format!("    并进来的宾语：{}", part.appended.join("、")))
                                .size(14.0)
                                .color(p.ok),
                        );
                    }
                }
            });
        }
    }

    fn lexicon(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        let lex = &self.comp.lex;
        prose(
            ui,
            "词典是编译器的\"语言定义\"：每个词的类别，以及它的**上位词**或大白话。它是纯文本数据，分层加载——\
             第 0 层手工维护，后面是一批批 AI 生成的扩充。编译流水线只看类别，不看具体的词，所以扩充词典不用改代码。",
        );
        card(ui, |ui| {
            egui::Grid::new("jargon_layers").num_columns(4).spacing([18.0, 4.0]).show(ui, |ui| {
                for h in ["层", "收录", "错误 / 警告 / 提示", ""] {
                    ui.label(RichText::new(h).strong());
                }
                ui.end_row();
                for (i, layer) in lex.layers.iter().enumerate() {
                    let count = |sev| lex.diags.iter().filter(|d| d.layer == i && d.diag.severity == sev).count();
                    ui.label(&layer.name);
                    ui.label(RichText::new(lex.count_by_layer(i).to_string()).font(mono(14.0)));
                    ui.label(
                        RichText::new(format!(
                            "{} / {} / {}",
                            count(bsc_core::Severity::Error),
                            count(bsc_core::Severity::Warning),
                            count(bsc_core::Severity::Note)
                        ))
                        .font(mono(14.0)),
                    );
                    ui.label(
                        RichText::new(if i == 0 { "功能词只能在这层定义；冲突时它优先" } else { "" }).color(p.muted),
                    );
                    ui.end_row();
                }
            });
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("按类别：").color(p.muted));
                for cat in [
                    Cat::Modifier,
                    Cat::Hollow,
                    Cat::Filler,
                    Cat::Vague,
                    Cat::Fixed,
                    Cat::FixedVerb,
                    Cat::Verb,
                    Cat::Content,
                    Cat::Abbrev,
                    Cat::Pattern,
                ] {
                    let n = lex.entries.iter().filter(|e| e.cat == cat).count();
                    chip(
                        ui,
                        RichText::new(format!("{} {n}", cat.name())).size(14.0).color(cat_color(&p, cat)),
                        p.card_bg,
                        Stroke::new(1.0, p.card_stroke),
                    );
                }
                let n = lex.entries.iter().filter(|e| matches!(e.cat, Cat::Func(_))).count();
                chip(
                    ui,
                    RichText::new(format!("功能词 {n}")).size(14.0).color(p.paren),
                    p.card_bg,
                    Stroke::new(1.0, p.card_stroke),
                );
            });
        });

        ui.add_space(6.0);
        card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("查词：");
                ui.add_sized([200.0, 24.0], TextEdit::singleline(&mut self.lookup));
            });
            let word = self.lookup.trim();
            match lex.get(word) {
                Some(e) => {
                    let entry = &lex.entries[e];
                    ui.label(format!("类别：{}　来自：{}", entry.cat.name(), lex.layers[entry.layer].name));
                    let chain = lex.chain(e);
                    if chain.len() > 1 {
                        ui.label(RichText::new(format!("降级链：{}", chain.join(" → "))).size(17.0).color(p.ok));
                    }
                    if let Some(n) = &entry.note {
                        ui.label(RichText::new(format!("备注：{n}")).color(p.muted));
                    }
                }
                None if word.is_empty() => {}
                None => {
                    ui.label(RichText::new("词典里没有这个词。").color(p.muted));
                }
            }
        });

        ui.add_space(6.0);
        ui.label(RichText::new("词典自己的校验结果").size(18.0).strong());
        let batch = self.batch_layer();
        let diags: Vec<_> = lex.diags.iter().filter(|d| Some(d.layer) != batch).collect();
        if diags.is_empty() {
            ui.label(RichText::new("没有问题。").color(p.ok));
        }
        for d in diags {
            lex_diag_view(ui, lex, d.layer, &d.diag);
        }
    }

    fn extend(&mut self, ui: &mut Ui, hover: &mut Vec<Span>) {
        let p = Palette::of(ui);
        prose(
            ui,
            "黑话的词汇在不断增加，词典要能随时扩充。最省事的办法是让一个便宜的 AI **批量生成词条**，粘贴到下面。\
             但 AI 生成的东西不可靠，词条一多问题也会越来越多，所以每一批都像源码一样先**编译检查**：",
        );
        callout(
            ui,
            CalloutKind::KeyPoint,
            "映射关系快速膨胀时会出的问题，以及编译器怎么应对",
            |ui| {
                for (problem, answer) in RISKS {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(*problem).strong());
                        prose_colored(ui, answer, 15.0, p.muted);
                    });
                }
            },
        );
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            if ui.button("载入示例批次（混进了各种问题）").clicked() {
                self.batch = EXAMPLE_BATCH.to_owned();
                self.set_batch();
            }
            if ui.button("清空").clicked() {
                self.batch.clear();
                self.set_batch();
            }
        });
        let edit = TextEdit::multiline(&mut self.batch)
            .font(mono(15.0))
            .desired_rows(10)
            .desired_width(f32::INFINITY)
            .hint_text("一行一个词条：词 | 类别 | 映射 | 备注\n例如：\n破圈 | 固定动 | 扩大影响\n抓手 | 虚名 | 着力点");
        if ui.add(edit).changed() {
            self.set_batch();
        }
        if local::AVAILABLE {
            self.local_files(ui);
        }
        let lex = &self.comp.lex;
        if let Some(layer) = self.batch_layer() {
            let diags: Vec<_> = lex.diags.iter().filter(|d| d.layer == layer).collect();
            let count = |sev| diags.iter().filter(|d| d.diag.severity == sev).count();
            ui.label(
                RichText::new(format!(
                    "这一批收录 {} 条；错误 {}（没有收录）、警告 {}、提示 {}。",
                    lex.count_by_layer(layer),
                    count(bsc_core::Severity::Error),
                    count(bsc_core::Severity::Warning),
                    count(bsc_core::Severity::Note)
                ))
                .strong(),
            );
            ui.add_space(4.0);
            ui.label(RichText::new("回归对比：加上这一批以后，哪些译文变了").size(18.0).strong());
            if self.regress.is_empty() {
                ui.label(RichText::new("示例讲话和当前输入的译文都没有变。").color(p.muted));
            }
            for (name, before, after) in &self.regress {
                card(ui, |ui| {
                    ui.label(RichText::new(name).color(p.muted));
                    ui.label(RichText::new(before).strikethrough().color(p.muted));
                    ui.label(RichText::new(after).color(p.ok));
                });
            }
            ui.add_space(4.0);
            ui.label(RichText::new("逐条校验").size(18.0).strong());
            for d in diags {
                lex_diag_view(ui, lex, layer, &d.diag);
            }
        }
        ui.add_space(8.0);
        self.todo_list(ui, hover);
        ui.add_space(8.0);
        self.references(ui);
    }

    /// 待办词表：当前输入（整篇真实文章）里没认出来的词，连同例句交给 AI。
    fn todo_list(&mut self, ui: &mut Ui, hover: &mut Vec<Span>) {
        let p = Palette::of(ui);
        ui.label(RichText::new("待办词表：语料里没认出来的词").size(18.0).strong());
        prose(
            ui,
            "把真实的文章（工作报告、年终总结、PPT 文字……）粘贴或拖进左边的输入框，这里列出编译器不认识的词，\
             按出现次数排序。连同**原文例句**一起交给 AI 填词条，比让 AI 凭空编词可靠：收进来的都是真在用的说法，\
             AI 也能看例句判断这个词在这里是不是空话。标着\"拼合\"的是一半已收录、一半没收录的四字格（统筹谋划），\
             最好整体收成一条。",
        );
        if self.candidates.is_empty() {
            ui.label(RichText::new("当前输入里的词都认识。").color(p.muted));
            return;
        }
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("共 {} 个，取前", self.candidates.len()));
            for n in [30, 50, 100] {
                ui.selectable_value(&mut self.candidate_limit, n, n.to_string());
            }
            ui.label("个。");
        });
        let shown = &self.candidates[..self.candidates.len().min(self.candidate_limit)];
        let words: Vec<(String, String)> = shown.iter().map(|c| (c.text.clone(), c.example.clone())).collect();
        let prompt = ai_prompt(&words);
        if ui.button(format!("复制给 AI 的提示词（含这 {} 个词和例句）", shown.len())).clicked() {
            ui.ctx().copy_text(prompt.clone());
        }
        egui::Grid::new("jargon_todo").striped(true).num_columns(3).show(ui, |ui| {
            for c in shown {
                let mut word = RichText::new(&c.text).strong();
                if c.kind == corpus::Kind::Phrase {
                    word = word.color(p.accent);
                }
                let mut hovered = ui.label(word).hovered();
                let tag = if c.kind == corpus::Kind::Phrase { "拼合 ×" } else { "×" };
                hovered |= ui.label(RichText::new(format!("{tag}{}", c.count)).color(p.muted)).hovered();
                hovered |= ui.label(RichText::new(&c.example).size(14.0).color(p.muted)).hovered();
                if hovered {
                    *hover = vec![c.span];
                }
                ui.end_row();
            }
        });
        egui::CollapsingHeader::new(RichText::new("提示词全文").size(17.0)).id_salt("jargon_prompt").show(ui, |ui| {
            ui.label(RichText::new(&prompt).font(mono(13.0)));
        });
    }

    /// 参考译文对照：人写的大白话 vs 编译器的译文；加了批次以后变了的标出来。
    fn references(&self, ui: &mut Ui) {
        let p = Palette::of(ui);
        ui.label(RichText::new("参考译文对照").size(18.0).strong());
        prose(
            ui,
            "几段真实原文和**人写的参考译文**（`crates/bsc-jargon/reference/reference.txt`）。编译器不追求和参考译文逐字相同；\
             加一批词以后，看译文是离参考更近还是更远——比只看\"有没有残留黑话\"更能发现删过头、把实词当空话这类问题。",
        );
        for r in &self.ref_cache {
            card(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(&r.name).strong());
                    if r.before_batch.is_some() {
                        ui.label(RichText::new("加这一批以后变了").color(p.accent));
                    }
                });
                if let Some(before) = &r.before_batch {
                    ui.label(RichText::new(before).strikethrough().color(p.muted));
                }
                ui.label(RichText::new(format!("编译器：{}", r.current)).color(p.ok));
                ui.label(format!("参考：{}", r.reference));
            });
        }
    }
}

impl JargonPage {
    /// 本地词典文件夹：每一批词条是一个 txt 文件，启动时自动加载。
    fn local_files(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        ui.add_space(4.0);
        card(ui, |ui| {
            ui.label(RichText::new("本地词典文件夹").strong());
            ui.label(RichText::new(local::dir_text()).font(mono(13.0)).color(p.muted));
            ui.label(
                RichText::new(
                    "每一批词条是这里的一个 txt 文件，启动时自动加载（排在内置词典后面）。删掉文件就是停用这一批。",
                )
                .size(13.0)
                .color(p.muted),
            );
            let mut remove = None;
            for (i, f) in self.files.iter().enumerate() {
                ui.horizontal(|ui| {
                    let n = self.comp.lex.layers.iter().position(|l| l.name == f.name);
                    let count = n.map_or(0, |n| self.comp.lex.count_by_layer(n));
                    ui.label(format!("{}（{count} 条）", f.name));
                    if ui.small_button("删除").clicked() {
                        remove = Some(i);
                    }
                });
            }
            if let Some(i) = remove {
                self.notice = local::remove(&self.files[i].name);
                self.reload_files();
            }
            ui.horizontal_wrapped(|ui| {
                let can_save = !self.batch.trim().is_empty();
                if ui.add_enabled(can_save, egui::Button::new("把上面粘贴的词条存成新的一批")).clicked() {
                    self.notice = local::save_batch(&self.batch);
                    self.batch.clear();
                    self.reload_files();
                }
                if ui.button("重新加载").clicked() {
                    self.reload_files();
                }
            });
            if !self.notice.is_empty() {
                ui.label(RichText::new(&self.notice).size(13.0).color(p.ok));
            }
        });
    }
}

fn with_batch(files: &[Layer], batch: &str) -> Vec<Layer> {
    let mut v = files.to_vec();
    v.push(Layer { name: BATCH_NAME.to_owned(), text: batch.to_owned() });
    v
}

/// 本地文件：原生版读写磁盘；Web 版没有文件系统，这些功能不显示。
#[cfg(not(target_arch = "wasm32"))]
mod local {
    use std::path::{Path, PathBuf};

    use bsc_jargon::lexicon::Layer;

    pub const AVAILABLE: bool = true;

    fn dir() -> Option<PathBuf> {
        eframe::storage_dir("BSc 编译原理实验室").map(|d| d.join("lexicon"))
    }

    pub fn dir_text() -> String {
        dir().map_or("（找不到用户目录）".to_owned(), |d| d.display().to_string())
    }

    pub fn load() -> Vec<Layer> {
        let Some(dir) = dir() else { return vec![] };
        let Ok(rd) = std::fs::read_dir(&dir) else { return vec![] };
        let mut files: Vec<PathBuf> =
            rd.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|x| x == "txt")).collect();
        files.sort();
        files
            .into_iter()
            .filter_map(|p| {
                let text = read(&p)?;
                Some(Layer { name: p.file_name()?.to_string_lossy().into_owned(), text })
            })
            .collect()
    }

    pub fn read(p: &Path) -> Option<String> {
        std::fs::read(p).ok().map(|b| String::from_utf8_lossy(&b).into_owned())
    }

    pub fn read_dropped(f: eframe::egui::DroppedFileHandle, sink: super::Pending, _: eframe::egui::Context) {
        let text = f.bytes().ok().map(|b| String::from_utf8_lossy(&b).into_owned());
        if let Ok(mut v) = sink.lock() {
            v.push((f.path().display().to_string(), text));
        }
    }

    pub fn save_batch(text: &str) -> String {
        let Some(dir) = dir() else { return "找不到用户目录，没有保存".to_owned() };
        if let Err(e) = std::fs::create_dir_all(&dir) {
            return format!("建不了文件夹：{e}");
        }
        let path = (1..).map(|n| dir.join(format!("batch-{n:03}.txt"))).find(|p| !p.exists()).unwrap();
        match std::fs::write(&path, text) {
            Ok(()) => format!("已保存到 {}", path.display()),
            Err(e) => format!("保存失败：{e}"),
        }
    }

    pub fn remove(name: &str) -> String {
        let Some(dir) = dir() else { return String::new() };
        match std::fs::remove_file(dir.join(name)) {
            Ok(()) => format!("已删除 {name}"),
            Err(e) => format!("删除失败：{e}"),
        }
    }

    /// 保存译文：拖进来的文件旁边存一份"原名.大白话.txt"，否则存到应用的数据文件夹。
    pub fn save_output(source: Option<&str>, report: &str) -> String {
        let path = match source.map(Path::new).filter(|p| p.is_absolute()) {
            Some(p) => {
                let stem = p.file_stem().map_or("译文".into(), |s| s.to_string_lossy().into_owned());
                p.with_file_name(format!("{stem}.大白话.txt"))
            }
            None => match dir().and_then(|d| d.parent().map(|p| p.join("大白话.txt"))) {
                Some(p) => p,
                None => return "找不到用户目录，没有保存".to_owned(),
            },
        };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match std::fs::write(&path, report) {
            Ok(()) => format!("已保存到 {}", path.display()),
            Err(e) => format!("保存失败：{e}"),
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod local {
    use bsc_jargon::lexicon::Layer;

    pub const AVAILABLE: bool = false;

    pub fn dir_text() -> String {
        String::new()
    }

    pub fn load() -> Vec<Layer> {
        vec![]
    }

    pub fn read_dropped(f: eframe::egui::DroppedFileHandle, sink: super::Pending, ctx: eframe::egui::Context) {
        wasm_bindgen_futures::spawn_local(async move {
            let text = f.bytes_async().await.ok().map(|b| String::from_utf8_lossy(&b).into_owned());
            if let Ok(mut v) = sink.lock() {
                v.push((f.path().display().to_string(), text));
            }
            ctx.request_repaint();
        });
    }

    pub fn save_batch(_: &str) -> String {
        String::new()
    }

    pub fn remove(_: &str) -> String {
        String::new()
    }

    pub fn save_output(_: Option<&str>, _: &str) -> String {
        String::new()
    }
}

const BATCH_NAME: &str = "你粘贴的批次";

const RISKS: &[(&str, &str)] = &[
    (
        "同一个词前后定义不一样",
        "按层决定：核心词典优先，后来的批次覆盖更早的批次；每处冲突都报 W3004，完全相同的重复报 N3004。",
    ),
    ("上位词绕成圈", "A → B → A 降级时会无限循环。加载时沿映射走一遍检测环（E3007），圈上的词不再降级。"),
    (
        "映射到的说法还是黑话",
        "用编译器自己的分词器切一遍映射的终点（W3008）；每次编译完还会把译文再分一次词，确认没有残留（W3102）。",
    ),
    (
        "单字、功能词把普通词切碎",
        "只有核心词典能定义单字词条和功能词（E3010 / E3003）；核心词典用\"内容\"类登记\"认为\"\"可以\"这类普通词来保护。",
    ),
    ("词越多越慢", "Aho-Corasick 自动机扫描一遍就找出全部候选词，时间和词典大小无关。"),
    ("切分有歧义", "候选词画成词图，用动态规划选代价最小的切分，而不是\"先到先得\"。"),
    ("加了新词，旧句子悄悄变了", "每加一批，自动重新编译示例讲话，列出译文的变化（回归对比）。"),
    ("文法跟着词典改", "不用改：句式文法的终结符是词的类别，新词只要分对类，就自动适用所有句式和优化。"),
    (
        "套话模板写坏了",
        "模板（坚持{X}不动摇）加载时变成句式文法的规则。槽位写错报 E3014；映射丢了或重复了槽位报 E3015（槽位里是实际内容，不能丢）；\
         字面部分有没收录的单字报 E3016（为它收单字会把普通词切碎）。",
    ),
];

fn why(ui: &mut Ui) {
    let p = Palette::of(ui);
    callout(ui, CalloutKind::KeyPoint, "为什么说这是一个编译问题", |ui| {
        prose(
            ui,
            "黑话看起来花样百出，其实高度公式化：词汇有限，句式固定，修饰语可以无限堆叠却几乎不携带信息。\
             \"有固定词汇、有固定句式\"——这正是编译器擅长处理的\"语言\"。",
        );
    });
    ui.add_space(6.0);
    ui.label(RichText::new("逐词替换（或者直接让 AI 改写）做不好的事").size(18.0).strong());
    for (what, how) in HARD {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(*what).strong());
            prose_colored(ui, how, 15.0, p.muted);
        });
    }
    ui.add_space(8.0);
    ui.label(RichText::new("编译原理 ↔ 黑话编译器").size(18.0).strong());
    card(ui, |ui| {
        egui::Grid::new("jargon_mapping").num_columns(3).striped(true).spacing([16.0, 8.0]).show(ui, |ui| {
            for h in ["编译器里的概念", "在黑话编译器里", "课程"] {
                ui.label(RichText::new(h).strong());
            }
            ui.end_row();
            for (concept, meaning, lesson) in MAPPING {
                ui.label(RichText::new(*concept).color(p.accent));
                ui.label(*meaning);
                ui.label(RichText::new(*lesson).color(p.muted));
                ui.end_row();
            }
        });
    });
    ui.add_space(6.0);
    ui.label(RichText::new("规划中：反向编译（大白话 → 黑话），用同一套语义角色 IR，换一个后端。").color(p.muted));
}

const HARD: &[(&str, &str)] = &[
    ("句式整体改写", "\"以 X 为抓手\"逐词替换会变成\"以 X 为入手的地方\"；编译器认出句式，改写成\"从 X 入手\"。"),
    ("整块删除没有信息的话", "\"形成全链路闭环\"的动词和宾语都是虚的，整块删掉，而不是翻译成\"有完整流程\"。"),
    ("动词堆叠", "\"统筹推进\"\"抓好落实\"只留中心动词。"),
    ("排比合并", "\"加强 A，强化 B\"降级后动词相同，合并成\"加强 A、B\"；折叠后重复的分句删掉。"),
    ("删完的善后", "修饰语删掉以后悬空的\"的\"\"和\"、整句删光后留下的\"我们要\"，都要处理好。"),
    (
        "可解释、可复现",
        "每一处改动都有理由；同一个词在全文里译法一致；同样的输入永远得到同样的输出，再编译一遍结果不变。",
    ),
    ("知道自己不会", "句式没认出来的分句退回逐词处理，并明确提醒哪一句只做了词语级的处理。"),
];

const MAPPING: &[(&str, &str, &str)] = &[
    ("词法分析", "Aho-Corasick 找候选词 + 词图最小代价切分", "第 1 章"),
    ("语法分析", "句式文法 + 加权 Earley（在歧义中挑最优）", "第 2 章"),
    ("语义分析 / IR", "语义角色：手段、焦点、途径、目的、动作", "第 3、4 章"),
    ("常量折叠", "固定搭配整体换成大白话：降本增效 → 省钱又提效", "第 5 章"),
    ("死代码消除", "删膨胀修饰语、删整块没有信息的话", "第 5 章"),
    ("降级 (lowering)", "沿上位词链降到朴素说法；句式改写", "第 4、6 章"),
    ("公共子表达式消除", "动词堆叠、排比合并、重复分句", "第 5 章"),
    ("代码生成", "按角色拼回中文，保留原来的标点", "第 6 章"),
    ("诊断信息", "词典校验、超出句式范围、残留黑话自检", "全书"),
];

// ------------------------------------------------------------------ 小部件

fn cat_color(p: &Palette, cat: Cat) -> Color32 {
    match cat {
        Cat::Modifier => p.muted,
        Cat::Hollow | Cat::Filler => p.keyword,
        Cat::Vague => p.operator,
        Cat::Fixed | Cat::FixedVerb | Cat::Abbrev | Cat::Pattern => p.number,
        Cat::Func(_) => p.paren,
        Cat::Verb | Cat::Content => p.text,
    }
}

fn legend(ui: &mut Ui) {
    let p = Palette::of(ui);
    ui.horizontal_wrapped(|ui| {
        for (name, cat) in [
            ("修饰语", Cat::Modifier),
            ("空洞动词", Cat::Hollow),
            ("虚指名词", Cat::Vague),
            ("固定搭配", Cat::Fixed),
            ("功能词", Cat::Func(bsc_jargon::lexicon::Func::Yi)),
        ] {
            let mut t = RichText::new(name).size(13.0).color(cat_color(&p, cat));
            if cat == Cat::Modifier {
                t = t.strikethrough();
            }
            ui.label(t);
        }
    });
}

/// 输入框的着色：按词的类别上色，修饰语加删除线，鼠标指向的部分加底色。
fn highlight(src: &str, colors: &[(Span, Color32, bool)], hover: &[Span], p: &Palette) -> LayoutJob {
    let mut cuts = vec![0, src.len()];
    for (s, _, _) in colors {
        cuts.extend([s.start, s.end]);
    }
    for h in hover {
        cuts.extend([h.start, h.end]);
    }
    cuts.retain(|&c| c <= src.len() && src.is_char_boundary(c));
    cuts.sort_unstable();
    cuts.dedup();
    let mut job = LayoutJob::default();
    for w in cuts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let seg = Span::new(a, b);
        let (fg, strike) =
            colors.iter().find(|(s, _, _)| s.contains(seg)).map_or((p.text, false), |(_, c, st)| (*c, *st));
        let bg = if hover.iter().any(|h| h.contains(seg)) { p.focus_bg } else { Color32::TRANSPARENT };
        job.append(
            &src[a..b],
            0.0,
            TextFormat {
                font_id: FontId::proportional(18.0),
                color: fg,
                background: bg,
                strikethrough: if strike { Stroke::new(1.0, fg) } else { Stroke::NONE },
                ..Default::default()
            },
        );
    }
    job
}

/// 画一个分句的词图。返回鼠标指向的词在原文里的位置。
fn lattice_view(ui: &mut Ui, lex: &Lexicon, c: &Clause) -> Option<Span> {
    let p = Palette::of(ui);
    const CELL: f32 = 30.0;
    const BAR: f32 = 22.0;
    const GAP: f32 = 4.0;
    let chars: Vec<char> = c.text.chars().collect();
    let offsets: Vec<usize> = c.text.char_indices().map(|(i, _)| c.span.start + i).chain([c.span.end]).collect();
    let chosen: Vec<usize> = c.best.clone();
    // 候选词（词典里的）按区间排进若干行，互不重叠
    let mut dict: Vec<usize> = (0..c.edges.len()).filter(|&k| c.edges[k].entry.is_some()).collect();
    dict.sort_by_key(|&k| (c.edges[k].start, std::cmp::Reverse(c.edges[k].end)));
    let mut rows: Vec<Vec<usize>> = Vec::new();
    for k in dict {
        let e = c.edges[k];
        match rows.iter_mut().find(|r| r.iter().all(|&o| c.edges[o].end <= e.start || c.edges[o].start >= e.end)) {
            Some(r) => r.push(k),
            None => rows.push(vec![k]),
        }
    }
    let unknown: Vec<(usize, usize)> = {
        // 未收录的普通词：最优路径上连续的未知字
        let mut v: Vec<(usize, usize)> = Vec::new();
        for &k in &chosen {
            let e = c.edges[k];
            if e.entry.is_some() || chars[e.start].is_whitespace() {
                continue;
            }
            match v.last_mut() {
                Some(last) if last.1 == e.start => last.1 = e.end,
                _ => v.push((e.start, e.end)),
            }
        }
        v
    };
    let height = CELL + (rows.len() + 1) as f32 * (BAR + GAP) + 6.0;
    let width = chars.len() as f32 * CELL + 8.0;
    let (resp, painter) = ui.allocate_painter(Vec2::new(width, height), Sense::hover());
    let x0 = resp.rect.left() + 4.0;
    let y0 = resp.rect.top();
    for (i, ch) in chars.iter().enumerate() {
        painter.text(
            egui::pos2(x0 + (i as f32 + 0.5) * CELL, y0 + CELL / 2.0),
            egui::Align2::CENTER_CENTER,
            ch.to_string(),
            FontId::proportional(18.0),
            p.text,
        );
    }
    let pointer = resp.hover_pos();
    let mut hovered = None;
    let bar_rect = |start: usize, end: usize, row: usize| {
        Rect::from_min_size(
            egui::pos2(x0 + start as f32 * CELL + 1.0, y0 + CELL + row as f32 * (BAR + GAP)),
            Vec2::new((end - start) as f32 * CELL - 2.0, BAR),
        )
    };
    for (ri, row) in rows.iter().enumerate() {
        for &k in row {
            let e = c.edges[k];
            let entry = &lex.entries[e.entry.unwrap()];
            let rect = bar_rect(e.start, e.end, ri);
            let on = chosen.contains(&k);
            let color = cat_color(&p, entry.cat);
            let fill = if on { color.gamma_multiply(0.35) } else { Color32::TRANSPARENT };
            painter.rect_filled(rect, CornerRadius::same(4), fill);
            painter.rect_stroke(
                rect,
                CornerRadius::same(4),
                Stroke::new(if on { 2.0 } else { 1.0 }, color),
                StrokeKind::Inside,
            );
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                entry.cat.name(),
                FontId::proportional(12.0),
                p.text,
            );
            if pointer.is_some_and(|q| rect.contains(q)) {
                hovered = Some((Span::new(offsets[e.start], offsets[e.end]), e.entry));
            }
        }
    }
    for &(a, b) in &unknown {
        let rect = bar_rect(a, b, rows.len());
        painter.rect_stroke(rect, CornerRadius::same(4), Stroke::new(1.0, p.muted), StrokeKind::Inside);
        painter.text(rect.center(), egui::Align2::CENTER_CENTER, "普通词", FontId::proportional(12.0), p.muted);
        if pointer.is_some_and(|q| rect.contains(q)) {
            hovered = Some((Span::new(offsets[a], offsets[b]), None));
        }
    }
    if let Some((span, entry)) = hovered {
        let text = match entry {
            Some(id) => {
                let e = &lex.entries[id];
                let chain = lex.chain(id);
                let map = if chain.len() > 1 { format!("，降级链 {}", chain.join(" → ")) } else { String::new() };
                format!("{}：{}{map}（{}）", e.word, e.cat.name(), lex.layers[e.layer].name)
            }
            None => "没收录的普通词，原样保留".to_owned(),
        };
        resp.on_hover_text(text);
        return Some(span);
    }
    None
}

fn slots(part: &Part) -> Vec<(&'static str, &Vec<usize>)> {
    let mut v = vec![("标记", &part.markers), ("动词", &part.verbs), ("对象", &part.obj), ("宾语", &part.obj2)];
    v.retain(|(_, ws)| !ws.is_empty());
    v
}

fn ir_row(
    ui: &mut Ui,
    ir: &ClauseIr,
    role: &str,
    slots: &[(&str, &Vec<usize>)],
    template: Option<&Template>,
    dead: bool,
    hover: &mut Vec<Span>,
) {
    let p = Palette::of(ui);
    ui.horizontal_wrapped(|ui| {
        let mut r = RichText::new(role).strong().color(p.accent);
        if dead {
            r = r.strikethrough().color(p.muted);
        }
        chip(ui, r, p.done_bg, Stroke::NONE);
        for (name, ws) in slots {
            ui.label(RichText::new(format!("{name}:")).size(13.0).color(p.muted));
            for &w in ws.iter() {
                let word = &ir.words[w];
                let text = match &word.now {
                    Some(now) => format!("{}→{now}", word.text),
                    None => word.text.clone(),
                };
                let mut t = RichText::new(text).size(15.0).color(cat_color(&p, word.cat));
                let mut fill = p.card_bg;
                if word.dead || dead {
                    t = t.strikethrough();
                    fill = p.error_bg;
                }
                if chip(ui, t, fill, Stroke::new(1.0, p.card_stroke)).hovered() {
                    *hover = vec![word.span];
                }
            }
        }
        if let Some(t) = template {
            let name = match t {
                Template::StartFrom => "句式改写：从 X 入手",
                Template::DrivenBy => "句式改写：靠 X 推动",
                Template::TreatAs => "句式改写：把 X 当作 Y",
                Template::Has(_) => "句式改写：让 X 有 Y",
                Template::Pattern(_) => "套话模板：整体换成大白话，槽位照搬",
            };
            ui.label(RichText::new(name).size(13.0).color(p.ok));
        }
    });
}

/// 词典的诊断：只显示涉及的那几行，而不是整个文件。
fn lex_diag_view(ui: &mut Ui, lex: &Lexicon, layer: usize, d: &Diagnostic) {
    let p = Palette::of(ui);
    let text = &lex.layers[layer].text;
    let Some(first) = d.labels.iter().map(|l| l.span.start).min() else {
        diagnostic_view(ui, text, d);
        return;
    };
    let last = d.labels.iter().map(|l| l.span.end).max().unwrap_or(first);
    let start = text[..first].rfind('\n').map_or(0, |i| i + 1);
    let end = text[last..].find('\n').map_or(text.len(), |i| last + i);
    let mut local = d.clone();
    for l in &mut local.labels {
        l.span = Span::new(l.span.start - start, l.span.end - start);
    }
    let line = line_col(text, first).line;
    ui.label(RichText::new(format!("{} 第 {line} 行", lex.layers[layer].name)).size(13.0).color(p.muted));
    diagnostic_view(ui, &text[start..end], &local);
}
