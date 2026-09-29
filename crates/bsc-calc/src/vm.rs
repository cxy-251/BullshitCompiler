//! 栈式虚拟机：一台只有一个栈的小机器。
//!
//! Java 的 JVM、Python 的 CPython、WebAssembly 都是栈式虚拟机，
//! 原理和这里一模一样，只是指令多得多。

use bsc_core::Diagnostic;

use crate::ast::{Ast, NodeKind};
use crate::codegen::{Instr, Op};

/// 执行完第 `pc` 条指令后的机器状态。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VmStep {
    pub pc: usize,
    pub stack: Vec<i64>,
}

#[derive(Clone, Debug)]
pub struct Execution {
    /// 每执行一条指令记一步。出错时最后一条出错的指令没有记录。
    pub steps: Vec<VmStep>,
    pub result: Result<i64, Diagnostic>,
}

pub fn run(code: &[Instr], ast: &Ast) -> Execution {
    let mut stack: Vec<i64> = Vec::new();
    let mut steps = Vec::with_capacity(code.len());

    for (pc, instr) in code.iter().enumerate() {
        let value = match instr.op {
            Op::Push(n) => Some(n),
            Op::Neg => {
                let a = stack.pop().expect("代码生成保证栈上有操作数");
                a.checked_neg()
            }
            op => {
                let b = stack.pop().expect("代码生成保证栈上有操作数");
                let a = stack.pop().expect("代码生成保证栈上有操作数");
                if op == Op::Div && b == 0 {
                    return Execution { steps, result: Err(div_by_zero(instr, ast)) };
                }
                match op {
                    Op::Add => a.checked_add(b),
                    Op::Sub => a.checked_sub(b),
                    Op::Mul => a.checked_mul(b),
                    Op::Div => a.checked_div(b),
                    Op::Push(_) | Op::Neg => unreachable!(),
                }
            }
        };
        let Some(value) = value else {
            return Execution { steps, result: Err(overflow(instr, ast)) };
        };
        stack.push(value);
        steps.push(VmStep { pc, stack: stack.clone() });
    }

    let result = match stack.as_slice() {
        [v] => Ok(*v),
        _ => unreachable!("代码生成保证最后栈上恰好剩一个数"),
    };
    Execution { steps, result }
}

fn div_by_zero(instr: &Instr, ast: &Ast) -> Diagnostic {
    let node = ast.node(instr.node);
    let mut d = Diagnostic::error("除以零").with_code("E0301").with_primary(node.span, "这次除法的除数是 0");
    if let NodeKind::Binary(_, _, rhs) = node.kind {
        d = d.with_secondary(ast.node(rhs).span, "它算出来是 0");
    }
    d.with_note("这是运行时错误：语法完全正确，要真正算到这一步才会发现问题")
}

fn overflow(instr: &Instr, ast: &Ast) -> Diagnostic {
    Diagnostic::error("计算结果溢出")
        .with_code("E0302")
        .with_primary(ast.node(instr.node).span, "这一步的结果超出了 64 位整数范围")
        .with_note(format!("64 位整数的范围是 {} 到 {}", i64::MIN, i64::MAX))
}
