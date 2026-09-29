//! 可空性、FIRST 集、FOLLOW 集——自顶向下和 LR 分析都要用到的三样东西。
//!
//! - **可空**：非终结符 A 能不能推出空串 ε。
//! - **FIRST(α)**：从符号串 α 推出的所有串里，可能出现在**第一个**位置的终结符。
//! - **FOLLOW(A)**：在某个句型里，可能**紧跟在** A 后面的终结符（包括输入结束 `$`）。
//!
//! 三者都用"不动点迭代"计算：先都设为空，然后反复扫描所有产生式、按规则往集合里加东西，
//! 直到某一轮什么都没加——这时的集合就是答案。（集合只会变大，且大小有上限，所以一定会停。）

use std::collections::BTreeSet;

use crate::{Grammar, Sym};

/// 哪些符号能推出空串（下标是符号编号；终结符永远是 false）。
pub fn nullable(g: &Grammar) -> Vec<bool> {
    let mut nullable = vec![false; g.num_symbols()];
    let mut changed = true;
    while changed {
        changed = false;
        for p in &g.productions {
            if !nullable[p.lhs] && p.rhs.iter().all(|&s| nullable[s]) {
                nullable[p.lhs] = true;
                changed = true;
            }
        }
    }
    nullable
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Nullable,
    First,
    Follow,
}

/// 一次"往集合里加东西"。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Step {
    /// 第几轮扫描（从 1 开始；0 是初始化）。
    pub round: usize,
    pub kind: Kind,
    /// 依据的产生式（初始化时为 `None`）。
    pub prod: Option<usize>,
    /// 被扩充的那个非终结符。
    pub target: Sym,
    /// 新加入的终结符（可空性为空列表）。
    pub added: Vec<Sym>,
    /// 给人看的理由。
    pub reason: String,
}

#[derive(Clone, Debug)]
pub struct Sets {
    pub nullable: Vec<bool>,
    /// 每个符号的 FIRST 集（终结符的 FIRST 就是它自己；不含 ε，ε 看 `nullable`）。
    pub first: Vec<BTreeSet<Sym>>,
    pub follow: Vec<BTreeSet<Sym>>,
    pub steps: Vec<Step>,
    /// FIRST（含可空性）用了几轮，FOLLOW 用了几轮（都包括最后那轮"没有变化"的确认）。
    pub first_rounds: usize,
    pub follow_rounds: usize,
}

impl Sets {
    /// 符号串的 FIRST 集，以及它能否推出空串。
    pub fn first_of(&self, seq: &[Sym]) -> (BTreeSet<Sym>, bool) {
        let mut out = BTreeSet::new();
        for &s in seq {
            out.extend(self.first[s].iter().copied());
            if !self.nullable[s] {
                return (out, false);
            }
        }
        (out, true)
    }
}

pub fn compute(g: &Grammar) -> Sets {
    let n = g.num_symbols();
    let mut nullable = vec![false; n];
    let mut first: Vec<BTreeSet<Sym>> = vec![BTreeSet::new(); n];
    for t in g.terminals().chain([g.eof]) {
        first[t].insert(t);
    }
    let mut steps = Vec::new();
    let names = |syms: &[Sym]| syms.iter().map(|&s| g.name(s).to_owned()).collect::<Vec<_>>().join(", ");

    // 第一阶段：可空性和 FIRST 一起迭代。
    let mut round = 0;
    loop {
        round += 1;
        let mut changed = false;
        for (pi, p) in g.productions.iter().enumerate() {
            let a = g.name(p.lhs);
            if !nullable[p.lhs] && p.rhs.iter().all(|&s| nullable[s]) {
                nullable[p.lhs] = true;
                changed = true;
                let reason = if p.rhs.is_empty() {
                    format!("{a} → ε，所以 {a} 能推出空串")
                } else {
                    format!("{} 右部的每个符号都能推出空串，所以 {a} 也能", g.prod_text(pi))
                };
                steps.push(Step { round, kind: Kind::Nullable, prod: Some(pi), target: p.lhs, added: vec![], reason });
            }
            // FIRST(A) ⊇ FIRST(X1)；X1 可空时再加 FIRST(X2)……
            for (k, &x) in p.rhs.iter().enumerate() {
                let new: Vec<Sym> = first[x].difference(&first[p.lhs]).copied().collect();
                if !new.is_empty() {
                    let prefix = if k == 0 {
                        String::new()
                    } else {
                        format!("前面的 {} 都能推出空串，", g.seq_text(&p.rhs[..k]))
                    };
                    let reason = if g.is_terminal(x) {
                        format!("{}：{prefix}{} 可以打头，加入 FIRST({a})", g.prod_text(pi), g.name(x))
                    } else {
                        format!(
                            "{}：{prefix}{} 可以打头，FIRST({}) 里的 {} 加入 FIRST({a})",
                            g.prod_text(pi),
                            g.name(x),
                            g.name(x),
                            names(&new)
                        )
                    };
                    first[p.lhs].extend(new.iter().copied());
                    changed = true;
                    steps.push(Step { round, kind: Kind::First, prod: Some(pi), target: p.lhs, added: new, reason });
                }
                if !nullable[x] {
                    break;
                }
            }
        }
        if !changed {
            break;
        }
    }
    let first_rounds = round;

    // 第二阶段：FOLLOW。
    let mut follow: Vec<BTreeSet<Sym>> = vec![BTreeSet::new(); n];
    follow[g.start].insert(g.eof);
    steps.push(Step {
        round: 0,
        kind: Kind::Follow,
        prod: None,
        target: g.start,
        added: vec![g.eof],
        reason: format!("开始符号 {} 后面紧跟着输入结束，所以 $ 加入 FOLLOW({})", g.name(g.start), g.name(g.start)),
    });
    let mut round = 0;
    loop {
        round += 1;
        let mut changed = false;
        for (pi, p) in g.productions.iter().enumerate() {
            for (k, &b) in p.rhs.iter().enumerate() {
                if g.is_terminal(b) {
                    continue;
                }
                let beta = &p.rhs[k + 1..];
                // FIRST(β) ⊆ FOLLOW(B)
                let mut fb = BTreeSet::new();
                let mut beta_nullable = true;
                for &s in beta {
                    fb.extend(first[s].iter().copied());
                    if !nullable[s] {
                        beta_nullable = false;
                        break;
                    }
                }
                let new: Vec<Sym> = fb.difference(&follow[b]).copied().collect();
                if !new.is_empty() {
                    let reason = format!(
                        "{}：{} 后面跟着 {}，它能打头的 {} 加入 FOLLOW({})",
                        g.prod_text(pi),
                        g.name(b),
                        g.seq_text(beta),
                        names(&new),
                        g.name(b)
                    );
                    follow[b].extend(new.iter().copied());
                    changed = true;
                    steps.push(Step { round, kind: Kind::Follow, prod: Some(pi), target: b, added: new, reason });
                }
                // β 可空（或 B 在末尾）：FOLLOW(A) ⊆ FOLLOW(B)
                if beta_nullable && b != p.lhs {
                    let new: Vec<Sym> = follow[p.lhs].difference(&follow[b]).copied().collect();
                    if !new.is_empty() {
                        let why = if beta.is_empty() {
                            format!("{} 在右部末尾", g.name(b))
                        } else {
                            format!("{} 后面的 {} 能推出空串", g.name(b), g.seq_text(beta))
                        };
                        let reason = format!(
                            "{}：{why}，能跟在 {} 后面的也能跟在 {} 后面，FOLLOW({}) 里的 {} 加入 FOLLOW({})",
                            g.prod_text(pi),
                            g.name(p.lhs),
                            g.name(b),
                            g.name(p.lhs),
                            names(&new),
                            g.name(b)
                        );
                        follow[b].extend(new.iter().copied());
                        changed = true;
                        steps.push(Step { round, kind: Kind::Follow, prod: Some(pi), target: b, added: new, reason });
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }
    Sets { nullable, first, follow, steps, first_rounds, follow_rounds: round }
}
