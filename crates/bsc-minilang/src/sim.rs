//! RV64 子集模拟器：执行寄存器分配之后的汇编，记录每一步的寄存器和内存变化。
//!
//! 内存只用来放栈。栈从高地址往低地址长：函数开头 `addi sp, sp, -16` 就是开辟 16 字节的栈帧。
//! `call f` 把返回地址存进 ra 再跳过去，`ret` 跳回 ra。`print` 是内置函数：打印 a0。

use std::collections::HashMap;

use crate::interp::binop;
use crate::regalloc::AsmFunc;
use crate::rv::{Cond, IOp, MInst, ROp, Reg};

/// 程序开始时 sp 的值。
pub const STACK_TOP: i64 = 0x1000;
/// main 的返回地址：返回到这里就结束。
const HALT: i64 = -1;

/// 一条可执行的指令，以及它属于哪个函数、哪个标签下面。
#[derive(Clone, Debug)]
pub struct Line {
    pub func: usize,
    pub label: Option<String>,
    pub inst: MInst,
}

/// 把所有函数排成一个指令序列（像汇编器那样），标签换成地址。
pub fn link(funcs: &[AsmFunc]) -> (Vec<Line>, HashMap<String, usize>) {
    let mut lines = Vec::new();
    let mut labels = HashMap::new();
    for (fi, f) in funcs.iter().enumerate() {
        for b in &f.blocks {
            labels.insert(b.label.clone(), lines.len());
            let mut first = true;
            for i in &b.insts {
                lines.push(Line { func: fi, label: first.then(|| b.label.clone()), inst: i.clone() });
                first = false;
            }
        }
    }
    (lines, labels)
}

/// 一步执行的效果。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Step {
    pub pc: usize,
    pub writes: Vec<(Reg, i64)>,
    pub store: Option<(i64, i64)>,
    pub output: Option<i64>,
    /// 进入（Some(函数)）或离开（None）一个函数。
    pub call: Option<Option<usize>>,
}

#[derive(Clone, Debug)]
pub struct Run {
    pub lines: Vec<Line>,
    pub steps: Vec<Step>,
    pub output: Vec<i64>,
    pub error: Option<String>,
    pub executed: usize,
}

/// 从 main 开始执行；只记录前 `trace_limit` 步。
pub fn run(funcs: &[AsmFunc], trace_limit: usize) -> Run {
    let (lines, labels) = link(funcs);
    let mut r = Run { lines, steps: vec![], output: vec![], error: None, executed: 0 };
    let Some(&start) = labels.get("main") else {
        r.error = Some("没有 main 函数".to_owned());
        return r;
    };
    let mut regs = [0i64; 32];
    regs[2] = STACK_TOP;
    regs[1] = HALT;
    let mut mem: HashMap<i64, i64> = HashMap::new();
    let mut pc = start;
    let res: Result<(), String> = (|| {
        loop {
            if r.executed >= 2_000_000 {
                return Err("执行的指令太多（死循环？）".to_owned());
            }
            r.executed += 1;
            let line = r.lines.get(pc).ok_or("跳到了程序外面")?;
            let get = |rg: &Reg| rg.number().map_or(0, |n| regs[n]);
            let mut st = Step { pc, writes: vec![], store: None, output: None, call: None };
            let mut next = pc + 1;
            let write = |rd: Reg, v: i64, st: &mut Step| {
                if let Some(n) = rd.number()
                    && n != 0
                {
                    st.writes.push((rd, v));
                }
            };
            match &line.inst {
                MInst::Li { rd, imm } => write(*rd, *imm, &mut st),
                MInst::Mv { rd, rs } => write(*rd, get(rs), &mut st),
                MInst::R { op, rd, rs1, rs2 } => {
                    let (a, b) = (get(rs1), get(rs2));
                    let v = match op {
                        ROp::Add => a.wrapping_add(b),
                        ROp::Sub => a.wrapping_sub(b),
                        ROp::Mul => a.wrapping_mul(b),
                        ROp::Div => binop(crate::ast::BinOp::Div, a, b)?,
                        ROp::Rem => binop(crate::ast::BinOp::Rem, a, b)?,
                        ROp::Slt => i64::from(a < b),
                    };
                    write(*rd, v, &mut st);
                }
                MInst::I { op, rd, rs1, imm } => {
                    let a = get(rs1);
                    let v = match op {
                        IOp::Addi => a.wrapping_add(*imm),
                        IOp::Slti => i64::from(a < *imm),
                        IOp::Xori => a ^ imm,
                    };
                    write(*rd, v, &mut st);
                }
                MInst::Neg { rd, rs } => write(*rd, get(rs).wrapping_neg(), &mut st),
                MInst::Seqz { rd, rs } => write(*rd, i64::from(get(rs) == 0), &mut st),
                MInst::Snez { rd, rs } => write(*rd, i64::from(get(rs) != 0), &mut st),
                MInst::Br { cond, rs1, rs2, target } => {
                    let (a, b) = (get(rs1), get(rs2));
                    let taken = match cond {
                        Cond::Beq => a == b,
                        Cond::Bne => a != b,
                        Cond::Blt => a < b,
                        Cond::Bge => a >= b,
                    };
                    if taken {
                        next = labels[target];
                    }
                }
                MInst::Bnez { rs, target } => {
                    if get(rs) != 0 {
                        next = labels[target];
                    }
                }
                MInst::J { target } => next = labels[target],
                MInst::Call { func } => {
                    if func == "print" {
                        st.output = Some(regs[10]);
                    } else {
                        let &t = labels.get(func).ok_or("调用了不存在的函数")?;
                        write(Reg::Ra, pc as i64 + 1, &mut st);
                        st.call = Some(Some(r.lines[t].func));
                        next = t;
                    }
                }
                MInst::Ret => {
                    st.call = Some(None);
                    if regs[1] == HALT {
                        next = usize::MAX;
                    } else {
                        next = regs[1] as usize;
                    }
                }
                MInst::Ld { rd, off, base } => {
                    let addr = get(base) + off;
                    let v = *mem.get(&addr).ok_or_else(|| format!("读取了没有写过的内存 {addr:#x}"))?;
                    write(*rd, v, &mut st);
                }
                MInst::Sd { rs, off, base } => {
                    let addr = get(base) + off;
                    if addr < STACK_TOP - 64 * 1024 {
                        return Err("栈溢出（递归太深？）".to_owned());
                    }
                    st.store = Some((addr, get(rs)));
                }
            }
            for &(rg, v) in &st.writes {
                regs[rg.number().unwrap()] = v;
            }
            if let Some((a, v)) = st.store {
                mem.insert(a, v);
            }
            if let Some(v) = st.output {
                r.output.push(v);
            }
            if r.steps.len() < trace_limit {
                r.steps.push(st);
            }
            if next == usize::MAX {
                return Ok(());
            }
            pc = next;
        }
    })();
    r.error = res.err();
    r
}
