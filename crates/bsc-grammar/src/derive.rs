//! 推导：从开始符号出发，一次把一个非终结符换成它某条产生式的右部，最终得到句子。
//!
//! "最左推导"每次都替换最左边的那个非终结符。一棵语法树对应唯一一个最左推导：
//! 按前序（先根、再从左到右的孩子）访问树的非终结符节点，依次展开即可。

use crate::{Grammar, Sym, Tree};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DerivStep {
    /// 这一步之后的句型：(符号, 对应语法树节点在 `Tree::flatten` 里的下标)。
    pub form: Vec<(Sym, usize)>,
    /// 这一步展开的节点和所用的产生式（第 0 步是开始符号本身，为 `None`）。
    pub expanded: Option<(usize, usize)>,
    /// 被替换的非终结符在上一步句型里的位置。
    pub at: usize,
}

/// 由语法树得到最左推导的每一步。
pub fn leftmost(tree: &Tree) -> Vec<DerivStep> {
    let flat = tree.flatten();
    let mut form: Vec<(Sym, usize)> = vec![(tree.sym, 0)];
    let mut steps = vec![DerivStep { form: form.clone(), expanded: None, at: 0 }];
    // 每次找最左边的、还没展开的非终结符节点（终结符节点没有产生式）
    while let Some(at) = form.iter().position(|&(_, node)| flat[node].0.prod.is_some()) {
        let node = form[at].1;
        let (t, kids) = &flat[node];
        let replacement: Vec<(Sym, usize)> = kids.iter().map(|&k| (flat[k].0.sym, k)).collect();
        form.splice(at..=at, replacement);
        steps.push(DerivStep { form: form.clone(), expanded: Some((node, t.prod.unwrap())), at });
    }
    steps
}

/// 用文法随机生成一个句子（`seed` 决定结果）。句型长度超过 `budget` 后，
/// 每次都选"最快能结束"的产生式，保证能停下来。文法有推不出句子的开始符号时返回 `None`。
pub fn generate(g: &Grammar, seed: u64, budget: usize) -> Option<Vec<Sym>> {
    // 每个符号最少需要几层展开才能变成全是终结符（无穷大表示推不出来）
    let mut height = vec![usize::MAX; g.num_symbols()];
    for t in g.terminals().chain([g.eof]) {
        height[t] = 0;
    }
    let mut changed = true;
    while changed {
        changed = false;
        for p in &g.productions {
            let h = p.rhs.iter().map(|&s| height[s]).max().unwrap_or(0);
            if h != usize::MAX && h + 1 < height[p.lhs] {
                height[p.lhs] = h + 1;
                changed = true;
            }
        }
    }
    if height[g.start] == usize::MAX {
        return None;
    }
    let prod_height = |p: usize| g.productions[p].rhs.iter().map(|&s| height[s]).max().unwrap_or(0);

    let mut rng = seed.wrapping_mul(0x9E3779B97F4A7C15) | 1;
    let mut next = |n: usize| {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        (rng % n as u64) as usize
    };
    let mut form = vec![g.start];
    for _ in 0..budget * 20 {
        let Some(at) = form.iter().position(|&s| !g.is_terminal(s)) else { return Some(form) };
        let choices: Vec<usize> =
            g.prods_of(form[at]).map(|(p, _)| p).filter(|&p| prod_height(p) != usize::MAX).collect();
        let pick = if form.len() > budget {
            *choices.iter().min_by_key(|&&p| prod_height(p)).unwrap()
        } else {
            choices[next(choices.len())]
        };
        form.splice(at..=at, g.productions[pick].rhs.iter().copied());
    }
    None
}
