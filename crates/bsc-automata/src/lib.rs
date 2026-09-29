//! # 有限自动机
//!
//! 第 1 章"词法分析"用到的全部算法：
//!
//! ```text
//! 正则表达式 ──regex.rs──▶ 语法树 ──nfa.rs (Thompson 构造)──▶ NFA
//!    ──dfa.rs (子集构造)──▶ DFA ──minimize.rs (划分细化)──▶ 最小 DFA
//!
//! 多条正则 + 优先级 ──scanner.rs──▶ 词法分析器（最长匹配）
//! ```
//!
//! 每个算法除了产出结果，还返回一份逐步记录（`*Step`），界面据此做单步回放。

pub mod charset;
pub mod dfa;
pub mod minimize;
pub mod nfa;
pub mod regex;
pub mod scanner;

/// 自动机状态的编号。
pub type StateId = usize;

/// 状态集合（NFA 在同一时刻可能同时处于多个状态）。用有序集合，方便显示和比较。
pub type StateSet = std::collections::BTreeSet<StateId>;

/// 把状态集合写成 `{0, 1, 4}` 的形式。
pub fn fmt_set(set: &StateSet) -> String {
    let items: Vec<String> = set.iter().map(ToString::to_string).collect();
    format!("{{{}}}", items.join(", "))
}
