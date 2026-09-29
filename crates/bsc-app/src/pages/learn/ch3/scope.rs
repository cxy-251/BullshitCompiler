//! 3.1 作用域与符号表。

use bsc_core::line_col;
use bsc_minilang::sema::{Analysis, Event, ScopeId, ScopeKind, SymbolId, SymbolKind};
use eframe::egui::{CornerRadius, Frame, Margin, RichText, Stroke, Ui};

use super::{Front, editor, syntax_errors};
use crate::theme::{Palette, mono};
use crate::widgets::{
    CalloutKind, Mark, Stepper, callout, card, chip, chip_button, diagnostic_view, prose, prose_sized, quiz,
    source_view,
};

const PRESETS: &[(&str, &str)] = &[
    (
        "嵌套与遮蔽",
        "fn main() {\n    let x = 1;\n    {\n        let x = true;\n        if x { print(1); }\n    }\n    let x = x + 1;\n    print(x);\n}",
    ),
    (
        "先用后定义的函数",
        "fn main() {\n    print(square(3));\n}\n\nfn square(n: int) -> int {\n    let r = n * n;\n    return r;\n}",
    ),
    (
        "常见错误",
        "fn main() {\n    let count = 0;\n    {\n        let t = 5;\n    }\n    print(cout);\n    print(t);\n    count = count + 1;\n}",
    ),
];

/// 属于"名字解析"的错误代码（其余的是类型错误，留给 3.2 课）。
const SCOPE_ERRORS: &[&str] = &["E1202", "E1203", "E1204", "E1205", "E1210", "E1212"];

pub struct Lesson {
    src: String,
    front: Front,
    /// 这一课关心的事件在 `events` 里的下标。
    steps: Vec<usize>,
    stepper: Stepper,
}

impl Default for Lesson {
    fn default() -> Self {
        let mut l = Self { src: String::new(), front: Front::new(""), steps: vec![], stepper: Stepper::default() };
        l.src = PRESETS[0].1.to_owned();
        l.rebuild();
        l
    }
}

/// 回放到某一步时的状态。
struct State {
    /// 从外到内，正在"打开"的作用域。
    active: Vec<ScopeId>,
    declared: Vec<bool>,
}

fn replay(a: &Analysis, upto: usize) -> State {
    let mut st = State { active: vec![], declared: vec![false; a.symbols.len()] };
    for e in &a.events[..upto] {
        match e {
            Event::EnterScope(s) => st.active.push(*s),
            Event::ExitScope(_) => {
                st.active.pop();
            }
            Event::Declare { sym, .. } => st.declared[*sym] = true,
            _ => {}
        }
    }
    st
}

impl Lesson {
    fn rebuild(&mut self) {
        self.front = Front::new(&self.src);
        self.steps = match &self.front.sema {
            Some(a) => a
                .events
                .iter()
                .enumerate()
                .filter(|(_, e)| match e {
                    Event::TypeOf { .. } => false,
                    Event::Error(i) => a.errors[*i].code.is_some_and(|c| SCOPE_ERRORS.contains(&c)),
                    _ => true,
                })
                .map(|(i, _)| i)
                .collect(),
            None => vec![],
        };
        self.stepper.reset();
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        ui.heading("3.1  作用域与符号表");
        ui.label(RichText::new("入门 · 约 25 分钟").color(p.muted));

        ui.add_space(8.0);
        ui.label(RichText::new("一句话").size(20.0).strong());
        prose(
            ui,
            "语法分析只保证句子\"结构对\"：`print(y);` 的语法完全正确，可 `y` 是什么？在哪定义的？\
             语义分析的第一件事就是**名字解析**：给程序里每一次使用名字的地方，找到它对应的那个声明。\
             编译器靠**符号表**记住\"目前有哪些名字、各是什么\"，靠**作用域**规定\"一个名字在哪一段代码里可见\"。",
        );
        callout(ui, CalloutKind::Analogy, "一层套一层的通讯录", |ui| {
            prose(
                ui,
                "每个 `{ }` 代码块都带一本自己的小通讯录，块套块，通讯录也就一本夹着一本。\
                 要找一个名字，先翻最里面那本；找不到，就翻外面一层；一直翻到最外层的\"全局通讯录\"。\
                 最先找到的那条就是答案——所以里层的同名条目会**遮住**外层的（遮蔽）。\
                 走出一个 `}`，这本小通讯录就被扔掉，里面的名字再也查不到了。",
            );
        });

        ui.add_space(8.0);
        ui.label(RichText::new("单步观察").size(20.0).strong());
        prose(
            ui,
            "下面是真正的语义分析器在工作。右侧的方框就是此刻的作用域链：最外面是全局作用域，越往里越深。\
             每一步可能是：进入/离开一个作用域、登记一个名字（声明）、查找一个名字（使用）。",
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
        callout(ui, CalloutKind::KeyPoint, "为什么函数可以先用后定义，变量却不行？", |ui| {
            prose(
                ui,
                "选\"先用后定义的函数\"例子：`main` 调用了写在它后面的 `square`，却没有报错。\
                 因为分析器分**两遍**：第一遍只扫一遍所有函数的签名，登记到全局作用域；第二遍才逐个检查函数体。\
                 变量则是\"执行到哪里、声明到哪里\"：`let` 语句之前，这个名字还不存在。",
            );
        });
        callout(ui, CalloutKind::KeyPoint, "let x = x + 1; 右边的 x 是谁？", |ui| {
            prose(
                ui,
                "分析器先检查初值 `x + 1`，**再**登记新的 `x`。所以右边的 `x` 查到的是之前那个旧的 `x`，\
                 新的 `x` 从下一条语句起才可见，并遮住旧的。在\"嵌套与遮蔽\"例子里一步步看，就能看到这个顺序。",
            );
        });

        ui.add_space(12.0);
        ui.label(RichText::new("小测验").size(20.0).strong());
        quiz(
            ui,
            "ch3-scope-1",
            "代码 `let a = 1; { let a = 2; } print(a);` 打印的是？",
            &["1", "2", "编译错误：a 重复定义"],
            0,
            "内层块里的 `a` 只在那对花括号里可见，离开 `}` 就被丢弃了。`print(a)` 往外找，找到的是值为 1 的那个。",
        );
        quiz(
            ui,
            "ch3-scope-2",
            "查找名字时，从哪一层作用域开始找？",
            &["全局作用域", "当前（最内层）作用域", "离声明最近的函数"],
            1,
            "从当前作用域开始，一层层往外找，第一个找到的就是答案。这正是遮蔽能生效的原因。",
        );

        ui.add_space(12.0);
        callout(ui, CalloutKind::Deeper, "静态作用域与动态作用域", |ui| {
            prose(
                ui,
                "我们这里\"名字指谁\"完全由源代码的嵌套结构决定，编译时就能确定——叫**静态作用域**（词法作用域），\
                 C、Java、Rust、Python、JavaScript 都是如此。早期的 Lisp、以及 Bash、Emacs Lisp 默认用的是**动态作用域**：\
                 名字指向运行时\"调用链上最近的那个\"，同一段代码被不同的函数调用，看到的变量可能不一样，很难推理。",
            );
            prose(
                ui,
                "静态作用域遇上\"函数里定义函数\"就引出了**闭包**：内层函数用到了外层的局部变量，外层函数返回后，\
                 这个变量还得活着——编译器要把它从栈上\"搬\"到堆上，和函数一起打包。",
            );
        });
        callout(ui, CalloutKind::Deeper, "真实编译器里的符号表", |ui| {
            prose(
                ui,
                "最直接的实现就是这里的做法：每个作用域一张哈希表，用父指针串成链，查找时沿链往外走。\
                 另一种常见做法是整个程序只用一张哈希表，每个名字对应一个\"声明栈\"：进入作用域时压栈，离开时弹栈，\
                 查找永远只看栈顶，是 O(1) 的。rustc 把作用域叫作 \"rib\"，名字解析是一个单独的阶段；\
                 一些函数式语言的实现则干脆把名字换成\"往外数第几层\"的编号（de Bruijn 下标），彻底消灭名字。",
            );
        });
        ui.add_space(24.0);
    }

    fn stepping(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        let Some(a) = &self.front.sema else { return };
        self.stepper.ui(ui, self.steps.len());
        let k = self.stepper.pos;
        let upto = if k == 0 { 0 } else { self.steps[k - 1] + 1 };
        let st = replay(a, upto);
        let last = k.checked_sub(1).map(|i| &a.events[self.steps[i]]);

        // 源码高亮
        let mut marks = Vec::new();
        let mut hl = Highlight::default();
        match last {
            Some(Event::EnterScope(s)) | Some(Event::ExitScope(s)) => {
                if let Some(n) = a.scopes[*s].node {
                    marks.push(Mark { span: self.front.parse.ast.node(n).span, bg: p.done_bg });
                }
            }
            Some(Event::Declare { sym, .. }) => {
                marks.push(Mark { span: a.symbols[*sym].span, bg: p.focus_bg });
                hl.declared = Some(*sym);
            }
            Some(Event::Lookup { node, name, searched, found }) => {
                if let Some(f) = found {
                    marks.push(Mark { span: a.symbols[*f].span, bg: p.ok.gamma_multiply(0.3) });
                }
                let sp = self.front.name_span(self.front.parse.ast.node(*node).span, name);
                marks.push(Mark { span: sp, bg: p.focus_bg });
                hl.searched = searched.clone();
                hl.found = *found;
            }
            _ => {}
        }
        card(ui, |ui| {
            source_view(ui, &self.front.src, &self.front.colors(&p), &marks, 17.0);
        });

        prose_sized(ui, &self.explain(a, last), 17.0);
        if let Some(Event::Error(i)) = last {
            diagnostic_view(ui, &self.front.src, &a.errors[*i]);
        }

        ui.label(RichText::new("作用域链（外 → 内）").strong());
        if st.active.is_empty() {
            card(ui, |ui| {
                let text = if k == 0 { "还没开始。" } else { "分析结束，所有作用域都已关闭。" };
                ui.label(RichText::new(text).color(p.muted));
            });
        } else {
            scope_box(ui, a, &st, &hl, 0);
        }

        if k == self.steps.len()
            && let Some(j) = self.summary(ui, a)
        {
            self.stepper.pos = j;
        }
    }

    fn explain(&self, a: &Analysis, last: Option<&Event>) -> String {
        let line = |sp: bsc_core::Span| line_col(&self.front.src, sp.start).line;
        match last {
            None => "从程序开头开始。按\"下一步\"或\"自动播放\"。".to_owned(),
            Some(Event::EnterScope(s)) => match &a.scopes[*s].kind {
                ScopeKind::Global => {
                    "进入全局作用域。先登记内置函数 `print`，再**第一遍**扫描：登记所有函数的签名。".to_owned()
                }
                ScopeKind::Function(name) => {
                    format!("开始检查函数 `{name}` 的函数体（第二遍）。先进入它的参数作用域，登记参数。")
                }
                ScopeKind::Block => {
                    let n = a.scopes[*s].node.map_or(0, |n| line(self.front.parse.ast.node(n).span));
                    format!("进入第 {n} 行的代码块，这是一层新的作用域。里面声明的名字，出了对应的 `}}` 就看不见了。")
                }
            },
            Some(Event::ExitScope(s)) => {
                let names: Vec<String> =
                    a.scopes[*s].symbols.iter().map(|&id| format!("`{}`", a.symbols[id].name)).collect();
                let what = a.scopes[*s].title();
                if names.is_empty() {
                    format!("离开{what}。")
                } else {
                    format!("离开{what}，其中的 {} 从此不可见。", names.join("、"))
                }
            }
            Some(Event::Declare { sym, shadows }) => {
                let s = &a.symbols[*sym];
                let scope = a.scopes[s.scope].title();
                let mut t = format!("在「{scope}」里登记：{}。", s.describe());
                if s.decl.is_none() {
                    t = format!("在「{scope}」里登记{}，它是语言自带的。", s.describe());
                } else if matches!(s.kind, SymbolKind::Function(_)) {
                    t.push_str("（第一遍只登记签名，函数体留到第二遍。）");
                }
                if let Some(old) = shadows {
                    let o = &a.symbols[*old];
                    let place = if o.decl.is_some() { format!("第 {} 行的", line(o.span)) } else { String::new() };
                    t.push_str(&format!("之前已经能看到一个{place}`{}`，从现在起它被新的这个**遮蔽**了。", o.name));
                }
                t
            }
            Some(Event::Lookup { name, searched, found, .. }) => {
                let path: Vec<String> = searched.iter().map(|&s| a.scopes[s].title()).collect();
                let path = path.join(" → ");
                match found {
                    Some(f) => {
                        let s = &a.symbols[*f];
                        let place = if s.decl.is_some() {
                            format!("（第 {} 行声明）", line(s.span))
                        } else {
                            String::new()
                        };
                        if searched.len() == 1 {
                            format!("查找 `{name}`：在当前作用域就找到了：{}{place}。", s.describe())
                        } else {
                            format!(
                                "查找 `{name}`：当前作用域没有，依次往外查 {path}，找到了：{}{place}。",
                                s.describe()
                            )
                        }
                    }
                    None => format!("查找 `{name}`：依次查了 {path}，一直到全局都没有！"),
                }
            }
            Some(Event::Error(_)) => "发现错误：".to_owned(),
            Some(Event::TypeOf { .. }) => String::new(),
        }
    }

    /// 播放完后：所有名字的使用 → 声明，点一下跳到那一步。
    fn summary(&self, ui: &mut Ui, a: &Analysis) -> Option<usize> {
        let p = Palette::of(ui);
        ui.add_space(6.0);
        ui.label(RichText::new("名字解析的结果：每个\"使用\"指向哪个\"声明\"（点击跳到那一步）").strong());
        let mut jump = None;
        card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                for (pos, &ei) in self.steps.iter().enumerate() {
                    let Event::Lookup { node, name, found, .. } = &a.events[ei] else { continue };
                    let at = line_col(&self.front.src, self.front.parse.ast.node(*node).span.start).line;
                    let (text, fill, stroke) = match found {
                        Some(f) => {
                            let s = &a.symbols[*f];
                            let to = if s.decl.is_some() {
                                format!("第 {} 行", line_col(&self.front.src, s.span.start).line)
                            } else {
                                "内置".to_owned()
                            };
                            (format!("第 {at} 行 {name} → {to}"), p.card_bg, Stroke::new(1.0, p.card_stroke))
                        }
                        None => (format!("第 {at} 行 {name} → 找不到"), p.error_bg, Stroke::new(1.0, p.error)),
                    };
                    if chip_button(ui, RichText::new(text).font(mono(14.0)), fill, stroke).clicked() {
                        jump = Some(pos + 1);
                    }
                }
            });
            let errs = a.errors.iter().filter(|d| d.code.is_some_and(|c| SCOPE_ERRORS.contains(&c))).count();
            let text = if errs == 0 {
                RichText::new("所有名字都找到了对应的声明。").color(p.ok)
            } else {
                RichText::new(format!("发现 {errs} 个名字相关的错误。")).color(p.error)
            };
            ui.label(text.strong());
        });
        jump
    }
}

#[derive(Default)]
struct Highlight {
    declared: Option<SymbolId>,
    searched: Vec<ScopeId>,
    found: Option<SymbolId>,
}

/// 画第 `level` 层作用域的方框，里面再套更内层的。
fn scope_box(ui: &mut Ui, a: &Analysis, st: &State, hl: &Highlight, level: usize) {
    let p = Palette::of(ui);
    let id = st.active[level];
    let scope = &a.scopes[id];
    let innermost = level + 1 == st.active.len();
    let searched = hl.searched.contains(&id);
    let found_here = hl.found.is_some_and(|f| a.symbols[f].scope == id);
    let (fill, stroke) = if found_here {
        (p.ok.gamma_multiply(0.12), Stroke::new(2.0, p.ok))
    } else if searched {
        (p.warn_bg, Stroke::new(2.0, p.paren))
    } else {
        (p.card_bg, Stroke::new(1.0, p.card_stroke))
    };
    Frame::new().fill(fill).stroke(stroke).corner_radius(CornerRadius::same(8)).inner_margin(Margin::same(10)).show(
        ui,
        |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new(scope.title()).strong());
                if innermost {
                    ui.label(RichText::new("← 当前作用域").small().color(p.accent));
                }
                if searched && !found_here {
                    ui.label(RichText::new("查过，没有").small().color(p.paren));
                } else if found_here {
                    ui.label(RichText::new("在这里找到").small().color(p.ok));
                }
            });
            let visible: Vec<SymbolId> = scope.symbols.iter().copied().filter(|&s| st.declared[s]).collect();
            if visible.is_empty() {
                ui.label(RichText::new("（还没有名字）").small().color(p.muted));
            } else {
                ui.horizontal_wrapped(|ui| {
                    for &s in &visible {
                        symbol_chip(ui, a, st, hl, s);
                    }
                });
            }
            if !innermost {
                ui.add_space(4.0);
                scope_box(ui, a, st, hl, level + 1);
            }
        },
    );
}

fn symbol_chip(ui: &mut Ui, a: &Analysis, st: &State, hl: &Highlight, s: SymbolId) {
    let p = Palette::of(ui);
    let sym = &a.symbols[s];
    // 被遮蔽：同一作用域里后面、或者更内层的作用域里，已经声明了同名的符号。
    let pos = st.active.iter().position(|&x| x == sym.scope).unwrap_or(0);
    let shadowed = st.active[pos..].iter().any(|&sc| {
        a.scopes[sc].symbols.iter().any(|&o| o != s && o > s && st.declared[o] && a.symbols[o].name == sym.name)
    });
    let mut text = RichText::new(sym.describe()).font(mono(14.0));
    let (fill, stroke) = if hl.declared == Some(s) {
        (p.focus_bg, Stroke::new(2.0, p.accent))
    } else if hl.found == Some(s) {
        (p.ok.gamma_multiply(0.25), Stroke::new(2.0, p.ok))
    } else {
        (p.node_fill, Stroke::new(1.0, p.node_stroke))
    };
    if shadowed {
        text = text.color(p.muted).strikethrough();
    }
    let r = chip(ui, text, fill, stroke);
    if shadowed {
        r.on_hover_text("被后面声明的同名变量遮蔽了，查找时不会再找到它");
    }
}
