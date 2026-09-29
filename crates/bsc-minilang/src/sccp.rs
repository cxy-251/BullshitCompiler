//! 稀疏条件常量传播（SCCP，Wegman & Zadeck 1991），在 SSA 形式上进行。
//!
//! 每个 SSA 变量的值是格上的一个点：
//!
//! ```text
//!            ⊤            还不知道（乐观地假设它可能是任何常量）
//!   … -1  0  1  2 …       确定是某个常量
//!            ⊥            不是常量（可能有多个不同的值）
//! ```
//!
//! 值只会往下走（⊤ → 常量 → ⊥），所以算法一定会停。
//!
//! "条件"：同时追踪哪些控制流边**可能被执行**。一开始只有入口可执行；条件跳转的条件是常量时，
//! 只把会走的那条边标为可执行。φ 只汇合来自可执行边的值——走不到的分支里的赋值不会"污染"结果。
//! "稀疏"：变量的值变了，只重新计算用到它的指令（沿 SSA 的定义-使用关系），而不是整个函数。

use std::collections::VecDeque;

use crate::cfg::{Cfg, EdgeKind};
use crate::interp::binop;
use crate::ir::{Inst, Operand, VarId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Val {
    Top,
    Const(i64),
    Bottom,
}

impl Val {
    /// 格上的"交"：两个值汇合后的结果。
    pub fn meet(self, o: Val) -> Val {
        match (self, o) {
            (Val::Top, x) | (x, Val::Top) => x,
            (Val::Const(a), Val::Const(b)) if a == b => Val::Const(a),
            _ => Val::Bottom,
        }
    }

    pub fn show(self) -> String {
        match self {
            Val::Top => "⊤".to_owned(),
            Val::Const(c) => c.to_string(),
            Val::Bottom => "⊥".to_owned(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    /// 控制流边第一次被标记为可执行（`from` 为空表示函数入口）。`first` 表示目标块是第一次被执行到。
    Edge { from: Option<usize>, to: usize, first: bool },
    /// 重新计算一条定义变量的指令（或 φ）。
    Eval { block: usize, inst: usize, before: Val, after: Val },
    /// 计算条件跳转：条件的值，以及因此变成可执行的后继。
    Branch { block: usize, inst: usize, cond: Val, targets: Vec<usize> },
}

#[derive(Clone, Debug)]
pub struct Sccp {
    pub input: Cfg,
    pub values: Vec<Val>,
    pub exec_blocks: Vec<bool>,
    pub exec_edges: Vec<(usize, usize)>,
    pub steps: Vec<Step>,
    /// 用结果改写后的程序：常量代入、删除常量的定义、删掉不可执行的边和块。
    pub output: Cfg,
}

/// `conditional = false` 时不追踪可执行边（所有分支都当作可能走），就是普通的 SSA 常量传播，用来对照。
pub fn sccp(cfg: &Cfg, conditional: bool) -> Sccp {
    let n = cfg.blocks.len();
    let mut uses: Vec<Vec<(usize, usize)>> = vec![Vec::new(); cfg.func.vars.len()];
    for (b, block) in cfg.blocks.iter().enumerate() {
        for (i, inst) in block.insts.iter().enumerate() {
            let mut ops = inst.uses();
            if let Inst::Phi { args, .. } = inst {
                ops.extend(args.iter().map(|(_, o)| *o));
            }
            for o in ops {
                if let Operand::Var(v) = o
                    && !uses[v].contains(&(b, i))
                {
                    uses[v].push((b, i));
                }
            }
        }
    }
    let mut s = Solver {
        cfg,
        conditional,
        values: vec![Val::Top; cfg.func.vars.len()],
        exec_blocks: vec![false; n],
        exec_edges: Vec::new(),
        flow: VecDeque::from([(None, 0)]),
        ssa: VecDeque::new(),
        steps: Vec::new(),
    };
    loop {
        if let Some((from, to)) = s.flow.pop_front() {
            if let Some(f) = from
                && s.exec_edges.contains(&(f, to))
            {
                continue;
            }
            if let Some(f) = from {
                s.exec_edges.push((f, to));
            }
            let first = !s.exec_blocks[to];
            s.steps.push(Step::Edge { from, to, first });
            s.exec_blocks[to] = true;
            for i in 0..cfg.blocks[to].insts.len() {
                let is_phi = matches!(cfg.blocks[to].insts[i], Inst::Phi { .. });
                if is_phi || first {
                    s.eval(to, i);
                }
            }
            if first && cfg.blocks[to].insts.last().is_none_or(|i| !i.is_terminator()) {
                // 没有跳转指令：顺序执行到下一块
                if let Some(&next) = cfg.blocks[to].succs.first() {
                    s.flow.push_back((Some(to), next));
                }
            }
        } else if let Some(v) = s.ssa.pop_front() {
            for &(b, i) in &uses[v] {
                if s.exec_blocks[b] {
                    s.eval(b, i);
                }
            }
        } else {
            break;
        }
    }

    let output = rewrite(cfg, &s.values, &s.exec_blocks, &s.exec_edges);
    Sccp {
        input: cfg.clone(),
        values: s.values,
        exec_blocks: s.exec_blocks,
        exec_edges: s.exec_edges,
        steps: s.steps,
        output,
    }
}

struct Solver<'a> {
    cfg: &'a Cfg,
    conditional: bool,
    values: Vec<Val>,
    exec_blocks: Vec<bool>,
    exec_edges: Vec<(usize, usize)>,
    flow: VecDeque<(Option<usize>, usize)>,
    ssa: VecDeque<VarId>,
    steps: Vec<Step>,
}

impl Solver<'_> {
    fn val(&self, o: Operand) -> Val {
        match o {
            Operand::Const(c) => Val::Const(c),
            Operand::Var(v) => self.values[v],
            Operand::Undef => Val::Top,
        }
    }

    fn eval(&mut self, b: usize, i: usize) {
        let cfg = self.cfg;
        let block = &cfg.blocks[b];
        let inst = &block.insts[i];
        let computed = match inst {
            Inst::Phi { args, .. } => args
                .iter()
                .filter(|(p, _)| self.exec_edges.contains(&(*p, b)))
                .fold(Val::Top, |acc, (_, o)| acc.meet(self.val(*o))),
            Inst::Copy { src, .. } => self.val(*src),
            Inst::Bin { op, a, b: rhs, .. } => match (self.val(*a), self.val(*rhs)) {
                (Val::Const(x), Val::Const(y)) => binop(*op, x, y).map_or(Val::Bottom, Val::Const),
                (Val::Bottom, _) | (_, Val::Bottom) => Val::Bottom,
                _ => Val::Top,
            },
            Inst::Un { op, a, .. } => match self.val(*a) {
                Val::Const(x) => Val::Const(match op {
                    crate::ast::UnOp::Neg => x.wrapping_neg(),
                    crate::ast::UnOp::Not => i64::from(x == 0),
                }),
                v => v,
            },
            // 参数、函数返回值：编译时不知道
            Inst::Param { .. } | Inst::Call { .. } => Val::Bottom,
            Inst::Branch { cond, .. } => {
                let c = self.val(*cond);
                let targets: Vec<usize> = match (c, self.conditional) {
                    (Val::Top, true) => vec![],
                    (Val::Const(x), true) => vec![if x != 0 { block.succs[0] } else { block.succs[1] }],
                    _ => block.succs.clone(),
                };
                for &t in &targets {
                    self.flow.push_back((Some(b), t));
                }
                self.steps.push(Step::Branch { block: b, inst: i, cond: c, targets });
                return;
            }
            Inst::Jump(_) => {
                self.flow.push_back((Some(b), block.succs[0]));
                return;
            }
            Inst::Return(_) | Inst::Label(_) => return,
        };
        let Some(d) = inst.def() else { return };
        let before = self.values[d];
        let after = before.meet(computed);
        self.steps.push(Step::Eval { block: b, inst: i, before, after });
        if after != before {
            self.values[d] = after;
            self.ssa.push_back(d);
        }
    }
}

fn rewrite(cfg: &Cfg, values: &[Val], exec_blocks: &[bool], exec_edges: &[(usize, usize)]) -> Cfg {
    let mut out = cfg.clone();
    // 去掉不会执行的边；条件跳转只剩一条边时变成无条件跳转
    for b in (0..out.blocks.len()).filter(|&b| exec_blocks[b]) {
        for s in out.blocks[b].succs.clone() {
            if !exec_edges.contains(&(b, s)) {
                out.remove_edge(b, s);
            }
        }
        if let Some(Inst::Branch { then, els, .. }) = out.blocks[b].insts.last().cloned()
            && out.blocks[b].succs.len() == 1
        {
            let kept_then = cfg.blocks[b].succs[0] == out.blocks[b].succs[0];
            *out.blocks[b].insts.last_mut().unwrap() = Inst::Jump(if kept_then { then } else { els });
            out.blocks[b].kinds[0] = EdgeKind::Jump;
        }
    }
    // 常量代入，删掉常量的定义
    for block in &mut out.blocks {
        let mut keep = Vec::new();
        for (i, inst) in block.insts.iter_mut().enumerate() {
            let subst = |o: &mut Operand| {
                if let Operand::Var(v) = *o
                    && let Val::Const(c) = values[v]
                {
                    *o = Operand::Const(c);
                }
            };
            for u in inst.uses_mut() {
                subst(u);
            }
            if let Inst::Phi { args, .. } = inst {
                for (_, o) in args.iter_mut() {
                    subst(o);
                }
            }
            let constant_def =
                inst.def().is_some_and(|d| matches!(values[d], Val::Const(_))) && !matches!(inst, Inst::Call { .. });
            if !constant_def {
                keep.push(i);
            }
        }
        block.insts = keep.iter().map(|&i| block.insts[i].clone()).collect();
        block.origin = keep.iter().map(|&i| block.origin[i]).collect();
    }
    out.reachable = exec_blocks.to_vec();
    out.compact()
}
