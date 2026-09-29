//! 第 2 章 · 语法分析：理解句子结构。

pub mod ambiguity;
pub mod grammar;
pub mod ll1;
pub mod lr;

/// 第 2 章各课的状态。
#[derive(Default)]
pub struct Chapter {
    pub grammar: grammar::Lesson,
    pub ambiguity: ambiguity::Lesson,
    pub ll1: ll1::Lesson,
    pub lr: lr::Lesson,
}
