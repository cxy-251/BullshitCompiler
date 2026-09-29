//! 优化：常量折叠与代数化简、死代码消除。都在（非 SSA 的）控制流图上进行。
//!
//! - **常量折叠**：编译时就能算出来的，不留到运行时——`t1 = 3 * 4` 变成 `t1 = 12`。
//!   配合**局部常量传播**（同一个块里，`x = 3` 之后用到 x 的地方直接换成 3），能连锁地折叠下去。
//!   同理还有**复制传播**：`x = y` 之后用到 x 的地方换成 y。
//! - **代数化简**：`x + 0`、`x * 1` 就是 `x`，`x * 0` 就是 0。
//! - **分支折叠**：条件是常量的条件跳转变成无条件跳转，走不到的块随之删掉。
//! - **死代码消除**：结果没人用、也没有副作用的指令可以删掉。"没人用"靠活跃变量分析判断；
//!   删掉一条指令后，它用到的变量可能也变得没人用，所以要一轮一轮地做，直到删无可删。

use std::collections::HashMap;

use crate::ast::{BinOp, UnOp};
use crate::cfg::{Cfg, EdgeKind};
use crate::dataflow::{Solution, live_after_each, liveness, solve};
use crate::interp::binop;
use crate::ir::{Inst, Operand, VarId};

/// 一处改动。`after` 为 `None` 表示删除。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    pub block: usize,
    pub inst: usize,
    pub before: String,
    pub after: Option<String>,
    pub rule: String,
}

#[derive(Clone, Debug)]
pub struct Folded {
    pub input: Cfg,
    /// 改动后、删除不可达块之前（块编号和 `input` 一一对应）。
    pub changed: Cfg,
    /// 最终结果（已删除不可达块）。
    pub output: Cfg,
    pub changes: Vec<Change>,
    /// 因为分支折叠而变得不可达的块。
    pub removed_blocks: Vec<usize>,
}

/// 局部常量传播 + 常量折叠 + 代数化简 + 分支折叠。
pub fn fold(cfg: &Cfg) -> Folded {
    let mut out = cfg.clone();
    let mut changes = Vec::new();
    for b in 0..out.blocks.len() {
        let mut known: HashMap<VarId, i64> = HashMap::new();
        // x = y 之后（x、y 都没再被赋值前），用到 x 的地方可以直接用 y
        let mut copies: HashMap<VarId, VarId> = HashMap::new();
        for i in 0..out.blocks[b].insts.len() {
            let old = out.blocks[b].insts[i].clone();
            let (new, rules) = simplify(&old, &known, &copies);
            if let Some(d) = new.def() {
                known.remove(&d);
                copies.retain(|k, v| *k != d && *v != d);
                match new {
                    Inst::Copy { src: Operand::Const(c), .. } => {
                        known.insert(d, c);
                    }
                    Inst::Copy { src: Operand::Var(v), .. } if v != d => {
                        copies.insert(d, v);
                    }
                    _ => {}
                }
            }
            if new == old {
                continue;
            }
            if let (Inst::Branch { .. }, Inst::Jump(_)) = (&old, &new) {
                // 只保留走得到的那条边
                let (taken, dropped) = if matches!(new, Inst::Jump(l) if Some(l) == target_label(&old, true)) {
                    (out.blocks[b].succs[0], out.blocks[b].succs[1])
                } else {
                    (out.blocks[b].succs[1], out.blocks[b].succs[0])
                };
                out.remove_edge(b, dropped);
                let k = out.blocks[b].succs.iter().position(|&s| s == taken).unwrap();
                out.blocks[b].kinds[k] = EdgeKind::Jump;
            }
            changes.push(Change {
                block: b,
                inst: i,
                before: cfg.inst_text(&old),
                after: Some(out.inst_text(&new)),
                rule: rules.join(" + "),
            });
            out.blocks[b].insts[i] = new;
        }
    }
    out.recompute_reachable();
    let removed_blocks = (0..out.blocks.len()).filter(|&b| !out.reachable[b]).collect();
    let output = out.compact();
    Folded { input: cfg.clone(), changed: out, output, changes, removed_blocks }
}

fn target_label(i: &Inst, then: bool) -> Option<usize> {
    match i {
        Inst::Branch { then: t, els, .. } => Some(if then { *t } else { *els }),
        _ => None,
    }
}

/// 化简一条指令，返回新指令和用到的规则。
fn simplify(inst: &Inst, known: &HashMap<VarId, i64>, copies: &HashMap<VarId, VarId>) -> (Inst, Vec<&'static str>) {
    let mut new = inst.clone();
    let mut rules = Vec::new();
    let (mut constants, mut copied) = (false, false);
    for u in new.uses_mut() {
        if let Operand::Var(v) = *u {
            if let Some(&c) = known.get(&v) {
                *u = Operand::Const(c);
                constants = true;
            } else if let Some(&w) = copies.get(&v) {
                *u = Operand::Var(w);
                copied = true;
            }
        }
    }
    if constants {
        rules.push("常量传播");
    }
    if copied {
        rules.push("复制传播");
    }
    use Operand::{Const, Var};
    let replaced = match &new {
        Inst::Bin { dst, op, a: Const(x), b: Const(y) } => match binop(*op, *x, *y) {
            Ok(v) => Some((Inst::Copy { dst: *dst, src: Const(v) }, "常量折叠")),
            Err(_) => None, // 除以零：留到运行时报错
        },
        Inst::Bin { dst, op, a, b } => {
            let dst = *dst;
            match (op, *a, *b) {
                (BinOp::Add, x, Const(0)) | (BinOp::Add, Const(0), x) | (BinOp::Sub, x, Const(0)) => {
                    Some((Inst::Copy { dst, src: x }, "代数化简"))
                }
                (BinOp::Mul, x, Const(1)) | (BinOp::Mul, Const(1), x) | (BinOp::Div, x, Const(1)) => {
                    Some((Inst::Copy { dst, src: x }, "代数化简"))
                }
                (BinOp::Mul, _, Const(0)) | (BinOp::Mul, Const(0), _) => {
                    Some((Inst::Copy { dst, src: Const(0) }, "代数化简"))
                }
                (BinOp::Sub, Var(x), Var(y)) if x == y => Some((Inst::Copy { dst, src: Const(0) }, "代数化简")),
                _ => None,
            }
        }
        Inst::Un { dst, op, a: Const(x) } => {
            let v = match op {
                UnOp::Neg => x.wrapping_neg(),
                UnOp::Not => i64::from(*x == 0),
            };
            Some((Inst::Copy { dst: *dst, src: Const(v) }, "常量折叠"))
        }
        Inst::Branch { cond: Const(c), then, els } => {
            Some((Inst::Jump(if *c != 0 { *then } else { *els }), "分支折叠"))
        }
        _ => None,
    };
    if let Some((i, r)) = replaced {
        new = i;
        rules.push(r);
    }
    (new, rules)
}

/// 死代码消除的一轮。
#[derive(Clone, Debug)]
pub struct DceRound {
    /// 这一轮开始时的控制流图。
    pub cfg: Cfg,
    pub live: Solution,
    /// 这一轮删掉的指令（编号对应本轮开始时的 `cfg`）。
    pub removed: Vec<Change>,
}

#[derive(Clone, Debug)]
pub struct Dce {
    pub rounds: Vec<DceRound>,
    pub output: Cfg,
}

/// 基于活跃变量的死代码消除：赋值的变量在这条指令之后不再活跃、指令也没有副作用（不是函数调用），就删掉。
pub fn dce(cfg: &Cfg) -> Dce {
    let mut cur = cfg.clone();
    let mut rounds = Vec::new();
    loop {
        let live = solve(&cur.succs(), &liveness(&cur));
        let mut removed = Vec::new();
        let mut next = cur.clone();
        for b in 0..cur.blocks.len() {
            let after = live_after_each(&cur, b, &live.out[b]);
            let mut keep = Vec::new();
            for (i, inst) in cur.blocks[b].insts.iter().enumerate() {
                let dead = match inst.def() {
                    Some(d) => !after[i].contains(&d) && !matches!(inst, Inst::Call { .. }),
                    None => false,
                };
                if dead {
                    let var = cur.func.vars[inst.def().unwrap()].name.clone();
                    removed.push(Change {
                        block: b,
                        inst: i,
                        before: cur.inst_text(inst),
                        after: None,
                        rule: format!("{var} 之后不再被用到"),
                    });
                } else {
                    keep.push(i);
                }
            }
            next.blocks[b].insts = keep.iter().map(|&i| cur.blocks[b].insts[i].clone()).collect();
            next.blocks[b].origin = keep.iter().map(|&i| cur.blocks[b].origin[i]).collect();
        }
        let done = removed.is_empty();
        rounds.push(DceRound { cfg: cur, live, removed });
        cur = next;
        if done {
            return Dce { rounds, output: cur };
        }
    }
}
