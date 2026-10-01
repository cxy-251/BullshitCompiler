//! 加权 Earley 分析：在有歧义的文法里，直接找出**代价最小**的那棵语法树。
//!
//! 第 2 章的 Earley 分析会找出所有语法树。自然语言的歧义非常多，一个十来个词的分句可能有成百上千种分析，
//! 全部列出来再挑不现实。做法是给每条产生式一个代价，每个 Earley 项目记住"到目前为止最便宜的推导"
//! 以及它是怎么来的（回溯指针）；完成一个项目时，如果找到了更便宜的推导就更新它，并把变化继续传播。
//! 这是 Viterbi 算法（最短路）和 Earley 算法的结合。
//!
//! 每个位置可以有好几个候选终结符：比如"坚持"平时是动词，在套话模板"坚持{X}不动摇"里是模板的字面部分。
//! 扫描时只要候选里有当前需要的终结符就能前进，由代价决定最后用哪一个。

use std::collections::HashMap;

use bsc_grammar::grammar::Grammar;
use bsc_grammar::{Sym, Tree};

#[derive(Clone, Copy, Debug)]
enum Back {
    Predict,
    /// 从上一个项目集里的第 `0` 个项目扫描过来。
    Scan(usize),
    /// (前一个项目所在的集合, 下标), (完成的子项目所在的集合, 下标)
    Complete((usize, usize), (usize, usize)),
}

#[derive(Clone, Copy, Debug)]
struct Item {
    prod: usize,
    dot: usize,
    origin: usize,
    cost: u32,
    back: Back,
}

#[derive(Clone, Debug)]
pub struct Parse {
    pub tree: Option<Tree>,
    pub cost: u32,
    /// 分析失败时，第一个"接不上"的记号（等于记号数表示句子没说完）。
    pub fail_at: Option<usize>,
    /// 一共产生了多少个项目（衡量工作量）。
    pub items: usize,
}

pub fn parse(g: &Grammar, tokens: &[Vec<Sym>], cost: &dyn Fn(usize) -> u32) -> Parse {
    let n = tokens.len();
    let mut sets: Vec<Vec<Item>> = vec![Vec::new(); n + 1];
    let mut keys: Vec<HashMap<(usize, usize, usize), usize>> = vec![HashMap::new(); n + 1];

    // 插入或改进一个项目，返回它的下标以及是否有变化
    fn upsert(
        sets: &mut [Vec<Item>],
        keys: &mut [HashMap<(usize, usize, usize), usize>],
        i: usize,
        it: Item,
    ) -> Option<usize> {
        let key = (it.prod, it.dot, it.origin);
        match keys[i].get(&key) {
            Some(&k) if sets[i][k].cost <= it.cost => None,
            Some(&k) => {
                sets[i][k] = it;
                Some(k)
            }
            None => {
                sets[i].push(it);
                keys[i].insert(key, sets[i].len() - 1);
                Some(sets[i].len() - 1)
            }
        }
    }

    for (p, prod) in g.productions.iter().enumerate() {
        if prod.lhs == g.start {
            upsert(&mut sets, &mut keys, 0, Item { prod: p, dot: 0, origin: 0, cost: cost(p), back: Back::Predict });
        }
    }
    let mut fail_at = None;
    for i in 0..=n {
        let mut work: Vec<usize> = (0..sets[i].len()).collect();
        while let Some(idx) = work.pop() {
            let it = sets[i][idx];
            let prod = &g.productions[it.prod];
            if it.dot == prod.rhs.len() {
                // 完成：回到起点，推进所有等着这个非终结符的项目
                let o = it.origin;
                for yi in 0..sets[o].len() {
                    let y = sets[o][yi];
                    let yp = &g.productions[y.prod];
                    if y.dot < yp.rhs.len() && yp.rhs[y.dot] == prod.lhs {
                        let new = Item {
                            prod: y.prod,
                            dot: y.dot + 1,
                            origin: y.origin,
                            cost: y.cost + it.cost,
                            back: Back::Complete((o, yi), (i, idx)),
                        };
                        if let Some(k) = upsert(&mut sets, &mut keys, i, new) {
                            work.push(k);
                        }
                    }
                }
            } else {
                let sym = prod.rhs[it.dot];
                if g.is_terminal(sym) {
                    if i < n && tokens[i].contains(&sym) {
                        let new = Item {
                            prod: it.prod,
                            dot: it.dot + 1,
                            origin: it.origin,
                            cost: it.cost,
                            back: Back::Scan(idx),
                        };
                        upsert(&mut sets, &mut keys, i + 1, new);
                    }
                } else {
                    for (q, qp) in g.productions.iter().enumerate() {
                        if qp.lhs == sym {
                            let new = Item { prod: q, dot: 0, origin: i, cost: cost(q), back: Back::Predict };
                            if let Some(k) = upsert(&mut sets, &mut keys, i, new)
                                && sets[i][k].dot == 0
                            {
                                work.push(k);
                            }
                        }
                    }
                }
            }
        }
        if i < n && sets[i + 1].is_empty() && fail_at.is_none() {
            fail_at = Some(i);
        }
    }
    let items = sets.iter().map(Vec::len).sum();
    let best = sets[n]
        .iter()
        .enumerate()
        .filter(|(_, it)| {
            it.origin == 0 && g.productions[it.prod].lhs == g.start && it.dot == g.productions[it.prod].rhs.len()
        })
        .min_by_key(|(_, it)| it.cost);
    match best {
        Some((idx, it)) => Parse { tree: Some(build(g, &sets, n, idx)), cost: it.cost, fail_at: None, items },
        None => Parse { tree: None, cost: 0, fail_at: Some(fail_at.unwrap_or(n)), items },
    }
}

fn build(g: &Grammar, sets: &[Vec<Item>], set: usize, idx: usize) -> Tree {
    let it = sets[set][idx];
    Tree {
        sym: g.productions[it.prod].lhs,
        prod: Some(it.prod),
        children: children(g, sets, set, idx),
        start: it.origin,
        end: set,
    }
}

fn children(g: &Grammar, sets: &[Vec<Item>], set: usize, idx: usize) -> Vec<Tree> {
    let it = sets[set][idx];
    match it.back {
        Back::Predict => vec![],
        Back::Scan(prev) => {
            let mut v = children(g, sets, set - 1, prev);
            // 叶子用的是这个项目扫描时需要的终结符（这个位置可能有好几个候选）
            let sym = g.productions[it.prod].rhs[it.dot - 1];
            v.push(Tree { sym, prod: None, children: vec![], start: set - 1, end: set });
            v
        }
        Back::Complete((ps, pi), (cs, ci)) => {
            let mut v = children(g, sets, ps, pi);
            v.push(build(g, sets, cs, ci));
            v
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 在随机的记号串上，加权 Earley 和第 2 章的 Earley 对"是不是合法句子"的判断完全一致；
    /// 找到的树覆盖整个输入、叶子正好是输入。
    #[test]
    fn agrees_with_plain_earley() {
        let g = Grammar::parse(crate::grammar::CLAUSE_GRAMMAR).unwrap();
        let terms: Vec<Sym> = g.terminals().filter(|&t| t != g.eof).collect();
        let mut seed = 11u64;
        for _ in 0..400 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let n = 1 + (seed >> 40) as usize % 7;
            let toks: Vec<Sym> = (0..n)
                .map(|_| {
                    seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                    terms[(seed >> 33) as usize % terms.len()]
                })
                .collect();
            let plain = bsc_grammar::earley::parse(&g, &toks, 1);
            let opts: Vec<Vec<Sym>> = toks.iter().map(|&t| vec![t]).collect();
            let weighted = parse(&g, &opts, &|p| crate::grammar::cost(&g, p));
            assert_eq!(plain.accepted, weighted.tree.is_some(), "{}", g.seq_text(&toks));
            if let Some(t) = weighted.tree {
                let mut leaves = Vec::new();
                fn walk(t: &Tree, out: &mut Vec<Sym>) {
                    if t.prod.is_none() {
                        out.push(t.sym);
                    }
                    for c in &t.children {
                        walk(c, out);
                    }
                }
                walk(&t, &mut leaves);
                assert_eq!(leaves, toks);
            }
        }
    }
}
