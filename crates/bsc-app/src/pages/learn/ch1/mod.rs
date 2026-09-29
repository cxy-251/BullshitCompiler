//! 第 1 章 · 词法分析：让机器认字。

pub mod dfa;
pub mod lexgen;
pub mod minimize;
pub mod nfa;
pub mod regex;

/// 第 1 章各课的状态。
#[derive(Default)]
pub struct Chapter {
    pub regex: regex::Lesson,
    pub nfa: nfa::Lesson,
    pub dfa: dfa::Lesson,
    pub minimize: minimize::Lesson,
    pub lexgen: lexgen::Lesson,
}
