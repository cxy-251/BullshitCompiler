//! 计算器编译器各阶段的逐步演示视图。第 0 课和实验台共用。
//!
//! 每个视图都只做"回放"：数据全部来自 `bsc_calc` 真实运行时记下的过程，
//! 界面不自己模拟任何算法。

use bsc_calc::ast::{Ast, NodeId};
use bsc_calc::codegen::Instr;
use bsc_calc::lexer::{Token, TokenKind};
use bsc_calc::parser::{ParseEvent, Rule};
use bsc_calc::{Compilation, Span, Stage};
use eframe::egui::{self, CornerRadius, Frame, Margin, RichText, Stroke, Ui};

use crate::theme::{Palette, mono};
use crate::widgets::{
    Mark, NodeStyle, Stepper, TreeNode, card, diagnostic_view, prose_sized, scrollable_tree, source_view, stack_view,
};

/// 四个阶段各自的播放器。
#[derive(Default)]
pub struct StageSteppers {
    pub lex: Stepper,
    pub parse: Stepper,
    pub codegen: Stepper,
    pub run: Stepper,
}

impl StageSteppers {
    pub fn reset(&mut self) {
        for s in [&mut self.lex, &mut self.parse, &mut self.codegen, &mut self.run] {
            s.reset();
        }
    }

    pub fn get(&mut self, stage: Stage) -> &mut Stepper {
        match stage {
            Stage::Lex => &mut self.lex,
            Stage::Parse => &mut self.parse,
            Stage::Codegen => &mut self.codegen,
            Stage::Run => &mut self.run,
        }
    }
}

pub fn stage_view(ui: &mut Ui, comp: &Compilation, stage: Stage, stepper: &mut Stepper) {
    match stage {
        Stage::Lex => lex_view(ui, comp, stepper),
        Stage::Parse => parse_view(ui, comp, stepper),
        Stage::Codegen => codegen_view(ui, comp, stepper),
        Stage::Run => run_view(ui, comp, stepper),
    }
}

// ---------------------------------------------------------------------------
// 词法分析

fn lex_view(ui: &mut Ui, comp: &Compilation, stepper: &mut Stepper) {
    let p = Palette::of(ui);
    let tokens = match &comp.tokens {
        Ok(t) => t,
        Err(d) => {
            ui.label("词法分析器在扫描时遇到了不认识的字符，停了下来：");
            diagnostic_view(ui, &comp.source, d);
            return;
        }
    };

    stepper.ui(ui, tokens.len());
    let k = stepper.pos;

    let colors = token_colors(&p, &tokens[..k]);
    let mut marks: Vec<Mark> =
        tokens[..k.saturating_sub(1)].iter().map(|t| Mark { span: t.span, bg: p.done_bg }).collect();
    if k > 0 {
        marks.push(Mark { span: tokens[k - 1].span, bg: p.focus_bg });
    } else {
        marks.push(Mark { span: Span::empty_at(0), bg: p.focus_bg });
    }
    card(ui, |ui| {
        ui.label(RichText::new("源码（已扫描的部分已着色）").small().color(p.muted));
        source_view(ui, &comp.source, &colors, &marks, 22.0);
    });

    prose_sized(ui, &lex_explain(comp, tokens, k), 17.0);

    ui.add_space(4.0);
    ui.label(RichText::new("得到的记号序列：").small().color(p.muted));
    ui.horizontal_wrapped(|ui| {
        for (i, t) in tokens[..k].iter().enumerate() {
            token_chip(ui, t, i + 1 == k);
        }
        if k == 0 {
            ui.label(RichText::new("（还没有）").color(p.muted));
        }
    });
}

fn lex_explain(comp: &Compilation, tokens: &[Token], k: usize) -> String {
    if k == 0 {
        return "准备就绪：词法分析器站在第一个字符前面，从左往右扫描。点「下一步」开始。".to_owned();
    }
    let t = tokens[k - 1];
    let prev_end = if k >= 2 { tokens[k - 2].span.end } else { 0 };
    let skipped = if comp.source[prev_end..t.span.start].chars().any(char::is_whitespace) {
        "先跳过空白字符（空格只起分隔作用，不产生记号）。然后"
    } else {
        ""
    };
    match t.kind {
        TokenKind::Int(n) => format!(
            "{skipped}看到数字字符，就一直往后读，直到遇到不是数字的字符为止——读到了 `{}`，得到一个【整数 {n}】记号。",
            t.span.text(&comp.source)
        ),
        TokenKind::Eof => format!("{skipped}源码读完了。最后补一个【输入结束】记号，好让语法分析器知道\"到头了\"。"),
        kind => format!("{skipped}看到 `{}`，它自己就是一个完整的记号：【{}】。", kind.symbol(), kind.name()),
    }
}

fn token_colors(p: &Palette, tokens: &[Token]) -> Vec<(Span, egui::Color32)> {
    tokens.iter().map(|t| (t.span, p.token(t.kind.class()))).collect()
}

/// 一个记号的"小卡片"：种类 + 值。
pub fn token_chip(ui: &mut Ui, t: &Token, current: bool) -> egui::Response {
    let p = Palette::of(ui);
    let color = p.token(t.kind.class());
    let text = match t.kind {
        TokenKind::Int(n) => format!("整数 {n}"),
        TokenKind::Eof => "结束".to_owned(),
        k => k.symbol().to_owned(),
    };
    Frame::new()
        .fill(if current { p.focus_bg } else { p.node_fill })
        .stroke(Stroke::new(if current { 2.0 } else { 1.0 }, color))
        .corner_radius(CornerRadius::same(5))
        .inner_margin(Margin::symmetric(8, 2))
        .show(ui, |ui| ui.add(egui::Label::new(RichText::new(text).font(mono(15.0)).color(color)).extend()))
        .response
}

// ---------------------------------------------------------------------------
// 语法分析

fn parse_view(ui: &mut Ui, comp: &Compilation, stepper: &mut Stepper) {
    let p = Palette::of(ui);
    let (Ok(tokens), Some(parse)) = (&comp.tokens, &comp.parse) else {
        ui.label(RichText::new("词法分析没有通过，语法分析无法开始。先回到「词法分析」看看错误。").color(p.error));
        return;
    };

    stepper.ui(ui, parse.events.len());
    let k = stepper.pos;

    // 回放前 k 个事件，还原出此刻的调用栈、读到的位置、已经建好的节点。
    let mut stack: Vec<Rule> = Vec::new();
    let mut next_token = 0;
    let mut built = vec![false; parse.ast.nodes.len()];
    for e in &parse.events[..k] {
        match *e {
            ParseEvent::Enter(r) => stack.push(r),
            ParseEvent::Exit(_) => {
                stack.pop();
            }
            ParseEvent::Consume(i) => next_token = (i + 1).min(tokens.len() - 1),
            ParseEvent::Build(n) => built[n.index()] = true,
        }
    }
    let last = k.checked_sub(1).map(|i| parse.events[i]);

    // 源码：已读过的记号着色，下一个要看的记号高亮。
    let colors = token_colors(&p, &tokens[..next_token]);
    let mut marks = vec![Mark { span: tokens[next_token].span, bg: p.focus_bg }];
    if let Some(ParseEvent::Consume(i)) = last {
        marks.push(Mark { span: tokens[i].span, bg: p.focus_bg });
        marks.remove(0);
    }
    card(ui, |ui| {
        ui.label(RichText::new("源码（已读过的记号已着色，高亮的是正在看的记号）").small().color(p.muted));
        source_view(ui, &comp.source, &colors, &marks, 22.0);
    });

    prose_sized(ui, &parse_explain(comp, tokens, &parse.ast, last), 17.0);

    let wide = ui.available_width() > 820.0;
    let left = |ui: &mut Ui| {
        ui.label(RichText::new("调用栈（最下面是正在执行的函数）").strong());
        if stack.is_empty() {
            ui.label(RichText::new(if k == 0 { "（还没开始）" } else { "（全部返回了）" }).color(p.muted));
        }
        for (depth, r) in stack.iter().enumerate() {
            let current = depth + 1 == stack.len();
            let text = RichText::new(format!("{}()", r.name())).font(mono(16.0));
            let text = if current { text.background_color(p.focus_bg).strong() } else { text };
            ui.horizontal(|ui| {
                ui.add_space(depth as f32 * 18.0);
                ui.label(text);
            });
        }
        ui.add_space(8.0);
        ui.label(RichText::new("文法规则（每条规则就是一个函数）").strong());
        for r in Rule::ALL {
            let active = stack.last() == Some(&r);
            let text = RichText::new(r.production()).font(mono(14.0));
            ui.label(if active { text.background_color(p.focus_bg) } else { text });
        }
    };
    let right = |ui: &mut Ui| {
        ui.label(RichText::new("已经建好的语法树节点").strong());
        let just_built = match last {
            Some(ParseEvent::Build(n)) => Some(n),
            _ => None,
        };
        let nodes = ast_tree(&parse.ast);
        scrollable_tree(ui, "parse_tree", &nodes, |i| {
            if !built[i] {
                return NodeStyle::hidden();
            }
            let mut s = NodeStyle::normal(&p);
            if just_built.is_some_and(|n| n.index() == i) {
                s.fill = p.focus_bg;
                s.stroke = p.accent;
                s.emphasized = true;
            }
            s
        });
    };
    if wide {
        ui.columns(2, |cols| {
            left(&mut cols[0]);
            right(&mut cols[1]);
        });
    } else {
        left(ui);
        right(ui);
    }

    if k == parse.events.len() {
        match &parse.root {
            Ok(_) => {
                ui.label(RichText::new("语法分析完成：所有记号都组织进了一棵树。").color(p.ok));
            }
            Err(d) => {
                ui.label("语法分析在这里发现了错误：");
                diagnostic_view(ui, &comp.source, d);
            }
        }
    }
}

fn parse_explain(comp: &Compilation, tokens: &[Token], ast: &Ast, last: Option<ParseEvent>) -> String {
    match last {
        None => "语法分析从最顶层的规则 expr 开始。每条文法规则都是一个函数，函数之间互相调用——这就是\"递归下降\"。"
            .to_owned(),
        Some(ParseEvent::Enter(r)) => format!("调用 {}()：看看接下来能不能读出\"{}\"。", r.name(), r.meaning()),
        Some(ParseEvent::Exit(r)) => format!("{}() 读完了自己负责的部分，把得到的子树交还给调用它的函数。", r.name()),
        Some(ParseEvent::Consume(i)) => {
            let t = tokens[i];
            match t.kind {
                TokenKind::Eof => "确认已经到了输入末尾。".to_owned(),
                _ => format!("读入记号 `{}`（{}），往后移一格。", t.span.text(&comp.source), t.kind.name()),
            }
        }
        Some(ParseEvent::Build(n)) => {
            let node = ast.node(n);
            format!("用刚读到的东西建一个节点【{}】，它代表源码里的 `{}`。", ast.label(n), node.span.text(&comp.source))
        }
    }
}

pub fn ast_tree(ast: &Ast) -> Vec<TreeNode> {
    (0..ast.nodes.len())
        .map(|i| {
            let id = NodeId(i as u32);
            TreeNode { label: ast.label(id), children: ast.children(id).into_iter().map(NodeId::index).collect() }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// 代码生成

fn codegen_view(ui: &mut Ui, comp: &Compilation, stepper: &mut Stepper) {
    let p = Palette::of(ui);
    let (Some(parse), Some(code)) = (&comp.parse, &comp.code) else {
        ui.label(RichText::new("前面的阶段没有通过，没有语法树可以翻译。").color(p.error));
        return;
    };

    stepper.ui(ui, code.len());
    let k = stepper.pos;
    let current = k.checked_sub(1).map(|i| code[i]);

    let mut marks = Vec::new();
    if let Some(instr) = current {
        marks.push(Mark { span: parse.ast.node(instr.node).span, bg: p.focus_bg });
    }
    card(ui, |ui| {
        ui.label(RichText::new("源码（高亮的是当前指令对应的部分）").small().color(p.muted));
        source_view(ui, &comp.source, &token_colors(&p, comp.tokens.as_deref().unwrap_or(&[])), &marks, 22.0);
    });

    let explain = match current {
        None => {
            "代码生成按\"后序\"遍历语法树：先翻译左孩子、再翻译右孩子、最后输出自己的运算。数字节点直接变成 `push`。"
                .to_owned()
        }
        Some(i) => format!("节点【{}】→ 输出 `{}`：{}。", parse.ast.label(i.node), i.op, i.op.explain()),
    };
    prose_sized(ui, &explain, 17.0);

    let mut emitted = vec![false; parse.ast.nodes.len()];
    for i in &code[..k] {
        emitted[i.node.index()] = true;
    }
    let tree = |ui: &mut Ui| {
        ui.label(RichText::new("语法树（已翻译的节点变成蓝色）").strong());
        scrollable_tree(ui, "codegen_tree", &ast_tree(&parse.ast), |i| {
            let mut s = NodeStyle::normal(&p);
            if emitted[i] {
                s.fill = p.done_bg;
            }
            if current.is_some_and(|c| c.node.index() == i) {
                s.fill = p.focus_bg;
                s.stroke = p.accent;
                s.emphasized = true;
            }
            s
        });
    };
    let list = |ui: &mut Ui| {
        ui.label(RichText::new("生成的指令").strong());
        instr_list(ui, code, k, current.map(|_| k - 1), None);
    };
    if ui.available_width() > 820.0 {
        ui.columns(2, |cols| {
            tree(&mut cols[0]);
            list(&mut cols[1]);
        });
    } else {
        tree(ui);
        list(ui);
    }
}

/// 指令清单：显示前 `shown` 条，`current` 高亮，`pc` 处画一个箭头。返回鼠标悬停的指令。
pub fn instr_list(
    ui: &mut Ui,
    code: &[Instr],
    shown: usize,
    current: Option<usize>,
    pc: Option<usize>,
) -> Option<usize> {
    let p = Palette::of(ui);
    let mut hovered = None;
    card(ui, |ui| {
        if shown == 0 {
            ui.label(RichText::new("（还没有指令）").color(p.muted));
        }
        for (i, instr) in code[..shown].iter().enumerate() {
            let mut text = RichText::new(format!("{i:>2}  {}", instr.op)).font(mono(16.0));
            if current == Some(i) {
                text = text.background_color(p.focus_bg);
            }
            if pc.is_some_and(|pc| i < pc) {
                text = text.color(p.muted);
            }
            let row = ui.horizontal(|ui| {
                let arrow = if pc == Some(i) { "→" } else { " " };
                ui.add_sized([18.0, 20.0], egui::Label::new(RichText::new(arrow).font(mono(16.0)).color(p.accent)));
                ui.label(text)
            });
            if row.inner.hovered() {
                hovered = Some(i);
            }
        }
    });
    hovered
}

// ---------------------------------------------------------------------------
// 执行

fn run_view(ui: &mut Ui, comp: &Compilation, stepper: &mut Stepper) {
    let p = Palette::of(ui);
    let (Some(parse), Some(code), Some(exec)) = (&comp.parse, &comp.code, &comp.exec) else {
        ui.label(RichText::new("前面的阶段没有通过，没有指令可以执行。").color(p.error));
        return;
    };
    let failed = exec.result.is_err();
    let len = exec.steps.len() + usize::from(failed);
    stepper.ui(ui, len);
    let k = stepper.pos;

    // 第 k 步执行的是第 k-1 条指令；出错时最后一步是出错的那条指令。
    let executed = k.checked_sub(1);
    let stack: &[i64] = match executed {
        Some(i) if i < exec.steps.len() => &exec.steps[i].stack,
        Some(_) => exec.steps.last().map_or(&[], |s| &s.stack),
        None => &[],
    };

    let mut marks = Vec::new();
    if let Some(i) = executed {
        marks.push(Mark { span: parse.ast.node(code[i].node).span, bg: p.focus_bg });
    }
    card(ui, |ui| {
        ui.label(RichText::new("源码（高亮的是刚执行的指令对应的部分）").small().color(p.muted));
        source_view(ui, &comp.source, &token_colors(&p, comp.tokens.as_deref().unwrap_or(&[])), &marks, 22.0);
    });

    let explain = match executed {
        None => "虚拟机从第 0 条指令开始，一条一条往下执行。开始时栈是空的。".to_owned(),
        Some(i) if failed && i == exec.steps.len() => format!("执行 `{}` 时出错了！", code[i].op),
        Some(i) => format!("执行 `{}`：{}。", code[i].op, code[i].op.explain()),
    };
    prose_sized(ui, &explain, 17.0);

    let list = |ui: &mut Ui| {
        ui.label(RichText::new("指令（箭头指向下一条要执行的）").strong());
        let pc = if k < code.len() && !(failed && k == len) { Some(k) } else { None };
        instr_list(ui, code, code.len(), executed, pc);
    };
    let stack_ui = |ui: &mut Ui| {
        ui.label(RichText::new("栈").strong());
        stack_view(ui, stack, executed.is_some());
    };
    ui.columns(2, |cols| {
        list(&mut cols[0]);
        stack_ui(&mut cols[1]);
    });

    if k == len {
        match &exec.result {
            Ok(v) => {
                ui.label(
                    RichText::new(format!("执行完毕：栈上剩下的唯一一个数 {v} 就是结果。")).color(p.ok).size(18.0),
                );
            }
            Err(d) => diagnostic_view(ui, &comp.source, d),
        }
    }
}
