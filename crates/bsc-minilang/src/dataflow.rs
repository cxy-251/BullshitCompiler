//! 数据流分析框架。
//!
//! 很多分析都能写成同一种形式：每个基本块有一个"进入时的信息" IN 和"离开时的信息" OUT，
//!
//! ```text
//! 正向：IN(b)  = ∪ OUT(p)，p 是 b 的前驱        OUT(b) = gen(b) ∪ (IN(b)  − kill(b))
//! 反向：OUT(b) = ∪ IN(s)， s 是 b 的后继        IN(b)  = gen(b) ∪ (OUT(b) − kill(b))
//! ```
//!
//! gen 是块自己"产生"的信息，kill 是块"杀死"的信息。求解用**工作表算法**：
//! 一开始所有块都在工作表里；每次取出一个块重新计算，结果变了就把受影响的邻居（正向是后继、反向是前驱）放回工作表，
//! 直到工作表为空。
//!
//! 这里实现两个经典分析：
//! - **活跃变量**（反向）：程序某处之后还会用到的变量。gen = 块里"先用后赋值"的变量，kill = 块里赋值过的变量。
//! - **到达定值**（正向）：程序某处可能"看到"的赋值语句。gen = 块里每个变量的最后一次赋值，kill = 其他对同名变量的赋值。

use std::collections::VecDeque;

use crate::cfg::Cfg;
use crate::dom::reverse_postorder;
use crate::ir::{Operand, VarId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Forward,
    Backward,
}

/// 一个 gen/kill 形式的数据流问题。集合元素是编号（变量编号、定值编号……），按从小到大排列。
#[derive(Clone, Debug)]
pub struct Problem {
    pub dir: Direction,
    pub gen_: Vec<Vec<usize>>,
    pub kill: Vec<Vec<usize>>,
}

/// 工作表算法的一步：取出块 `block` 重新计算。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Step {
    pub block: usize,
    /// 取出之前的工作表。
    pub worklist: Vec<usize>,
    /// 汇合的结果（正向是 IN，反向是 OUT）。
    pub meet: Vec<usize>,
    /// 传递函数的结果（正向是 OUT，反向是 IN）。
    pub result: Vec<usize>,
    pub changed: bool,
    /// 因为结果变了而放回工作表的块。
    pub pushed: Vec<usize>,
}

#[derive(Clone, Debug)]
pub struct Solution {
    pub inn: Vec<Vec<usize>>,
    pub out: Vec<Vec<usize>>,
    pub steps: Vec<Step>,
}

fn union_into(a: &mut Vec<usize>, b: &[usize]) {
    for &x in b {
        if let Err(i) = a.binary_search(&x) {
            a.insert(i, x);
        }
    }
}

fn transfer(gen_: &[usize], kill: &[usize], x: &[usize]) -> Vec<usize> {
    let mut r: Vec<usize> = x.iter().copied().filter(|v| kill.binary_search(v).is_err()).collect();
    union_into(&mut r, gen_);
    r
}

fn preds_of(succs: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let mut preds = vec![Vec::new(); succs.len()];
    for (b, ss) in succs.iter().enumerate() {
        for &s in ss {
            if !preds[s].contains(&b) {
                preds[s].push(b);
            }
        }
    }
    preds
}

/// 工作表算法。正向按逆后序、反向按后序排初始工作表，收敛最快。
pub fn solve(succs: &[Vec<usize>], p: &Problem) -> Solution {
    let n = succs.len();
    let preds = preds_of(succs);
    let mut inn = vec![Vec::new(); n];
    let mut out = vec![Vec::new(); n];
    let mut order = reverse_postorder(succs);
    if p.dir == Direction::Backward {
        order.reverse();
    }
    let mut work: VecDeque<usize> = order.into_iter().collect();
    let mut steps = Vec::new();
    while let Some(b) = work.pop_front() {
        let snapshot: Vec<usize> = std::iter::once(b).chain(work.iter().copied()).collect();
        let (from, to) = match p.dir {
            Direction::Forward => (&preds[b], &succs[b]),
            Direction::Backward => (&succs[b], &preds[b]),
        };
        let mut meet = Vec::new();
        for &x in from {
            union_into(&mut meet, if p.dir == Direction::Forward { &out[x] } else { &inn[x] });
        }
        let result = transfer(&p.gen_[b], &p.kill[b], &meet);
        let (m, r) = match p.dir {
            Direction::Forward => (&mut inn[b], &mut out[b]),
            Direction::Backward => (&mut out[b], &mut inn[b]),
        };
        *m = meet.clone();
        let changed = *r != result;
        *r = result.clone();
        let mut pushed = Vec::new();
        if changed {
            for &t in to {
                if !work.contains(&t) {
                    work.push_back(t);
                    pushed.push(t);
                }
            }
        }
        steps.push(Step { block: b, worklist: snapshot, meet, result, changed, pushed });
    }
    Solution { inn, out, steps }
}

/// 最朴素的解法（对照测试用）：一轮一轮地把所有块都算一遍，直到一整轮没有变化。
pub fn solve_round_robin(succs: &[Vec<usize>], p: &Problem) -> (Vec<Vec<usize>>, Vec<Vec<usize>>) {
    let n = succs.len();
    let preds = preds_of(succs);
    let (mut inn, mut out) = (vec![Vec::new(); n], vec![Vec::new(); n]);
    loop {
        let mut changed = false;
        for b in 0..n {
            let mut meet = Vec::new();
            match p.dir {
                Direction::Forward => {
                    for &x in &preds[b] {
                        union_into(&mut meet, &out[x]);
                    }
                    let r = transfer(&p.gen_[b], &p.kill[b], &meet);
                    changed |= r != out[b] || meet != inn[b];
                    (inn[b], out[b]) = (meet, r);
                }
                Direction::Backward => {
                    for &x in &succs[b] {
                        union_into(&mut meet, &inn[x]);
                    }
                    let r = transfer(&p.gen_[b], &p.kill[b], &meet);
                    changed |= r != inn[b] || meet != out[b];
                    (out[b], inn[b]) = (meet, r);
                }
            }
        }
        if !changed {
            return (inn, out);
        }
    }
}

/// 活跃变量分析的 gen（块里先用后赋值的变量）和 kill（块里赋值过的变量）。
pub fn liveness(cfg: &Cfg) -> Problem {
    let mut gen_ = Vec::new();
    let mut kill = Vec::new();
    for b in &cfg.blocks {
        let (mut g, mut k): (Vec<usize>, Vec<usize>) = (Vec::new(), Vec::new());
        for inst in &b.insts {
            for u in inst.uses() {
                if let Operand::Var(v) = u
                    && !k.contains(&v)
                    && !g.contains(&v)
                {
                    g.push(v);
                }
            }
            if let Some(d) = inst.def()
                && !k.contains(&d)
            {
                k.push(d);
            }
        }
        g.sort_unstable();
        k.sort_unstable();
        gen_.push(g);
        kill.push(k);
    }
    Problem { dir: Direction::Backward, gen_, kill }
}

/// 块里每条指令之后活跃的变量（由块出口的活跃集合往回推）。
pub fn live_after_each(cfg: &Cfg, b: usize, live_out: &[usize]) -> Vec<Vec<usize>> {
    let insts = &cfg.blocks[b].insts;
    let mut live = live_out.to_vec();
    let mut res = vec![Vec::new(); insts.len()];
    for (i, inst) in insts.iter().enumerate().rev() {
        res[i] = live.clone();
        if let Some(d) = inst.def() {
            live.retain(|&v| v != d);
        }
        for u in inst.uses() {
            if let Operand::Var(v) = u {
                union_into(&mut live, &[v]);
            }
        }
    }
    res
}

/// 一处定值（赋值）：第几块第几条指令，给哪个变量赋值。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Def {
    pub block: usize,
    pub inst: usize,
    pub var: VarId,
}

/// 到达定值分析。定值编号就是在返回的列表里的下标，显示时写成 d1、d2……
pub fn reaching_definitions(cfg: &Cfg) -> (Problem, Vec<Def>) {
    let mut defs = Vec::new();
    for (b, block) in cfg.blocks.iter().enumerate() {
        for (i, inst) in block.insts.iter().enumerate() {
            if let Some(v) = inst.def() {
                defs.push(Def { block: b, inst: i, var: v });
            }
        }
    }
    let mut gen_ = Vec::new();
    let mut kill = Vec::new();
    for b in 0..cfg.blocks.len() {
        // 块里每个变量最后一次赋值
        let mut g: Vec<usize> = Vec::new();
        for (id, d) in defs.iter().enumerate().filter(|(_, d)| d.block == b) {
            g.retain(|&x| defs[x].var != d.var);
            g.push(id);
        }
        let vars: Vec<VarId> = g.iter().map(|&x| defs[x].var).collect();
        let k: Vec<usize> = (0..defs.len()).filter(|&x| vars.contains(&defs[x].var) && !g.contains(&x)).collect();
        g.sort_unstable();
        gen_.push(g);
        kill.push(k);
    }
    (Problem { dir: Direction::Forward, gen_, kill }, defs)
}
