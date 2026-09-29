//! 中间表示：三地址码。
//!
//! 语法树是嵌套的，机器指令是一条一条的。三地址码介于两者之间：每条指令最多一个运算、
//! 最多三个"地址"（两个操作数 + 一个结果），嵌套的表达式拆成一串临时变量：
//!
//! ```text
//! y = (a + b) * c;      ⇒      t1 = a + b
//!                              y = t1 * c
//! ```
//!
//! `if`、`while` 和短路求值的 `&&` `||` 变成标签和跳转。这一步叫"降级"（lowering）。

use bsc_core::Span;

use crate::ast::{Ast, BinOp, NodeId, NodeKind, UnOp};
use crate::sema::{Analysis, SymbolId, SymbolKind, Type};

pub type VarId = usize;
pub type Label = usize;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Var {
    /// 显示用的名字：用户变量用原名（重名的加 `'`），临时变量是 t1、t2……，SSA 版本是 x.1、x.2……
    pub name: String,
    pub temp: bool,
    /// SSA 形式里：它是哪个原变量的第几个版本。
    pub ssa: Option<(VarId, u32)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operand {
    Var(VarId),
    Const(i64),
    /// 没有定义过的值（SSA 构造时某条路径上变量还没赋值）。
    Undef,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Inst {
    /// 从调用者那里取第 `index` 个参数。
    Param {
        dst: VarId,
        index: usize,
    },
    Copy {
        dst: VarId,
        src: Operand,
    },
    Bin {
        dst: VarId,
        op: BinOp,
        a: Operand,
        b: Operand,
    },
    Un {
        dst: VarId,
        op: UnOp,
        a: Operand,
    },
    Call {
        dst: Option<VarId>,
        func: String,
        args: Vec<Operand>,
    },
    /// SSA 的 φ 函数：从哪个前驱块来，就取哪个值。`args` 按前驱块的顺序排列。
    Phi {
        dst: VarId,
        args: Vec<(usize, Operand)>,
    },
    Label(Label),
    Jump(Label),
    Branch {
        cond: Operand,
        then: Label,
        els: Label,
    },
    Return(Option<Operand>),
}

impl Inst {
    /// 是否结束一个基本块（跳转、条件跳转、返回）。
    pub fn is_terminator(&self) -> bool {
        matches!(self, Inst::Jump(_) | Inst::Branch { .. } | Inst::Return(_))
    }

    /// 这条指令定义（赋值）的变量。
    pub fn def(&self) -> Option<VarId> {
        match self {
            Inst::Param { dst, .. }
            | Inst::Copy { dst, .. }
            | Inst::Bin { dst, .. }
            | Inst::Un { dst, .. }
            | Inst::Phi { dst, .. } => Some(*dst),
            Inst::Call { dst, .. } => *dst,
            _ => None,
        }
    }

    pub fn def_mut(&mut self) -> Option<&mut VarId> {
        match self {
            Inst::Param { dst, .. }
            | Inst::Copy { dst, .. }
            | Inst::Bin { dst, .. }
            | Inst::Un { dst, .. }
            | Inst::Phi { dst, .. } => Some(dst),
            Inst::Call { dst, .. } => dst.as_mut(),
            _ => None,
        }
    }

    /// 这条指令读取的操作数（φ 的参数除外）。
    pub fn uses_mut(&mut self) -> Vec<&mut Operand> {
        match self {
            Inst::Copy { src, .. } => vec![src],
            Inst::Bin { a, b, .. } => vec![a, b],
            Inst::Un { a, .. } => vec![a],
            Inst::Call { args, .. } => args.iter_mut().collect(),
            Inst::Branch { cond, .. } => vec![cond],
            Inst::Return(Some(v)) => vec![v],
            _ => vec![],
        }
    }

    pub fn uses(&self) -> Vec<Operand> {
        self.clone().uses_mut().into_iter().map(|o| *o).collect()
    }

    /// 跳转目标。
    pub fn targets(&self) -> Vec<Label> {
        match self {
            Inst::Jump(l) => vec![*l],
            Inst::Branch { then, els, .. } => vec![*then, *els],
            _ => vec![],
        }
    }
}

/// 一个函数的三地址码。
#[derive(Clone, Debug)]
pub struct Function {
    pub name: String,
    pub params: usize,
    pub vars: Vec<Var>,
    pub code: Vec<Inst>,
    /// 每条指令来自源码的哪一段。
    pub origin: Vec<Span>,
    pub labels: usize,
}

impl Function {
    pub fn operand(&self, o: Operand) -> String {
        match o {
            Operand::Var(v) => self.vars[v].name.clone(),
            Operand::Const(c) => c.to_string(),
            Operand::Undef => "未定义".to_owned(),
        }
    }

    /// 指令的文字形式。`target` 决定跳转目标怎么写（线性代码里写标签，控制流图里写块名）。
    pub fn inst_text(&self, i: &Inst, target: &dyn Fn(Label) -> String, block: &dyn Fn(usize) -> String) -> String {
        let v = |v: VarId| self.vars[v].name.clone();
        let o = |x: Operand| self.operand(x);
        match i {
            Inst::Param { dst, index } => format!("{} = param {index}", v(*dst)),
            Inst::Copy { dst, src } => format!("{} = {}", v(*dst), o(*src)),
            Inst::Bin { dst, op, a, b } => format!("{} = {} {} {}", v(*dst), o(*a), op.symbol(), o(*b)),
            Inst::Un { dst, op, a } => format!("{} = {}{}", v(*dst), op.symbol(), o(*a)),
            Inst::Call { dst, func, args } => {
                let a: Vec<String> = args.iter().map(|x| o(*x)).collect();
                match dst {
                    Some(d) => format!("{} = call {func}({})", v(*d), a.join(", ")),
                    None => format!("call {func}({})", a.join(", ")),
                }
            }
            Inst::Phi { dst, args } => {
                let a: Vec<String> = args.iter().map(|(b, x)| format!("{}: {}", block(*b), o(*x))).collect();
                format!("{} = φ({})", v(*dst), a.join(", "))
            }
            Inst::Label(l) => format!("{}:", target(*l)),
            Inst::Jump(l) => format!("goto {}", target(*l)),
            Inst::Branch { cond, then, els } => {
                format!("if {} goto {} else goto {}", o(*cond), target(*then), target(*els))
            }
            Inst::Return(Some(x)) => format!("return {}", o(*x)),
            Inst::Return(None) => "return".to_owned(),
        }
    }

    /// 线性代码里的写法：标签写成 L1、L2……
    pub fn linear_text(&self, i: usize) -> String {
        self.inst_text(&self.code[i], &|l| format!("L{l}"), &|b| format!("B{b}"))
    }
}

/// 生成一条指令时的记录（4.1 课回放用）。
#[derive(Clone, Debug)]
pub struct Emit {
    pub func: usize,
    pub inst: usize,
    /// 产生这条指令的语法树节点。
    pub node: NodeId,
    pub note: String,
}

#[derive(Clone, Debug)]
pub struct Lowered {
    pub funcs: Vec<Function>,
    pub events: Vec<Emit>,
}

/// 把通过了语义分析的程序降级成三地址码。调用前必须确认没有任何错误。
pub fn lower(src: &str, ast: &Ast, root: NodeId, sema: &Analysis) -> Lowered {
    let mut out = Lowered { funcs: Vec::new(), events: Vec::new() };
    for &f in &ast.node(root).children {
        if let NodeKind::Function { name, .. } = &ast.node(f).kind {
            let mut l = Lower {
                src,
                ast,
                sema,
                func: Function {
                    name: name.clone(),
                    params: 0,
                    vars: Vec::new(),
                    code: Vec::new(),
                    origin: Vec::new(),
                    labels: 0,
                },
                vars: Vec::new(),
                alias: Vec::new(),
                events: Vec::new(),
                index: out.funcs.len(),
            };
            l.function(f);
            out.events.extend(l.events);
            out.funcs.push(l.func);
        }
    }
    out
}

struct Lower<'a> {
    src: &'a str,
    ast: &'a Ast,
    sema: &'a Analysis,
    func: Function,
    /// 符号 → 变量。
    vars: Vec<(SymbolId, VarId)>,
    /// 紧挨着的两个标签合并成一个：`alias[l]` 是 l 实际使用的标签。
    alias: Vec<Label>,
    events: Vec<Emit>,
    index: usize,
}

impl Lower<'_> {
    fn text(&self, n: NodeId) -> &str {
        self.ast.node(n).span.text(self.src)
    }

    fn emit(&mut self, inst: Inst, node: NodeId, note: String) {
        self.events.push(Emit { func: self.index, inst: self.func.code.len(), node, note });
        self.func.code.push(inst);
        self.func.origin.push(self.ast.node(node).span);
    }

    fn terminated(&self) -> bool {
        self.func.code.last().is_some_and(Inst::is_terminator)
    }

    fn new_label(&mut self) -> Label {
        self.alias.push(self.alias.len());
        self.alias.len() - 1
    }

    fn place(&mut self, l: Label, node: NodeId, what: &str) {
        // 紧跟在另一个标签后面的标签没有必要单独存在，直接合并
        if let Some(Inst::Label(prev)) = self.func.code.last() {
            self.alias[l] = *prev;
            return;
        }
        self.emit(Inst::Label(l), node, format!("放下标签：{what}"));
    }

    fn new_var(&mut self, name: &str, temp: bool) -> VarId {
        let mut n = name.to_owned();
        while self.func.vars.iter().any(|v| v.name == n) {
            n.push('\'');
        }
        self.func.vars.push(Var { name: n, temp, ssa: None });
        self.func.vars.len() - 1
    }

    fn temp(&mut self) -> VarId {
        let mut k = self.func.vars.iter().filter(|v| v.temp).count() + 1;
        while self.func.vars.iter().any(|v| v.name == format!("t{k}")) {
            k += 1;
        }
        self.new_var(&format!("t{k}"), true)
    }

    fn var_of(&self, sym: SymbolId) -> VarId {
        self.vars.iter().find(|(s, _)| *s == sym).map(|(_, v)| *v).expect("变量在使用前一定声明过")
    }

    fn declare(&mut self, decl: NodeId) -> VarId {
        let sym = self.sema.declared[decl.index()].expect("声明节点有符号");
        let name = self.sema.symbols[sym].name.clone();
        let v = self.new_var(&name, false);
        self.vars.push((sym, v));
        v
    }

    fn function(&mut self, f: NodeId) {
        let kids = self.ast.node(f).children.clone();
        let body = *kids.last().unwrap();
        for &p in &kids {
            if let NodeKind::Param { name } = &self.ast.node(p).kind {
                let name = name.clone();
                let v = self.declare(p);
                let index = self.func.params;
                self.func.params += 1;
                self.emit(
                    Inst::Param { dst: v, index },
                    p,
                    format!("参数 `{name}` 的值由调用者传进来（第 {index} 个）"),
                );
            }
        }
        self.block(body);
        if !self.terminated() {
            let what = if self.func.code.is_empty() { "函数是空的" } else { "执行到函数末尾" };
            self.emit(Inst::Return(None), body, format!("{what}，返回"));
        }
        // 把合并掉的标签换成实际的标签，再按出现顺序重新编号成 L1、L2……
        let resolve = |alias: &[Label], mut l: Label| {
            while alias[l] != l {
                l = alias[l];
            }
            l
        };
        let mut order = vec![usize::MAX; self.alias.len()];
        let mut next = 1;
        for i in &self.func.code {
            if let Inst::Label(l) = i {
                order[*l] = next;
                next += 1;
            }
        }
        let alias = self.alias.clone();
        for i in &mut self.func.code {
            match i {
                Inst::Label(l) | Inst::Jump(l) => *l = order[resolve(&alias, *l)],
                Inst::Branch { then, els, .. } => {
                    *then = order[resolve(&alias, *then)];
                    *els = order[resolve(&alias, *els)];
                }
                _ => {}
            }
        }
        self.func.labels = next;
    }

    fn block(&mut self, b: NodeId) {
        for s in self.ast.node(b).children.clone() {
            self.statement(s);
        }
    }

    fn statement(&mut self, s: NodeId) {
        let kids = self.ast.node(s).children.clone();
        match self.ast.node(s).kind.clone() {
            NodeKind::Let { .. } => {
                // 初值里的同名名字（`let x = x + 1` 右边的 x）解析到的是旧符号，和新变量互不干扰
                let v = self.declare(s);
                self.value_into(*kids.last().unwrap(), v, s);
            }
            NodeKind::ExprStmt => {
                self.expr(kids[0], None);
            }
            NodeKind::Return => {
                let v = kids.first().map(|&e| self.expr(e, None));
                let note = match v {
                    Some(v) => format!("返回 {}", self.func.operand(v)),
                    None => "返回".to_owned(),
                };
                self.emit(Inst::Return(v), s, note);
            }
            NodeKind::If => {
                let c = self.expr(kids[0], None);
                let then_l = self.new_label();
                let end_l = self.new_label();
                let else_l = if kids.len() == 3 { self.new_label() } else { end_l };
                let cond_text = self.text(kids[0]).to_owned();
                self.emit(
                    Inst::Branch { cond: c, then: then_l, els: else_l },
                    s,
                    format!(
                        "if：条件 `{cond_text}` 成立就跳到 then 分支，否则跳到{}",
                        if kids.len() == 3 { " else 分支" } else { " if 语句之后" }
                    ),
                );
                self.place(then_l, kids[1], "then 分支的开头");
                self.block(kids[1]);
                if kids.len() == 3 {
                    if !self.terminated() {
                        self.emit(Inst::Jump(end_l), s, "then 分支执行完，跳过 else 分支".to_owned());
                    }
                    self.place(else_l, kids[2], "else 分支的开头");
                    if matches!(self.ast.node(kids[2]).kind, NodeKind::If) {
                        self.statement(kids[2]);
                    } else {
                        self.block(kids[2]);
                    }
                }
                self.place(end_l, s, "if 语句结束后的汇合点");
            }
            NodeKind::While => {
                let cond_l = self.new_label();
                let body_l = self.new_label();
                let end_l = self.new_label();
                self.place(cond_l, kids[0], "每一轮循环从这里检查条件");
                let c = self.expr(kids[0], None);
                let cond_text = self.text(kids[0]).to_owned();
                self.emit(
                    Inst::Branch { cond: c, then: body_l, els: end_l },
                    s,
                    format!("while：条件 `{cond_text}` 成立就进入循环体，否则跳出循环"),
                );
                self.place(body_l, kids[1], "循环体的开头");
                self.block(kids[1]);
                if !self.terminated() {
                    self.emit(Inst::Jump(cond_l), s, "循环体执行完，跳回去再检查条件".to_owned());
                }
                self.place(end_l, s, "循环结束后从这里继续");
            }
            NodeKind::Block => self.block(s),
            _ => {}
        }
    }

    /// 把表达式的值放进变量 `dst`：能直接算进去的就直接算，否则补一条复制。
    fn value_into(&mut self, e: NodeId, dst: VarId, stmt: NodeId) {
        let o = self.expr(e, Some(dst));
        if o != Operand::Var(dst) {
            let note = format!("把 {} 复制给 {}", self.func.operand(o), self.func.vars[dst].name);
            self.emit(Inst::Copy { dst, src: o }, stmt, note);
        }
    }

    /// 结果放在 `dst` 里（给了的话）或新的临时变量里。
    fn target(&mut self, dst: Option<VarId>) -> (VarId, String) {
        match dst {
            Some(d) => (d, format!("直接写进 {}", self.func.vars[d].name)),
            None => {
                let t = self.temp();
                (t, format!("放进新的临时变量 {}", self.func.vars[t].name))
            }
        }
    }

    /// 生成计算表达式的指令，返回代表结果的操作数。
    fn expr(&mut self, e: NodeId, dst: Option<VarId>) -> Operand {
        let kids = self.ast.node(e).children.clone();
        match self.ast.node(e).kind.clone() {
            NodeKind::Int(n) => Operand::Const(n),
            NodeKind::Bool(b) => Operand::Const(i64::from(b)),
            NodeKind::Var(_) => {
                let sym = self.sema.resolved[e.index()].expect("名字已解析");
                Operand::Var(self.var_of(sym))
            }
            NodeKind::Binary(op @ (BinOp::And | BinOp::Or)) => {
                // 短路求值：左边已经能决定结果时，右边根本不算
                let (d, _) = self.target(dst);
                self.value_into(kids[0], d, e);
                let rhs = self.new_label();
                let end = self.new_label();
                let dn = self.func.vars[d].name.clone();
                let (inst, note) = if op == BinOp::And {
                    (
                        Inst::Branch { cond: Operand::Var(d), then: rhs, els: end },
                        format!("`&&` 短路：{dn} 为假时结果就是假，跳过右边"),
                    )
                } else {
                    (
                        Inst::Branch { cond: Operand::Var(d), then: end, els: rhs },
                        format!("`||` 短路：{dn} 为真时结果就是真，跳过右边"),
                    )
                };
                self.emit(inst, e, note);
                self.place(rhs, kids[1], "计算右边的操作数");
                self.value_into(kids[1], d, e);
                self.place(end, e, "短路求值结束");
                Operand::Var(d)
            }
            NodeKind::Binary(op) => {
                let a = self.expr(kids[0], None);
                let b = self.expr(kids[1], None);
                let (d, where_) = self.target(dst);
                let text = self.text(e).to_owned();
                self.emit(Inst::Bin { dst: d, op, a, b }, e, format!("计算 `{text}`，结果{where_}"));
                Operand::Var(d)
            }
            NodeKind::Unary(op) => {
                let a = self.expr(kids[0], None);
                let (d, where_) = self.target(dst);
                let text = self.text(e).to_owned();
                self.emit(Inst::Un { dst: d, op, a }, e, format!("计算 `{text}`，结果{where_}"));
                Operand::Var(d)
            }
            NodeKind::Assign { .. } => {
                let sym = self.sema.resolved[e.index()].expect("名字已解析");
                let v = self.var_of(sym);
                self.value_into(kids[0], v, e);
                Operand::Undef
            }
            NodeKind::Call { name } => {
                let args: Vec<Operand> = kids.iter().map(|&a| self.expr(a, None)).collect();
                let returns = self.sema.resolved[e.index()].is_some_and(|s| match &self.sema.symbols[s].kind {
                    SymbolKind::Function(sig) | SymbolKind::Builtin(sig) => sig.ret != Type::Unit,
                    _ => false,
                });
                if returns {
                    let (d, where_) = self.target(dst);
                    self.emit(
                        Inst::Call { dst: Some(d), func: name.clone(), args },
                        e,
                        format!("调用 {name}，返回值{where_}"),
                    );
                    Operand::Var(d)
                } else {
                    self.emit(
                        Inst::Call { dst: None, func: name.clone(), args },
                        e,
                        format!("调用 {name}（没有返回值）"),
                    );
                    Operand::Undef
                }
            }
            _ => Operand::Undef,
        }
    }
}
