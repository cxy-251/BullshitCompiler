//! 1.6 从正则到词法分析器。

use bsc_automata::scanner::{BuildError, Lexeme, Lexer, Rule, ScanEvent};
use bsc_core::Span;
use eframe::egui::{self, RichText, TextEdit, Ui};

use crate::automata_view::min_graph;
use crate::theme::{Palette, mono};
use crate::widgets::{
    CalloutKind, GEdgeStyle, GNodeStyle, Mark, Stepper, callout, card, diagnostic_view, graph_view, prose, prose_sized,
    quiz, source_view,
};

struct RuleRow {
    name: String,
    pattern: String,
    skip: bool,
}

pub struct Lesson {
    rules: Vec<RuleRow>,
    input: String,
    lexer: Result<Lexer, Box<BuildError>>,
    stepper: Stepper,
}

impl Default for Lesson {
    fn default() -> Self {
        let rules = [
            ("IF", "if", false),
            ("IDENT", "[a-z_][a-z0-9_]*", false),
            ("NUMBER", "[0-9]+", false),
            ("LE", "<=", false),
            ("LT", "<", false),
            ("ASSIGN", "=", false),
            ("空白", "\\s+", true),
        ]
        .into_iter()
        .map(|(n, p, s)| RuleRow { name: n.to_owned(), pattern: p.to_owned(), skip: s })
        .collect();
        let mut l = Self {
            rules,
            input: "if iffy <= 10 x = 3".to_owned(),
            lexer: Lexer::build(vec![]),
            stepper: Stepper::default(),
        };
        l.rebuild();
        l
    }
}

impl Lesson {
    fn rebuild(&mut self) {
        let rules = self
            .rules
            .iter()
            .map(|r| if r.skip { Rule::skip(&r.name, &r.pattern) } else { Rule::new(&r.name, &r.pattern) })
            .collect();
        self.lexer = Lexer::build(rules);
        self.stepper.reset();
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        ui.heading("1.6  从正则到词法分析器");
        ui.label(RichText::new("入门 · 约 25 分钟 · 本章的收官").color(p.muted));

        ui.add_space(8.0);
        ui.label(RichText::new("一句话").size(20.0).strong());
        ui.label(
            "把这一章串起来：给每种记号写一条正则，程序自动把它们合成一个 DFA，再用这个 DFA 扫描源码——\
             一个词法分析器就\"生成\"出来了。经典工具 lex / flex 做的正是这件事。",
        );
        card(ui, |ui| {
            for line in [
                "每条规则的正则 → 各自的 NFA（Thompson 构造，1.3）",
                "新建一个起点，用 ε 边连向所有 NFA → 一个大 NFA，接受状态记着\"是哪条规则\"",
                "子集构造（1.4）→ DFA → 最小化（1.5，不同规则的接受状态不会被合并）",
                "扫描：用 DFA 一个记号一个记号地切",
            ] {
                ui.label(RichText::new(line).font(mono(14.0)));
            }
        });

        ui.add_space(8.0);
        ui.label(RichText::new("两条消除歧义的规则").size(20.0).strong());
        callout(ui, CalloutKind::KeyPoint, "最长匹配", |ui| {
            prose(
                ui,
                "`iffy` 可以切成 `if` + `fy`，也可以是一个完整的标识符。规定：**尽量往后读**。\
                 做法是一直沿着 DFA 走，每经过一个接受状态就记下\"读到这里可以结束\"；走不动了，就退回最近一次记下的位置。",
            );
        });
        callout(ui, CalloutKind::KeyPoint, "规则优先级", |ui| {
            prose(
                ui,
                "`if` 既能被 IF 规则匹配，也能被 IDENT 规则匹配，而且一样长。规定：**写在前面的规则优先**。\
                 所以关键字的规则要写在标识符前面——试试把 IF 挪到 IDENT 后面，看看 `if` 会变成什么。",
            );
        });

        ui.add_space(8.0);
        ui.label(RichText::new("亲手生成一个词法分析器").size(20.0).strong());
        if self.rules_editor(ui) {
            self.rebuild();
        }

        match &self.lexer {
            Err(e) => {
                let r = &self.rules[e.rule];
                ui.label(RichText::new(format!("第 {} 条规则 {} 有问题：", e.rule + 1, r.name)).color(p.error));
                diagnostic_view(ui, &r.pattern, &e.diagnostic);
            }
            Ok(_) => self.scanning(ui),
        }

        ui.add_space(12.0);
        ui.label(RichText::new("小测验").size(20.0).strong());
        quiz(
            ui,
            "ch1-lex-1",
            "规则依次是 IF `if`、IDENT `[a-z]+`。输入 `ifx` 会被切成？",
            &["IF(if) IDENT(x)", "IDENT(ifx)", "报错"],
            1,
            "最长匹配优先于规则优先级：IDENT 能匹配更长的 `ifx`，所以整体是一个标识符。优先级只在一样长时才起作用。",
        );
        quiz(
            ui,
            "ch1-lex-2",
            "如果把 IDENT 写在 IF 前面，输入 `if` 会被识别成？",
            &["IF", "IDENT", "两个都是"],
            1,
            "两条规则都能匹配 `if`，一样长，于是写在前面的 IDENT 胜出——关键字就失效了。这是写词法规则时最常见的错误之一。",
        );
        quiz(
            ui,
            "ch1-lex-3",
            "只有 `<` 和 `<=` 两条规则，输入 `<5` 时，扫描器读到 `5` 发现无路可走，接下来？",
            &["报错", "退回到最近一次接受的位置，产出 `<`", "跳过 `5`"],
            1,
            "读完 `<` 时经过了接受状态，已经记下\"读到这里可以是 LT\"。读 `5` 失败后退回那里，产出 LT，再从 `5` 开始下一个记号。",
        );

        ui.add_space(12.0);
        callout(ui, CalloutKind::Deeper, "手写还是生成？", |ui| {
            ui.label(
                "lex/flex、Rust 的 logos 等工具都是这样生成词法分析器的。不过很多真实的编译器（GCC、Clang、rustc、Go）\
                 选择手写词法分析器：记号种类不多，手写更容易给出好的错误信息，也更容易处理字符串插值、嵌套注释这类\
                 \"不那么正则\"的东西。下一章要写的 mini-lang，词法分析器也是手写的（见 1.1）——但它内部走的仍然是同样的思路：\
                 看当前字符决定状态，最长匹配，关键字表优先。",
            );
        });
        ui.add_space(24.0);
    }

    /// 规则编辑表格。返回规则是否改变。
    fn rules_editor(&mut self, ui: &mut Ui) -> bool {
        let p = Palette::of(ui);
        let mut changed = false;
        let mut remove = None;
        let mut move_up = None;
        card(ui, |ui| {
            ui.label(RichText::new("规则（越靠上优先级越高）").strong());
            egui::Grid::new("lex_rules").num_columns(5).spacing([10.0, 6.0]).show(ui, |ui| {
                for h in ["名称", "正则", "跳过", "", ""] {
                    ui.label(RichText::new(h).small().color(p.muted));
                }
                ui.end_row();
                for (i, r) in self.rules.iter_mut().enumerate() {
                    // Grid 的列宽取决于上一帧的内容，输入框要用固定尺寸，否则永远撑不开。
                    changed |= ui.add_sized([96.0, 24.0], TextEdit::singleline(&mut r.name)).changed();
                    changed |=
                        ui.add_sized([240.0, 24.0], TextEdit::singleline(&mut r.pattern).font(mono(15.0))).changed();
                    changed |= ui.checkbox(&mut r.skip, "").changed();
                    if ui.add_enabled(i > 0, egui::Button::new("↑").small()).on_hover_text("提高优先级").clicked()
                    {
                        move_up = Some(i);
                    }
                    if ui.small_button("删除").clicked() {
                        remove = Some(i);
                    }
                    ui.end_row();
                }
            });
            if ui.button("添加规则").clicked() {
                self.rules.push(RuleRow {
                    name: format!("R{}", self.rules.len() + 1),
                    pattern: "x".to_owned(),
                    skip: false,
                });
                changed = true;
            }
            ui.label(
                RichText::new("\"跳过\"的规则匹配到的内容（空白、注释）会被直接丢掉，不产生记号。")
                    .small()
                    .color(p.muted),
            );
        });
        if let Some(i) = move_up {
            self.rules.swap(i, i - 1);
            changed = true;
        }
        if let Some(i) = remove {
            self.rules.remove(i);
            changed = true;
        }
        changed
    }

    fn scanning(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        let Ok(lexer) = &self.lexer else { return };
        ui.label(
            RichText::new(format!(
                "生成结果：合并后的 NFA 有 {} 个状态 → 子集构造得到 {} 个 DFA 状态 → 最小化后 {} 个。",
                lexer.nfa.nfa.num_states,
                lexer.dfa.states.len(),
                lexer.min.dfa.states.len()
            ))
            .color(p.ok),
        );

        let changed = card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("要扫描的源码：");
                ui.add(TextEdit::singleline(&mut self.input).font(mono(20.0)).desired_width(f32::INFINITY)).changed()
            })
            .inner
        });
        if changed {
            self.stepper.reset();
        }

        let scan = lexer.scan(&self.input);
        self.stepper.ui(ui, scan.events.len());
        let k = self.stepper.pos;

        // 回放前 k 个事件
        let mut emitted: Vec<(Lexeme, bool)> = Vec::new();
        let mut begin = 0;
        let mut pos = 0;
        let mut state = lexer.min.dfa.start;
        let mut last_accept: Option<(usize, usize)> = None;
        for e in &scan.events[..k] {
            match *e {
                ScanEvent::Begin { pos: b } => {
                    begin = b;
                    pos = b;
                    state = lexer.min.dfa.start;
                    last_accept = None;
                }
                ScanEvent::Advance { pos: np, state: s, accept, .. } => {
                    pos = np;
                    state = s;
                    if let Some(r) = accept {
                        last_accept = Some((np, r));
                    }
                }
                ScanEvent::Emit { lexeme, skipped, .. } => emitted.push((lexeme, skipped)),
                ScanEvent::Stuck { .. } | ScanEvent::Error { .. } => {}
            }
        }
        let last = k.checked_sub(1).map(|i| &scan.events[i]);

        // 源码：已产出的记号按规则着色，正在尝试的部分高亮
        let mut marks: Vec<Mark> =
            emitted.iter().filter(|(_, s)| !s).map(|(l, _)| Mark { span: l.span, bg: p.category(l.rule) }).collect();
        if !matches!(last, Some(ScanEvent::Emit { .. })) && pos > begin {
            marks.push(Mark { span: Span::new(begin, pos), bg: p.focus_bg });
        }
        if let Some((end, _)) = last_accept
            && !matches!(last, Some(ScanEvent::Emit { .. }))
        {
            marks.push(Mark { span: Span::empty_at(end), bg: p.ok });
        }
        card(ui, |ui| {
            source_view(ui, &self.input, &[], &marks, 22.0);
            ui.label(
                RichText::new("黄色：正在尝试的部分；绿色竖线：最近一次可以结束记号的位置。").small().color(p.muted),
            );
        });

        let rule_name = |r: usize| lexer.rules[r].name.as_str();
        let explain = match last {
            None => "从第一个字符开始。".to_owned(),
            Some(ScanEvent::Begin { pos }) => format!("从位置 {pos} 开始识别一个新记号，DFA 回到起点 0。"),
            Some(ScanEvent::Advance { ch, state, accept, .. }) => match accept {
                Some(r) => format!(
                    "读 `{}` → 状态 {state}。这是接受状态（{}）：记下\"读到这里可以是一个 {}\"，但先别停，继续往后试——最长匹配。",
                    show(*ch),
                    rule_name(*r),
                    rule_name(*r)
                ),
                None => format!("读 `{}` → 状态 {state}。还不能结束，继续。", show(*ch)),
            },
            Some(ScanEvent::Stuck { ch: Some(c), state, .. }) => {
                format!("在状态 {state} 读 `{}`：无路可走，这个记号不能再长了。", show(*c))
            }
            Some(ScanEvent::Stuck { ch: None, .. }) => "输入读完了，这个记号不能再长了。".to_owned(),
            Some(ScanEvent::Emit { lexeme, skipped, backtrack }) => {
                let back = if *backtrack > 0 { format!("退回 {backtrack} 个字符，") } else { String::new() };
                let text = lexeme.span.text(&self.input);
                if *skipped {
                    format!(
                        "{back}回到最近一次接受的位置：`{}` 是 {}，按规则跳过。",
                        show_str(text),
                        rule_name(lexeme.rule)
                    )
                } else {
                    format!("{back}回到最近一次接受的位置，产出记号 {} `{}`。", rule_name(lexeme.rule), text)
                }
            }
            Some(ScanEvent::Error { .. }) => "从这个字符开始，一个接受状态都没经过——没有任何规则能匹配。".to_owned(),
        };
        prose_sized(ui, &explain, 17.0);

        ui.label(RichText::new("产出的记号").strong());
        ui.horizontal_wrapped(|ui| {
            let shown: Vec<_> = emitted.iter().filter(|(_, s)| !s).collect();
            if shown.is_empty() {
                ui.label(RichText::new("（还没有）").color(p.muted));
            }
            for (l, _) in shown {
                egui::Frame::new()
                    .fill(p.category(l.rule))
                    .corner_radius(egui::CornerRadius::same(5))
                    .inner_margin(egui::Margin::symmetric(8, 2))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(format!("{} {}", rule_name(l.rule), l.span.text(&self.input)))
                                .font(mono(15.0))
                                .color(p.text),
                        );
                    });
            }
        });
        if k == scan.events.len()
            && let Some(d) = &scan.error
        {
            diagnostic_view(ui, &self.input, d);
        }

        ui.label(RichText::new("生成的最小 DFA（接受状态下方写着对应的规则）").strong());
        let mut g = min_graph(&lexer.min);
        for (i, node) in g.nodes.iter_mut().enumerate() {
            node.note = lexer.min.dfa.states[i].accept.map(|r| rule_name(r).to_owned());
        }
        let in_token = !matches!(last, None | Some(ScanEvent::Emit { .. } | ScanEvent::Error { .. }));
        graph_view(
            ui,
            "lex_dfa",
            &g,
            |s| if in_token && s == state { GNodeStyle::focus(&p) } else { GNodeStyle::normal(&p) },
            |_| GEdgeStyle::normal(&p),
        );
    }
}

fn show(c: char) -> String {
    match c {
        ' ' => "空格".to_owned(),
        '\n' => "换行".to_owned(),
        '\t' => "制表符".to_owned(),
        c => c.to_string(),
    }
}

fn show_str(s: &str) -> String {
    if s.chars().all(|c| c == ' ') { format!("{} 个空格", s.len()) } else { s.escape_default().to_string() }
}
