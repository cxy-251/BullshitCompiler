//! 代码生成：把语法树翻译成栈式虚拟机的指令。
//!
//! 栈式机器只有一个"栈"：`push` 把数放到栈顶，`add` 这类指令从栈顶取两个数、
//! 算完再把结果放回去。所以翻译规则非常简单——**先翻译左右两个孩子，再输出
//! 自己的运算指令**（后序遍历）。`1 + 2 * 3` 就变成：
//!
//! ```text
//! push 1
//! push 2
//! push 3
//! mul        ; 栈: [1, 6]
//! add        ; 栈: [7]
//! ```
//!
//! 每条指令都记着自己是从哪个语法树节点翻译来的（这叫"源码映射"，
//! 调试器能把机器指令对应回源码行，靠的就是它）。

use std::fmt;

use crate::ast::{Ast, BinOp, NodeId, NodeKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Push(i64),
    Add,
    Sub,
    Mul,
    Div,
    Neg,
}

impl Op {
    /// 这条指令做什么（给新手看的解释）。
    pub fn explain(self) -> String {
        match self {
            Op::Push(n) => format!("把 {n} 放到栈顶"),
            Op::Add => "弹出两个数，把它们的和放回栈顶".to_owned(),
            Op::Sub => "弹出两个数，把 次顶 − 栈顶 放回栈顶".to_owned(),
            Op::Mul => "弹出两个数，把它们的积放回栈顶".to_owned(),
            Op::Div => "弹出两个数，把 次顶 ÷ 栈顶 放回栈顶".to_owned(),
            Op::Neg => "把栈顶的数取负".to_owned(),
        }
    }
}

impl fmt::Display for Op {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Op::Push(n) => write!(f, "push {n}"),
            Op::Add => f.write_str("add"),
            Op::Sub => f.write_str("sub"),
            Op::Mul => f.write_str("mul"),
            Op::Div => f.write_str("div"),
            Op::Neg => f.write_str("neg"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Instr {
    pub op: Op,
    /// 这条指令来自哪个语法树节点。
    pub node: NodeId,
}

/// 生成指令。指令的顺序也就是逐步回放时"一条条生成出来"的顺序。
pub fn codegen(ast: &Ast, root: NodeId) -> Vec<Instr> {
    let mut code = Vec::new();
    emit(ast, root, &mut code);
    code
}

fn emit(ast: &Ast, id: NodeId, code: &mut Vec<Instr>) {
    let op = match ast.node(id).kind {
        NodeKind::Num(n) => Op::Push(n),
        NodeKind::Neg(x) => {
            emit(ast, x, code);
            Op::Neg
        }
        NodeKind::Binary(op, l, r) => {
            emit(ast, l, code);
            emit(ast, r, code);
            match op {
                BinOp::Add => Op::Add,
                BinOp::Sub => Op::Sub,
                BinOp::Mul => Op::Mul,
                BinOp::Div => Op::Div,
            }
        }
    };
    code.push(Instr { op, node: id });
}

/// 指令清单的文本形式，每行一条。
pub fn listing(code: &[Instr]) -> String {
    code.iter().map(|i| format!("{}\n", i.op)).collect()
}
