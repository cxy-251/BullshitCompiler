//! mini-lang 的语义分析：名字解析（作用域与符号表）+ 类型检查。
//!
//! 语法分析只保证"句子结构对"，`let x = y + true;` 在语法上完全正确，但 `y` 可能根本没定义、
//! 数字加布尔值也没有意义。语义分析回答两个问题：
//!
//! 1. **这个名字指的是谁？** 每个代码块是一个作用域，作用域一层套一层。查找名字时从当前作用域开始，
//!    找不到就去外面一层找，一直找到最外层（全局）。找到的那个声明就是它指的东西。
//! 2. **类型对不对？** 按类型规则自底向上给每个表达式算出类型：`1 + 2` 是 int，`1 < 2` 是 bool；
//!    `1 + true` 违反规则，报错。
//!
//! 分析分两遍：先把所有函数的签名登记到全局作用域（这样函数可以调用定义在它后面的函数），
//! 再逐个检查函数体。

use bsc_core::{Diagnostic, Span};

use crate::ast::{Ast, BinOp, NodeId, NodeKind, UnOp};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Type {
    Int,
    Bool,
    /// 没有值（比如不返回值的函数、赋值表达式）。
    Unit,
    /// 出错的表达式的类型。它和任何类型都"兼容"，这样一个错误不会引出一串连锁错误。
    Error,
}

impl Type {
    pub fn name(self) -> &'static str {
        match self {
            Type::Int => "int",
            Type::Bool => "bool",
            Type::Unit => "unit",
            Type::Error => "?",
        }
    }

    fn from_name(name: &str) -> Option<Type> {
        match name {
            "int" => Some(Type::Int),
            "bool" => Some(Type::Bool),
            "unit" => Some(Type::Unit),
            _ => None,
        }
    }

    /// 两个类型是否匹配（Error 和谁都匹配）。
    fn compatible(self, other: Type) -> bool {
        self == other || self == Type::Error || other == Type::Error
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FnSig {
    pub params: Vec<Type>,
    pub ret: Type,
}

impl FnSig {
    pub fn text(&self) -> String {
        let ps: Vec<&str> = self.params.iter().map(|t| t.name()).collect();
        format!("({}) -> {}", ps.join(", "), self.ret.name())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SymbolKind {
    /// 内置函数（比如 print）。
    Builtin(FnSig),
    Function(FnSig),
    Param,
    Local {
        mutable: bool,
    },
}

pub type SymbolId = usize;
pub type ScopeId = usize;

#[derive(Clone, Debug)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    /// 变量的类型；函数是返回类型。
    pub ty: Type,
    /// 声明它的语法树节点（内置函数没有）。
    pub decl: Option<NodeId>,
    /// 声明处的源码位置（只含名字所在的那一段）。
    pub span: Span,
    pub scope: ScopeId,
}

impl Symbol {
    pub fn describe(&self) -> String {
        match &self.kind {
            SymbolKind::Builtin(sig) => format!("内置函数 {}{}", self.name, sig.text()),
            SymbolKind::Function(sig) => format!("函数 {}{}", self.name, sig.text()),
            SymbolKind::Param => format!("参数 {}: {}", self.name, self.ty.name()),
            SymbolKind::Local { mutable: true } => format!("可变变量 {}: {}", self.name, self.ty.name()),
            SymbolKind::Local { mutable: false } => format!("变量 {}: {}", self.name, self.ty.name()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScopeKind {
    Global,
    Function(String),
    Block,
}

#[derive(Clone, Debug)]
pub struct Scope {
    pub kind: ScopeKind,
    pub parent: Option<ScopeId>,
    /// 对应的语法树节点（全局作用域没有）。
    pub node: Option<NodeId>,
    pub symbols: Vec<SymbolId>,
}

impl Scope {
    pub fn title(&self) -> String {
        match &self.kind {
            ScopeKind::Global => "全局作用域".to_owned(),
            ScopeKind::Function(name) => format!("函数 {name} 的参数"),
            ScopeKind::Block => "代码块".to_owned(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    EnterScope(ScopeId),
    ExitScope(ScopeId),
    /// 在当前作用域登记一个名字；`shadows` 是被它遮住的外层同名符号。
    Declare {
        sym: SymbolId,
        shadows: Option<SymbolId>,
    },
    /// 查找名字：依次查过的作用域（从内到外），以及结果。
    Lookup {
        node: NodeId,
        name: String,
        searched: Vec<ScopeId>,
        found: Option<SymbolId>,
    },
    /// 算出了一个表达式的类型，以及依据。
    TypeOf {
        node: NodeId,
        ty: Type,
        why: String,
    },
    Error(usize),
}

#[derive(Clone, Debug)]
pub struct Analysis {
    pub scopes: Vec<Scope>,
    pub symbols: Vec<Symbol>,
    /// 每个表达式节点的类型。
    pub types: Vec<Option<Type>>,
    /// 每个使用名字的节点（变量、赋值、调用）解析到的符号。
    pub resolved: Vec<Option<SymbolId>>,
    /// 每个声明节点（函数、参数、let）对应的符号。
    pub declared: Vec<Option<SymbolId>>,
    pub events: Vec<Event>,
    pub errors: Vec<Diagnostic>,
}

pub fn analyze(src: &str, ast: &Ast, root: NodeId) -> Analysis {
    let n = ast.nodes.len();
    let mut s = Sema {
        src,
        ast,
        a: Analysis {
            scopes: Vec::new(),
            symbols: Vec::new(),
            types: vec![None; n],
            resolved: vec![None; n],
            declared: vec![None; n],
            events: Vec::new(),
            errors: Vec::new(),
        },
        current: 0,
        ret: Type::Unit,
    };
    s.program(root);
    s.a
}

struct Sema<'a> {
    src: &'a str,
    ast: &'a Ast,
    a: Analysis,
    current: ScopeId,
    /// 当前函数的返回类型。
    ret: Type,
}

impl Sema<'_> {
    fn node(&self, id: NodeId) -> &crate::ast::Node {
        self.ast.node(id)
    }

    fn error(&mut self, d: Diagnostic) {
        self.a.events.push(Event::Error(self.a.errors.len()));
        self.a.errors.push(d);
    }

    fn enter(&mut self, kind: ScopeKind, node: Option<NodeId>) {
        let parent = if self.a.scopes.is_empty() { None } else { Some(self.current) };
        self.a.scopes.push(Scope { kind, parent, node, symbols: Vec::new() });
        self.current = self.a.scopes.len() - 1;
        self.a.events.push(Event::EnterScope(self.current));
    }

    fn exit(&mut self) {
        self.a.events.push(Event::ExitScope(self.current));
        self.current = self.a.scopes[self.current].parent.unwrap_or(0);
    }

    /// 声明的名字在源码中的位置：节点范围里第一次出现这个名字的地方。
    fn name_span(&self, node: NodeId, name: &str) -> Span {
        let sp = self.node(node).span;
        let text = sp.text(self.src);
        match text.find(name) {
            Some(i) => Span::new(sp.start + i, sp.start + i + name.len()),
            None => sp,
        }
    }

    /// 在当前作用域登记名字。同一作用域里重名时报错（函数、参数）或允许遮蔽（let）。
    fn declare(&mut self, name: &str, kind: SymbolKind, ty: Type, decl: Option<NodeId>, span: Span) -> SymbolId {
        let shadows = self.lookup_quiet(name);
        let id = self.a.symbols.len();
        self.a.symbols.push(Symbol { name: name.to_owned(), kind, ty, decl, span, scope: self.current });
        self.a.scopes[self.current].symbols.push(id);
        if let Some(d) = decl {
            self.a.declared[d.index()] = Some(id);
        }
        self.a.events.push(Event::Declare { sym: id, shadows });
        id
    }

    fn lookup_quiet(&self, name: &str) -> Option<SymbolId> {
        let mut scope = Some(self.current);
        while let Some(s) = scope {
            // 同一作用域里后声明的优先（let 可以遮蔽同一块里前面的同名变量）
            if let Some(&sym) = self.a.scopes[s].symbols.iter().rev().find(|&&id| self.a.symbols[id].name == name) {
                return Some(sym);
            }
            scope = self.a.scopes[s].parent;
        }
        None
    }

    /// 解析名字：从当前作用域往外找，记录查过的每一层。
    fn lookup(&mut self, node: NodeId, name: &str) -> Option<SymbolId> {
        let mut searched = Vec::new();
        let mut found = None;
        let mut scope = Some(self.current);
        while let Some(s) = scope {
            searched.push(s);
            if let Some(&sym) = self.a.scopes[s].symbols.iter().rev().find(|&&id| self.a.symbols[id].name == name) {
                found = Some(sym);
                break;
            }
            scope = self.a.scopes[s].parent;
        }
        self.a.events.push(Event::Lookup { node, name: name.to_owned(), searched, found });
        self.a.resolved[node.index()] = found;
        if found.is_none() {
            let span = self.name_span(node, name);
            let mut d = Diagnostic::error(format!("找不到名字 `{name}`"))
                .with_code("E1202")
                .with_primary(span, "从这里往外层作用域一直找到全局，都没有这个名字");
            // 拼写相近的名字给个提示
            if let Some(similar) = self.similar_name(name) {
                d = d.with_help(format!("你是不是想写 `{similar}`？"));
            } else {
                d = d.with_help("使用变量之前要先用 `let` 声明");
            }
            self.error(d);
        }
        found
    }

    fn similar_name(&self, name: &str) -> Option<String> {
        let mut scope = Some(self.current);
        let mut best: Option<(usize, String)> = None;
        while let Some(s) = scope {
            for &id in &self.a.scopes[s].symbols {
                let cand = &self.a.symbols[id].name;
                let d = edit_distance(name, cand);
                if d <= 2 && d < name.chars().count() && best.as_ref().is_none_or(|(bd, _)| d < *bd) {
                    best = Some((d, cand.clone()));
                }
            }
            scope = self.a.scopes[s].parent;
        }
        best.map(|(_, n)| n)
    }

    fn set_type(&mut self, node: NodeId, ty: Type, why: String) -> Type {
        self.a.types[node.index()] = Some(ty);
        self.a.events.push(Event::TypeOf { node, ty, why });
        ty
    }

    fn type_node(&mut self, node: NodeId) -> Type {
        let NodeKind::Type { name } = &self.node(node).kind else { return Type::Error };
        match Type::from_name(name) {
            Some(t) => t,
            None => {
                let span = self.node(node).span;
                self.error(
                    Diagnostic::error(format!("不认识的类型 `{name}`"))
                        .with_code("E1201")
                        .with_primary(span, "")
                        .with_note("mini-lang 只有 int（整数）和 bool（布尔值）两种类型"),
                );
                Type::Error
            }
        }
    }

    // ---------------------------------------------------------------- 程序与函数

    fn program(&mut self, root: NodeId) {
        self.enter(ScopeKind::Global, None);
        self.declare(
            "print",
            SymbolKind::Builtin(FnSig { params: vec![Type::Int], ret: Type::Unit }),
            Type::Unit,
            None,
            Span::default(),
        );
        // 第一遍：登记所有函数的签名
        let funcs: Vec<NodeId> = self.node(root).children.clone();
        for &f in &funcs {
            let NodeKind::Function { name, has_ret } = self.node(f).kind.clone() else { continue };
            let kids = self.node(f).children.clone();
            let params: Vec<NodeId> =
                kids.iter().copied().filter(|&c| matches!(self.node(c).kind, NodeKind::Param { .. })).collect();
            let param_types: Vec<Type> = params.iter().map(|&p| self.type_node(self.node(p).children[0])).collect();
            let ret = if has_ret { self.type_node(kids[kids.len() - 2]) } else { Type::Unit };
            let span = self.name_span(f, &name);
            if let Some(prev) = self.a.scopes[0].symbols.iter().copied().find(|&id| self.a.symbols[id].name == name) {
                let prev_span = self.a.symbols[prev].span;
                let mut d = Diagnostic::error(format!("函数 `{name}` 定义了不止一次"))
                    .with_code("E1203")
                    .with_primary(span, "又定义了一次");
                d = if matches!(self.a.symbols[prev].kind, SymbolKind::Builtin(_)) {
                    d.with_note(format!("`{name}` 是内置函数的名字"))
                } else {
                    d.with_secondary(prev_span, "第一次定义在这里")
                };
                self.error(d);
                continue;
            }
            self.declare(&name, SymbolKind::Function(FnSig { params: param_types, ret }), ret, Some(f), span);
        }
        // 第二遍：检查每个函数体
        for &f in &funcs {
            if matches!(self.node(f).kind, NodeKind::Function { .. }) {
                self.function(f);
            }
        }
        self.exit();
    }

    fn function(&mut self, f: NodeId) {
        let NodeKind::Function { name, has_ret } = self.node(f).kind.clone() else { return };
        let kids = self.node(f).children.clone();
        let body = *kids.last().unwrap();
        self.ret = if has_ret { self.type_node(kids[kids.len() - 2]) } else { Type::Unit };
        self.enter(ScopeKind::Function(name.clone()), Some(f));
        let params: Vec<NodeId> =
            kids.iter().copied().filter(|&c| matches!(self.node(c).kind, NodeKind::Param { .. })).collect();
        for p in params {
            let NodeKind::Param { name: pname } = self.node(p).kind.clone() else { continue };
            let ty = self.type_node(self.node(p).children[0]);
            let span = self.name_span(p, &pname);
            if let Some(&dup) = self.a.scopes[self.current].symbols.iter().find(|&&id| self.a.symbols[id].name == pname)
            {
                let prev = self.a.symbols[dup].span;
                self.error(
                    Diagnostic::error(format!("参数 `{pname}` 重名了"))
                        .with_code("E1204")
                        .with_primary(span, "")
                        .with_secondary(prev, "前面已经有一个同名参数"),
                );
                continue;
            }
            self.declare(&pname, SymbolKind::Param, ty, Some(p), span);
        }
        self.block(body);
        if self.ret != Type::Unit && self.ret != Type::Error && !self.always_returns(body) {
            let close = self.node(body).span;
            let end = Span::new(close.end.saturating_sub(1), close.end);
            self.error(
                Diagnostic::error(format!("函数 `{name}` 可能没有返回值"))
                    .with_code("E1209")
                    .with_primary(end, format!("执行到这里还没有 return，但函数声明要返回 {}", self.ret.name()))
                    .with_help("在函数末尾加一条 return 语句；或者确保 if 和 else 两个分支都有 return"),
            );
        }
        self.exit();
    }

    /// 一段代码是否在所有路径上都会执行到 return。
    fn always_returns(&self, node: NodeId) -> bool {
        let n = self.node(node);
        match n.kind {
            NodeKind::Return => true,
            NodeKind::Block => n.children.iter().any(|&c| self.always_returns(c)),
            NodeKind::If => {
                n.children.len() == 3 && self.always_returns(n.children[1]) && self.always_returns(n.children[2])
            }
            _ => false,
        }
    }

    // ---------------------------------------------------------------- 语句

    fn block(&mut self, b: NodeId) {
        self.enter(ScopeKind::Block, Some(b));
        for s in self.node(b).children.clone() {
            self.statement(s);
        }
        self.exit();
    }

    fn statement(&mut self, s: NodeId) {
        let kids = self.node(s).children.clone();
        match self.node(s).kind.clone() {
            NodeKind::Let { name, mutable, has_type } => {
                // 先检查初值、再登记名字：`let x = x + 1;` 右边的 x 指的是外面那个 x。
                let init = *kids.last().unwrap();
                let init_ty = self.expr(init);
                let ty = if has_type {
                    let declared = self.type_node(kids[0]);
                    if !declared.compatible(init_ty) {
                        let sp = self.node(init).span;
                        self.error(
                            Diagnostic::error("初值的类型和声明的不一样")
                                .with_code("E1206")
                                .with_primary(sp, format!("这是 {}", init_ty.name()))
                                .with_secondary(self.node(kids[0]).span, format!("声明的是 {}", declared.name())),
                        );
                    }
                    declared
                } else {
                    if init_ty == Type::Unit {
                        let sp = self.node(init).span;
                        self.error(
                            Diagnostic::error("这个表达式没有值，不能用来给变量赋初值")
                                .with_code("E1206")
                                .with_primary(sp, "它的类型是 unit（比如不返回值的函数调用）"),
                        );
                    }
                    init_ty
                };
                let span = self.name_span(s, &name);
                self.declare(&name, SymbolKind::Local { mutable }, ty, Some(s), span);
            }
            NodeKind::If => {
                self.condition(kids[0], "if");
                self.block(kids[1]);
                if let Some(&e) = kids.get(2) {
                    if matches!(self.node(e).kind, NodeKind::If) { self.statement(e) } else { self.block(e) }
                }
            }
            NodeKind::While => {
                self.condition(kids[0], "while");
                self.block(kids[1]);
            }
            NodeKind::Return => {
                let span = self.node(s).span;
                match kids.first() {
                    Some(&v) => {
                        let t = self.expr(v);
                        if self.ret == Type::Unit && t != Type::Error {
                            self.error(
                                Diagnostic::error("这个函数没有声明返回类型，却返回了一个值")
                                    .with_code("E1208")
                                    .with_primary(self.node(v).span, format!("这是 {}", t.name()))
                                    .with_help(format!("在函数的参数列表后面写上 `-> {}`", t.name())),
                            );
                        } else if !t.compatible(self.ret) {
                            self.error(Diagnostic::error("返回值的类型不对").with_code("E1208").with_primary(
                                self.node(v).span,
                                format!("这是 {}，函数声明要返回 {}", t.name(), self.ret.name()),
                            ));
                        }
                    }
                    None if self.ret != Type::Unit && self.ret != Type::Error => {
                        self.error(
                            Diagnostic::error("return 后面缺少返回值")
                                .with_code("E1208")
                                .with_primary(span, format!("函数声明要返回 {}", self.ret.name())),
                        );
                    }
                    None => {}
                }
            }
            NodeKind::ExprStmt => {
                self.expr(kids[0]);
            }
            NodeKind::Block => self.block(s),
            _ => {}
        }
    }

    fn condition(&mut self, c: NodeId, what: &str) {
        let t = self.expr(c);
        if !t.compatible(Type::Bool) {
            let span = self.node(c).span;
            let mut d = Diagnostic::error(format!("{what} 的条件必须是 bool"))
                .with_code("E1207")
                .with_primary(span, format!("这是 {}", t.name()));
            if t == Type::Int {
                d = d.with_help("整数不会自动当成真假，写成比较的形式，比如 `x != 0`");
            }
            self.error(d);
        }
    }

    // ---------------------------------------------------------------- 表达式

    fn expr(&mut self, e: NodeId) -> Type {
        let kids = self.node(e).children.clone();
        match self.node(e).kind.clone() {
            NodeKind::Int(n) => self.set_type(e, Type::Int, format!("整数字面量 {n} 的类型是 int")),
            NodeKind::Bool(b) => self.set_type(e, Type::Bool, format!("{b} 是 bool")),
            NodeKind::Var(name) => {
                let ty = match self.lookup(e, &name) {
                    None => Type::Error,
                    Some(sym) => match &self.a.symbols[sym].kind {
                        SymbolKind::Function(_) | SymbolKind::Builtin(_) => {
                            let sp = self.node(e).span;
                            self.error(
                                Diagnostic::error(format!("`{name}` 是函数，不能当作值使用"))
                                    .with_code("E1210")
                                    .with_primary(sp, "")
                                    .with_help(format!("调用它要写成 `{name}(…)`")),
                            );
                            Type::Error
                        }
                        _ => self.a.symbols[sym].ty,
                    },
                };
                let why = match self.a.resolved[e.index()] {
                    Some(sym) if ty != Type::Error => {
                        format!("`{name}` 指向 {}，所以类型是 {}", self.a.symbols[sym].describe(), ty.name())
                    }
                    _ => format!("`{name}` 出错了，类型未知"),
                };
                self.set_type(e, ty, why)
            }
            NodeKind::Assign { name } => {
                let vt = self.expr(kids[0]);
                if let Some(sym) = self.lookup(e, &name) {
                    let s = self.a.symbols[sym].clone();
                    match s.kind {
                        SymbolKind::Local { mutable: true } => {
                            if !vt.compatible(s.ty) {
                                self.error(
                                    Diagnostic::error("赋值的类型不对")
                                        .with_code("E1206")
                                        .with_primary(self.node(kids[0]).span, format!("这是 {}", vt.name()))
                                        .with_secondary(s.span, format!("`{name}` 是 {}", s.ty.name())),
                                );
                            }
                        }
                        SymbolKind::Local { mutable: false } | SymbolKind::Param => {
                            let sp = self.name_span(e, &name);
                            let what = if s.kind == SymbolKind::Param { "参数" } else { "变量" };
                            let mut d = Diagnostic::error(format!("不能给不可变的{what} `{name}` 赋值"))
                                .with_code("E1205")
                                .with_primary(sp, "")
                                .with_secondary(s.span, "在这里声明");
                            if s.kind != SymbolKind::Param {
                                d = d.with_help(format!("声明时写成 `let mut {name}`，它才能被修改"));
                            }
                            self.error(d);
                        }
                        _ => {
                            let sp = self.name_span(e, &name);
                            self.error(
                                Diagnostic::error(format!("`{name}` 是函数，不能赋值"))
                                    .with_code("E1205")
                                    .with_primary(sp, ""),
                            );
                        }
                    }
                }
                self.set_type(e, Type::Unit, "赋值表达式本身没有值，类型是 unit".to_owned())
            }
            NodeKind::Unary(op) => {
                let t = self.expr(kids[0]);
                let (want, name) = match op {
                    UnOp::Neg => (Type::Int, "取负"),
                    UnOp::Not => (Type::Bool, "取反 !"),
                };
                if !t.compatible(want) {
                    self.error(
                        Diagnostic::error(format!("{name}只能用在 {} 上", want.name()))
                            .with_code("E1211")
                            .with_primary(self.node(kids[0]).span, format!("这是 {}", t.name())),
                    );
                    return self.set_type(e, Type::Error, "出错了".to_owned());
                }
                let res = if t == Type::Error { Type::Error } else { want };
                self.set_type(e, res, format!("{name}：{} → {}", want.name(), want.name()))
            }
            NodeKind::Binary(op) => {
                let (l, r) = (self.expr(kids[0]), self.expr(kids[1]));
                self.binary(e, op, l, r)
            }
            NodeKind::Call { name } => {
                let arg_types: Vec<Type> = kids.iter().map(|&a| self.expr(a)).collect();
                let Some(sym) = self.lookup(e, &name) else {
                    return self.set_type(e, Type::Error, "调用了不存在的函数".to_owned());
                };
                let sig = match &self.a.symbols[sym].kind {
                    SymbolKind::Function(sig) | SymbolKind::Builtin(sig) => sig.clone(),
                    _ => {
                        let sp = self.name_span(e, &name);
                        self.error(
                            Diagnostic::error(format!("`{name}` 不是函数，不能调用"))
                                .with_code("E1212")
                                .with_primary(sp, "")
                                .with_secondary(
                                    self.a.symbols[sym].span,
                                    format!("它是{}", self.a.symbols[sym].describe()),
                                ),
                        );
                        return self.set_type(e, Type::Error, "出错了".to_owned());
                    }
                };
                if sig.params.len() != arg_types.len() {
                    let sp = self.node(e).span;
                    self.error(
                        Diagnostic::error(format!(
                            "函数 `{name}` 需要 {} 个参数，这里给了 {} 个",
                            sig.params.len(),
                            arg_types.len()
                        ))
                        .with_code("E1213")
                        .with_primary(sp, "")
                        .with_note(format!("{name} 的签名是 {}", sig.text())),
                    );
                } else {
                    for (i, (&want, &got)) in sig.params.iter().zip(&arg_types).enumerate() {
                        if !got.compatible(want) {
                            self.error(
                                Diagnostic::error(format!("第 {} 个参数的类型不对", i + 1))
                                    .with_code("E1213")
                                    .with_primary(
                                        self.node(kids[i]).span,
                                        format!("这是 {}，需要 {}", got.name(), want.name()),
                                    ),
                            );
                        }
                    }
                }
                self.set_type(e, sig.ret, format!("调用 {name}{}，结果是返回类型 {}", sig.text(), sig.ret.name()))
            }
            _ => Type::Error,
        }
    }

    fn binary(&mut self, e: NodeId, op: BinOp, l: Type, r: Type) -> Type {
        use BinOp::*;
        let sym = op.symbol();
        let (operand, result, rule) = match op {
            Add | Sub | Mul | Div | Rem => (Some(Type::Int), Type::Int, format!("`{sym}`：int {sym} int → int")),
            Lt | Le | Gt | Ge => (Some(Type::Int), Type::Bool, format!("`{sym}`：int {sym} int → bool")),
            And | Or => (Some(Type::Bool), Type::Bool, format!("`{sym}`：bool {sym} bool → bool")),
            Eq | Ne => (None, Type::Bool, format!("`{sym}`：两边类型相同 → bool")),
        };
        if l == Type::Error || r == Type::Error {
            return self.set_type(e, Type::Error, "操作数出错了，结果类型未知（不再重复报错）".to_owned());
        }
        let ok = match operand {
            Some(t) => l == t && r == t,
            None => l == r && l != Type::Unit,
        };
        if !ok {
            let mut d = Diagnostic::error(format!("`{sym}` 两边的类型不对"))
                .with_code("E1214")
                .with_primary(self.node(e).span, format!("左边是 {}，右边是 {}", l.name(), r.name()));
            d = match operand {
                Some(t) => d.with_note(format!("`{sym}` 要求两边都是 {}", t.name())),
                None => d.with_note(format!("`{sym}` 要求两边类型相同")),
            };
            self.error(d);
            return self.set_type(e, Type::Error, "类型不匹配".to_owned());
        }
        self.set_type(e, result, rule)
    }
}

/// 编辑距离（用来猜"你是不是想写……"）。
fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut cur = vec![i; b.len() + 1];
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
        }
        prev = cur;
    }
    prev[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;
    use crate::parser::parse_program;

    fn check(src: &str) -> Vec<&'static str> {
        let toks = lex(src).tokens;
        let p = parse_program(src, &toks);
        assert!(p.errors.is_empty(), "{:?}", p.errors);
        analyze(src, &p.ast, p.root).errors.iter().map(|d| d.code.unwrap()).collect()
    }

    #[test]
    fn well_typed_program() {
        let src = "fn main() {\n  let n = 10;\n  print(sum(n));\n}\n\
                   fn sum(n: int) -> int {\n  let mut s = 0;\n  let mut i = 1;\n  while i <= n { s = s + i; i = i + 1; }\n  \
                   if s > 100 { return 100; } else { return s; }\n}";
        assert_eq!(check(src), Vec::<&str>::new());
    }

    #[test]
    fn common_mistakes() {
        assert_eq!(check("fn f() { let x = 1; x = 2; }"), ["E1205"]);
        assert_eq!(check("fn f() { print(y); }"), ["E1202"]);
        assert_eq!(check("fn f() { let x = 1 + true; }"), ["E1214"]);
        assert_eq!(check("fn f() { if 1 { } }"), ["E1207"]);
        assert_eq!(check("fn f() -> int { let x = 1; }"), ["E1209"]);
        assert_eq!(check("fn f(a: int) -> int { return f(a, a); }"), ["E1213"]);
        // 一个错误不引出连锁错误：y 未定义，y + 1 和 (y + 1) * 2 不再重复报错
        assert_eq!(check("fn f() { let z = (y + 1) * 2 + true; }"), ["E1202"]);
    }

    #[test]
    fn scopes_and_shadowing() {
        let src = "fn f() {\n  let x = 1;\n  {\n    let x = true;\n    if x { }\n  }\n  let y = x + 1;\n}";
        assert_eq!(check(src), Vec::<&str>::new());
        // 内层块里的变量出了块就看不见了
        assert_eq!(check("fn f() { { let t = 1; } print(t); }"), ["E1202"]);
        // 拼写提示
        let toks = lex("fn f() { let count = 1; print(cout); }").tokens;
        let p = parse_program("fn f() { let count = 1; print(cout); }", &toks);
        let a = analyze("fn f() { let count = 1; print(cout); }", &p.ast, p.root);
        assert!(a.errors[0].help.as_ref().unwrap().contains("count"));
    }
}
