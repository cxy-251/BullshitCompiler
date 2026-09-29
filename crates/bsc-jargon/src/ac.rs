//! Aho-Corasick 多模式匹配自动机。
//!
//! 词典可能有成千上万个词。对每个位置逐个词去比，词典越大越慢；Aho-Corasick 把所有词建成一棵**字典树**，
//! 再给每个结点加一条**失败链接**：匹配不下去时，跳到"当前已读内容的最长后缀、同时又是某个词的前缀"的结点，
//! 继续往下走，不用回退输入。于是扫一遍输入，就能找出所有词的所有出现位置，时间只和输入长度、匹配个数有关，
//! 和词典大小无关——词典膨胀也不怕。
//!
//! 它其实就是第 1 章讲的 DFA 思想：字典树是状态，失败链接让每个状态在任何输入字符下都有去处。

use std::collections::{HashMap, VecDeque};

#[derive(Clone, Debug, Default)]
pub struct AhoCorasick {
    goto: Vec<HashMap<char, usize>>,
    fail: Vec<usize>,
    /// 到达这个状态时匹配到的词条（包括沿失败链接能到的更短的词）。
    out: Vec<Vec<usize>>,
    /// 每个词条的长度（字符数）。
    len: HashMap<usize, usize>,
}

/// 一次匹配：第 `start`..`end` 个字符是词条 `entry`。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Match {
    pub start: usize,
    pub end: usize,
    pub entry: usize,
}

impl AhoCorasick {
    /// `words`：(词条编号, 词)。
    pub fn build<'a>(words: impl IntoIterator<Item = (usize, &'a str)>) -> Self {
        let mut ac = AhoCorasick { goto: vec![HashMap::new()], fail: vec![0], out: vec![vec![]], len: HashMap::new() };
        for (id, w) in words {
            let mut s = 0;
            for c in w.chars() {
                s = match ac.goto[s].get(&c) {
                    Some(&t) => t,
                    None => {
                        ac.goto.push(HashMap::new());
                        ac.fail.push(0);
                        ac.out.push(vec![]);
                        let t = ac.goto.len() - 1;
                        ac.goto[s].insert(c, t);
                        t
                    }
                };
            }
            ac.out[s].push(id);
            ac.len.insert(id, w.chars().count());
        }
        // 按层（广度优先）计算失败链接：孩子的失败链接 = 沿父亲的失败链接找能接上同一个字符的状态
        let mut queue: VecDeque<usize> = ac.goto[0].values().copied().collect();
        while let Some(s) = queue.pop_front() {
            let edges: Vec<(char, usize)> = ac.goto[s].iter().map(|(&c, &t)| (c, t)).collect();
            for (c, t) in edges {
                queue.push_back(t);
                let mut f = ac.fail[s];
                let target = loop {
                    if let Some(&x) = ac.goto[f].get(&c) {
                        break x;
                    }
                    if f == 0 {
                        break 0;
                    }
                    f = ac.fail[f];
                };
                ac.fail[t] = target;
                let inherited = ac.out[target].clone();
                ac.out[t].extend(inherited);
            }
        }
        ac
    }

    /// 状态个数（字典树结点数）。
    pub fn states(&self) -> usize {
        self.goto.len()
    }

    /// 找出所有词条的所有出现位置。
    pub fn find_all(&self, text: &[char]) -> Vec<Match> {
        let mut out = Vec::new();
        let mut s = 0;
        for (i, &c) in text.iter().enumerate() {
            loop {
                if let Some(&t) = self.goto[s].get(&c) {
                    s = t;
                    break;
                }
                if s == 0 {
                    break;
                }
                s = self.fail[s];
            }
            for &e in &self.out[s] {
                let l = self.len[&e];
                out.push(Match { start: i + 1 - l, end: i + 1, entry: e });
            }
        }
        out.sort_by_key(|m| (m.start, m.end, m.entry));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 和最朴素的做法（每个位置、每个词逐一比较）结果完全一致。
    #[test]
    fn agrees_with_naive_search() {
        let words = ["赋能", "能力", "全链路", "链路", "闭环", "全", "路闭", "以", "为抓手", "抓手"];
        let ac = AhoCorasick::build(words.iter().enumerate().map(|(i, w)| (i, *w)));
        let mut seed = 3u64;
        let alphabet: Vec<char> = "赋能力全链路闭环以为抓手的".chars().collect();
        for _ in 0..500 {
            let n = (seed % 20) as usize;
            let text: Vec<char> = (0..n)
                .map(|_| {
                    seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                    alphabet[(seed >> 33) as usize % alphabet.len()]
                })
                .collect();
            let mut naive = Vec::new();
            for start in 0..text.len() {
                for (i, w) in words.iter().enumerate() {
                    let w: Vec<char> = w.chars().collect();
                    if text[start..].starts_with(&w) {
                        naive.push(Match { start, end: start + w.len(), entry: i });
                    }
                }
            }
            naive.sort_by_key(|m| (m.start, m.end, m.entry));
            assert_eq!(ac.find_all(&text), naive);
        }
    }
}
