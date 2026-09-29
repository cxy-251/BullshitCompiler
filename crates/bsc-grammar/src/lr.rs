//! LR 分析：自底向上、"移进-归约"。
//!
//! 分析器维护一个栈，每一步二选一：
//! - **移进**：把下一个记号压进栈；
//! - **归约**：栈顶几个符号正好是某条产生式的右部，把它们弹出、换成左部的非终结符。
//!
//! 什么时候移进、什么时候按哪条产生式归约，由一个有限自动机决定——它的状态是"LR(0) 项目集"。
//! 项目 `E → E • + T` 的意思是"正在匹配 E → E + T，已经看到了 E，接下来期待 + T"。
//!
//! ```text
//! 闭包(I)：• 后面是非终结符 B，就把 B 的所有产生式 B → • γ 加进来，重复到不变。
//! goto(I, X)：把 I 里 • 后面是 X 的项目的 • 右移一格，再求闭包。
//! 从 闭包({S' → • S}) 出发，对每个状态、每个符号求 goto，得到全部状态（项目集规范族）。
//! ```
//!
//! **SLR(1) 分析表**：状态里有 `A → α •`（走到头了）就在 FOLLOW(A) 的每个终结符上填"归约"；
//! 有 `A → α • a β` 就在 a 上填"移进"。一格有两个动作就是冲突。

use std::collections::{BTreeMap, BTreeSet};

use crate::first_follow::{Sets, compute};
use crate::{Grammar, Sym, Tree};

/// LR(0) 项目：产生式 + 点的位置。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LrItem {
    pub prod: usize,
    pub dot: usize,
}

/// 构造自动机的一步。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BuildStep {
    /// 初始状态 0 = 闭包({S' → • S})。
    Start,
    /// 从状态 `from` 读符号 `sym` 到达 `to`；`is_new` 表示 `to` 是这一步新发现的。
    Goto { from: usize, sym: Sym, to: usize, is_new: bool },
}

#[derive(Clone, Debug)]
pub struct Automaton {
    /// 增广文法（最后一条产生式是 S' → S）。
    pub g: Grammar,
    /// 每个状态的项目（核心项目在前）。
    pub states: Vec<Vec<LrItem>>,
    /// 每个状态核心项目的个数。
    pub kernel: Vec<usize>,
    pub goto: BTreeMap<(usize, Sym), usize>,
    pub steps: Vec<BuildStep>,
}

impl Automaton {
    pub fn build(original: &Grammar) -> Automaton {
        let g = original.augmented();
        let start_prod = g.productions.len() - 1;
        let mut states: Vec<Vec<LrItem>> = Vec::new();
        let mut kernel = Vec::new();
        let mut index: BTreeMap<Vec<LrItem>, usize> = BTreeMap::new();
        let mut goto = BTreeMap::new();
        let mut steps = vec![BuildStep::Start];

        let k0 = vec![LrItem { prod: start_prod, dot: 0 }];
        index.insert(k0.clone(), 0);
        kernel.push(k0.len());
        states.push(closure(&g, k0));

        let mut i = 0;
        while i < states.len() {
            // 按符号在状态里第一次出现（• 后面）的顺序求 goto，编号更贴近手算的习惯
            let mut syms: Vec<Sym> = Vec::new();
            for it in &states[i] {
                if let Some(&x) = g.productions[it.prod].rhs.get(it.dot)
                    && !syms.contains(&x)
                {
                    syms.push(x);
                }
            }
            for x in syms {
                let mut k: Vec<LrItem> = states[i]
                    .iter()
                    .filter(|it| g.productions[it.prod].rhs.get(it.dot) == Some(&x))
                    .map(|it| LrItem { prod: it.prod, dot: it.dot + 1 })
                    .collect();
                k.sort();
                let (to, is_new) = match index.get(&k) {
                    Some(&j) => (j, false),
                    None => {
                        let j = states.len();
                        index.insert(k.clone(), j);
                        kernel.push(k.len());
                        states.push(closure(&g, k));
                        (j, true)
                    }
                };
                goto.insert((i, x), to);
                steps.push(BuildStep::Goto { from: i, sym: x, to, is_new });
            }
            i += 1;
        }
        Automaton { g, states, kernel, goto, steps }
    }

    /// 项目的写法：`E → E • + T`。
    pub fn item_text(&self, it: LrItem) -> String {
        let p = &self.g.productions[it.prod];
        let mut parts: Vec<&str> = p.rhs.iter().map(|&s| self.g.name(s)).collect();
        parts.insert(it.dot, "•");
        format!("{} → {}", self.g.name(p.lhs), parts.join(" "))
    }
}

fn closure(g: &Grammar, kernel: Vec<LrItem>) -> Vec<LrItem> {
    let mut items = kernel;
    let mut seen: BTreeSet<LrItem> = items.iter().copied().collect();
    let mut k = 0;
    while k < items.len() {
        let it = items[k];
        k += 1;
        if let Some(&b) = g.productions[it.prod].rhs.get(it.dot)
            && !g.is_terminal(b)
        {
            for (p, _) in g.prods_of(b) {
                let new = LrItem { prod: p, dot: 0 };
                if seen.insert(new) {
                    items.push(new);
                }
            }
        }
    }
    items
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum LrAction {
    Shift(usize),
    Reduce(usize),
    Accept,
}

#[derive(Clone, Debug)]
pub struct SlrTable {
    /// (状态, 终结符) → 动作列表（多于一个是冲突）。
    pub action: BTreeMap<(usize, Sym), Vec<LrAction>>,
    /// (状态, 非终结符) → 状态。
    pub goto: BTreeMap<(usize, Sym), usize>,
    /// 增广文法上的 FIRST/FOLLOW（界面展示用）。
    pub sets: Sets,
}

impl SlrTable {
    pub fn build(a: &Automaton) -> SlrTable {
        let g = &a.g;
        let sets = compute(g);
        let start_prod = g.productions.len() - 1;
        let mut action: BTreeMap<(usize, Sym), Vec<LrAction>> = BTreeMap::new();
        let mut put = |key: (usize, Sym), act: LrAction| {
            let cell = action.entry(key).or_default();
            if !cell.contains(&act) {
                cell.push(act);
                cell.sort();
            }
        };
        for (s, items) in a.states.iter().enumerate() {
            for it in items {
                let p = &g.productions[it.prod];
                match p.rhs.get(it.dot) {
                    Some(&x) if g.is_terminal(x) => put((s, x), LrAction::Shift(a.goto[&(s, x)])),
                    Some(_) => {}
                    None if it.prod == start_prod => put((s, g.eof), LrAction::Accept),
                    None => {
                        for &f in &sets.follow[p.lhs] {
                            put((s, f), LrAction::Reduce(it.prod));
                        }
                    }
                }
            }
        }
        let goto = a.goto.iter().filter(|((_, x), _)| !g.is_terminal(*x)).map(|(&k, &v)| (k, v)).collect();
        SlrTable { action, goto, sets }
    }

    pub fn conflicts(&self) -> Vec<(usize, Sym)> {
        self.action.iter().filter(|(_, v)| v.len() > 1).map(|(&k, _)| k).collect()
    }
}

#[derive(Clone, Debug)]
pub struct LrStep {
    /// 动作之前的状态栈与符号栈（符号栈比状态栈少一个：最底下的状态 0 没有符号）。
    pub states: Vec<usize>,
    pub syms: Vec<Sym>,
    pub pos: usize,
    /// `None` 表示出错（表里这一格是空的）。
    pub action: Option<LrAction>,
}

/// 移进-归约分析。冲突的格子取第一个动作（移进优先于归约）。
pub fn parse(a: &Automaton, table: &SlrTable, tokens: &[Sym]) -> (Vec<LrStep>, Option<Tree>) {
    let g = &a.g;
    let mut states = vec![0usize];
    let mut syms: Vec<Sym> = Vec::new();
    let mut trees: Vec<Tree> = Vec::new();
    let mut pos = 0;
    let mut steps = Vec::new();
    loop {
        let s = *states.last().unwrap();
        let la = tokens.get(pos).copied().unwrap_or(g.eof);
        let act = table.action.get(&(s, la)).and_then(|v| v.first().copied());
        steps.push(LrStep { states: states.clone(), syms: syms.clone(), pos, action: act });
        match act {
            None => return (steps, None),
            Some(LrAction::Accept) => return (steps, trees.pop()),
            Some(LrAction::Shift(t)) => {
                states.push(t);
                syms.push(la);
                trees.push(Tree { sym: la, prod: None, children: vec![], start: pos, end: pos + 1 });
                pos += 1;
            }
            Some(LrAction::Reduce(p)) => {
                let prod = &g.productions[p];
                let n = prod.rhs.len();
                states.truncate(states.len() - n);
                syms.truncate(syms.len() - n);
                let children = trees.split_off(trees.len() - n);
                let start = children.first().map_or(pos, |c| c.start);
                trees.push(Tree { sym: prod.lhs, prod: Some(p), children, start, end: pos });
                let top = *states.last().unwrap();
                let Some(&next) = table.goto.get(&(top, prod.lhs)) else { return (steps, None) };
                states.push(next);
                syms.push(prod.lhs);
            }
        }
        if steps.len() > 10_000 {
            return (steps, None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 龙书图 4.31：表达式文法的 LR(0) 自动机有 12 个状态（I0–I11），SLR 分析表没有冲突。
    #[test]
    fn dragon_book_expression_grammar() {
        let g = Grammar::parse("E -> E + T | T\nT -> T * F | F\nF -> ( E ) | id").unwrap();
        let a = Automaton::build(&g);
        assert_eq!(a.states.len(), 12);
        let t = SlrTable::build(&a);
        assert!(t.conflicts().is_empty());
        let toks: Vec<Sym> = g.tokenize("id * ( id + id )").unwrap().into_iter().map(|(s, _)| s).collect();
        let (steps, tree) = parse(&a, &t, &toks);
        assert_eq!(steps.last().unwrap().action, Some(LrAction::Accept));
        assert_eq!(tree.unwrap().to_bracketed(&a.g), "E(T(T(F(id)) * F(( E(E(T(F(id))) + T(F(id))) ))))");
    }

    #[test]
    fn ambiguous_grammar_has_conflicts() {
        let g = Grammar::parse("E -> E + E | id").unwrap();
        let t = SlrTable::build(&Automaton::build(&g));
        assert!(!t.conflicts().is_empty());
    }
}
