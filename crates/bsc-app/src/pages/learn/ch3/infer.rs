//! 3.3 类型推导（Hindley–Milner）。

use bsc_core::Diagnostic;
use bsc_types::infer::{Event, var_name};
use bsc_types::{ExprId, Inference, Program, Ty, infer, parse};
use eframe::egui::{self, RichText, Ui};

use super::editor;
use crate::theme::{Palette, mono};
use crate::widgets::{
    CalloutKind, Mark, NodeStyle, Stepper, TreeNode, callout, card, diagnostic_view, prose, prose_sized, quiz,
    scrollable_tree, source_view,
};

const PRESETS: &[(&str, &str)] = &[
    ("恒等函数", "fun x -> x"),
    ("加一", "fun x -> x + 1"),
    ("用两次", "fun f -> fun x -> f (f x)"),
    ("函数组合", "fun f -> fun g -> fun x -> f (g x)"),
    ("let 多态", "let id = fun x -> x in\nif id true then id 1 else 2"),
    ("参数不能多态", "(fun id -> if id true then id 1 else 2)\n  (fun x -> x)"),
    ("无限类型", "fun x -> x x"),
    ("类型不匹配", "1 + true"),
];

/// 每种语法结构带来的"方程"。
const RULES: &[(&str, &str)] = &[
    ("42   true", "int、bool，直接知道"),
    ("x", "到环境里查 x 的类型模式；∀ 后面的变量换成新变量（实例化）"),
    ("fun x -> e", "x 的类型设为新变量 α，推出 e 的类型 τ，整体是 α → τ"),
    ("f a", "结果设为新变量 β，方程：f 的类型 = a 的类型 → β"),
    ("let x = e1 in e2", "推出 e1 的类型，泛化（加上 ∀），再在 e2 里使用 x"),
    ("if c then a else b", "方程：c = bool，a = b"),
    ("a + b   a - b   a * b", "方程：a = int，b = int；结果 int"),
    ("a < b", "方程：a = int，b = int；结果 bool"),
    ("a == b", "方程：a = b；结果 bool"),
];

pub struct Lesson {
    src: String,
    prog: Result<Program, Diagnostic>,
    inf: Option<Inference>,
    stepper: Stepper,
}

impl Default for Lesson {
    fn default() -> Self {
        let mut l = Self { src: PRESETS[2].1.to_owned(), prog: parse("0"), inf: None, stepper: Stepper::default() };
        l.rebuild();
        l
    }
}

fn event_node(e: &Event) -> Option<ExprId> {
    match e {
        Event::Visit { node, .. }
        | Event::Fresh { node, .. }
        | Event::Instantiate { node, .. }
        | Event::Unify { node, .. }
        | Event::Generalize { node, .. }
        | Event::Typed { node, .. } => Some(*node),
        Event::Bind { .. } | Event::Fail(_) => None,
    }
}

impl Lesson {
    fn rebuild(&mut self) {
        self.prog = parse(&self.src);
        self.inf = self.prog.as_ref().ok().map(infer);
        self.stepper.reset();
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        ui.heading("3.3  类型推导");
        ui.label(RichText::new("进阶 · 约 40 分钟").color(p.muted));

        ui.add_space(8.0);
        ui.label(RichText::new("一句话").size(20.0).strong());
        prose(
            ui,
            "mini-lang 要求你把参数类型都写出来：`fn sum(n: int)`。可在 OCaml、Haskell 里，你一个类型都不写，\
             编译器照样能查出所有类型错误——它自己把类型**推导**了出来。方法和解方程一模一样：\
             不知道的类型先设成未知数（**类型变量** α、β……），每个语法结构带来几个方程，最后用**合一**算法把方程解出来。",
        );
        callout(ui, CalloutKind::Analogy, "侦探推理", |ui| {
            prose(
                ui,
                "看到 `fun x -> x + 1`：x 是什么类型？先记作\"嫌疑人 α\"。线索一：x 出现在 `+` 的左边，而 `+` 只收 int，\
                 所以 α = int。结论：这个函数收一个 int、返回一个 int，类型是 `int → int`。\
                 如果线索之间互相矛盾（α 既要是 int 又要是 bool），那就是类型错误。",
            );
        });

        ui.add_space(8.0);
        ui.label(RichText::new("一门不写类型的小语言").size(20.0).strong());
        prose(
            ui,
            "这一课换一门迷你的函数式语言（ML 风格）：只有表达式，函数写成 `fun x -> 函数体`，调用就是把参数写在后面 `f x`，\
             多个参数就连着写 `f x y`。`→` 是函数类型，`int → bool` 表示\"收 int，返回 bool\"。",
        );
        card(ui, |ui| {
            egui::Grid::new("hm_rules").num_columns(2).striped(true).spacing([24.0, 6.0]).show(ui, |ui| {
                for (form, rule) in RULES {
                    ui.label(RichText::new(*form).font(mono(14.0)));
                    ui.label(*rule);
                    ui.end_row();
                }
            });
        });

        ui.add_space(8.0);
        ui.label(RichText::new("合一：解类型方程").size(20.0).strong());
        prose(ui, "要让两个类型相等，按下面几条规则拆开来看：");
        card(ui, |ui| {
            for line in [
                "• 类型变量 α = 任何类型 τ：记下替换 \"α = τ\"（前提是 τ 里不含 α 自己，见下文\"无限类型\"）",
                "• int = int、bool = bool：本来就相等，什么都不用做",
                "• (a1 → r1) = (a2 → r2)：拆成两个方程 a1 = a2 和 r1 = r2",
                "• int = bool、int = (… → …)：无解，报类型错误",
            ] {
                ui.label(line);
            }
        });

        ui.add_space(8.0);
        ui.label(RichText::new("单步观察").size(20.0).strong());
        prose(
            ui,
            "左边是到目前为止解出来的替换（方程的解），右边是语法树，每个节点下面是它当前的类型——\
             代入了已知的替换，所以随着方程一个个解出来，树上的类型会越来越具体。",
        );
        if editor(ui, &mut self.src, PRESETS, 2, false) {
            self.rebuild();
        }
        match &self.prog {
            Err(d) => {
                let d = d.clone();
                diagnostic_view(ui, &self.src, &d);
            }
            Ok(_) => self.stepping(ui),
        }

        ui.add_space(8.0);
        callout(ui, CalloutKind::KeyPoint, "let 多态：同一个 id 既用于 bool 又用于 int", |ui| {
            prose(
                ui,
                "对比\"let 多态\"和\"参数不能多态\"两个例子：做的事一样，前者通过、后者报错。\
                 `let id = fun x -> x` 推出 id 的类型 `α → α` 后，α 和外面的任何东西都没关系，于是把它**泛化**成\
                 `∀α. α → α`——\"对任何类型 α 都成立\"。之后每次使用 id，都把 α 换成一个全新的变量（**实例化**），\
                 `id true` 和 `id 1` 各用各的，互不干扰。",
            );
            prose(
                ui,
                "而函数参数 `id` 的类型只是一个普通的未知数 α，没有 ∀。`id true` 解出 α = bool 之后，\
                 `id 1` 就要求 bool = int，矛盾。为什么参数不能泛化？因为推导函数体时，调用者会传进什么还不知道，\
                 它可能根本不是多态的函数。允许\"参数也多态\"的类型系统（System F）表达力更强，但类型推导就变成不可判定的了。",
            );
        });
        callout(ui, CalloutKind::KeyPoint, "无限类型与 occurs check", |ui| {
            prose(
                ui,
                "`fun x -> x x`：x 的类型是 α，`x x` 又要求 α = α → β。α 等于一个包含它自己的类型，\
                 展开就是 `((… → β) → β) → β`，无穷无尽。合一在绑定 α = τ 之前要先检查 τ 里有没有 α（**occurs check**），\
                 有就报错。",
            );
        });

        ui.add_space(12.0);
        ui.label(RichText::new("小测验").size(20.0).strong());
        quiz(
            ui,
            "ch3-infer-1",
            "`fun f -> fun x -> f x` 的类型是？",
            &["∀α β. (α → β) → α → β", "∀α. α → α", "int → int"],
            0,
            "f 被拿来调用，所以 f 的类型是 α → β，其中 α 是 x 的类型、β 是结果。整个函数先收 f、再收 x，最后得到 β。",
        );
        quiz(
            ui,
            "ch3-infer-2",
            "为什么 `let` 定义的函数可以多态，函数参数却不行？",
            &[
                "let 的值在使用前已经完整推导出来，确定哪些类型变量是\"自由的\"；参数的类型要等调用者决定",
                "因为 let 写起来更短",
                "这是个随意的规定，没有原因",
            ],
            0,
            "泛化只能针对已经推导完、且和环境无关的类型变量。参数的类型在函数体里还是未知的，泛化它就不安全了。",
        );

        ui.add_space(12.0);
        callout(ui, CalloutKind::Deeper, "主类型、算法 W 与真实语言", |ui| {
            prose(
                ui,
                "Hindley–Milner 有个漂亮的性质：只要表达式有类型，推导出的就是**最一般的类型**（主类型），\
                 其它所有正确的类型都是它的特例——`∀α. α → α` 可以特化成 `int → int`，反过来不行。\
                 这里的实现是 Milner 1978 年论文里算法 W 的变体（全局替换，也叫算法 J）；Damas 和 Milner 在 1982 年证明了它的正确性与完备性。",
            );
            prose(
                ui,
                "OCaml、F#、Haskell、Elm 都以 HM 为核心，再加上各自的扩展（类型类、多态记录等）。\
                 Rust、Swift、C++ 的 `auto`、TypeScript 做的是**局部**类型推导：函数签名必须写出来，只在函数内部推导——\
                 签名既是文档，也让错误信息更容易看懂。HM 的最坏情况是指数级的（可以构造出类型大小指数增长的程序），\
                 但真实代码里几乎总是接近线性。",
            );
        });
        ui.add_space(24.0);
    }

    fn stepping(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        let (Ok(prog), Some(inf)) = (&self.prog, &self.inf) else { return };
        self.stepper.ui(ui, inf.events.len());
        let k = self.stepper.pos;
        let last = k.checked_sub(1).map(|i| &inf.events[i]);
        // 当前涉及的节点：Bind 没有节点，就沿用前面最近的那个。
        let current = inf.events[..k].iter().rev().find_map(event_node);

        let mut marks = Vec::new();
        if let Some(n) = current {
            marks.push(Mark { span: prog.expr(n).span, bg: p.focus_bg });
        }
        if let Some(Event::Fail(d)) = last
            && let Some(sp) = d.primary_span()
        {
            marks.push(Mark { span: sp, bg: p.error_bg });
        }
        card(ui, |ui| {
            source_view(ui, &prog.src, &[], &marks, 18.0);
        });

        let explain = match last {
            None => "从整个表达式开始，自顶向下推导，每个子表达式推完再回到上一层。".to_owned(),
            Some(Event::Visit { node, rule }) => format!("处理 `{}`。规则：{rule}。", prog.text(*node)),
            Some(Event::Fresh { var, why, .. }) => format!("引入新的类型变量 **{}**：{why}。", var_name(*var)),
            Some(Event::Instantiate { name, scheme, ty, .. }) => {
                if scheme.vars.is_empty() {
                    format!("环境里 `{name}` 的类型是 `{}`，没有 ∀，直接使用。", scheme.ty.show())
                } else {
                    format!(
                        "环境里 `{name}` 的类型模式是 `{}`。把 ∀ 的变量换成新变量（实例化），这次使用的类型是 `{}`。",
                        scheme.show(),
                        ty.show()
                    )
                }
            }
            Some(Event::Unify { left, right, why, .. }) => {
                format!("列方程：`{} = {}`。原因：{why}。", left.show(), right.show())
            }
            Some(Event::Bind { var, ty }) => {
                format!(
                    "合一解出：**{} = {}**。以后所有出现 {} 的地方都可以代换掉。",
                    var_name(*var),
                    ty.show(),
                    var_name(*var)
                )
            }
            Some(Event::Generalize { name, ty, scheme, .. }) => {
                if scheme.vars.is_empty() {
                    format!("`{name}` 的类型是 `{}`，里面没有可以泛化的变量。", ty.show())
                } else {
                    format!(
                        "`{name}` 的类型是 `{}`，其中的变量和环境无关，**泛化**成 `{}`：之后每次使用 {name} 都可以换成不同的类型。",
                        ty.show(),
                        scheme.show()
                    )
                }
            }
            Some(Event::Typed { node, ty }) => {
                let resolved = inf.resolve_at(ty, k);
                if resolved == *ty {
                    format!("`{}` 推导完毕，类型是 `{}`。", prog.text(*node), ty.show())
                } else {
                    format!(
                        "`{}` 推导完毕，类型是 `{}`，代入已知的替换就是 `{}`。",
                        prog.text(*node),
                        ty.show(),
                        resolved.show()
                    )
                }
            }
            Some(Event::Fail(_)) => "方程无解，推导失败：".to_owned(),
        };
        prose_sized(ui, &explain, 17.0);
        if let Some(Event::Fail(d)) = last {
            diagnostic_view(ui, &prog.src, d);
        }

        let wide = ui.available_width() > 820.0;
        let binds = inf.binds_at(k);
        let just_bound = matches!(last, Some(Event::Bind { .. }));
        let subst_panel = |ui: &mut Ui| {
            ui.label(RichText::new("替换（已解出的方程）").strong());
            card(ui, |ui| {
                if binds.is_empty() {
                    ui.label(RichText::new("（还没有）").color(p.muted));
                }
                for (i, (v, t)) in binds.iter().enumerate() {
                    let mut text = RichText::new(format!("{} = {}", var_name(*v), t.show())).font(mono(16.0));
                    if just_bound && i + 1 == binds.len() {
                        text = text.background_color(p.focus_bg).strong();
                    }
                    ui.label(text);
                }
            });
        };
        let tree_panel = |ui: &mut Ui| {
            ui.label(RichText::new("语法树与当前类型").strong());
            let mut typed: Vec<Option<Ty>> = vec![None; prog.exprs.len()];
            for e in &inf.events[..k] {
                if let Event::Typed { node, ty } = e {
                    typed[node.index()] = Some(ty.clone());
                }
            }
            let nodes: Vec<TreeNode> = (0..prog.exprs.len())
                .map(|i| {
                    let id = ExprId(i as u32);
                    let ty = typed[i].as_ref().map_or("…".to_owned(), |t| inf.resolve_at(t, k).show());
                    TreeNode {
                        label: format!("{}\n{ty}", prog.label(id)),
                        children: prog.children(id).iter().map(|c| c.index()).collect(),
                    }
                })
                .collect();
            scrollable_tree(ui, "hm_tree", &nodes, |i| {
                let mut s = NodeStyle::normal(&p);
                if typed[i].is_none() {
                    s.fill = p.card_bg;
                    s.stroke = p.card_stroke;
                }
                if current.is_some_and(|c| c.index() == i) {
                    s.fill = p.focus_bg;
                    s.stroke = p.accent;
                    s.emphasized = true;
                }
                s
            });
        };
        if wide {
            ui.columns(2, |c| {
                subst_panel(&mut c[0]);
                tree_panel(&mut c[1]);
            });
        } else {
            subst_panel(ui);
            tree_panel(ui);
        }

        if k == inf.events.len() {
            match &inf.result {
                Ok(s) => {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new("推导结果：").color(p.ok).strong());
                        ui.label(RichText::new(s.show()).font(mono(18.0)).color(p.ok).strong());
                    });
                    if !s.vars.is_empty() {
                        prose(
                            ui,
                            "结果里剩下没解出来的变量说明：这些位置放什么类型都行，于是整体加上 ∀ 成为多态类型（变量已按出现顺序重新命名）。",
                        );
                    }
                }
                Err(_) => {
                    ui.label(RichText::new("推导失败：这个表达式没有类型。").color(p.error).strong());
                }
            }
        }
    }
}
