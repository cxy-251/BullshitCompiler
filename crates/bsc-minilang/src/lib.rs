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
//! `interp` 直接执行控制流图，用来验证各步变换不改变程序行为。优化、后端随课程推进逐步加入。

pub mod ast;
pub mod cfg;
pub mod dom;
pub mod interp;
pub mod ir;
pub mod lexer;
pub mod parser;
pub mod sema;
pub mod ssa;
