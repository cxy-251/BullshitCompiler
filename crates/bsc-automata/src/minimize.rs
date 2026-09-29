//! DFA 最小化：划分细化法（Moore 算法）。
//!
//! 两个状态"等价"，是说从它们出发，对**任何**后续输入，接受与否都完全一样——
//! 那它们就可以合并成一个状态。最小化就是找出所有等价的状态并合并。
//!
//! 直接判断"任何输入"做不到，于是反过来：先假设能合并的都合并，再不断找反例拆开。
//!
//! ```text
//! 初始划分：{接受状态} 和 {非接受状态} 两组（接受不同规则的也要分开）
//! 重复：
//!     找一个组 G 和一类字符 c，使得 G 里的状态读 c 之后落到了不同的组
//!     → 它们不可能等价，按"落到哪个组"把 G 拆开
//! 直到没有组可拆。每组合并成最小 DFA 的一个状态。
//! ```
//!
//! 为了让"读 c 之后没有路可走"也能参与比较，先补一个**死状态**：
//! 所有缺失的边都指向它，它自己读什么都回到自己，永远不接受。最后再把它删掉。
//!
//! Hopcroft 算法是同一思路的高效版本：它用一个"待处理的划分者"工作表，
//! 每次只拿较小的那一半去切别的组，把复杂度降到 O(n·k·log n)。

use std::collections::{BTreeMap, VecDeque};

use crate::StateId;
use crate::dfa::{DState, Dfa};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MinStep {
    /// 初始划分。
    Init { blocks: Vec<Vec<StateId>> },
    /// 第 `block` 组在原子 `atom` 上"意见不一"，被拆开。
    Split {
        block: usize,
        atom: usize,
        /// 组里每个状态读 `atom` 后落到的组（拆分前的组编号）。
        targets: Vec<(StateId, usize)>,
        /// 拆分后的完整划分。
        blocks: Vec<Vec<StateId>>,
    },
    /// 再也拆不动了。
    Done { blocks: Vec<Vec<StateId>> },
}

impl MinStep {
    pub fn blocks(&self) -> &[Vec<StateId>] {
        match self {
            MinStep::Init { blocks } | MinStep::Split { blocks, .. } | MinStep::Done { blocks } => blocks,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Minimization {
    /// 最小 DFA。
    pub dfa: Dfa,
    /// 最小 DFA 的每个状态由原 DFA 的哪些状态合并而来。
    pub members: Vec<Vec<StateId>>,
    /// 补上的死状态在划分里的编号（等于原 DFA 的状态数）；原 DFA 是完全的则为 `None`。
    pub dead: Option<StateId>,
    pub steps: Vec<MinStep>,
}

pub fn minimize(dfa: &Dfa) -> Minimization {
    let n = dfa.states.len();
    let k = dfa.alphabet.len();

    // 补死状态，得到"完全"的转移表 delta[s][a]。
    let needs_dead = dfa.states.iter().any(|s| s.trans.iter().any(Option::is_none));
    let dead = needs_dead.then_some(n);
    let total = n + usize::from(needs_dead);
    let delta: Vec<Vec<StateId>> = (0..total)
        .map(|s| (0..k).map(|a| if s < n { dfa.states[s].trans[a].unwrap_or(n) } else { n }).collect())
        .collect();
    let accept = |s: StateId| if s < n { dfa.states[s].accept } else { None };

    // 初始划分：按接受标记分组，不接受的在最前面。
    let mut groups: BTreeMap<Option<usize>, Vec<StateId>> = BTreeMap::new();
    for s in 0..total {
        groups.entry(accept(s)).or_default().push(s);
    }
    let mut blocks: Vec<Vec<StateId>> = groups.into_values().collect();
    let mut steps = vec![MinStep::Init { blocks: blocks.clone() }];

    loop {
        let block_of = block_index(&blocks, total);
        let split = blocks.iter().enumerate().filter(|(_, b)| b.len() > 1).find_map(|(bi, b)| {
            (0..k).find_map(|a| {
                let targets: Vec<(StateId, usize)> = b.iter().map(|&s| (s, block_of[delta[s][a]])).collect();
                let first = targets[0].1;
                targets.iter().any(|&(_, t)| t != first).then_some((bi, a, targets))
            })
        });
        let Some((bi, atom, targets)) = split else { break };

        // 按目标组拆开，组内保持原顺序；第一组留在原位置，其余追加到末尾。
        let mut by_target: BTreeMap<usize, Vec<StateId>> = BTreeMap::new();
        for &(s, t) in &targets {
            by_target.entry(t).or_default().push(s);
        }
        let mut parts: Vec<Vec<StateId>> = by_target.into_values().collect();
        parts.sort_by_key(|p| p[0]);
        let mut parts = parts.into_iter();
        blocks[bi] = parts.next().unwrap();
        blocks.extend(parts);
        steps.push(MinStep::Split { block: bi, atom, targets, blocks: blocks.clone() });
    }
    steps.push(MinStep::Done { blocks: blocks.clone() });

    // 由最终划分构造最小 DFA：去掉死状态所在的组，从起点所在的组出发按广度优先编号。
    let block_of = block_index(&blocks, total);
    let dead_block = dead.map(|d| block_of[d]);
    let mut new_id: BTreeMap<usize, StateId> = BTreeMap::new();
    let mut order: Vec<usize> = Vec::new();
    let mut queue = VecDeque::from([block_of[dfa.start]]);
    new_id.insert(block_of[dfa.start], 0);
    order.push(block_of[dfa.start]);
    while let Some(b) = queue.pop_front() {
        let rep = blocks[b][0];
        for a in 0..k {
            let t = block_of[delta[rep][a]];
            if Some(t) != dead_block && !new_id.contains_key(&t) {
                new_id.insert(t, order.len());
                order.push(t);
                queue.push_back(t);
            }
        }
    }
    let states = order
        .iter()
        .map(|&b| {
            let rep = blocks[b][0];
            DState {
                nfa_set: Default::default(),
                accept: accept(rep),
                trans: (0..k).map(|a| new_id.get(&block_of[delta[rep][a]]).copied()).collect(),
            }
        })
        .collect();
    let members = order.iter().map(|&b| blocks[b].clone()).collect();

    Minimization { dfa: Dfa { alphabet: dfa.alphabet.clone(), states, start: 0 }, members, dead, steps }
}

fn block_index(blocks: &[Vec<StateId>], total: usize) -> Vec<usize> {
    let mut idx = vec![0; total];
    for (bi, b) in blocks.iter().enumerate() {
        for &s in b {
            idx[s] = bi;
        }
    }
    idx
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dfa::subset_construction;
    use crate::nfa::thompson;
    use crate::regex::Regex;

    fn min(src: &str) -> Minimization {
        let (d, _) = subset_construction(&thompson(&Regex::parse(src).unwrap()).nfa);
        minimize(&d)
    }

    #[test]
    fn dragon_book_example() {
        // 龙书例 3.40：(a|b)*abb 的 DFA 从 5 个状态最小化为 4 个（A 与 C 合并）。
        let m = min("(a|b)*abb");
        assert_eq!(m.dfa.states.len(), 4);
        assert!(m.members.iter().any(|g| g.len() == 2));
        assert_eq!(m.dead, None); // 这个 DFA 本来就是完全的
        assert!(m.dfa.accepts_str("babb") && !m.dfa.accepts_str("abba"));
    }

    #[test]
    fn classic_sizes() {
        assert_eq!(min("a*").dfa.states.len(), 1);
        assert_eq!(min("(a|b)*a(a|b)").dfa.states.len(), 4);
        assert_eq!(min("a|b").dfa.states.len(), 2);
        assert_eq!(min("(a*b*)*").dfa.states.len(), 1);
    }

    #[test]
    fn dead_state_is_added_and_removed() {
        let m = min("ab");
        assert!(m.dead.is_some());
        assert_eq!(m.dfa.states.len(), 3);
        assert!(m.dfa.states.iter().all(|s| s.trans.iter().filter(|t| t.is_some()).count() <= 1));
    }

    #[test]
    fn different_tags_never_merge() {
        use crate::nfa::thompson_many;
        let rs: Vec<Regex> = ["a", "b"].iter().map(|s| Regex::parse(s).unwrap()).collect();
        let (d, _) = subset_construction(&thompson_many(&rs.iter().collect::<Vec<_>>()).nfa);
        let m = minimize(&d);
        let accepts: Vec<_> = m.dfa.states.iter().filter_map(|s| s.accept).collect();
        assert_eq!(accepts.len(), 2);
    }
}
