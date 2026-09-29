//! LL(1) 分析：自顶向下、只看一个记号就决定用哪条产生式。
//!
//! 分析表 M[A, a] 回答："栈顶是非终结符 A、下一个记号是 a 时，用哪条产生式展开 A？"
//!
//! ```text
//! 对每条产生式 A → α：
//!     FIRST(α) 里的每个终结符 a：M[A, a] 填入 A → α
//!     如果 α 能推出空串：FOLLOW(A) 里的每个终结符 b：M[A, b] 也填入 A → α
//! ```
//!
//! 某一格被填了不止一条产生式，就是"冲突"——只看一个记号没法决定，这个文法不是 LL(1) 的。
//!
//! 分析时用一个显式的栈代替递归下降里的函数调用：栈顶是终结符就和输入比对，
//! 是非终结符就查表、把产生式右部倒着压栈。

use std::collections::BTreeMap;

use crate::first_follow::Sets;
use crate::{Grammar, Sym, Tree};

#[derive(Clone, Debug)]
pub struct Table {
    /// (非终结符, 终结符) → 产生式列表（多于一条就是冲突）。
    pub cells: BTreeMap<(Sym, Sym), Vec<usize>>,
}

impl Table {
    pub fn build(g: &Grammar, sets: &Sets) -> Table {
        let mut cells: BTreeMap<(Sym, Sym), Vec<usize>> = BTreeMap::new();
        for (pi, p) in g.productions.iter().enumerate() {
            let (first, nullable) = sets.first_of(&p.rhs);
            let mut targets: Vec<Sym> = first.into_iter().collect();
            if nullable {
                targets.extend(sets.follow[p.lhs].iter().copied());
            }
            for a in targets {
                let cell = cells.entry((p.lhs, a)).or_default();
                if !cell.contains(&pi) {
                    cell.push(pi);
                }
            }
        }
        Table { cells }
    }

    pub fn get(&self, nt: Sym, t: Sym) -> &[usize] {
        self.cells.get(&(nt, t)).map_or(&[], Vec::as_slice)
    }

    /// 有冲突的格子。
    pub fn conflicts(&self) -> Vec<(Sym, Sym)> {
        self.cells.iter().filter(|(_, v)| v.len() > 1).map(|(&k, _)| k).collect()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    /// 栈顶终结符和输入一致，一起消掉。
    Match(Sym),
    /// 查表，用产生式展开栈顶的非终结符。
    Expand(usize),
    Accept,
    /// 出错：栈顶符号、当前输入。
    Error {
        top: Sym,
        lookahead: Sym,
    },
}

#[derive(Clone, Debug)]
pub struct ParseStep {
    /// 执行动作**之前**的栈（从栈底到栈顶）。
    pub stack: Vec<Sym>,
    /// 当前输入位置（记号下标）。
    pub pos: usize,
    pub action: Action,
}

/// 表驱动的 LL(1) 分析。冲突的格子取第一条产生式。
/// 返回每一步的记录，以及成功时的语法树。
pub fn parse(g: &Grammar, table: &Table, tokens: &[Sym]) -> (Vec<ParseStep>, Option<Tree>) {
    let mut stack = vec![g.eof, g.start];
    let mut pos = 0;
    let mut steps = Vec::new();
    let mut expansions = Vec::new();
    let la = |pos: usize| tokens.get(pos).copied().unwrap_or(g.eof);
    loop {
        let top = *stack.last().unwrap();
        let a = la(pos);
        let action = if top == g.eof && a == g.eof {
            Action::Accept
        } else if g.is_terminal(top) {
            if top == a { Action::Match(a) } else { Action::Error { top, lookahead: a } }
        } else {
            match table.get(top, a).first() {
                Some(&p) => Action::Expand(p),
                None => Action::Error { top, lookahead: a },
            }
        };
        steps.push(ParseStep { stack: stack.clone(), pos, action: action.clone() });
        match action {
            Action::Accept => break,
            Action::Error { .. } => return (steps, None),
            Action::Match(_) => {
                stack.pop();
                pos += 1;
            }
            Action::Expand(p) => {
                stack.pop();
                stack.extend(g.productions[p].rhs.iter().rev());
                expansions.push(p);
            }
        }
        if steps.len() > 10_000 {
            return (steps, None);
        }
    }
    // 展开的顺序就是语法树的前序，按顺序重建树。
    let mut it = expansions.into_iter();
    let mut tok = 0;
    fn build(g: &Grammar, sym: Sym, it: &mut impl Iterator<Item = usize>, tok: &mut usize) -> Tree {
        let start = *tok;
        if g.is_terminal(sym) {
            *tok += 1;
            return Tree { sym, prod: None, children: vec![], start, end: *tok };
        }
        let p = it.next().expect("展开序列与记号一致");
        let children = g.productions[p].rhs.iter().map(|&s| build(g, s, it, tok)).collect();
        Tree { sym, prod: Some(p), children, start, end: *tok }
    }
    let tree = build(g, g.start, &mut it, &mut tok);
    (steps, Some(tree))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::first_follow::compute;

    /// 龙书例 4.30：消除左递归后的表达式文法的 FIRST、FOLLOW 与 LL(1) 分析表。
    #[test]
    fn dragon_book_expression_grammar() {
        let g = Grammar::parse("E -> T E'\nE' -> + T E' | ε\nT -> F T'\nT' -> * F T' | ε\nF -> ( E ) | id").unwrap();
        let s = compute(&g);
        let set = |xs: &std::collections::BTreeSet<Sym>| {
            let mut v: Vec<&str> = xs.iter().map(|&x| g.name(x)).collect();
            v.sort();
            v.join(" ")
        };
        let sym = |n: &str| g.symbol(n).unwrap();
        assert_eq!(set(&s.first[sym("E")]), "( id");
        assert_eq!(set(&s.first[sym("E'")]), "+");
        assert!(s.nullable[sym("E'")] && s.nullable[sym("T'")] && !s.nullable[sym("E")]);
        assert_eq!(set(&s.follow[sym("E")]), "$ )");
        assert_eq!(set(&s.follow[sym("T")]), "$ ) +");
        assert_eq!(set(&s.follow[sym("F")]), "$ ) * +");

        let t = Table::build(&g, &s);
        assert!(t.conflicts().is_empty());
        assert_eq!(t.get(sym("E'"), sym(")")), &[2]); // E' → ε
        let toks: Vec<Sym> = g.tokenize("id + id * id").unwrap().into_iter().map(|(s, _)| s).collect();
        let (steps, tree) = parse(&g, &t, &toks);
        assert!(tree.is_some());
        assert_eq!(steps.last().unwrap().action, Action::Accept);
    }

    #[test]
    fn left_recursion_conflicts() {
        let g = Grammar::parse("E -> E + T | T\nT -> id").unwrap();
        let t = Table::build(&g, &compute(&g));
        assert_eq!(t.conflicts(), vec![(g.symbol("E").unwrap(), g.symbol("id").unwrap())]);
    }
}
