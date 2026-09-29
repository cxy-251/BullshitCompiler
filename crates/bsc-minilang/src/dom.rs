//! 支配关系、支配树、支配边界。
//!
//! 在控制流图里，如果从入口到 n 的**每一条**路径都经过 d，就说 d **支配** n（d dom n）。
//! 每个结点都支配它自己，入口支配所有结点。
//!
//! 计算方法是一个不动点迭代（数据流分析的雏形）：
//!
//! ```text
//! Dom(入口) = {入口}
//! Dom(n)    = {n} ∪ ( ∩ Dom(p)，p 取遍 n 的所有前驱 )
//! ```
//!
//! 一开始除入口外都设成"全部结点"，然后反复按上式更新，直到一轮下来什么都没变。
//!
//! - **直接支配者** idom(n)：n 的严格支配者里离 n 最近的那个。把每个结点连到它的 idom，得到**支配树**。
//! - **支配边界** DF(d)：d 支配了 n 的某个前驱、却不严格支配 n 的那些 n——也就是 d 的"势力范围"刚好结束的地方。
//!   SSA 构造时，φ 函数就放在变量定义所在块的支配边界上。

/// 计算过程的一步。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    /// 开始第 k 轮迭代（从 1 数）。
    Round(usize),
    /// 按公式重新算 Dom(node)。`preds` 是参与求交的前驱。
    Update {
        node: usize,
        preds: Vec<usize>,
        before: Vec<usize>,
        after: Vec<usize>,
    },
    /// 一轮下来没有任何变化，迭代结束。
    Stable(usize),
    Idom {
        node: usize,
        idom: usize,
    },
    /// 求支配边界：从汇合点 join 的前驱 pred 出发往支配树上走，走到的 runner 的 DF 里加上 join。
    Df {
        join: usize,
        pred: usize,
        runner: usize,
    },
}

#[derive(Clone, Debug)]
pub struct Dominators {
    /// Dom(n)，从小到大排列。
    pub dom: Vec<Vec<usize>>,
    pub idom: Vec<Option<usize>>,
    /// 支配树上的孩子。
    pub children: Vec<Vec<usize>>,
    pub df: Vec<Vec<usize>>,
    /// 逆后序（迭代时按这个顺序访问结点，收敛最快）。
    pub rpo: Vec<usize>,
    pub steps: Vec<Step>,
}

fn preds_of(succs: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let mut preds = vec![Vec::new(); succs.len()];
    for (b, ss) in succs.iter().enumerate() {
        for &s in ss {
            if !preds[s].contains(&b) {
                preds[s].push(b);
            }
        }
    }
    preds
}

/// 逆后序：深度优先遍历，按"离开结点"的顺序倒过来排。
pub fn reverse_postorder(succs: &[Vec<usize>]) -> Vec<usize> {
    let mut seen = vec![false; succs.len()];
    let mut post = Vec::new();
    // 显式栈：(结点, 下一个要看的后继序号)
    let mut stack = vec![(0usize, 0usize)];
    seen[0] = true;
    while let Some(&mut (n, ref mut i)) = stack.last_mut() {
        if let Some(&s) = succs[n].get(*i) {
            *i += 1;
            if !seen[s] {
                seen[s] = true;
                stack.push((s, 0));
            }
        } else {
            post.push(n);
            stack.pop();
        }
    }
    post.reverse();
    post
}

/// 入口是 0 号结点；所有结点都必须能从入口走到。
pub fn dominators(succs: &[Vec<usize>]) -> Dominators {
    let n = succs.len();
    let preds = preds_of(succs);
    let rpo = reverse_postorder(succs);
    let all: Vec<usize> = (0..n).collect();
    let mut dom: Vec<Vec<usize>> = (0..n).map(|i| if i == 0 { vec![0] } else { all.clone() }).collect();
    let mut steps = Vec::new();

    let mut round = 0;
    loop {
        round += 1;
        steps.push(Step::Round(round));
        let mut changed = false;
        for &b in rpo.iter().filter(|&&b| b != 0) {
            let mut new: Vec<usize> = all.clone();
            for &p in &preds[b] {
                new.retain(|x| dom[p].contains(x));
            }
            if !new.contains(&b) {
                new.push(b);
                new.sort_unstable();
            }
            steps.push(Step::Update { node: b, preds: preds[b].clone(), before: dom[b].clone(), after: new.clone() });
            if new != dom[b] {
                dom[b] = new;
                changed = true;
            }
        }
        if !changed {
            steps.push(Step::Stable(round));
            break;
        }
    }

    // 直接支配者：严格支配者中，被其余所有严格支配者支配的那个（它的 Dom 集合恰好是 n 的严格支配者全体）
    let mut idom = vec![None; n];
    let mut children = vec![Vec::new(); n];
    for &b in rpo.iter().filter(|&&b| b != 0) {
        let strict: Vec<usize> = dom[b].iter().copied().filter(|&d| d != b).collect();
        let d = *strict.iter().find(|&&d| dom[d].len() == strict.len()).expect("入口支配一切，严格支配者非空");
        idom[b] = Some(d);
        children[d].push(b);
        steps.push(Step::Idom { node: b, idom: d });
    }

    // 支配边界（Cooper–Harvey–Kennedy 的做法）
    let mut df: Vec<Vec<usize>> = vec![Vec::new(); n];
    for &b in &rpo {
        // 汇合点：有两个以上前驱；入口只要有前驱（回边）也算，因为还有一条"从函数外进来"的隐含边
        if preds[b].len() < 2 && !(b == 0 && !preds[b].is_empty()) {
            continue;
        }
        for &p in &preds[b] {
            let mut runner = p;
            while Some(runner) != idom[b] {
                if !df[runner].contains(&b) {
                    df[runner].push(b);
                }
                steps.push(Step::Df { join: b, pred: p, runner });
                match idom[runner] {
                    Some(r) => runner = r,
                    None => break,
                }
            }
        }
    }
    for d in &mut df {
        d.sort_unstable();
    }
    Dominators { dom, idom, children, df, rpo, steps }
}

/// 直接按定义算支配关系（对照测试用）：去掉 d 以后从入口走不到 n，就说明 d 支配 n。
pub fn dominators_by_definition(succs: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let n = succs.len();
    let reach_without = |d: usize| {
        let mut seen = vec![false; n];
        let mut stack = vec![0];
        while let Some(x) = stack.pop() {
            if x == d || seen[x] {
                continue;
            }
            seen[x] = true;
            stack.extend(succs[x].iter().copied());
        }
        seen
    };
    let mut dom = vec![Vec::new(); n];
    for d in 0..n {
        let seen = reach_without(d);
        for (m, s) in seen.iter().enumerate() {
            if m == d || !s {
                dom[m].push(d);
            }
        }
    }
    dom
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 随机生成的图（只保留从入口能到达的部分）上，迭代算法和"按定义"的结果必须一致。
    #[test]
    fn iterative_agrees_with_definition() {
        let mut seed = 12345u64;
        let mut rand = |m: usize| {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (seed >> 33) as usize % m
        };
        for _ in 0..300 {
            let n = 2 + rand(9);
            // 先连一条链保证都可达，再随机加边（包括回边、自环）
            let mut succs: Vec<Vec<usize>> = (0..n).map(|i| if i + 1 < n { vec![i + 1] } else { vec![] }).collect();
            for _ in 0..rand(2 * n) {
                let (a, b) = (rand(n), rand(n));
                if !succs[a].contains(&b) {
                    succs[a].push(b);
                }
            }
            let d = dominators(&succs);
            assert_eq!(d.dom, dominators_by_definition(&succs), "{succs:?}");
            // 支配边界的定义：d 支配 n 的某个前驱，但不严格支配 n
            let preds = preds_of(&succs);
            for x in 0..n {
                let expect: Vec<usize> = (0..n)
                    .filter(|&m| preds[m].iter().any(|&p| d.dom[p].contains(&x)) && !(d.dom[m].contains(&x) && m != x))
                    .collect();
                assert_eq!(d.df[x], expect, "DF({x}) of {succs:?}");
            }
        }
    }
}
