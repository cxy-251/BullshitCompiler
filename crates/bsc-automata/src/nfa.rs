//! NFA（非确定有限自动机）与 Thompson 构造法。
//!
//! NFA 是一张图：圆圈是状态，箭头上写着字符。从起始状态出发，每读一个字符
//! 就沿着写着这个字符的箭头走；读完输入时如果停在接受状态（双圈），就算匹配成功。
//!
//! "非确定"指两件事：
//! 1. 同一个状态、同一个字符，可能有多条箭头可走；
//! 2. 有些箭头上写的是 ε，表示"不读字符也能走过去"。
//!
//! 所以 NFA 在同一时刻可能**同时**处于好几个状态——模拟它时要维护一个状态集合。
//!
//! ## Thompson 构造法
//!
//! 把正则表达式的语法树自底向上翻译：每个节点变成一个"片段"（一个入口、一个出口），
//! 再按节点类型用 ε 边把孩子的片段接起来：
//!
//! ```text
//! 字符 a:     (s) --a--> (f)
//! 连接 AB:    [A] --ε--> [B]
//! 选择 A|B:   (s) --ε--> [A] --ε--> (f)
//!              └---ε--> [B] --ε---┘
//! 重复 A*:    (s) --ε--> [A] --ε--> (f)，并且 A 的出口 --ε--> A 的入口，(s) --ε--> (f)
//! ```

use std::collections::{BTreeMap, VecDeque};

use crate::charset::CharSet;
use crate::regex::{Regex, RegexId, RegexKind};
use crate::{StateId, StateSet};

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Label {
    Eps,
    Set(CharSet),
}

impl Label {
    pub fn text(&self) -> String {
        match self {
            Label::Eps => "ε".to_owned(),
            Label::Set(s) => s.label(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edge {
    pub from: StateId,
    pub to: StateId,
    pub label: Label,
}

#[derive(Clone, Debug, Default)]
pub struct Nfa {
    pub num_states: usize,
    pub edges: Vec<Edge>,
    pub start: StateId,
    /// 接受状态 → 标记。单条正则时标记都是 0；词法分析器里标记是规则编号，
    /// 编号越小优先级越高。
    pub accepts: BTreeMap<StateId, usize>,
}

/// Thompson 构造的一步：为语法树的一个节点造出一个片段。
#[derive(Clone, Debug)]
pub struct ThompsonStep {
    /// 第几条正则（多条正则合并时有用）。
    pub pattern: usize,
    /// 对应的语法树节点；`None` 表示最后"把多条正则合并起来"那一步。
    pub node: Option<RegexId>,
    /// 这一步新建的状态。
    pub new_states: Vec<StateId>,
    /// 这一步新建的边（`Nfa::edges` 中的下标）。
    pub new_edges: Vec<usize>,
    /// 这个片段的入口和出口。
    pub start: StateId,
    pub accept: StateId,
}

#[derive(Clone, Debug)]
pub struct Construction {
    pub nfa: Nfa,
    pub steps: Vec<ThompsonStep>,
}

/// 用 Thompson 构造法把一条正则表达式变成 NFA。
pub fn thompson(regex: &Regex) -> Construction {
    thompson_many(&[regex])
}

/// 把多条正则分别构造成 NFA，再用一个新的起始状态经 ε 边连向它们。
/// 第 i 条正则的接受状态带标记 i。只有一条正则时不做合并。
pub fn thompson_many(regexes: &[&Regex]) -> Construction {
    let mut b = Builder::default();
    let mut frags = Vec::new();
    for (i, r) in regexes.iter().enumerate() {
        b.pattern = i;
        let (s, f) = b.build(r, r.root);
        b.accepts.insert(f, i);
        frags.push((s, f));
    }
    let start = if let [(s, _)] = frags[..] {
        s
    } else {
        let s = b.new_state();
        for &(fs, _) in &frags {
            b.edge(s, fs, Label::Eps);
        }
        b.finish_step(None, s, s);
        s
    };
    b.into_construction(start)
}

#[derive(Default)]
struct Builder {
    num_states: usize,
    edges: Vec<Edge>,
    accepts: BTreeMap<StateId, usize>,
    steps: Vec<ThompsonStep>,
    pattern: usize,
    // 当前这一步里新建的东西
    cur_states: Vec<StateId>,
    cur_edges: Vec<usize>,
}

impl Builder {
    fn new_state(&mut self) -> StateId {
        let s = self.num_states;
        self.num_states += 1;
        self.cur_states.push(s);
        s
    }

    fn edge(&mut self, from: StateId, to: StateId, label: Label) {
        self.cur_edges.push(self.edges.len());
        self.edges.push(Edge { from, to, label });
    }

    fn finish_step(&mut self, node: Option<RegexId>, start: StateId, accept: StateId) {
        self.steps.push(ThompsonStep {
            pattern: self.pattern,
            node,
            new_states: std::mem::take(&mut self.cur_states),
            new_edges: std::mem::take(&mut self.cur_edges),
            start,
            accept,
        });
    }

    /// 后序遍历：先造孩子的片段，再造自己的。返回片段的 (入口, 出口)。
    fn build(&mut self, r: &Regex, id: RegexId) -> (StateId, StateId) {
        let (s, f) = match &r.node(id).kind {
            RegexKind::Empty => {
                let (s, f) = (self.new_state(), self.new_state());
                self.edge(s, f, Label::Eps);
                (s, f)
            }
            RegexKind::Set(set) => {
                let (s, f) = (self.new_state(), self.new_state());
                self.edge(s, f, Label::Set(set.clone()));
                (s, f)
            }
            RegexKind::Concat(a, b) => {
                let (a_s, a_f) = self.build(r, *a);
                let (b_s, b_f) = self.build(r, *b);
                self.edge(a_f, b_s, Label::Eps);
                (a_s, b_f)
            }
            RegexKind::Alt(a, b) => {
                let (a_s, a_f) = self.build(r, *a);
                let (b_s, b_f) = self.build(r, *b);
                let (s, f) = (self.new_state(), self.new_state());
                self.edge(s, a_s, Label::Eps);
                self.edge(s, b_s, Label::Eps);
                self.edge(a_f, f, Label::Eps);
                self.edge(b_f, f, Label::Eps);
                (s, f)
            }
            RegexKind::Star(a) | RegexKind::Plus(a) | RegexKind::Optional(a) => {
                let kind = r.node(id).kind.clone();
                let (a_s, a_f) = self.build(r, *a);
                let (s, f) = (self.new_state(), self.new_state());
                self.edge(s, a_s, Label::Eps);
                self.edge(a_f, f, Label::Eps);
                if !matches!(kind, RegexKind::Plus(_)) {
                    self.edge(s, f, Label::Eps); // 可以一次都不走
                }
                if !matches!(kind, RegexKind::Optional(_)) {
                    self.edge(a_f, a_s, Label::Eps); // 可以再来一次
                }
                (s, f)
            }
        };
        self.finish_step(Some(id), s, f);
        (s, f)
    }

    /// 按从起点出发的广度优先顺序给状态重新编号，让编号大致"从左到右"递增，更好读。
    fn into_construction(self, start: StateId) -> Construction {
        let n = self.num_states;
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
        for (i, e) in self.edges.iter().enumerate() {
            adj[e.from].push(i);
        }
        let mut order = vec![usize::MAX; n];
        let mut next = 0;
        let mut queue = VecDeque::from([start]);
        order[start] = 0;
        next += 1;
        while let Some(s) = queue.pop_front() {
            for &ei in &adj[s] {
                let t = self.edges[ei].to;
                if order[t] == usize::MAX {
                    order[t] = next;
                    next += 1;
                    queue.push_back(t);
                }
            }
        }
        // Thompson 构造出的每个状态都能从起点到达，这里只是防御。
        for o in order.iter_mut().filter(|o| **o == usize::MAX) {
            *o = next;
            next += 1;
        }

        let edges = self.edges.into_iter().map(|e| Edge { from: order[e.from], to: order[e.to], ..e }).collect();
        let accepts = self.accepts.into_iter().map(|(s, t)| (order[s], t)).collect();
        let steps = self
            .steps
            .into_iter()
            .map(|st| ThompsonStep {
                new_states: st.new_states.into_iter().map(|s| order[s]).collect(),
                start: order[st.start],
                accept: order[st.accept],
                ..st
            })
            .collect();
        Construction { nfa: Nfa { num_states: n, edges, start: order[start], accepts }, steps }
    }
}

/// 模拟 NFA 的一步：读入一个字符后的状态集合。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SimStep {
    /// 这一步读入的字符及其字节位置；第 0 步（开始前）为 `None`。
    pub ch: Option<(usize, char)>,
    /// 沿字符边走一步后到达的状态（还没算 ε 闭包）。
    pub moved: StateSet,
    /// 再加上 ε 闭包后的状态集合——这才是"当前所在的状态"。
    pub current: StateSet,
}

impl Nfa {
    pub fn is_accepting(&self, s: StateId) -> bool {
        self.accepts.contains_key(&s)
    }

    /// 集合里优先级最高（标记最小）的接受标记。
    pub fn accept_tag(&self, set: &StateSet) -> Option<usize> {
        set.iter().filter_map(|s| self.accepts.get(s)).min().copied()
    }

    /// ε 闭包：从集合里的状态出发，只走 ε 边能到达的全部状态（包括它们自己）。
    pub fn eps_closure(&self, set: &StateSet) -> StateSet {
        let mut result = set.clone();
        let mut stack: Vec<StateId> = set.iter().copied().collect();
        while let Some(s) = stack.pop() {
            for e in self.edges.iter().filter(|e| e.from == s && e.label == Label::Eps) {
                if result.insert(e.to) {
                    stack.push(e.to);
                }
            }
        }
        result
    }

    /// 从集合里的状态出发，沿着能接受字符 `c` 的边走一步。
    pub fn move_on(&self, set: &StateSet, c: char) -> StateSet {
        self.edges
            .iter()
            .filter(|e| set.contains(&e.from) && matches!(&e.label, Label::Set(cs) if cs.contains(c)))
            .map(|e| e.to)
            .collect()
    }

    /// 同 [`Nfa::move_on`]，但按字符集走（子集构造用）。
    pub fn move_on_set(&self, set: &StateSet, atom: &CharSet) -> StateSet {
        let probe = atom.first().expect("原子字符集非空");
        self.move_on(set, probe)
    }

    /// 逐字符模拟，记录每一步的状态集合。状态集合变空时提前停止。
    pub fn simulate(&self, input: &str) -> Vec<SimStep> {
        let start: StateSet = [self.start].into_iter().collect();
        let mut current = self.eps_closure(&start);
        let mut steps = vec![SimStep { ch: None, moved: start, current: current.clone() }];
        for (i, c) in input.char_indices() {
            if current.is_empty() {
                break;
            }
            let moved = self.move_on(&current, c);
            current = self.eps_closure(&moved);
            steps.push(SimStep { ch: Some((i, c)), moved, current: current.clone() });
        }
        steps
    }

    /// 整个输入是否被接受。
    pub fn accepts_str(&self, input: &str) -> bool {
        let steps = self.simulate(input);
        steps.len() == input.chars().count() + 1 && steps.last().is_some_and(|s| self.accept_tag(&s.current).is_some())
    }

    /// NFA 边上出现过的所有字符集。
    pub fn charsets(&self) -> Vec<CharSet> {
        let mut sets: Vec<CharSet> = self
            .edges
            .iter()
            .filter_map(|e| match &e.label {
                Label::Set(s) => Some(s.clone()),
                Label::Eps => None,
            })
            .collect();
        sets.sort();
        sets.dedup();
        sets
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nfa(src: &str) -> Construction {
        thompson(&Regex::parse(src).unwrap())
    }

    #[test]
    fn single_char() {
        let c = nfa("a");
        assert_eq!(c.nfa.num_states, 2);
        assert_eq!(c.nfa.start, 0);
        assert_eq!(c.nfa.accepts, [(1, 0)].into_iter().collect());
        assert_eq!(c.steps.len(), 1);
    }

    #[test]
    fn dragon_book_example_sizes() {
        // 龙书 3.7 节的例子 (a|b)*abb。书上把连接的两个片段首尾状态合并，得到 11 个状态；
        // 本实现用 ε 边连接（更直观），5 个字符片段各 2 个状态 + 选择 2 + 星号 2 = 14 个。
        let c = nfa("(a|b)*abb");
        assert_eq!(c.nfa.num_states, 14);
        // 语法树的每个节点一步：5 个字符 + 1 个选择 + 1 个星号 + 3 个连接
        assert_eq!(c.steps.len(), 10);
        let total: usize = c.steps.iter().map(|s| s.new_states.len()).sum();
        assert_eq!(total, c.nfa.num_states);
    }

    #[test]
    fn states_are_numbered_bfs() {
        let c = nfa("ab");
        // 0 -a-> 1 -ε-> 2 -b-> 3
        assert_eq!(
            c.nfa.edges.iter().map(|e| (e.from, e.to)).collect::<std::collections::BTreeSet<_>>(),
            [(0, 1), (1, 2), (2, 3)].into_iter().collect()
        );
    }

    #[test]
    fn simulation() {
        let c = nfa("(a|b)*abb");
        assert!(c.nfa.accepts_str("abb"));
        assert!(c.nfa.accepts_str("babb"));
        assert!(!c.nfa.accepts_str("ab"));
        assert!(!c.nfa.accepts_str("abbc"));
        let steps = c.nfa.simulate("ab");
        assert_eq!(steps.len(), 3);
        assert!(steps[0].current.contains(&c.nfa.start));
    }

    #[test]
    fn many_patterns_get_tags() {
        let rs: Vec<Regex> = ["if", "[a-z]+"].iter().map(|s| Regex::parse(s).unwrap()).collect();
        let c = thompson_many(&rs.iter().collect::<Vec<_>>());
        let tags: Vec<usize> = c.nfa.accepts.values().copied().collect();
        assert_eq!(tags.len(), 2);
        assert!(tags.contains(&0) && tags.contains(&1));
        assert_eq!(c.steps.last().unwrap().node, None);
    }
}
