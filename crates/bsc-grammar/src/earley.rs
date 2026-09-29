//! Earley 分析：能处理任意上下文无关文法（包括左递归和有歧义的文法）。
//!
//! 它为输入的每个位置 i 维护一个"项目集" S(i)。项目 `A → α • β, 起点 k` 的意思是：
//! "正在尝试用 A → αβ 匹配从第 k 个记号开始的一段，α 部分已经匹配到了位置 i"。
//! 三个操作反复进行，直到没有新项目：
//!
//! - **预测**：• 后面是非终结符 B，就把 B 的所有产生式 `B → • γ, 起点 i` 加入 S(i)；
//! - **扫描**：• 后面是终结符，且正好等于第 i 个记号，就把 • 右移一格放进 S(i+1)；
//! - **完成**：• 已经在最后（A 匹配完了），就回到 S(k)，把所有等着 A 的项目的 • 右移一格。
//!
//! 最后 S(n) 里有 `开始符号 → … •, 起点 0`，就说明整个输入是一个合法的句子。
//! 这时再从各个项目集里把语法树"拼"出来——有歧义的文法可以拼出不止一棵。

use std::collections::HashSet;

use crate::first_follow::nullable;
use crate::{Grammar, Sym, Tree};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Item {
    pub prod: usize,
    pub dot: usize,
    pub origin: usize,
}

#[derive(Clone, Debug)]
pub struct EarleyResult {
    /// S(0) … S(n)。
    pub sets: Vec<Vec<Item>>,
    pub accepted: bool,
    /// 找到的语法树（最多 `max_trees` 棵）。
    pub trees: Vec<Tree>,
    /// 语法树数量达到了上限（可能还有更多）。
    pub more_trees: bool,
    /// 分析失败时，第一个"接不上"的记号下标（等于记号数表示输入提前结束）。
    pub fail_at: Option<usize>,
}

pub fn parse(g: &Grammar, tokens: &[Sym], max_trees: usize) -> EarleyResult {
    let n = tokens.len();
    let nullable = nullable(g);
    let mut sets: Vec<Vec<Item>> = vec![Vec::new(); n + 1];
    let mut seen: Vec<HashSet<Item>> = vec![HashSet::new(); n + 1];

    let add = |sets: &mut Vec<Vec<Item>>, seen: &mut Vec<HashSet<Item>>, i: usize, it: Item| {
        if seen[i].insert(it) {
            sets[i].push(it);
        }
    };
    for (p, _) in g.prods_of(g.start) {
        add(&mut sets, &mut seen, 0, Item { prod: p, dot: 0, origin: 0 });
    }

    for i in 0..=n {
        let mut k = 0;
        while k < sets[i].len() {
            let it = sets[i][k];
            k += 1;
            let rhs = &g.productions[it.prod].rhs;
            if let Some(&x) = rhs.get(it.dot) {
                if g.is_terminal(x) {
                    if i < n && tokens[i] == x {
                        add(&mut sets, &mut seen, i + 1, Item { dot: it.dot + 1, ..it });
                    }
                } else {
                    for (p, _) in g.prods_of(x) {
                        add(&mut sets, &mut seen, i, Item { prod: p, dot: 0, origin: i });
                    }
                    // 能推出空串的非终结符：直接跳过它（Aycock–Horspool 的处理），
                    // 否则 B → ε 在同一个集合里完成时，等着 B 的项目可能还没加入。
                    if nullable[x] {
                        add(&mut sets, &mut seen, i, Item { dot: it.dot + 1, ..it });
                    }
                }
            } else {
                let lhs = g.productions[it.prod].lhs;
                let waiting: Vec<Item> = sets[it.origin]
                    .iter()
                    .filter(|w| g.productions[w.prod].rhs.get(w.dot) == Some(&lhs))
                    .copied()
                    .collect();
                for w in waiting {
                    add(&mut sets, &mut seen, i, Item { dot: w.dot + 1, ..w });
                }
            }
        }
    }

    let accepted = sets[n].iter().any(|it| {
        it.origin == 0 && g.productions[it.prod].lhs == g.start && it.dot == g.productions[it.prod].rhs.len()
    });
    let fail_at = if accepted {
        None
    } else {
        // 最后一个非空的集合之后的那个记号接不上；都非空则是输入提前结束。
        Some((0..n).find(|&i| sets[i + 1].is_empty()).unwrap_or(n))
    };

    let mut trees = Vec::new();
    let mut more_trees = false;
    if accepted {
        let completed: HashSet<(Sym, usize, usize, usize)> = sets
            .iter()
            .enumerate()
            .flat_map(|(j, set)| {
                set.iter()
                    .filter(|it| it.dot == g.productions[it.prod].rhs.len())
                    .map(move |it| (g.productions[it.prod].lhs, it.origin, j, it.prod))
            })
            .collect();
        let mut ex = Extractor { g, tokens, completed: &completed, limit: max_trees + 1, visiting: HashSet::new() };
        trees = ex.trees(g.start, 0, n);
        if trees.len() > max_trees {
            trees.truncate(max_trees);
            more_trees = true;
        }
    }
    EarleyResult { sets, accepted, trees, more_trees, fail_at }
}

/// 从完成项目里拼出语法树。
struct Extractor<'a> {
    g: &'a Grammar,
    tokens: &'a [Sym],
    /// (非终结符, 起点, 终点, 产生式)
    completed: &'a HashSet<(Sym, usize, usize, usize)>,
    limit: usize,
    /// 正在展开的 (符号, 起点, 终点)，防止 A → A 这类循环无限展开。
    visiting: HashSet<(Sym, usize, usize)>,
}

impl Extractor<'_> {
    fn spans(&self, x: Sym, i: usize, k: usize) -> bool {
        if self.g.is_terminal(x) {
            k == i + 1 && self.tokens.get(i) == Some(&x)
        } else {
            self.completed.iter().any(|&(s, a, b, _)| s == x && a == i && b == k)
        }
    }

    fn trees(&mut self, x: Sym, i: usize, j: usize) -> Vec<Tree> {
        if self.g.is_terminal(x) {
            return if self.spans(x, i, j) {
                vec![Tree { sym: x, prod: None, children: vec![], start: i, end: j }]
            } else {
                vec![]
            };
        }
        if !self.visiting.insert((x, i, j)) {
            return vec![];
        }
        let mut prods: Vec<usize> =
            self.completed.iter().filter(|&&(s, a, b, _)| s == x && a == i && b == j).map(|&(_, _, _, p)| p).collect();
        prods.sort_unstable();
        let mut out = Vec::new();
        for p in prods {
            let rhs = self.g.productions[p].rhs.clone();
            for children in self.splits(&rhs, 0, i, j) {
                out.push(Tree { sym: x, prod: Some(p), children, start: i, end: j });
                if out.len() >= self.limit {
                    break;
                }
            }
            if out.len() >= self.limit {
                break;
            }
        }
        self.visiting.remove(&(x, i, j));
        out
    }

    /// 把区间 [i, j) 分给 rhs[idx..] 的各个符号，返回所有可能的孩子列表。
    fn splits(&mut self, rhs: &[Sym], idx: usize, i: usize, j: usize) -> Vec<Vec<Tree>> {
        let Some(&x) = rhs.get(idx) else {
            return if i == j { vec![vec![]] } else { vec![] };
        };
        let mut out = Vec::new();
        for k in i..=j {
            if !self.spans(x, i, k) {
                continue;
            }
            let rests = self.splits(rhs, idx + 1, k, j);
            if rests.is_empty() {
                continue;
            }
            for t in self.trees(x, i, k) {
                for rest in &rests {
                    let mut kids = vec![t.clone()];
                    kids.extend(rest.iter().cloned());
                    out.push(kids);
                    if out.len() >= self.limit {
                        return out;
                    }
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ambiguity_gives_two_trees() {
        let g = Grammar::parse("E -> E + E | E * E | num").unwrap();
        let toks: Vec<Sym> = g.tokenize("1 + 2 * 3").unwrap().into_iter().map(|(s, _)| s).collect();
        let r = parse(&g, &toks, 10);
        assert!(r.accepted);
        let shapes: Vec<String> = r.trees.iter().map(|t| t.to_bracketed(&g)).collect();
        assert_eq!(shapes.len(), 2);
        assert!(shapes.contains(&"E(E(E(num) + E(num)) * E(num))".to_owned()));
        assert!(shapes.contains(&"E(E(num) + E(E(num) * E(num)))".to_owned()));
        // 卡特兰数：n 个运算符有 Catalan(n) 种加括号方式，4 个运算符是 14 种
        let toks: Vec<Sym> = g.tokenize("1+2+3+4+5").unwrap().into_iter().map(|(s, _)| s).collect();
        assert_eq!(parse(&g, &toks, 100).trees.len(), 14);
    }

    #[test]
    fn epsilon_and_failure() {
        let g = Grammar::parse("S -> a S b | ε").unwrap();
        let toks = |s: &str| -> Vec<Sym> { s.chars().map(|c| g.symbol(&c.to_string()).unwrap()).collect() };
        assert!(parse(&g, &toks("aabb"), 5).accepted);
        assert!(parse(&g, &[], 5).accepted);
        let r = parse(&g, &toks("aab"), 5);
        assert_eq!(r.fail_at, Some(3));
        assert_eq!(parse(&g, &toks("abb"), 5).fail_at, Some(2));
    }
}
