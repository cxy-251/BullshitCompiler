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
//! 目前完成的是词法分析（`lexer`）；语法分析、语义分析、中间表示等随课程推进逐步加入。

pub mod lexer;
