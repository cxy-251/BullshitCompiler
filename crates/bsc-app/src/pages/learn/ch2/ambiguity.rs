//! 2.2 歧义与优先级。

use bsc_grammar::{Grammar, Tree, earley};
use eframe::egui::{RichText, Ui};

use crate::grammar_view::GrammarInput;
use crate::theme::{Palette, mono};
use crate::widgets::{CalloutKind, NodeStyle, callout, card, prose, quiz, scrollable_tree};

const PRESETS: &[(&str, &str, &str)] = &[
    ("有歧义的文法", "E -> E + E | E - E | E * E | ( E ) | num", "1 + 2 * 3"),
    ("分层：消除优先级歧义", "E -> E + T | E - T | T\nT -> T * F | F\nF -> ( E ) | num", "1 + 2 * 3"),
    ("结合性：左递归", "E -> E - T | T\nT -> num", "8 - 3 - 2"),
    ("结合性：右递归（错误）", "E -> T - E | T\nT -> num", "8 - 3 - 2"),
    ("有歧义：减法", "E -> E - E | num", "8 - 3 - 2"),
];

const MAX_TREES: usize = 6;

pub struct Lesson {
    input: GrammarInput,
}

impl Default for Lesson {
    fn default() -> Self {
        let (_, g, s) = PRESETS[0];
        Self { input: GrammarInput::new(g, s) }
    }
}

/// 按语法树求值：`E op E` 做运算，`( E )` 取中间，单个孩子直接传上来，数字取原文。
fn eval(g: &Grammar, input: &GrammarInput, t: &Tree) -> Option<i64> {
    if g.is_terminal(t.sym) {
        return input.token_text(t.start).parse().ok();
    }
    match &t.children[..] {
        [x] => eval(g, input, x),
        [l, op, r] if g.is_terminal(op.sym) => {
            if g.name(l.sym) == "(" {
                return eval(g, input, op);
            }
            let (a, b) = (eval(g, input, l)?, eval(g, input, r)?);
            match g.name(op.sym) {
                "+" => a.checked_add(b),
                "-" => a.checked_sub(b),
                "*" => a.checked_mul(b),
                "/" => a.checked_div(b),
                _ => None,
            }
        }
        _ => None,
    }
}

impl Lesson {
    pub fn ui(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        ui.heading("2.2  歧义与优先级");
        ui.label(RichText::new("入门 · 约 20 分钟").color(p.muted));

        ui.add_space(8.0);
        ui.label(RichText::new("一句话").size(20.0).strong());
        ui.label(
            "如果同一个句子能画出两棵不同的语法树，这个文法就是\"有歧义的\"。对编程语言来说这是灾难：\
             树的形状决定了计算顺序，两棵树可能算出两个不同的结果。",
        );
        callout(ui, CalloutKind::Analogy, "\"咬死了猎人的狗\"", |ui| {
            ui.label(
                "这句话可以是\"(咬死了猎人)的狗\"——一条狗；也可以是\"咬死了(猎人的狗)\"——一个动作。\
                 同一串字，两种结构，两种意思。人靠上下文猜，编译器不能猜，只能在设计文法时就把歧义消灭掉。",
            );
        });

        ui.add_space(8.0);
        ui.label(RichText::new("亲眼看看歧义").size(20.0).strong());
        prose(
            ui,
            "默认的文法把加、减、乘都写成 `E op E`，看起来很自然。输入 `1 + 2 * 3`，下面列出这个文法能给出的**所有**语法树，\
             以及按每棵树计算的结果。再试试其他例子。",
        );
        self.input.editors(ui, PRESETS);
        if self.input.show_errors(ui) {
            self.trees(ui);
        }

        ui.add_space(8.0);
        ui.label(RichText::new("消除歧义：把优先级写进文法的层次").size(20.0).strong());
        prose(
            ui,
            "办法和第 0 课的计算器一样：每一级优先级一个非终结符。`E` 负责加减，`T` 负责乘除，`F` 负责最小的单位。\
             `E -> E + T` 的右边是 `T` 而不是 `E`——这意味着加号的右边只能是一个\"乘除式\"，`2 * 3` 只能先组成 `T`，\
             于是乘法一定在树的下层、先算。点上面的\"分层\"例子看看，树只剩一棵了。",
        );
        callout(ui, CalloutKind::KeyPoint, "结合性：左递归还是右递归", |ui| {
            prose(
                ui,
                "同级运算符（比如两个减号）谁先算，由递归的方向决定。`E -> E - T` 是**左递归**：左边还可以是更长的 `E`，\
                 树向左下方长，`8 - 3 - 2` 是 `(8 - 3) - 2 = 3`，左结合，正确。`E -> T - E` 是**右递归**，\
                 树向右下方长，算出 `8 - (3 - 2) = 7`，错了。点\"结合性\"两个例子对比一下。",
            );
            prose(ui, "赋值 `a = b = c` 和乘方 `2 ^ 3 ^ 2` 通常是右结合的，这时就该用右递归。");
        });

        ui.add_space(12.0);
        ui.label(RichText::new("小测验").size(20.0).strong());
        quiz(
            ui,
            "ch2-amb-1",
            "文法 `E -> E + E | num` 下，`1 + 2 + 3` 有几棵语法树？",
            &["1 棵", "2 棵", "3 棵"],
            1,
            "`(1 + 2) + 3` 和 `1 + (2 + 3)` 两棵。对加法来说结果碰巧一样，但对减法就不一样了——歧义仍然是个问题。",
        );
        quiz(
            ui,
            "ch2-amb-2",
            "分层文法中，为什么乘法比加法\"绑得紧\"？",
            &["因为 * 写在 + 的后面", "因为乘法由更下层的非终结符 T 负责，一定先组成子树", "因为分析器会特殊处理 *"],
            1,
            "分析器对所有符号一视同仁。优先级完全来自文法的层次：越靠下层的非终结符，在树里越深，越先算。",
        );

        ui.add_space(12.0);
        callout(ui, CalloutKind::Deeper, "悬空 else 与判定歧义", |ui| {
            prose(
                ui,
                "经典的歧义还有\"悬空 else\"：`if a then if b then x else y` 里的 `else` 属于哪个 `if`？\
                 C、Java 等语言规定属于最近的那个，解析器（比如 yacc）遇到这种冲突时默认\"移进\"来实现这条规定。\
                 Rust 和 Go 干脆要求 `if` 的分支必须用花括号，从语法上消除了这个问题。",
            );
            ui.label(
                "一个理论上的坏消息：\"任意给一个上下文无关文法，判断它有没有歧义\"是不可判定的——不存在能对所有文法给出答案的算法。\
                 所以实践中都是用 LL(1)、LR(1) 这样的\"保证无歧义\"的文法类（后面的 2.5、2.6 课），构造分析表时如果出现冲突，就说明文法有问题。",
            );
        });
        ui.add_space(24.0);
    }

    fn trees(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        let Ok(g) = &self.input.grammar else { return };
        let r = earley::parse(g, &self.input.syms(), MAX_TREES);
        if !r.accepted {
            ui.label(RichText::new("这个句子不符合文法。").color(p.error));
            return;
        }
        let summary = match (r.trees.len(), r.more_trees) {
            (1, _) => RichText::new("只有 1 棵语法树：这个句子没有歧义。").color(p.ok),
            (n, false) => RichText::new(format!("共有 {n} 棵不同的语法树：有歧义！")).color(p.error),
            (n, true) => {
                RichText::new(format!("至少有 {n} 棵不同的语法树（只显示前 {n} 棵）：有歧义！")).color(p.error)
            }
        };
        ui.label(summary.size(17.0).strong());
        let values: Vec<Option<i64>> = r.trees.iter().map(|t| eval(g, &self.input, t)).collect();
        let distinct: std::collections::BTreeSet<i64> = values.iter().flatten().copied().collect();
        if distinct.len() > 1 {
            ui.label(RichText::new("而且不同的树算出了不同的结果！").color(p.error));
        }
        let cols = if ui.available_width() > 820.0 && r.trees.len() > 1 { 2 } else { 1 };
        ui.columns(cols, |c| {
            for (i, t) in r.trees.iter().enumerate() {
                let ui = &mut c[i % cols];
                card(ui, |ui| {
                    let v = values[i].map_or("（无法求值）".to_owned(), |v| format!("按这棵树计算：{v}"));
                    ui.label(RichText::new(format!("第 {} 棵    {v}", i + 1)).strong());
                    ui.label(RichText::new(t.to_bracketed(g)).font(mono(12.0)).color(p.muted));
                    let nodes = self.input.tree_nodes(g, t);
                    scrollable_tree(ui, &format!("amb_tree_{i}"), &nodes, |_| NodeStyle::normal(&p));
                });
            }
        });
    }
}
