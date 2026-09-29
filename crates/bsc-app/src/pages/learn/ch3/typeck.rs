//! 3.2 类型检查。

use bsc_minilang::ast::{Ast, NodeId, NodeKind};
use bsc_minilang::sema::{Event, Type};
use eframe::egui::{self, RichText, Ui};

use super::{Front, editor, syntax_errors};
use crate::theme::{Palette, mono};
use crate::widgets::{
    CalloutKind, Mark, NodeStyle, Stepper, TreeNode, callout, card, diagnostic_view, prose, prose_sized, quiz,
    scrollable_tree, source_view,
};

const PRESETS: &[(&str, &str)] = &[
    (
        "正确的程序",
        "fn main() {\n    let n = 10;\n    print(sum(n));\n}\n\nfn sum(n: int) -> int {\n    let mut s = 0;\n    let mut i = 1;\n    while i <= n {\n        s = s + i;\n        i = i + 1;\n    }\n    return s;\n}",
    ),
    (
        "类型错误",
        "fn main() {\n    let a = 1 + true;\n    let b = (a + 1) * 2;\n    let n = 10;\n    if n { print(n); }\n    let ok: bool = n;\n    print(double(ok));\n}\n\nfn double(n: int) -> int {\n    return n * 2;\n}",
    ),
    (
        "返回值",
        "fn sign(x: int) -> int {\n    if x > 0 {\n        return 1;\n    } else if x < 0 {\n        return -1;\n    }\n}\n\nfn hello() {\n    return 42;\n}",
    ),
];

/// 类型规则：语法结构 → 要求与结果。
const RULES: &[(&str, &str)] = &[
    ("整数 0 1 42 …", "int"),
    ("true  false", "bool"),
    ("变量 x", "查符号表：它声明时的类型"),
    ("a + b   a - b   a * b   a / b   a % b", "a、b 都是 int ⇒ int"),
    ("a < b   a <= b   a > b   a >= b", "a、b 都是 int ⇒ bool"),
    ("a && b   a || b", "a、b 都是 bool ⇒ bool"),
    ("a == b   a != b", "a、b 类型相同 ⇒ bool"),
    ("-a   !a", "int ⇒ int；bool ⇒ bool"),
    ("f(a, b)", "实参逐个和参数类型一致 ⇒ f 的返回类型"),
    ("x = e", "x 可变，e 和 x 类型相同 ⇒ unit（没有值）"),
    ("let x: T = e", "e 的类型是 T；不写 T 时，x 的类型就是 e 的类型"),
    ("if c { … }   while c { … }", "c 必须是 bool"),
    ("return e", "e 的类型和函数声明的返回类型一致"),
];

pub struct Lesson {
    src: String,
    front: Front,
    steps: Vec<usize>,
    parent: Vec<Option<NodeId>>,
    stepper: Stepper,
}

impl Default for Lesson {
    fn default() -> Self {
        let mut l = Self {
            src: String::new(),
            front: Front::new(""),
            steps: vec![],
            parent: vec![],
            stepper: Stepper::default(),
        };
        l.src = PRESETS[0].1.to_owned();
        l.rebuild();
        l
    }
}

fn is_expr(k: &NodeKind) -> bool {
    matches!(
        k,
        NodeKind::Binary(_)
            | NodeKind::Unary(_)
            | NodeKind::Assign { .. }
            | NodeKind::Call { .. }
            | NodeKind::Int(_)
            | NodeKind::Bool(_)
            | NodeKind::Var(_)
    )
}

impl Lesson {
    fn rebuild(&mut self) {
        self.front = Front::new(&self.src);
        let ast = &self.front.parse.ast;
        self.parent = vec![None; ast.nodes.len()];
        for (i, n) in ast.nodes.iter().enumerate() {
            for c in &n.children {
                self.parent[c.index()] = Some(NodeId(i as u32));
            }
        }
        self.steps = match &self.front.sema {
            Some(a) => a
                .events
                .iter()
                .enumerate()
                .filter(|(_, e)| matches!(e, Event::TypeOf { .. } | Event::Error(_)))
                .map(|(i, _)| i)
                .collect(),
            None => vec![],
        };
        self.stepper.reset();
    }

    /// 包含这个节点的最大的表达式（一路往上，直到父节点不再是表达式）。
    fn expr_root(&self, mut n: NodeId) -> NodeId {
        let ast = &self.front.parse.ast;
        while let Some(pa) = self.parent[n.index()]
            && is_expr(&ast.node(pa).kind)
        {
            n = pa;
        }
        n
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        ui.heading("3.2  类型检查");
        ui.label(RichText::new("入门 · 约 25 分钟").color(p.muted));

        ui.add_space(8.0);
        ui.label(RichText::new("一句话").size(20.0).strong());
        prose(
            ui,
            "名字都找到主人之后，下一个问题是：**这些值能这么用吗？** `1 + true` 语法没问题、名字也都认识，但\"数字加真假\"没有意义。\
             类型检查按一套**类型规则**，自底向上给每个表达式算出类型：先算叶子（数字、变量），再按运算符的规则算出上一层，\
             一路算到整个表达式。任何一处不满足规则，就报类型错误——在程序运行**之前**。",
        );
        callout(ui, CalloutKind::Analogy, "插头和插座", |ui| {
            prose(
                ui,
                "类型就像插头的形状：`+` 这个插座只接受两个 int 形状的插头，插进去之后，它自己又变成一个 int 形状的插头，\
                 可以继续往上插。`<` 接受两个 int，但变出来的是 bool 形状，只能插进 `if`、`while`、`&&` 这类要 bool 的插座。\
                 类型检查就是在程序运行之前，把每个插头和插座比一遍。",
            );
        });

        ui.add_space(8.0);
        ui.label(RichText::new("mini-lang 的类型规则").size(20.0).strong());
        card(ui, |ui| {
            egui::Grid::new("typeck_rules").num_columns(2).striped(true).spacing([24.0, 6.0]).show(ui, |ui| {
                for (form, rule) in RULES {
                    ui.label(RichText::new(*form).font(mono(14.0)));
                    ui.label(*rule);
                    ui.end_row();
                }
            });
        });

        ui.add_space(8.0);
        ui.label(RichText::new("单步观察").size(20.0).strong());
        prose(
            ui,
            "每一步给一个表达式定下类型，并说明依据的是哪条规则。下方的树是当前这一整个表达式，\
             每个节点下面标着已经算出的类型——可以看到类型是从叶子往根\"流\"上去的。",
        );
        if editor(ui, &mut self.src, PRESETS, 6, true) {
            self.rebuild();
        }
        if self.front.sema.is_some() {
            self.stepping(ui);
        } else {
            syntax_errors(ui, &self.front);
        }

        ui.add_space(8.0);
        callout(ui, CalloutKind::KeyPoint, "一个错误，只报一次", |ui| {
            prose(
                ui,
                "选\"类型错误\"例子：`1 + true` 出错后，`a` 的类型是什么？如果随便当成 int，可能掩盖问题；\
                 如果不管它，后面每个用到 `a` 的地方都会再报一遍错，一个笔误引出一长串错误，新手会被吓到。\
                 这里的做法是给它一个特殊的**错误类型**（显示为 `?`），它和任何类型都兼容，也不再引发新的报错。\
                 于是 `(a + 1) * 2` 安静地得到 `?`，真正的错误只报一次。rustc 里对应的东西叫 `{type error}`。",
            );
        });
        callout(ui, CalloutKind::KeyPoint, "有些检查要看控制流", |ui| {
            prose(
                ui,
                "\"函数可能没有返回值\"不是看某一个表达式就能判断的：要看所有可能的执行路径是不是都遇到了 `return`。\
                 选\"返回值\"例子：`if … else if …` 少了最后的 `else`，当 x 等于 0 时两个分支都不走，函数就从末尾掉出去了。\
                 这种沿着控制流的分析，第 5 章讲数据流分析时会系统地展开。",
            );
        });

        ui.add_space(12.0);
        ui.label(RichText::new("小测验").size(20.0).strong());
        quiz(
            ui,
            "ch3-typeck-1",
            "`(1 < 2) + 3` 为什么是类型错误？",
            &["1 < 2 的结果是 bool，而 + 要求两边都是 int", "3 太大了", "括号里不能写比较"],
            0,
            "类型检查自底向上：先得出 `1 < 2` 是 bool，再检查 `+` 的规则，发现左边不是 int。",
        );
        quiz(
            ui,
            "ch3-typeck-2",
            "`let y = x + 1;` 里 x 没有定义。好的编译器会报几个错？",
            &["1 个：找不到 x", "2 个：找不到 x，以及 + 两边类型不对", "3 个：再加上 y 的类型未知"],
            0,
            "x 找不到时给它错误类型 `?`，`?` 和任何类型都兼容，所以后面的 `+`、`let` 都不再报错。",
        );

        ui.add_space(12.0);
        callout(ui, CalloutKind::Deeper, "类型规则的数学写法", |ui| {
            prose(
                ui,
                "教材和论文里，类型规则写成**推理规则**：横线上面是前提，下面是结论。`Γ ⊢ e : τ` 读作\
                 \"在环境 Γ（也就是符号表）下，表达式 e 的类型是 τ\"。加法和 if 语句的规则写出来是这样：",
            );
            card(ui, |ui| {
                ui.label(
                    RichText::new(
                        " Γ ⊢ a : int    Γ ⊢ b : int              Γ(x) = τ\n\
                         ─────────────────────────── (加法)    ────────── (变量)\n\
                         \x20     Γ ⊢ a + b : int                   Γ ⊢ x : τ",
                    )
                    .font(mono(15.0)),
                );
            });
            prose(
                ui,
                "这套写法把\"类型检查器\"变成了可以证明的东西。最重要的性质叫**类型安全**（soundness）：\
                 通过了类型检查的程序，运行时不会出现\"把整数当函数调用\"这类错误——Robin Milner 的名言：\
                 \"Well-typed programs cannot go wrong.\" 证明通常分两步：前进（well-typed 的程序要么算完了，要么还能再走一步）\
                 和保持（走一步之后类型不变）。",
            );
        });
        ui.add_space(24.0);
    }

    fn stepping(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        let Some(a) = &self.front.sema else { return };
        let ast = &self.front.parse.ast;
        self.stepper.ui(ui, self.steps.len());
        let k = self.stepper.pos;
        let upto = if k == 0 { 0 } else { self.steps[k - 1] + 1 };
        let last = k.checked_sub(1).map(|i| &a.events[self.steps[i]]);

        let mut known: Vec<Option<Type>> = vec![None; ast.nodes.len()];
        for e in &a.events[..upto] {
            if let Event::TypeOf { node, ty, .. } = e {
                known[node.index()] = Some(*ty);
            }
        }
        let current = match last {
            Some(Event::TypeOf { node, .. }) => Some(*node),
            _ => None,
        };

        let mut marks = Vec::new();
        if let Some(n) = current {
            marks.push(Mark { span: ast.node(self.expr_root(n)).span, bg: p.done_bg });
            let bg = if known[n.index()] == Some(Type::Error) { p.error_bg } else { p.focus_bg };
            marks.push(Mark { span: ast.node(n).span, bg });
        }
        card(ui, |ui| {
            source_view(ui, &self.front.src, &self.front.colors(&p), &marks, 17.0);
        });

        match last {
            None => {
                prose_sized(ui, "从程序开头开始。按\"下一步\"或\"自动播放\"。", 17.0);
            }
            Some(Event::TypeOf { node, ty, why }) => {
                let text = ast.node(*node).span.text(&self.front.src);
                prose_sized(ui, &format!("`{text}` 的类型是 `{}`。依据：{why}", ty.name()), 17.0);
            }
            Some(Event::Error(i)) => {
                prose_sized(ui, "违反了类型规则：", 17.0);
                diagnostic_view(ui, &self.front.src, &a.errors[*i]);
            }
            Some(_) => {}
        }

        if let Some(n) = current {
            ui.label(RichText::new("当前表达式的类型树（从叶子往上算）").strong());
            let root = self.expr_root(n);
            let (nodes, ids) = subtree(ast, root, &known);
            scrollable_tree(ui, "typeck_tree", &nodes, |i| {
                let id = ids[i];
                let mut s = NodeStyle::normal(&p);
                if known[id.index()].is_none() {
                    s.fill = p.card_bg;
                    s.stroke = p.card_stroke;
                } else if known[id.index()] == Some(Type::Error) {
                    s.fill = p.error_bg;
                    s.stroke = p.error;
                }
                if id == n {
                    s.fill = p.focus_bg;
                    s.stroke = p.accent;
                    s.emphasized = true;
                }
                s
            });
        }

        if k == self.steps.len() && k > 0 {
            let n = a.errors.len();
            let text = if n == 0 {
                RichText::new("类型检查通过：每个表达式都符合类型规则。").color(p.ok)
            } else {
                RichText::new(format!("检查完成，共发现 {n} 个错误。")).color(p.error)
            };
            ui.label(text.strong());
        }
    }
}

/// 以 `root` 为根的子树，节点标签下面标上已知的类型。返回树节点和它们对应的语法树编号。
fn subtree(ast: &Ast, root: NodeId, known: &[Option<Type>]) -> (Vec<TreeNode>, Vec<NodeId>) {
    let mut ids = Vec::new();
    let mut stack = vec![root];
    while let Some(n) = stack.pop() {
        ids.push(n);
        stack.extend(ast.node(n).children.iter().rev());
    }
    let local = |n: NodeId| ids.iter().position(|&x| x == n).unwrap();
    let nodes = ids
        .iter()
        .map(|&n| {
            let ty = known[n.index()].map_or("…", |t| t.name());
            TreeNode {
                label: format!("{}\n{ty}", ast.label(n)),
                children: ast.node(n).children.iter().map(|&c| local(c)).collect(),
            }
        })
        .collect();
    (nodes, ids)
}
