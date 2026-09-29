//! 2.3 递归下降。

use bsc_core::Span;
use bsc_minilang::lexer::{Token, lex};
use bsc_minilang::parser::{Event, ParseOutput, parse_program};
use eframe::egui::{self, RichText, TextEdit, Ui};

use super::replay::{call_stack, partial_tree, replay, source};
use crate::theme::{Palette, mono};
use crate::widgets::{CalloutKind, Mark, Stepper, callout, card, diagnostic_view, prose, prose_sized, quiz};

const PRESETS: &[(&str, &str)] = &[
    ("简单函数", "fn double(x: int) -> int {\n    let y = x * 2;\n    return y;\n}"),
    (
        "分支与循环",
        "fn sum(n: int) -> int {\n    let mut s = 0;\n    let mut i = 1;\n    while i <= n {\n        s = s + i;\n        i = i + 1;\n    }\n    if s > 100 { return 100; } else { return s; }\n}",
    ),
    ("有错误的程序", "fn main() {\n    let x = ;\n    let y = 1\n    x = y * ;\n    return y;\n}"),
];

/// mini-lang 语句部分的文法，每条规则旁边是实现它的函数。
const GRAMMAR: &[(&str, &str)] = &[
    ("程序     → 函数*", "parse_program"),
    ("函数     → fn 名字 ( 参数,* ) [-> 类型] 块", "parse_function"),
    ("块       → { 语句* }", "parse_block"),
    ("语句     → let语句 | if语句 | while语句 | return语句 | 块 | 表达式语句", "parse_statement"),
    ("let语句  → let [mut] 名字 [: 类型] = 表达式 ;", "parse_let"),
    ("if语句   → if 表达式 块 [else (块 | if语句)]", "parse_if"),
    ("while语句 → while 表达式 块", "parse_while"),
    ("表达式   → （交给 Pratt 分析法，见 2.4 课）", "parse_expr"),
];

pub struct Lesson {
    src: String,
    tokens: Vec<Token>,
    out: ParseOutput,
    stepper: Stepper,
}

impl Default for Lesson {
    fn default() -> Self {
        let mut l = Self {
            src: String::new(),
            tokens: vec![],
            out: parse_program("", &lex("").tokens),
            stepper: Stepper::default(),
        };
        l.set(PRESETS[0].1);
        l
    }
}

impl Lesson {
    fn set(&mut self, src: &str) {
        self.src = src.to_owned();
        self.reparse();
    }

    fn reparse(&mut self) {
        self.tokens = lex(&self.src).tokens;
        self.out = parse_program(&self.src, &self.tokens);
        self.stepper.reset();
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        ui.heading("2.3  递归下降");
        ui.label(RichText::new("入门 · 约 25 分钟").color(p.muted));

        ui.add_space(8.0);
        ui.label(RichText::new("一句话").size(20.0).strong());
        ui.label(
            "把文法的每条规则写成一个函数，规则里出现别的非终结符，就调用对应的函数——这就是递归下降。\
             它是最直观、也是真实编译器用得最多的语法分析方法：GCC、Clang、rustc、Go 的编译器都是手写的递归下降。",
        );
        callout(ui, CalloutKind::Analogy, "层层分派的任务", |ui| {
            ui.label(
                "老板说\"解析这个程序\"，程序由函数组成，于是把每个函数分派给\"函数组\"；函数组发现里面有代码块，\
                 分派给\"块组\"；块组看到一条 while 语句，分派给\"循环组\"……每个组只关心自己那一层，完成后把结果（一棵子树）交回上级。\
                 此刻谁在干活、谁在等下属交差，就是\"调用栈\"。",
            );
        });

        ui.add_space(8.0);
        ui.label(RichText::new("文法规则 ↔ 函数").size(20.0).strong());
        card(ui, |ui| {
            egui::Grid::new("rd_grammar").num_columns(2).striped(true).spacing([24.0, 6.0]).show(ui, |ui| {
                for (rule, func) in GRAMMAR {
                    ui.label(RichText::new(*rule).font(mono(14.0)));
                    ui.label(RichText::new(*func).font(mono(14.0)).color(p.accent));
                    ui.end_row();
                }
            });
        });
        prose(
            ui,
            "`parse_statement` 只看第一个记号就能决定是哪种语句：`let` 开头就调用 `parse_let`，`while` 开头就调用 `parse_while`……\
             这种\"看一个记号就能选对分支\"的特性，正是 2.5 课要讲的 LL(1)。",
        );

        ui.add_space(8.0);
        ui.label(RichText::new("单步观察").size(20.0).strong());
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
                .add(TextEdit::multiline(&mut self.src).font(mono(16.0)).desired_rows(5).desired_width(f32::INFINITY))
                .changed();
            changed
        });
        if changed {
            self.reparse();
        }
        self.stepping(ui);

        ui.add_space(8.0);
        callout(ui, CalloutKind::KeyPoint, "左递归会让递归下降死循环", |ui| {
            prose(
                ui,
                "如果文法写成 `表达式 → 表达式 + 项`，对应的函数第一件事就是调用自己：`parse_表达式()` 调用 `parse_表达式()` 调用……\
                 一个记号都没读，就无限递归、栈溢出了。所以递归下降处理\"左结合的一串运算\"时，改用循环：\
                 先解析一个项，然后\"只要下一个是 + 就再吃一个项\"。第 0 课的计算器就是这么写的；下一课的 Pratt 分析法把这个循环做得更通用。",
            );
        });
        callout(ui, CalloutKind::KeyPoint, "出错了怎么办：错误恢复", |ui| {
            prose(
                ui,
                "好的编译器不会遇到第一个错误就停下。选\"有错误的程序\"例子单步看：这里用了两种恢复策略。\
                 **恐慌模式**：报告错误后跳过记号，直到遇到 `;`、`}` 或下一条语句开头的关键字，再从那里继续。\
                 **插入式**：行末缺分号时，报错后假装分号在那里，直接继续——否则恐慌模式会把下一行也跳过，漏掉那里的错误。",
            );
        });

        ui.add_space(12.0);
        ui.label(RichText::new("小测验").size(20.0).strong());
        quiz(
            ui,
            "ch2-rd-1",
            "递归下降分析器里，`parse_block` 遇到 `while` 开头的语句会？",
            &["直接报错", "调用 parse_statement，它再调用 parse_while", "跳过这条语句"],
            1,
            "块由若干语句组成，所以 parse_block 反复调用 parse_statement；parse_statement 看到 while 就交给 parse_while。",
        );
        quiz(
            ui,
            "ch2-rd-2",
            "调用栈最深的时候，说明分析器正处在？",
            &["嵌套最深的语法结构里", "程序的末尾", "出错的地方"],
            0,
            "调用栈和语法树的\"当前路径\"一一对应：栈越深，正在分析的结构嵌套得越深。",
        );

        ui.add_space(12.0);
        callout(ui, CalloutKind::Deeper, "预测分析与回溯", |ui| {
            ui.label(
                "像这样只看下一个记号就能选对分支、从不回头的递归下降，叫\"预测分析\"。如果文法做不到这一点，\
                 可以让分析器先试一个分支，失败了再退回来试另一个——\"回溯\"。回溯写起来灵活，但可能慢到指数级。\
                 PEG（解析表达式文法）和它的\"packrat\"实现用记忆化把回溯的代价降到线性。",
            );
        });
        ui.add_space(24.0);
    }

    fn stepping(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        let out = &self.out;
        self.stepper.ui(ui, out.events.len());
        let k = self.stepper.pos;
        let r = replay(out, k);

        let mut extra = Vec::new();
        if let Some(Event::Recover { from, to }) = r.last
            && to > from
        {
            let span = Span::new(self.tokens[from].span.start, self.tokens[to - 1].span.end);
            extra.push(Mark { span, bg: p.warn_bg });
        }
        source(ui, &self.src, &self.tokens, &r, &extra);

        let explain = match &r.last {
            None => "从 parse_program 开始。".to_owned(),
            Some(Event::Enter(rule)) => format!("调用 `{}`：{}。", rule.name(), rule.meaning()),
            Some(Event::Exit(rule)) => format!("`{}` 完成，返回上一层。", rule.name()),
            Some(Event::Consume(i)) => format!("读入记号 `{}`。", self.tokens[*i].span.text(&self.src)),
            Some(Event::Build(n)) => format!("建好一个语法树节点【{}】。", out.ast.label(*n)),
            Some(Event::Pratt { op: Some(i), bind: true, .. }) => {
                format!(
                    "表达式里遇到运算符 `{}`，把它和右边的操作数组合起来（细节见 2.4 课）。",
                    self.tokens[*i].span.text(&self.src)
                )
            }
            Some(Event::Pratt { .. }) => "下一个记号不能继续组成当前表达式，表达式到此结束。".to_owned(),
            Some(Event::Error(_)) => "发现语法错误！".to_owned(),
            Some(Event::Recover { from, to }) => {
                if to > from {
                    format!("错误恢复：跳过 {} 个记号（黄色部分），从下一条语句继续。", to - from)
                } else {
                    "错误恢复：已经在下一条语句的开头了，直接继续。".to_owned()
                }
            }
        };
        prose_sized(ui, &explain, 17.0);
        if let Some(Event::Error(i)) = r.last {
            diagnostic_view(ui, &self.src, &out.errors[i]);
        }

        if ui.available_width() > 820.0 {
            ui.columns(2, |c| {
                call_stack(&mut c[0], &r);
                c[1].label(RichText::new("已经建好的语法树节点").strong());
                partial_tree(&mut c[1], "rd_tree", &out.ast, &r);
            });
        } else {
            call_stack(ui, &r);
            ui.label(RichText::new("已经建好的语法树节点").strong());
            partial_tree(ui, "rd_tree", &out.ast, &r);
        }
        if k == out.events.len() {
            let text = if out.errors.is_empty() {
                RichText::new("分析完成，没有语法错误。").color(p.ok)
            } else {
                RichText::new(format!("分析完成，共发现 {} 个语法错误（一次全部报告出来）。", out.errors.len()))
                    .color(p.error)
            };
            ui.label(text.strong());
        }
    }
}
