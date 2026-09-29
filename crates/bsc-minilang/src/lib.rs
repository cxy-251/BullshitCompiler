//! # mini-lang
//!
//! 课程后半段使用的教学语言：有变量、函数、分支和循环，语法接近 Rust。
//!
//! ```text
//! // 求 1 + 2 + … + n
//! fn sum(n: int) -> int {
//!     let mut s = 0;
//!     let mut i = 1;
//!     while i <= n {
//!         s = s + i;
//!         i = i + 1;
//!     }
//!     return s;
//! }
//! ```
//!
//! 编译流水线：词法分析（`lexer`）→ 语法分析（`parser`，语句用递归下降、表达式用 Pratt）→
//! 语义分析（`sema`）→ 三地址码（`ir`）→ 控制流图（`cfg`）→ 支配树（`dom`）→ SSA（`ssa`）；
//! `interp` 直接执行控制流图，用来验证各步变换不改变程序行为。
//! 优化：`dataflow`（数据流分析框架：活跃变量、到达定值）、`opt`（常量折叠、代数化简、死代码消除）、
//! `sccp`（稀疏条件常量传播）。后端随课程推进逐步加入。

pub mod ast;
pub mod cfg;
pub mod dataflow;
pub mod dom;
pub mod interp;
pub mod ir;
pub mod lexer;
pub mod opt;
pub mod parser;
pub mod sccp;
pub mod sema;
pub mod ssa;
