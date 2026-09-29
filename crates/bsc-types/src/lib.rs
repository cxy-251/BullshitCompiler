//! # Hindley–Milner 类型推导
//!
//! 一门迷你函数式语言（ML 风格），程序里**不写任何类型**，编译器自己推出每个表达式的类型：
//!
//! ```text
//! e ::= 整数 | true | false | 名字
//!     | fun x -> e                 函数（只有一个参数）
//!     | e e                        调用（函数应用）
//!     | let x = e in e             局部定义
//!     | if e then e else e
//!     | e + e | e - e | e * e | e < e | e == e
//!     | ( e )
//! ```
//!
//! 推导的思路像解方程：
//! 1. 不知道的类型先用**类型变量**（α、β……）代替；
//! 2. 每种语法结构带来一些"方程"，比如调用 `f x` 要求 f 的类型 = (x 的类型 → 结果的类型)；
//! 3. 用**合一**（unification）解方程，得到"α = int"这样的**替换**；
//! 4. `let` 定义的值，把没被确定的类型变量**泛化**成"对任何类型都成立"（∀α），
//!    这样 `let id = fun x -> x` 之后，`id 1` 和 `id true` 都能用。

pub mod infer;
pub mod syntax;

pub use infer::{Inference, Scheme, Ty, infer};
pub use syntax::{Expr, ExprId, ExprKind, Program, parse};
