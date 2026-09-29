//! # 上下文无关文法
//!
//! 第 2 章"语法分析"用到的文法工具：
//!
//! - `grammar`：文法的表示与文本解析（`E -> E + T | T` 这样的写法），把句子切成终结符
//! - `earley`：Earley 分析——能处理**任意**上下文无关文法（包括有歧义的），找出所有语法树
//! - `derive`：由语法树得到最左推导；由文法随机生成句子
//! - `first_follow`：FIRST / FOLLOW 集合的不动点计算
//! - `ll1`：LL(1) 分析表与表驱动的自顶向下分析
//! - `lr`：LR(0) 项目集自动机、SLR(1) 分析表与移进-归约分析
//!
//! 除了结果，每个算法都返回逐步记录，界面据此回放。

pub mod derive;
pub mod earley;
pub mod first_follow;
pub mod grammar;
pub mod ll1;
pub mod lr;

pub use grammar::{Grammar, Production, Sym};

/// 语法树：每个节点是一个文法符号；非终结符节点记着用了哪条产生式。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tree {
    pub sym: Sym,
    /// 非终结符用的产生式；终结符为 `None`。
    pub prod: Option<usize>,
    pub children: Vec<Tree>,
    /// 覆盖的记号区间 `[start, end)`（记号下标）。
    pub start: usize,
    pub end: usize,
}

impl Tree {
    /// 前序遍历的节点列表，界面画树用：(节点, 孩子在列表里的下标)。
    pub fn flatten(&self) -> Vec<(&Tree, Vec<usize>)> {
        fn go<'a>(t: &'a Tree, out: &mut Vec<(&'a Tree, Vec<usize>)>) -> usize {
            let me = out.len();
            out.push((t, Vec::new()));
            let kids: Vec<usize> = t.children.iter().map(|c| go(c, out)).collect();
            out[me].1 = kids;
            me
        }
        let mut out = Vec::new();
        go(self, &mut out);
        out
    }

    /// 用括号写出整棵树，比如 `E(E(T(F(num))) + T(F(num)))`。测试和对比两棵树用。
    pub fn to_bracketed(&self, g: &Grammar) -> String {
        if self.children.is_empty() {
            return if self.prod.is_some() { format!("{}(ε)", g.name(self.sym)) } else { g.name(self.sym).to_owned() };
        }
        let kids: Vec<String> = self.children.iter().map(|c| c.to_bracketed(g)).collect();
        format!("{}({})", g.name(self.sym), kids.join(" "))
    }
}
