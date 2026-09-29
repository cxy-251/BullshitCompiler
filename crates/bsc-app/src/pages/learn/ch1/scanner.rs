//! 1.1 手写一个扫描器。

use bsc_core::{Span, line_col};
use bsc_minilang::lexer::{LexResult, StepKind, TokenClass, TokenKind, lex};
use eframe::egui::{self, Color32, RichText, TextEdit, Ui};

use crate::theme::{Palette, mono};
use crate::widgets::{
    CalloutKind, Mark, Stepper, callout, card, chip, diagnostic_view, prose, prose_sized, quiz, source_view,
};

const PRESETS: &[(&str, &str)] = &[
    (
        "正常的程序",
        "// 求 1 + 2 + … + n\nfn sum(n: int) -> int {\n    let mut s = 0;\n    let mut i = 1;\n    while i <= n {\n        s = s + i;   /* 累加 */\n        i = i + 1;\n    }\n    return s;\n}",
    ),
    ("中文名字", "let 单价 = 12;\nlet 数量 = 3;\nlet 总价 = 单价 * 数量;"),
    ("有错误的程序", "let x = 1；\nlet 2y = x & 3;\nif x >= 1 { x = x - 1 }"),
];

/// 伪代码的各行，扫描器每一步对应其中一行。
const PSEUDO: &[&str] = &[
    "循环，直到读完所有字符：",
    "  空白                → 跳过",
    "  // 或 /*            → 注释，跳过",
    "  字母或 _            → 读完整个单词，再查关键字表",
    "  数字                → 读完所有数字",
    "  = < > ! - & |       → 偷看下一个字符，决定单字符还是双字符",
    "  + * ( ) { } ; 等    → 直接产出",
    "  都不是              → 报错，跳过这个字符继续",
    "最后补一个\"输入结束\"记号",
];

fn pseudo_line(kind: StepKind) -> usize {
    match kind {
        StepKind::Whitespace => 1,
        StepKind::LineComment | StepKind::BlockComment => 2,
        StepKind::Word { .. } => 3,
        StepKind::Number => 4,
        StepKind::Symbol { peeked: true, .. } => 5,
        StepKind::Symbol { peeked: false, .. } => 6,
        StepKind::Error => 7,
        StepKind::Eof => 8,
    }
}

pub struct Lesson {
    src: String,
    result: LexResult,
    stepper: Stepper,
}

impl Default for Lesson {
    fn default() -> Self {
        let src = PRESETS[0].1.to_owned();
        Self { result: lex(&src), src, stepper: Stepper::default() }
    }
}

pub fn class_color(p: &Palette, class: TokenClass) -> Color32 {
    match class {
        TokenClass::Keyword => p.keyword,
        TokenClass::Ident => p.text,
        TokenClass::Number => p.number,
        TokenClass::Operator => p.operator,
        TokenClass::Punct => p.paren,
        TokenClass::End => p.muted,
    }
}

impl Lesson {
    pub fn ui(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        ui.heading("1.1  手写一个扫描器");
        ui.label(RichText::new("入门 · 约 20 分钟").color(p.muted));

        ui.add_space(8.0);
        ui.label(RichText::new("一句话").size(20.0).strong());
        prose(
            ui,
            "第 0 课的计算器只认识数字和几个符号。真正的编程语言要复杂得多：有关键字、变量名、`<=` 这样由两个字符组成的运算符，\
             还有注释。这一课看看一个真实的词法分析器（也叫扫描器）是怎么手写出来的——它就是后面课程要用的 mini-lang 的词法分析器。",
        );
        callout(ui, CalloutKind::Analogy, "给没有空格的句子断词", |ui| {
            ui.label(
                "中文句子没有空格，\"南京市长江大桥\"可以断成\"南京市 / 长江大桥\"，也能断成\"南京 / 市长 / 江大桥\"。\
                 人靠常识判断，机器却需要明确的规则。编程语言在设计时就刻意避免了这种歧义，扫描器只需要遵守几条简单的规则。",
            );
        });

        ui.add_space(8.0);
        ui.label(RichText::new("五条规则").size(20.0).strong());
        for (title, body) in [
            ("看第一个字符定种类", "字母开头的是名字或关键字，数字开头的是整数，`/` 开头的可能是除号也可能是注释……"),
            (
                "最长匹配",
                "一直读到不能再读为止：`count1` 是一个名字，不是 `count` 和 `1`；`<=` 是一个记号，不是 `<` 和 `=`。",
            ),
            ("向前看一个字符", "看到 `=` 还不能下结论，要偷看下一个字符：是 `=` 就组成 `==`，否则就是单个 `=`。"),
            (
                "先读完，再查关键字表",
                "`iffy` 要先当作名字整个读完，再去关键字表里查——它不在表里，所以是名字，而不是 `if` + `fy`。",
            ),
            ("记下位置", "每个记号都记住自己在源码的第几个字节到第几个字节，出错时才能准确指出位置。"),
        ] {
            callout(ui, CalloutKind::KeyPoint, title, |ui| {
                prose(ui, body);
            });
        }

        ui.add_space(8.0);
        ui.label(RichText::new("单步观察").size(20.0).strong());
        ui.label("编辑下面的代码，然后一步一步看扫描器怎么切。");
        let changed = card(ui, |ui| {
            let mut changed = false;
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("例子：").color(p.muted));
                for (name, code) in PRESETS {
                    if ui.button(*name).clicked() {
                        self.src = (*code).to_owned();
                        changed = true;
                    }
                }
            });
            changed |= ui
                .add(TextEdit::multiline(&mut self.src).font(mono(16.0)).desired_rows(6).desired_width(f32::INFINITY))
                .changed();
            changed
        });
        if changed {
            self.result = lex(&self.src);
            self.stepper.reset();
        }
        self.stepping(ui);

        ui.add_space(12.0);
        ui.label(RichText::new("小测验").size(20.0).strong());
        quiz(
            ui,
            "ch1-scan-1",
            "`x<=y` 会被切成几个记号（不算\"输入结束\"）？",
            &["3 个", "4 个", "5 个"],
            0,
            "`x`、`<=`、`y`。最长匹配让 `<` 和 `=` 组成一个记号。",
        );
        quiz(
            ui,
            "ch1-scan-2",
            "为什么扫描器要先把整个单词读完，再去查关键字表？",
            &["这样运行更快", "否则 `iffy` 会被错切成 `if` 和 `fy`", "因为关键字都比较短"],
            1,
            "如果一边读一边匹配关键字，读到 `if` 时就会过早地下结论。先读完整个单词，再整体判断，才符合最长匹配。",
        );
        quiz(
            ui,
            "ch1-scan-3",
            "扫描器读到 `/` 时还不能确定它是什么，它应该？",
            &["回头看前一个字符", "偷看下一个字符", "一直读到行尾"],
            1,
            "下一个字符是 `/` 就是行注释，是 `*` 就是块注释，否则是除号。这就是\"向前看一个字符\"。",
        );

        ui.add_space(12.0);
        callout(ui, CalloutKind::Deeper, "手写的扫描器其实就是一个 DFA", |ui| {
            ui.label(
                "回头看伪代码：\"看当前字符决定走哪个分支\"就是 DFA 的状态转移，\"读完整个单词\"的循环就是 DFA 的自环，\
                 \"偷看下一个字符\"就是在两个状态之间选择。手写扫描器只是把 DFA 直接写成了 if/else 和循环。\
                 1.6 课会看到，也可以反过来：先写正则，再自动生成 DFA。",
            );
            ui.label(
                "关键字的处理有两种做法：放进 DFA 里（lex/flex 的做法，靠规则优先级），或者像这里一样先按名字读完再查表。\
                 后者让 DFA 小得多，是手写扫描器的常见选择。",
            );
            ui.label(
                "mini-lang 允许用中文做名字，靠的是 Unicode 的\"字母\"分类。Rust、Python、Java 等语言也允许，\
                 具体规则由 Unicode 标准的 XID_Start / XID_Continue 属性规定。",
            );
        });
        ui.add_space(24.0);
    }

    fn stepping(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        let r = &self.result;
        self.stepper.ui(ui, r.steps.len());
        let k = self.stepper.pos;
        let current = k.checked_sub(1).map(|i| r.steps[i]);

        // 源码：已产出的记号按种类着色，注释变灰，当前这一步高亮。
        let mut colors: Vec<(Span, Color32)> = Vec::new();
        let mut marks: Vec<Mark> = Vec::new();
        for (i, s) in r.steps[..k].iter().enumerate() {
            match (s.token, s.kind) {
                (Some(t), _) => colors.push((t.span, class_color(&p, t.kind.class()))),
                (None, StepKind::LineComment | StepKind::BlockComment) => colors.push((s.span, p.muted)),
                (None, StepKind::Error) => marks.push(Mark { span: s.span, bg: p.error_bg }),
                _ => {}
            }
            if i + 1 == k && s.kind != StepKind::Whitespace {
                marks.push(Mark { span: s.span, bg: p.focus_bg });
            }
        }
        if k == 0 {
            marks.push(Mark { span: Span::empty_at(0), bg: p.focus_bg });
        } else if let Some(s) = current
            && s.kind == StepKind::Whitespace
        {
            marks.push(Mark { span: s.span, bg: p.done_bg });
        }

        let source = |ui: &mut Ui| {
            card(ui, |ui| source_view(ui, &self.src, &colors, &marks, 17.0));
        };
        let pseudo = |ui: &mut Ui| {
            ui.label(RichText::new("扫描器的伪代码（高亮的是这一步走的分支）").strong());
            card(ui, |ui| {
                // 左对齐布局，保留每行开头的缩进（见 source_view 里的说明）
                ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                    let active = current.map(|s| pseudo_line(s.kind));
                    for (i, line) in PSEUDO.iter().enumerate() {
                        let mut t = RichText::new(*line).font(mono(14.0));
                        if active == Some(i) {
                            t = t.background_color(p.focus_bg);
                        }
                        ui.label(t);
                    }
                });
            });
        };
        let tokens = |ui: &mut Ui| {
            ui.label(RichText::new("产出的记号（鼠标指向可以看到位置）").strong());
            card(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    let mut any = false;
                    for s in &r.steps[..k] {
                        let Some(t) = s.token else { continue };
                        any = true;
                        let lc = line_col(&self.src, t.span.start);
                        let color = class_color(&p, t.kind.class());
                        let is_current = current.is_some_and(|c| c.token == Some(t));
                        let text = match t.kind {
                            TokenKind::Eof => "输入结束".to_owned(),
                            _ => format!("{} {}", t.kind.name(), t.span.text(&self.src)),
                        };
                        chip(
                            ui,
                            RichText::new(text).font(mono(14.0)).color(color),
                            if is_current { p.focus_bg } else { p.node_fill },
                            egui::Stroke::new(if is_current { 2.0 } else { 1.0 }, color),
                        )
                        .on_hover_text(format!("第 {} 行第 {} 列", lc.line, lc.col));
                    }
                    if !any {
                        ui.label(RichText::new("（还没有）").color(p.muted));
                    }
                });
            });
        };
        // 记号列表放在整行宽度里：egui 的分栏里，自动换行的一排小卡片会撑破栏宽。
        if ui.available_width() > 860.0 {
            ui.columns(2, |cols| {
                source(&mut cols[0]);
                pseudo(&mut cols[1]);
            });
        } else {
            source(ui);
            pseudo(ui);
        }
        tokens(ui);

        let explain = match current {
            None => "扫描器站在第一个字符前面。".to_owned(),
            Some(s) => {
                let text = s.span.text(&self.src);
                let first = text.chars().next().unwrap_or(' ');
                match s.kind {
                    StepKind::Whitespace => {
                        "遇到空白（空格、换行或制表符），一直跳过。空白只用来分隔记号，不产生记号。".to_owned()
                    }
                    StepKind::LineComment => {
                        "看到 `/`，偷看下一个也是 `/`：这是行注释，跳到行尾。注释对程序没有意义，直接丢掉。".to_owned()
                    }
                    StepKind::BlockComment => "看到 `/*`：这是块注释，一直跳到 `*/`。".to_owned(),
                    StepKind::Word { keyword: true } => format!(
                        "看到字母 `{first}`：一直读字母、数字或下划线，读到 `{text}`。查关键字表——`{text}` 在表里，是关键字。"
                    ),
                    StepKind::Word { keyword: false } => format!(
                        "看到 `{first}`：一直读字母、数字或下划线，读到 `{text}`。查关键字表——不在表里，是一个名字（标识符）。"
                    ),
                    StepKind::Number => format!("看到数字：一直读数字，读到 `{text}`，得到整数 {text}。"),
                    StepKind::Symbol { peeked: false, .. } => format!("`{text}` 自己就是一个完整的记号，直接产出。"),
                    StepKind::Symbol { lookahead, two_char: true, .. } => format!(
                        "看到 `{first}`，它可能是单独的符号，也可能是双字符运算符的开头。偷看下一个字符是 `{}`，于是两个一起组成 `{text}`。",
                        lookahead.unwrap_or(' ')
                    ),
                    StepKind::Symbol { lookahead, two_char: false, .. } => {
                        let next = match lookahead {
                            Some(c) if c.is_whitespace() => "空白".to_owned(),
                            Some(c) => format!("`{c}`"),
                            None => "文件末尾".to_owned(),
                        };
                        format!(
                            "看到 `{text}`，偷看下一个字符是 {next}，组不成双字符运算符，所以它单独是一个 `{text}`。"
                        )
                    }
                    StepKind::Error => {
                        "这里出错了，报告错误后跳过，继续往后扫描——这样一次就能发现多个错误。".to_owned()
                    }
                    StepKind::Eof => "所有字符都读完了，补上\"输入结束\"记号。".to_owned(),
                }
            }
        };
        prose_sized(ui, &explain, 17.0);

        // 出错的步骤：显示对应的诊断信息
        if let Some(s) = current
            && s.kind == StepKind::Error
        {
            let idx = r.steps[..k].iter().filter(|st| st.kind == StepKind::Error).count() - 1;
            if let Some(d) = r.errors.get(idx) {
                diagnostic_view(ui, &self.src, d);
            }
        }
        if k == r.steps.len() {
            let n = r.tokens.iter().filter(|t| t.kind != TokenKind::Eof).count();
            let summary = if r.errors.is_empty() {
                RichText::new(format!("扫描完成：共 {n} 个记号，没有错误。")).color(p.ok)
            } else {
                RichText::new(format!("扫描完成：共 {n} 个记号，{} 个错误。", r.errors.len())).color(p.error)
            };
            ui.label(summary.strong());
        }
    }
}
