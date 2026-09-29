//! DFA（确定有限自动机）与子集构造法。
//!
//! DFA 的每个状态、每个字符，**最多只有一条**出边，也没有 ε 边。
//! 所以运行 DFA 只需要记住"当前在哪个状态"，每读一个字符查一次表——非常快。
//! 真实的词法分析器（比如 lex/flex 生成的）跑的都是 DFA。
//!
//! ## 子集构造法
//!
//! 模拟 NFA 时，我们维护的是"当前可能所在的状态集合"。子集构造法的想法是：
//! **把每一个可能出现的集合，事先算出来，当成 DFA 的一个状态。**
//!
//! ```text
//! D0 = ε闭包({NFA 起点})
//! 对每个还没处理的 DFA 状态 D，对每一类字符 c：
//!     U = ε闭包(move(D, c))
//!     如果 U 是新出现的集合，就把它作为新的 DFA 状态加入待处理列表
//!     添加转移 D --c--> U
//! ```
//!
//! 集合里只要包含 NFA 的接受状态，这个 DFA 状态就是接受状态。

use std::collections::{BTreeMap, VecDeque};

use crate::charset::{CharSet, partition};
use crate::nfa::Nfa;
use crate::{StateId, StateSet};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DState {
    /// 这个 DFA 状态对应的 NFA 状态集合（最小化后的 DFA 里为空）。
    pub nfa_set: StateSet,
    /// 接受标记（多条规则时是优先级最高的规则编号）；`None` 表示不接受。
    pub accept: Option<usize>,
    /// 按字母表原子编号索引的转移；`None` 表示没有这条边（读到就失败）。
    pub trans: Vec<Option<StateId>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dfa {
    /// 字母表：两两不相交的字符集（"原子"）。转移按原子编号索引。
    pub alphabet: Vec<CharSet>,
    pub states: Vec<DState>,
    pub start: StateId,
}

/// 子集构造的一步。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SubsetStep {
    /// 第一步：起始状态 D0 = ε闭包({NFA 起点})。
    Start { closure: StateSet },
    /// 处理状态 `from` 在字母表原子 `atom` 上的转移。
    Explore {
        from: StateId,
        atom: usize,
        /// move：沿字符边走一步到达的 NFA 状态。
        moved: StateSet,
        /// 再求 ε 闭包。
        closure: StateSet,
        /// 转移到的 DFA 状态；集合为空时没有转移。
        to: Option<StateId>,
        /// `to` 是否是这一步新发现的状态。
        is_new: bool,
    },
}

/// 子集构造。返回 DFA 和逐步记录。
pub fn subset_construction(nfa: &Nfa) -> (Dfa, Vec<SubsetStep>) {
    let alphabet = partition(&nfa.charsets());
    let mut states: Vec<DState> = Vec::new();
    let mut index: BTreeMap<StateSet, StateId> = BTreeMap::new();
    let mut steps = Vec::new();

    let d0 = nfa.eps_closure(&[nfa.start].into_iter().collect());
    steps.push(SubsetStep::Start { closure: d0.clone() });
    index.insert(d0.clone(), 0);
    states.push(DState { accept: nfa.accept_tag(&d0), nfa_set: d0, trans: vec![None; alphabet.len()] });

    let mut work = VecDeque::from([0]);
    while let Some(d) = work.pop_front() {
        for (ai, atom) in alphabet.iter().enumerate() {
            let moved = nfa.move_on_set(&states[d].nfa_set, atom);
            let closure = nfa.eps_closure(&moved);
            let (to, is_new) = if closure.is_empty() {
                (None, false)
            } else if let Some(&existing) = index.get(&closure) {
                (Some(existing), false)
            } else {
                let id = states.len();
                index.insert(closure.clone(), id);
                states.push(DState {
                    accept: nfa.accept_tag(&closure),
                    nfa_set: closure.clone(),
                    trans: vec![None; alphabet.len()],
                });
                work.push_back(id);
                (Some(id), true)
            };
            states[d].trans[ai] = to;
            steps.push(SubsetStep::Explore { from: d, atom: ai, moved, closure, to, is_new });
        }
    }

    (Dfa { alphabet, states, start: 0 }, steps)
}

impl Dfa {
    /// 字符 `c` 属于字母表的哪个原子。
    pub fn atom_of(&self, c: char) -> Option<usize> {
        self.alphabet.iter().position(|a| a.contains(c))
    }

    pub fn next(&self, s: StateId, c: char) -> Option<StateId> {
        self.states[s].trans[self.atom_of(c)?]
    }

    /// 从起点开始读输入，返回经过的状态序列（遇到无路可走时停止）。
    pub fn run(&self, input: &str) -> Vec<StateId> {
        let mut path = vec![self.start];
        let mut s = self.start;
        for c in input.chars() {
            match self.next(s, c) {
                Some(t) => {
                    s = t;
                    path.push(t);
                }
                None => break,
            }
        }
        path
    }

    pub fn accepts_str(&self, input: &str) -> bool {
        let path = self.run(input);
        path.len() == input.chars().count() + 1 && self.states[*path.last().unwrap()].accept.is_some()
    }

    /// 画图用：把同一对状态之间的多条边合并，返回 (起点, 终点, 原子编号列表)。
    pub fn merged_edges(&self) -> Vec<(StateId, StateId, Vec<usize>)> {
        let mut map: BTreeMap<(StateId, StateId), Vec<usize>> = BTreeMap::new();
        for (s, st) in self.states.iter().enumerate() {
            for (a, t) in st.trans.iter().enumerate() {
                if let Some(t) = t {
                    map.entry((s, *t)).or_default().push(a);
                }
            }
        }
        map.into_iter().map(|((s, t), atoms)| (s, t, atoms)).collect()
    }

    /// 若干原子合起来的字符集的写法。
    pub fn atoms_label(&self, atoms: &[usize]) -> String {
        let set = atoms.iter().fold(CharSet::empty(), |acc, &a| acc.union(&self.alphabet[a]));
        set.label()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nfa::thompson;
    use crate::regex::Regex;

    fn dfa(src: &str) -> (Dfa, Vec<SubsetStep>) {
        subset_construction(&thompson(&Regex::parse(src).unwrap()).nfa)
    }

    #[test]
    fn dragon_book_example() {
        // 龙书图 3.35：(a|b)*abb 经子集构造得到 5 个状态 A–E。
        let (d, steps) = dfa("(a|b)*abb");
        assert_eq!(d.states.len(), 5);
        assert_eq!(d.alphabet, vec![CharSet::single('a'), CharSet::single('b')]);
        assert_eq!(steps.len(), 1 + 5 * 2);
        assert_eq!(d.states.iter().filter(|s| s.accept.is_some()).count(), 1);
        assert!(d.accepts_str("abb") && d.accepts_str("aabb") && !d.accepts_str("abab"));
    }

    #[test]
    fn overlapping_classes_are_split() {
        let (d, _) = dfa("[a-z]+|if");
        assert_eq!(d.alphabet.len(), 3); // [a-eg-hj-z]、f、i
        assert!(d.accepts_str("if") && d.accepts_str("xyz") && !d.accepts_str("i1"));
    }
}
