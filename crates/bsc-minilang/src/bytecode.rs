//! 栈式虚拟机：字节码编译器 + 解释执行。
//!
//! 和第 0 课的计算器一样，操作数放在一个栈上：`Push 2`、`Push 3`、`Add` 弹出两个数、压入 5。
//! mini-lang 多了变量和函数，所以再加上：
//!
//! - **局部变量槽**：每个函数调用有一组编号的槽，`Load k` / `Store k` 读写第 k 个槽（参数占前几个槽）；
//! - **跳转**：`Jump` 和 `JumpIfFalse` 实现 if、while、短路求值；
//! - **调用帧**：`Call` 新建一帧（参数从栈上搬进新帧的槽），`Ret` 销毁当前帧、把返回值留在调用者的栈上。
//!
//! JVM、CPython、WebAssembly 都是这种栈式设计。

use bsc_core::Span;

use crate::ast::{Ast, BinOp, NodeId, NodeKind, UnOp};
use crate::interp::binop;
use crate::sema::{Analysis, SymbolId, SymbolKind, Type};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Push(i64),
    Load(usize),
    Store(usize),
    Bin(BinOp),
    Un(UnOp),
    Jump(usize),
    JumpIfFalse(usize),
    /// 调用第几个函数、几个参数。
    Call(usize, usize),
    Print,
    /// 返回栈顶的值。
    Ret,
    /// 不带值返回。
    RetVoid,
    /// 丢掉栈顶（表达式语句的值没人用）。
    Pop,
}

impl Op {
    pub fn text(&self, p: &Program) -> String {
        match self {
            Op::Push(n) => format!("push {n}"),
            Op::Load(k) => format!("load {k}"),
            Op::Store(k) => format!("store {k}"),
            Op::Bin(op) => match op {
                BinOp::Add => "add",
                BinOp::Sub => "sub",
                BinOp::Mul => "mul",
                BinOp::Div => "div",
                BinOp::Rem => "rem",
                BinOp::Eq => "eq",
                BinOp::Ne => "ne",
                BinOp::Lt => "lt",
                BinOp::Le => "le",
                BinOp::Gt => "gt",
                BinOp::Ge => "ge",
                BinOp::And => "and",
                BinOp::Or => "or",
            }
            .to_owned(),
            Op::Un(UnOp::Neg) => "neg".to_owned(),
            Op::Un(UnOp::Not) => "not".to_owned(),
            Op::Jump(t) => format!("jump {t}"),
            Op::JumpIfFalse(t) => format!("jump_if_false {t}"),
            Op::Call(f, n) => format!("call {} ({n} 个参数)", p.funcs[*f].name),
            Op::Print => "print".to_owned(),
            Op::Ret => "ret".to_owned(),
            Op::RetVoid => "ret_void".to_owned(),
            Op::Pop => "pop".to_owned(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Code {
    pub name: String,
    pub params: usize,
    /// 每个局部变量槽的名字（参数在前）。
    pub slots: Vec<String>,
    pub ops: Vec<Op>,
    /// 每条指令来自源码的哪一段。
    pub spans: Vec<Span>,
}

#[derive(Clone, Debug)]
pub struct Program {
    pub funcs: Vec<Code>,
}

/// 编译通过了语义分析的程序。
pub fn compile(ast: &Ast, root: NodeId, sema: &Analysis) -> Program {
    let funcs_nodes: Vec<NodeId> = ast
        .node(root)
        .children
        .iter()
        .copied()
        .filter(|&f| matches!(ast.node(f).kind, NodeKind::Function { .. }))
        .collect();
    let names: Vec<String> = funcs_nodes
        .iter()
        .map(|&f| match &ast.node(f).kind {
            NodeKind::Function { name, .. } => name.clone(),
            _ => unreachable!(),
        })
        .collect();
    let funcs = funcs_nodes
        .iter()
        .zip(&names)
        .map(|(&f, name)| {
            let mut c = Compiler {
                ast,
                sema,
                names: &names,
                code: Code { name: name.clone(), params: 0, slots: vec![], ops: vec![], spans: vec![] },
                slot_of: vec![],
            };
            c.function(f);
            c.code
        })
        .collect();
    Program { funcs }
}

struct Compiler<'a> {
    ast: &'a Ast,
    sema: &'a Analysis,
    names: &'a [String],
    code: Code,
    slot_of: Vec<(SymbolId, usize)>,
}

impl Compiler<'_> {
    fn emit(&mut self, op: Op, node: NodeId) -> usize {
        self.code.ops.push(op);
        self.code.spans.push(self.ast.node(node).span);
        self.code.ops.len() - 1
    }

    fn here(&self) -> usize {
        self.code.ops.len()
    }

    fn patch(&mut self, at: usize, target: usize) {
        match &mut self.code.ops[at] {
            Op::Jump(t) | Op::JumpIfFalse(t) => *t = target,
            _ => unreachable!(),
        }
    }

    fn new_slot(&mut self, decl: NodeId) -> usize {
        let sym = self.sema.declared[decl.index()].expect("声明节点有符号");
        let slot = self.code.slots.len();
        self.code.slots.push(self.sema.symbols[sym].name.clone());
        self.slot_of.push((sym, slot));
        slot
    }

    fn slot(&self, node: NodeId) -> usize {
        let sym = self.sema.resolved[node.index()].expect("名字已解析");
        self.slot_of.iter().rev().find(|(s, _)| *s == sym).map(|(_, k)| *k).expect("变量有槽")
    }

    fn function(&mut self, f: NodeId) {
        let kids = self.ast.node(f).children.clone();
        for &p in &kids {
            if matches!(self.ast.node(p).kind, NodeKind::Param { .. }) {
                self.new_slot(p);
                self.code.params += 1;
            }
        }
        let body = *kids.last().unwrap();
        self.block(body);
        if !matches!(self.code.ops.last(), Some(Op::Ret | Op::RetVoid)) {
            self.emit(Op::RetVoid, body);
        }
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
                self.expr(*kids.last().unwrap());
                let slot = self.new_slot(s);
                self.emit(Op::Store(slot), s);
            }
            NodeKind::ExprStmt => {
                if self.expr(kids[0]) {
                    self.emit(Op::Pop, s);
                }
            }
            NodeKind::Return => match kids.first() {
                Some(&e) => {
                    self.expr(e);
                    self.emit(Op::Ret, s);
                }
                None => {
                    self.emit(Op::RetVoid, s);
                }
            },
            NodeKind::If => {
                self.expr(kids[0]);
                let jf = self.emit(Op::JumpIfFalse(0), s);
                self.block(kids[1]);
                if let Some(&e) = kids.get(2) {
                    let j = self.emit(Op::Jump(0), s);
                    let else_at = self.here();
                    self.patch(jf, else_at);
                    if matches!(self.ast.node(e).kind, NodeKind::If) {
                        self.statement(e);
                    } else {
                        self.block(e);
                    }
                    let end = self.here();
                    self.patch(j, end);
                } else {
                    let end = self.here();
                    self.patch(jf, end);
                }
            }
            NodeKind::While => {
                let top = self.here();
                self.expr(kids[0]);
                let jf = self.emit(Op::JumpIfFalse(0), s);
                self.block(kids[1]);
                self.emit(Op::Jump(top), s);
                let end = self.here();
                self.patch(jf, end);
            }
            NodeKind::Block => self.block(s),
            _ => {}
        }
    }

    /// 生成计算表达式的代码，返回它是否在栈上留下了一个值。
    fn expr(&mut self, e: NodeId) -> bool {
        let kids = self.ast.node(e).children.clone();
        match self.ast.node(e).kind.clone() {
            NodeKind::Int(n) => {
                self.emit(Op::Push(n), e);
            }
            NodeKind::Bool(b) => {
                self.emit(Op::Push(i64::from(b)), e);
            }
            NodeKind::Var(_) => {
                let k = self.slot(e);
                self.emit(Op::Load(k), e);
            }
            NodeKind::Binary(BinOp::And) => {
                // a && b：a 为假直接得 0，不算 b
                self.expr(kids[0]);
                let jf = self.emit(Op::JumpIfFalse(0), e);
                self.expr(kids[1]);
                let j = self.emit(Op::Jump(0), e);
                let f = self.here();
                self.patch(jf, f);
                self.emit(Op::Push(0), e);
                let end = self.here();
                self.patch(j, end);
            }
            NodeKind::Binary(BinOp::Or) => {
                // a || b：a 为真直接得 1，不算 b
                self.expr(kids[0]);
                let jf = self.emit(Op::JumpIfFalse(0), e);
                self.emit(Op::Push(1), e);
                let j = self.emit(Op::Jump(0), e);
                let r = self.here();
                self.patch(jf, r);
                self.expr(kids[1]);
                let end = self.here();
                self.patch(j, end);
            }
            NodeKind::Binary(op) => {
                self.expr(kids[0]);
                self.expr(kids[1]);
                self.emit(Op::Bin(op), e);
            }
            NodeKind::Unary(op) => {
                self.expr(kids[0]);
                self.emit(Op::Un(op), e);
            }
            NodeKind::Assign { .. } => {
                self.expr(kids[0]);
                let k = self.slot(e);
                self.emit(Op::Store(k), e);
                return false;
            }
            NodeKind::Call { name } => {
                for &a in &kids {
                    self.expr(a);
                }
                let sym = self.sema.resolved[e.index()];
                let returns = sym.is_some_and(|s| match &self.sema.symbols[s].kind {
                    SymbolKind::Function(sig) | SymbolKind::Builtin(sig) => sig.ret != Type::Unit,
                    _ => false,
                });
                if name == "print" {
                    self.emit(Op::Print, e);
                } else {
                    let f = self.names.iter().position(|n| *n == name).expect("函数存在");
                    self.emit(Op::Call(f, kids.len()), e);
                }
                return returns;
            }
            _ => return false,
        }
        true
    }
}

/// 一个调用帧。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    pub func: usize,
    pub pc: usize,
    pub locals: Vec<Option<i64>>,
    /// 这一帧的操作数从栈的哪个位置开始。
    pub base: usize,
}

/// 执行某条指令**之前**的完整状态。
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub frames: Vec<Frame>,
    pub stack: Vec<i64>,
    pub output: usize,
}

#[derive(Clone, Debug)]
pub struct Run {
    pub output: Vec<i64>,
    /// 前若干步的快照（最后一个是结束后的状态）。
    pub trace: Vec<Snapshot>,
    pub error: Option<String>,
    /// 一共执行了多少条指令。
    pub steps: usize,
}

/// 从 main 开始执行。只记录前 `trace_limit` 步的快照。
pub fn run(p: &Program, trace_limit: usize) -> Run {
    let mut r = Run { output: vec![], trace: vec![], error: None, steps: 0 };
    let Some(main) = p.funcs.iter().position(|f| f.name == "main") else {
        r.error = Some("没有 main 函数".to_owned());
        return r;
    };
    let mut frames = vec![Frame { func: main, pc: 0, locals: vec![None; p.funcs[main].slots.len()], base: 0 }];
    let mut stack: Vec<i64> = Vec::new();
    let result: Result<(), String> = (|| {
        loop {
            if r.trace.len() < trace_limit {
                r.trace.push(Snapshot { frames: frames.clone(), stack: stack.clone(), output: r.output.len() });
            }
            if r.steps >= 1_000_000 {
                return Err("执行的指令太多（死循环？）".to_owned());
            }
            r.steps += 1;
            let fr = frames.last_mut().unwrap();
            let op = p.funcs[fr.func].ops[fr.pc];
            fr.pc += 1;
            let mut pop = || stack.pop().ok_or("栈是空的");
            match op {
                Op::Push(n) => stack.push(n),
                Op::Load(k) => {
                    let v = fr.locals[k].ok_or_else(|| format!("变量 {} 还没有值", p.funcs[fr.func].slots[k]))?;
                    stack.push(v);
                }
                Op::Store(k) => {
                    let v = pop()?;
                    fr.locals[k] = Some(v);
                }
                Op::Bin(op) => {
                    let b = pop()?;
                    let a = pop()?;
                    stack.push(binop(op, a, b)?);
                }
                Op::Un(op) => {
                    let a = pop()?;
                    stack.push(match op {
                        UnOp::Neg => a.wrapping_neg(),
                        UnOp::Not => i64::from(a == 0),
                    });
                }
                Op::Jump(t) => fr.pc = t,
                Op::JumpIfFalse(t) => {
                    if pop()? == 0 {
                        frames.last_mut().unwrap().pc = t;
                    }
                }
                Op::Print => {
                    let v = pop()?;
                    r.output.push(v);
                }
                Op::Pop => {
                    pop()?;
                }
                Op::Call(f, n) => {
                    if frames.len() > 200 {
                        return Err("递归太深".to_owned());
                    }
                    let args = stack.split_off(stack.len() - n);
                    let mut locals = vec![None; p.funcs[f].slots.len()];
                    for (i, a) in args.into_iter().enumerate() {
                        locals[i] = Some(a);
                    }
                    frames.push(Frame { func: f, pc: 0, locals, base: stack.len() });
                }
                Op::Ret | Op::RetVoid => {
                    let v = if op == Op::Ret { Some(pop()?) } else { None };
                    let done = frames.pop().unwrap();
                    stack.truncate(done.base);
                    if let Some(v) = v {
                        stack.push(v);
                    }
                    if frames.is_empty() {
                        return Ok(());
                    }
                }
            }
        }
    })();
    if r.trace.len() < trace_limit {
        r.trace.push(Snapshot { frames: frames.clone(), stack: stack.clone(), output: r.output.len() });
    }
    r.error = result.err();
    r
}
