//! 基本块与控制流图（CFG）。
//!
//! **基本块**是一段"只能从头进、从尾出"的指令：中间没有跳进来的标签，也没有跳出去的跳转。
//! 划分方法是先找出每块的第一条指令（"首指令"，leader）：
//!
//! 1. 函数的第一条指令；
//! 2. 跳转的目标（标签处）；
//! 3. 紧跟在跳转、返回之后的那条指令。
//!
//! 从一个首指令到下一个首指令之前，就是一个基本块。块与块之间按跳转关系连边，就得到控制流图。

use std::ops::Range;

use bsc_core::Span;

use crate::ir::{Function, Inst, Label};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeaderWhy {
    First,
    Target(Label),
    AfterJump,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdgeKind {
    /// 顺序执行到下一块。
    Fall,
    Jump,
    /// 条件成立。
    True,
    /// 条件不成立。
    False,
}

#[derive(Clone, Debug)]
pub struct Block {
    /// 块名 B0、B1……（删除不可达块后保持原名，方便对照）。
    pub name: String,
    /// 块开头的标签（如果有）。
    pub label: Option<Label>,
    /// 块里的指令（不含开头的标签）。
    pub insts: Vec<Inst>,
    pub origin: Vec<Span>,
    /// 在线性代码中的范围（含开头的标签）。
    pub range: Range<usize>,
    pub succs: Vec<usize>,
    pub kinds: Vec<EdgeKind>,
    pub preds: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    Leader {
        inst: usize,
        why: LeaderWhy,
    },
    Block(usize),
    Edge {
        from: usize,
        to: usize,
        kind: EdgeKind,
    },
    Unreachable(usize),
    /// 第一条指令就是跳转目标（函数一开头就是循环），另加一个空的入口块，保证入口没有前驱。
    EntryBlock,
}

#[derive(Clone, Debug)]
pub struct Cfg {
    /// 变量表等信息（`code` 是划分前的线性代码）。
    pub func: Function,
    pub blocks: Vec<Block>,
    /// 从入口能不能走到。
    pub reachable: Vec<bool>,
    pub steps: Vec<Step>,
}

pub fn build(func: &Function) -> Cfg {
    let code = &func.code;
    let mut steps = Vec::new();
    let mut leader = vec![None; code.len()];
    let mark = |i: usize, why: LeaderWhy, leader: &mut Vec<Option<LeaderWhy>>| {
        if i < code.len() && leader[i].is_none() {
            leader[i] = Some(why);
        }
    };
    // 按指令顺序扫描，记下首指令及原因
    for (i, inst) in code.iter().enumerate() {
        if i == 0 {
            mark(0, LeaderWhy::First, &mut leader);
        }
        if let Inst::Label(l) = inst {
            mark(i, LeaderWhy::Target(*l), &mut leader);
        }
        if inst.is_terminator() {
            mark(i + 1, LeaderWhy::AfterJump, &mut leader);
        }
    }
    for (i, why) in leader.iter().enumerate() {
        if let Some(why) = why {
            steps.push(Step::Leader { inst: i, why: *why });
        }
    }

    let starts: Vec<usize> = (0..code.len()).filter(|&i| leader[i].is_some()).collect();
    let mut blocks = Vec::new();
    if matches!(code.first(), Some(Inst::Label(_))) {
        blocks.push(Block {
            name: "B0".to_owned(),
            label: None,
            insts: vec![],
            origin: vec![],
            range: 0..0,
            succs: vec![],
            kinds: vec![],
            preds: vec![],
        });
        steps.push(Step::EntryBlock);
    }
    let first = blocks.len();
    for (k, &s) in starts.iter().enumerate() {
        let k = k + first;
        let e = starts.get(k + 1 - first).copied().unwrap_or(code.len());
        let (label, body) = match code[s] {
            Inst::Label(l) => (Some(l), s + 1),
            _ => (None, s),
        };
        blocks.push(Block {
            name: format!("B{k}"),
            label,
            insts: code[body..e].to_vec(),
            origin: func.origin[body..e].to_vec(),
            range: s..e,
            succs: vec![],
            kinds: vec![],
            preds: vec![],
        });
        steps.push(Step::Block(k));
    }

    let block_of = |l: Label| blocks.iter().position(|b| b.label == Some(l)).expect("标签一定在某个块的开头");
    let mut edges = Vec::new();
    for (k, b) in blocks.iter().enumerate() {
        match b.insts.last() {
            Some(Inst::Jump(l)) => edges.push((k, block_of(*l), EdgeKind::Jump)),
            Some(Inst::Branch { then, els, .. }) => {
                edges.push((k, block_of(*then), EdgeKind::True));
                edges.push((k, block_of(*els), EdgeKind::False));
            }
            Some(Inst::Return(_)) => {}
            _ if k + 1 < blocks.len() => edges.push((k, k + 1, EdgeKind::Fall)),
            _ => {}
        }
    }
    for (from, to, kind) in edges {
        blocks[from].succs.push(to);
        blocks[from].kinds.push(kind);
        blocks[to].preds.push(from);
        steps.push(Step::Edge { from, to, kind });
    }

    // 从入口出发走不到的块（比如 return 后面的代码）
    let mut reachable = vec![false; blocks.len()];
    let mut stack = vec![0];
    while let Some(b) = stack.pop() {
        if b < blocks.len() && !reachable[b] {
            reachable[b] = true;
            stack.extend(blocks[b].succs.iter().copied());
        }
    }
    for (k, r) in reachable.iter().enumerate() {
        if !r {
            steps.push(Step::Unreachable(k));
        }
    }
    Cfg { func: func.clone(), blocks, reachable, steps }
}

impl Cfg {
    /// 删掉不可达的块（块名不变），后面的支配树、SSA 都在这个图上做。
    pub fn compact(&self) -> Cfg {
        let keep: Vec<usize> = (0..self.blocks.len()).filter(|&b| self.reachable[b]).collect();
        let new_id = |b: usize| keep.iter().position(|&k| k == b);
        let blocks = keep
            .iter()
            .map(|&b| {
                let old = &self.blocks[b];
                let mut nb = old.clone();
                nb.succs = old.succs.iter().filter_map(|&s| new_id(s)).collect();
                nb.kinds =
                    old.succs.iter().zip(&old.kinds).filter(|(s, _)| new_id(**s).is_some()).map(|(_, k)| *k).collect();
                nb.preds = old.preds.iter().filter_map(|&p| new_id(p)).collect();
                // φ 的参数按前驱块编号记录，也要跟着换号；来自被删掉的块的参数直接去掉
                for inst in &mut nb.insts {
                    if let Inst::Phi { args, .. } = inst {
                        *args = args.iter().filter_map(|&(p, o)| new_id(p).map(|q| (q, o))).collect();
                    }
                }
                nb
            })
            .collect::<Vec<_>>();
        let n = blocks.len();
        Cfg { func: self.func.clone(), blocks, reachable: vec![true; n], steps: vec![] }
    }

    /// 重新计算从入口能走到哪些块（改过边之后调用）。
    pub fn recompute_reachable(&mut self) {
        self.reachable = vec![false; self.blocks.len()];
        let mut stack = vec![0];
        while let Some(b) = stack.pop() {
            if b < self.blocks.len() && !self.reachable[b] {
                self.reachable[b] = true;
                stack.extend(self.blocks[b].succs.iter().copied());
            }
        }
    }

    /// 删掉边 from → to，同时更新前驱表和 to 里 φ 的参数。
    pub fn remove_edge(&mut self, from: usize, to: usize) {
        let b = &mut self.blocks[from];
        if let Some(i) = b.succs.iter().position(|&s| s == to) {
            b.succs.remove(i);
            b.kinds.remove(i);
        }
        let t = &mut self.blocks[to];
        t.preds.retain(|&p| p != from);
        for inst in &mut t.insts {
            if let Inst::Phi { args, .. } = inst {
                args.retain(|(p, _)| *p != from);
            }
        }
    }

    /// 每个块的后继（给支配树等图算法用）。
    pub fn succs(&self) -> Vec<Vec<usize>> {
        self.blocks.iter().map(|b| b.succs.clone()).collect()
    }

    /// 块里的指令在控制流图里的写法：跳转目标写块名。
    pub fn inst_text(&self, inst: &Inst) -> String {
        let target =
            |l: Label| self.blocks.iter().find(|b| b.label == Some(l)).map_or(format!("L{l}"), |b| b.name.clone());
        let block = |b: usize| self.blocks[b].name.clone();
        self.func.inst_text(inst, &target, &block)
    }
}
