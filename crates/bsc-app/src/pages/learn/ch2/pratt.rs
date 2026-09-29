//! 2.4 Pratt 分析法。

use bsc_minilang::lexer::{Token, lex};
use bsc_minilang::parser::{Event, OPERATOR_TABLE, ParseOutput, Rule, parse_expression};
use eframe::egui::{self, RichText, TextEdit, Ui};

use super::replay::{call_stack, partial_tree, replay, source};
use crate::theme::{Palette, mono};
use crate::widgets::{CalloutKind, Mark, Stepper, callout, card, diagnostic_view, prose, prose_sized, quiz};

const PRESETS: &[&str] =
    &["1 + 2 * 3", "8 - 3 - 2", "a = b = 1", "-x * y", "a < b + 1 && ok", "f(1, 2) * 3", "1 + * 2"];

pub struct Lesson {
    src: String,
    tokens: Vec<Token>,
    out: ParseOutput,
    stepper: Stepper,
}

impl Default for Lesson {
    fn default() -> Self {
        let src = PRESETS[0].to_owned();
        let tokens = lex(&src).tokens;
        let out = parse_expression(&src, &tokens);
        Self { src, tokens, out, stepper: Stepper::default() }
    }
}

impl Lesson {
    pub fn ui(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        ui.heading("2.4  Pratt 分析法");
        ui.label(RichText::new("入门 · 约 25 分钟 · 需要先学 2.2、2.3").color(p.muted));

        ui.add_space(8.0);
        ui.label(RichText::new("一句话").size(20.0).strong());
        prose(
            ui,
            "2.2 课用\"分层文法\"处理优先级：每一级优先级一个非终结符。mini-lang 有赋值、或、且、比较、加减、乘除、负号七级，\
             写成递归下降就是七个几乎一模一样的函数。Pratt 分析法（1973 年）只用**一个函数、一张表**就解决了：\
             表里给每个运算符一对数字——\"绑定力\"。",
        );
        callout(ui, CalloutKind::Analogy, "拔河", |ui| {
            prose(
                ui,
                "看 `1 + 2 * 3` 里的 `2`：它左边是 `+`，右边是 `*`，两个运算符都想把它拉过去当自己的操作数。\
                 谁的力气大归谁：`*` 对左边的拉力是 13，`+` 对右边的拉力只有 12，所以 `2` 归 `*`，先算 `2 * 3`。\
                 每个运算符有两只手：左手的拉力叫\"左绑定力\"，右手的叫\"右绑定力\"。",
            );
        });

        ui.add_space(8.0);
        ui.label(RichText::new("绑定力表").size(20.0).strong());
        card(ui, |ui| {
            egui::Grid::new("bp_table").num_columns(4).striped(true).spacing([24.0, 6.0]).show(ui, |ui| {
                for h in ["运算符", "左绑定力", "右绑定力", "说明"] {
                    ui.label(RichText::new(h).strong());
                }
                ui.end_row();
                for (op, l, r, note) in OPERATOR_TABLE {
                    ui.label(RichText::new(*op).font(mono(15.0)).color(p.operator));
                    ui.label(RichText::new(if *l == 0 { "—".to_owned() } else { l.to_string() }).font(mono(15.0)));
                    ui.label(RichText::new(r.to_string()).font(mono(15.0)));
                    ui.label(*note);
                    ui.end_row();
                }
            });
        });
        prose(
            ui,
            "数字越大绑得越紧（优先级越高）。**左 < 右** 是左结合：`8 - 3 - 2` 里中间的 `3`，左边 `-` 的右手拉力 12 大于右边 `-` 的左手拉力 11，\
             于是 `3` 归左边，得到 `(8 - 3) - 2`。赋值是 **左 > 右**：右结合，`a = b = 1` 是 `a = (b = 1)`。",
        );

        ui.add_space(8.0);
        ui.label(RichText::new("算法").size(20.0).strong());
        card(ui, |ui| {
            for line in [
                "parse_expr(最小绑定力 m):",
                "    左 = 解析一个最小单位（数字、变量、括号、负号开头的……）",
                "    循环：",
                "        op = 下一个记号",
                "        如果 op 不是运算符，或者 op 的左绑定力 < m：返回 左",
                "        吃掉 op",
                "        右 = parse_expr(op 的右绑定力)",
                "        左 = (左 op 右)",
            ] {
                ui.label(RichText::new(line).font(mono(14.0)));
            }
        });
        prose(
            ui,
            "\"最小绑定力 m\"的意思是：只有拉力至少为 m 的运算符，才能把当前的\"左\"拉走；拉力不够，就把\"左\"原样交还给调用者。",
        );

        ui.add_space(8.0);
        ui.label(RichText::new("单步观察").size(20.0).strong());
        let changed = card(ui, |ui| {
            let mut changed = false;
            ui.horizontal(|ui| {
                ui.label("表达式：");
                changed |=
                    ui.add(TextEdit::singleline(&mut self.src).font(mono(20.0)).desired_width(f32::INFINITY)).changed();
            });
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("例子：").color(p.muted));
                for s in PRESETS {
                    if ui.button(RichText::new(*s).font(mono(14.0))).clicked() {
                        self.src = (*s).to_owned();
                        changed = true;
                    }
                }
            });
            changed
        });
        if changed {
            self.tokens = lex(&self.src).tokens;
            self.out = parse_expression(&self.src, &self.tokens);
            self.stepper.reset();
        }
        self.stepping(ui);

        ui.add_space(12.0);
        ui.label(RichText::new("小测验").size(20.0).strong());
        quiz(
            ui,
            "ch2-pratt-1",
            "想让新运算符 `^`（乘方）比乘除优先、而且右结合（`2^3^2` = `2^(3^2)`），绑定力该设成？",
            &["(15, 16)", "(16, 15)", "(12, 11)"],
            1,
            "比乘除（13, 14）大才能更优先；左 > 右才是右结合。注意前缀负号的右绑定力是 15，所以 `-2^2` 会是 `(-2)^2`——\
             如果想要数学上的 `-(2^2)`，需要把负号的绑定力调得比乘方低。",
        );
        quiz(
            ui,
            "ch2-pratt-2",
            "为什么 `-x * y` 被分析成 `(-x) * y`？",
            &["负号写在最前面", "负号的右绑定力 15 比 * 的左绑定力 13 大，x 被负号抢走了", "乘法总是最后算"],
            1,
            "x 夹在负号和 * 之间，负号的右手拉力 15 > * 的左手拉力 13，所以 x 先和负号组合。",
        );

        ui.add_space(12.0);
        callout(ui, CalloutKind::Deeper, "为什么大家都爱 Pratt", |ui| {
            prose(
                ui,
                "Vaughan Pratt 在 1973 年的论文《自顶向下的运算符优先分析》中提出了这个方法。它本质上就是递归下降，\
                 只是把\"优先级的层次\"从代码结构里抽出来，放进了一张表：加一个新运算符只需要在表里加一行。\
                 后缀运算符（`x?`、函数调用 `f(x)`、数组下标 `a[i]`）和三元运算符 `a ? b : c` 也能用同样的框架处理。\
                 rust-analyzer、许多 JavaScript 引擎和教科书《Crafting Interpreters》都用它来解析表达式。",
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
        let text = |i: usize| self.tokens[i].span.text(&self.src).to_owned();

        let mut extra = Vec::new();
        if let Some(Event::Pratt { op: Some(i), .. }) = r.last {
            extra.push(Mark { span: self.tokens[i].span, bg: p.focus_bg });
        }
        source(ui, &self.src, &self.tokens, &r, &extra);

        let explain = match &r.last {
            None => "从 parse_expr(0) 开始：最小绑定力 0，任何运算符都能参与。".to_owned(),
            Some(Event::Enter(Rule::Expr(bp))) => {
                format!("调用 `parse_expr({bp})`：接下来只接受左绑定力至少为 {bp} 的运算符。")
            }
            Some(Event::Exit(Rule::Expr(bp))) => format!("`parse_expr({bp})` 返回它组合好的子树。"),
            Some(Event::Enter(Rule::Primary)) => "调用 `parse_primary` 解析一个最小单位。".to_owned(),
            Some(Event::Exit(_)) => "返回。".to_owned(),
            Some(Event::Enter(rule)) => format!("调用 `{}`。", rule.name()),
            Some(Event::Consume(i)) => format!("吃掉 `{}`。", text(*i)),
            Some(Event::Build(n)) => format!("建好节点【{}】。", out.ast.label(*n)),
            Some(Event::Pratt { min_bp, op: None, .. }) => {
                format!("下一个记号不是二元运算符，当前这层（最小绑定力 {min_bp}）的\"左\"就是结果，返回。")
            }
            Some(Event::Pratt { min_bp, op: Some(i), lbp, rbp, bind: true }) => format!(
                "看到 `{}`：左绑定力 {lbp} ≥ 最小绑定力 {min_bp}，它拉得动当前的\"左\"。吃掉它，用右绑定力 {rbp} 去解析右边。",
                text(*i)
            ),
            Some(Event::Pratt { min_bp, op: Some(i), lbp, bind: false, .. }) => format!(
                "看到 `{}`：左绑定力 {lbp} < 最小绑定力 {min_bp}，拉不动当前的\"左\"——把它交还给上一层，由上一层的运算符来处理 `{}`。",
                text(*i),
                text(*i)
            ),
            Some(Event::Error(_)) => "发现语法错误！".to_owned(),
            Some(Event::Recover { .. }) => "跳过出错的部分。".to_owned(),
        };
        prose_sized(ui, &explain, 17.0);
        if let Some(Event::Error(i)) = r.last {
            diagnostic_view(ui, &self.src, &out.errors[i]);
        }

        if ui.available_width() > 820.0 {
            ui.columns(2, |c| {
                call_stack(&mut c[0], &r);
                c[1].label(RichText::new("已经组合好的子树").strong());
                partial_tree(&mut c[1], "pratt_tree", &out.ast, &r);
            });
        } else {
            call_stack(ui, &r);
            partial_tree(ui, "pratt_tree", &out.ast, &r);
        }
        if k == out.events.len() && out.errors.is_empty() {
            ui.label(RichText::new(format!("结果：{}", out.ast.sexpr(out.root))).font(mono(15.0)).color(p.ok));
        }
    }
}
