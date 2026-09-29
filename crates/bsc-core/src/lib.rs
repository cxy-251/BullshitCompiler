//! BSc 公共基础设施。
//!
//! 所有编译器（计算器语言、mini-lang、黑话编译器）共用这里的两样东西：
//!
//! - [`Span`]：一段源码的位置（字节区间）。编译器的每个产物——记号、语法树节点、
//!   指令——都记着自己来自源码的哪一段，界面才能做"点一下指令，高亮对应源码"。
//! - [`Diagnostic`]：结构化的错误/警告信息。错误不是一串字符串，而是
//!   "什么问题 + 在哪 + 怎么改"，同一份数据既能渲染成终端文本，也能在界面上画出来。

mod diagnostic;
mod span;

pub use diagnostic::{Diagnostic, Label, Severity};
pub use span::{LineCol, Span, display_width, line_col};
