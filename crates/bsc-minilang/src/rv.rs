//! RISC-V 后端：机器指令与指令选择。
//!
//! 目标是 RV64IM 的一个子集（64 位整数 + 乘除法）。指令选择把三地址码翻译成机器指令，
//! 这时还不管寄存器够不够用：每个 IR 变量先对应一个**虚拟寄存器**（写作 `%x`），
//! 真正的寄存器由下一步的寄存器分配决定。
//!
//! 指令选择的核心是**模式匹配**：一条（或相邻几条）IR 指令可以有多种翻译方式，
//! 选能覆盖得最多、指令最少的那种。比如 `t = i < n; if t goto A else goto B` 两条 IR，
//! 朴素的翻译是 `slt` + `bnez` + `j`，而 RISC-V 有"比较并跳转"指令，一条 `blt i, n, A` 就够了。

use std::collections::HashMap;

use crate::ast::{BinOp, UnOp};
use crate::cfg::Cfg;
use crate::ir::{Inst, Label, Operand};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Reg {
    Zero,
    Ra,
    Sp,
    /// 参数 / 返回值寄存器 a0–a7。
    A(u8),
    /// 临时寄存器 t0–t6（调用者保存）。
    T(u8),
    /// 保存寄存器 s1–s11（被调用者保存）。
    S(u8),
    /// 虚拟寄存器（寄存器分配之前）。
    V(u32),
}

impl Reg {
    /// 在 RISC-V 的 32 个寄存器里的编号（虚拟寄存器没有）。
    pub fn number(self) -> Option<usize> {
        Some(match self {
            Reg::Zero => 0,
            Reg::Ra => 1,
            Reg::Sp => 2,
            Reg::T(n @ 0..=2) => 5 + n as usize,
            Reg::S(n @ 1) => 8 + n as usize,
            Reg::A(n) => 10 + n as usize,
            Reg::S(n) => 16 + n as usize,
            Reg::T(n) => 25 + n as usize,
            Reg::V(_) => return None,
        })
    }

    pub fn name(self, vregs: &[String]) -> String {
        match self {
            Reg::Zero => "zero".to_owned(),
            Reg::Ra => "ra".to_owned(),
            Reg::Sp => "sp".to_owned(),
            Reg::A(n) => format!("a{n}"),
            Reg::T(n) => format!("t{n}"),
            Reg::S(n) => format!("s{n}"),
            Reg::V(v) => format!("%{}", vregs.get(v as usize).map_or("?", String::as_str)),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ROp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Slt,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IOp {
    Addi,
    Slti,
    Xori,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cond {
    Beq,
    Bne,
    Blt,
    Bge,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MInst {
    Li { rd: Reg, imm: i64 },
    Mv { rd: Reg, rs: Reg },
    R { op: ROp, rd: Reg, rs1: Reg, rs2: Reg },
    I { op: IOp, rd: Reg, rs1: Reg, imm: i64 },
    Neg { rd: Reg, rs: Reg },
    Seqz { rd: Reg, rs: Reg },
    Snez { rd: Reg, rs: Reg },
    Br { cond: Cond, rs1: Reg, rs2: Reg, target: String },
    Bnez { rs: Reg, target: String },
    J { target: String },
    Call { func: String },
    Ret,
    Ld { rd: Reg, off: i64, base: Reg },
    Sd { rs: Reg, off: i64, base: Reg },
}

impl MInst {
    pub fn text(&self, vregs: &[String]) -> String {
        let r = |x: &Reg| x.name(vregs);
        match self {
            MInst::Li { rd, imm } => format!("li    {}, {imm}", r(rd)),
            MInst::Mv { rd, rs } => format!("mv    {}, {}", r(rd), r(rs)),
            MInst::R { op, rd, rs1, rs2 } => {
                let name = match op {
                    ROp::Add => "add",
                    ROp::Sub => "sub",
                    ROp::Mul => "mul",
                    ROp::Div => "div",
                    ROp::Rem => "rem",
                    ROp::Slt => "slt",
                };
                format!("{name:<5} {}, {}, {}", r(rd), r(rs1), r(rs2))
            }
            MInst::I { op, rd, rs1, imm } => {
                let name = match op {
                    IOp::Addi => "addi",
                    IOp::Slti => "slti",
                    IOp::Xori => "xori",
                };
                format!("{name:<5} {}, {}, {imm}", r(rd), r(rs1))
            }
            MInst::Neg { rd, rs } => format!("neg   {}, {}", r(rd), r(rs)),
            MInst::Seqz { rd, rs } => format!("seqz  {}, {}", r(rd), r(rs)),
            MInst::Snez { rd, rs } => format!("snez  {}, {}", r(rd), r(rs)),
            MInst::Br { cond, rs1, rs2, target } => {
                let name = match cond {
                    Cond::Beq => "beq",
                    Cond::Bne => "bne",
                    Cond::Blt => "blt",
                    Cond::Bge => "bge",
                };
                format!("{name:<5} {}, {}, {target}", r(rs1), r(rs2))
            }
            MInst::Bnez { rs, target } => format!("bnez  {}, {target}", r(rs)),
            MInst::J { target } => format!("j     {target}"),
            MInst::Call { func } => format!("call  {func}"),
            MInst::Ret => "ret".to_owned(),
            MInst::Ld { rd, off, base } => format!("ld    {}, {off}({})", r(rd), r(base)),
            MInst::Sd { rs, off, base } => format!("sd    {}, {off}({})", r(rs), r(base)),
        }
    }

    /// 写入的寄存器。
    pub fn defs(&self) -> Vec<Reg> {
        match self {
            MInst::Li { rd, .. }
            | MInst::Mv { rd, .. }
            | MInst::R { rd, .. }
            | MInst::I { rd, .. }
            | MInst::Neg { rd, .. }
            | MInst::Seqz { rd, .. }
            | MInst::Snez { rd, .. }
            | MInst::Ld { rd, .. } => vec![*rd],
            _ => vec![],
        }
    }

    /// 读取的寄存器。
    pub fn uses(&self) -> Vec<Reg> {
        match self {
            MInst::Mv { rs, .. } | MInst::Neg { rs, .. } | MInst::Seqz { rs, .. } | MInst::Snez { rs, .. } => {
                vec![*rs]
            }
            MInst::R { rs1, rs2, .. } | MInst::Br { rs1, rs2, .. } => vec![*rs1, *rs2],
            MInst::I { rs1, .. } => vec![*rs1],
            MInst::Bnez { rs, .. } => vec![*rs],
            MInst::Ld { base, .. } => vec![*base],
            MInst::Sd { rs, base, .. } => vec![*rs, *base],
            _ => vec![],
        }
    }

    /// 把每个寄存器按 `f` 换掉（寄存器分配后改写用）。
    pub fn map_regs(&mut self, mut f: impl FnMut(Reg) -> Reg) {
        match self {
            MInst::Li { rd, .. } => *rd = f(*rd),
            MInst::Mv { rd, rs } | MInst::Neg { rd, rs } | MInst::Seqz { rd, rs } | MInst::Snez { rd, rs } => {
                *rs = f(*rs);
                *rd = f(*rd);
            }
            MInst::R { rd, rs1, rs2, .. } => {
                *rs1 = f(*rs1);
                *rs2 = f(*rs2);
                *rd = f(*rd);
            }
            MInst::I { rd, rs1, .. } => {
                *rs1 = f(*rs1);
                *rd = f(*rd);
            }
            MInst::Br { rs1, rs2, .. } => {
                *rs1 = f(*rs1);
                *rs2 = f(*rs2);
            }
            MInst::Bnez { rs, .. } => *rs = f(*rs),
            MInst::Ld { rd, base, .. } => {
                *base = f(*base);
                *rd = f(*rd);
            }
            MInst::Sd { rs, base, .. } => {
                *rs = f(*rs);
                *base = f(*base);
            }
            _ => {}
        }
    }
}

#[derive(Clone, Debug)]
pub struct MBlock {
    pub label: String,
    pub insts: Vec<MInst>,
    pub succs: Vec<usize>,
}

/// 一个函数的机器代码。
#[derive(Clone, Debug)]
pub struct MFunc {
    pub name: String,
    /// 虚拟寄存器的名字（下标就是虚拟寄存器编号）。
    pub vregs: Vec<String>,
    pub blocks: Vec<MBlock>,
}

impl MFunc {
    /// 函数的出口标签（返回前恢复现场的代码在这里）。
    pub fn exit_label(&self) -> String {
        format!("{}_exit", self.name)
    }
}

/// 指令选择的一步：哪几条 IR 指令，用了哪个模式，生成了哪几条机器指令。
#[derive(Clone, Debug)]
pub struct Step {
    pub block: usize,
    /// IR 指令在块里的下标。
    pub ir: Vec<usize>,
    pub pattern: &'static str,
    /// 生成的机器指令在机器块里的下标范围。
    pub out: std::ops::Range<usize>,
}

const IMM_MIN: i64 = -2048;
const IMM_MAX: i64 = 2047;

fn fits(c: i64) -> bool {
    (IMM_MIN..=IMM_MAX).contains(&c)
}

/// 指令选择。`smart = false` 时逐条"宏展开"：常量一律先装进寄存器，不融合比较和跳转，用来对比。
pub fn select(cfg: &Cfg, smart: bool) -> (MFunc, Vec<Step>) {
    let name = cfg.func.name.clone();
    let mut vregs: Vec<String> = cfg.func.vars.iter().map(|v| v.name.clone()).collect();
    let block_label = |b: usize| format!("{}_{}", name, cfg.blocks[b].name);
    let label_block: HashMap<Label, usize> =
        cfg.blocks.iter().enumerate().filter_map(|(i, b)| b.label.map(|l| (l, i))).collect();
    // 每个变量被用到几次（判断比较的结果是不是只给紧跟着的跳转用）
    let mut use_count = vec![0usize; cfg.func.vars.len()];
    for b in &cfg.blocks {
        for i in &b.insts {
            for u in i.uses() {
                if let Operand::Var(v) = u {
                    use_count[v] += 1;
                }
            }
        }
    }
    let exit = format!("{name}_exit");
    let mut blocks = Vec::new();
    let mut steps = Vec::new();
    for (b, block) in cfg.blocks.iter().enumerate() {
        let mut out: Vec<MInst> = Vec::new();
        let next_block = b + 1;
        let target_of = |l: Label| *label_block.get(&l).expect("跳转目标是某个块的标签");
        let mut i = 0;
        while i < block.insts.len() {
            let start = out.len();
            let mut e = Emit { out: &mut out, vregs: &mut vregs, smart };
            let (pattern, used) = match &block.insts[i] {
                Inst::Param { dst, index } => {
                    e.push(MInst::Mv { rd: Reg::V(*dst as u32), rs: Reg::A(*index as u8) });
                    ("取参数：mv 目标, a参数号", 1)
                }
                Inst::Copy { dst, src } => {
                    let rd = Reg::V(*dst as u32);
                    match src {
                        Operand::Const(c) => {
                            e.push(MInst::Li { rd, imm: *c });
                            ("装入常量 li", 1)
                        }
                        Operand::Var(v) => {
                            e.push(MInst::Mv { rd, rs: Reg::V(*v as u32) });
                            ("复制 mv", 1)
                        }
                        Operand::Undef => ("未定义的值：不生成代码", 1),
                    }
                }
                Inst::Bin { dst, op, a, b: rhs } => {
                    // 比较紧跟条件跳转、而且结果只给这个跳转用：融合成一条"比较并跳转"
                    let fused = match block.insts.get(i + 1) {
                        Some(Inst::Branch { cond: Operand::Var(c), then, els })
                            if smart
                                && *c == *dst
                                && use_count[*dst] == 1
                                && matches!(
                                    op,
                                    BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge | BinOp::Eq | BinOp::Ne
                                ) =>
                        {
                            Some((*then, *els))
                        }
                        _ => None,
                    };
                    match fused {
                        Some((then, els)) => {
                            let (x, y) = (e.reg(*a), e.reg(*rhs));
                            let (cond, rs1, rs2) = match op {
                                BinOp::Lt => (Cond::Blt, x, y),
                                BinOp::Ge => (Cond::Bge, x, y),
                                BinOp::Gt => (Cond::Blt, y, x),
                                BinOp::Le => (Cond::Bge, y, x),
                                BinOp::Eq => (Cond::Beq, x, y),
                                _ => (Cond::Bne, x, y),
                            };
                            let (tb, eb) = (target_of(then), target_of(els));
                            e.push(MInst::Br { cond, rs1, rs2, target: block_label(tb) });
                            if eb != next_block {
                                e.push(MInst::J { target: block_label(eb) });
                            }
                            ("比较并跳转 b<条件>（两条 IR 合成一条）", 2)
                        }
                        None => (e.binary(Reg::V(*dst as u32), *op, *a, *rhs), 1),
                    }
                }
                Inst::Un { dst, op, a } => {
                    let rs = e.reg(*a);
                    let rd = Reg::V(*dst as u32);
                    match op {
                        UnOp::Neg => {
                            e.push(MInst::Neg { rd, rs });
                            ("取负 neg", 1)
                        }
                        UnOp::Not => {
                            e.push(MInst::Seqz { rd, rs });
                            ("逻辑非 seqz（等于 0 得 1）", 1)
                        }
                    }
                }
                Inst::Call { dst, func, args } => {
                    for (j, a) in args.iter().enumerate() {
                        match a {
                            Operand::Const(c) => e.push(MInst::Li { rd: Reg::A(j as u8), imm: *c }),
                            Operand::Var(v) => e.push(MInst::Mv { rd: Reg::A(j as u8), rs: Reg::V(*v as u32) }),
                            Operand::Undef => {}
                        }
                    }
                    e.push(MInst::Call { func: func.clone() });
                    if let Some(d) = dst {
                        e.push(MInst::Mv { rd: Reg::V(*d as u32), rs: Reg::A(0) });
                    }
                    ("函数调用：实参放进 a0、a1…，call，返回值在 a0", 1)
                }
                Inst::Branch { cond, then, els } => {
                    let (tb, eb) = (target_of(*then), target_of(*els));
                    match cond {
                        Operand::Const(c) => {
                            let t = if *c != 0 { tb } else { eb };
                            e.push(MInst::J { target: block_label(t) });
                        }
                        _ => {
                            let rs = e.reg(*cond);
                            e.push(MInst::Bnez { rs, target: block_label(tb) });
                            if !smart || eb != next_block {
                                e.push(MInst::J { target: block_label(eb) });
                            }
                        }
                    }
                    ("条件跳转 bnez + j", 1)
                }
                Inst::Jump(l) => {
                    let t = target_of(*l);
                    if !smart || t != next_block {
                        e.push(MInst::J { target: block_label(t) });
                        ("无条件跳转 j", 1)
                    } else {
                        ("跳到紧挨着的下一块：不用生成跳转", 1)
                    }
                }
                Inst::Return(v) => {
                    match v {
                        Some(Operand::Const(c)) => e.push(MInst::Li { rd: Reg::A(0), imm: *c }),
                        Some(Operand::Var(x)) => e.push(MInst::Mv { rd: Reg::A(0), rs: Reg::V(*x as u32) }),
                        _ => {}
                    }
                    e.push(MInst::J { target: exit.clone() });
                    ("返回：返回值放进 a0，跳到出口", 1)
                }
                Inst::Phi { .. } | Inst::Label(_) => ("", 1),
            };
            steps.push(Step { block: b, ir: (i..i + used).collect(), pattern, out: start..out.len() });
            i += used;
        }
        blocks.push(MBlock { label: block_label(b), insts: out, succs: block.succs.clone() });
    }
    (MFunc { name, vregs, blocks }, steps)
}

struct Emit<'a> {
    out: &'a mut Vec<MInst>,
    vregs: &'a mut Vec<String>,
    smart: bool,
}

impl Emit<'_> {
    fn push(&mut self, i: MInst) {
        self.out.push(i);
    }

    /// 操作数放进寄存器：变量就是它的虚拟寄存器；0 可以直接用 zero 寄存器；其他常量要先 li 进一个新的虚拟寄存器。
    fn reg(&mut self, o: Operand) -> Reg {
        match o {
            Operand::Var(v) => Reg::V(v as u32),
            Operand::Const(0) if self.smart => Reg::Zero,
            Operand::Const(c) => {
                self.vregs.push(format!("k{c}"));
                let r = Reg::V(self.vregs.len() as u32 - 1);
                self.push(MInst::Li { rd: r, imm: c });
                r
            }
            Operand::Undef => Reg::Zero,
        }
    }

    fn binary(&mut self, rd: Reg, op: BinOp, a: Operand, b: Operand) -> &'static str {
        use Operand::Const;
        // 带立即数的指令：一个操作数是小常量
        if self.smart {
            match (op, a, b) {
                (BinOp::Add, x, Const(c)) | (BinOp::Add, Const(c), x) if fits(c) && !matches!(x, Const(_)) => {
                    let rs1 = self.reg(x);
                    self.push(MInst::I { op: IOp::Addi, rd, rs1, imm: c });
                    return "加立即数 addi";
                }
                (BinOp::Sub, x, Const(c)) if fits(-c) && !matches!(x, Const(_)) => {
                    let rs1 = self.reg(x);
                    self.push(MInst::I { op: IOp::Addi, rd, rs1, imm: -c });
                    return "减常量 = 加负数 addi";
                }
                (BinOp::Lt, x, Const(c)) if fits(c) && !matches!(x, Const(_)) => {
                    let rs1 = self.reg(x);
                    self.push(MInst::I { op: IOp::Slti, rd, rs1, imm: c });
                    return "小于立即数 slti";
                }
                _ => {}
            }
        }
        let (x, y) = (self.reg(a), self.reg(b));
        let r = |op| MInst::R { op, rd, rs1: x, rs2: y };
        match op {
            BinOp::Add => self.push(r(ROp::Add)),
            BinOp::Sub => self.push(r(ROp::Sub)),
            BinOp::Mul => self.push(r(ROp::Mul)),
            BinOp::Div => self.push(r(ROp::Div)),
            BinOp::Rem => self.push(r(ROp::Rem)),
            BinOp::Lt => self.push(r(ROp::Slt)),
            BinOp::Gt => self.push(MInst::R { op: ROp::Slt, rd, rs1: y, rs2: x }),
            BinOp::Le | BinOp::Ge => {
                let (s1, s2) = if op == BinOp::Le { (y, x) } else { (x, y) };
                self.push(MInst::R { op: ROp::Slt, rd, rs1: s1, rs2: s2 });
                self.push(MInst::I { op: IOp::Xori, rd, rs1: rd, imm: 1 });
                return "小于等于 / 大于等于：slt 再取反 xori";
            }
            BinOp::Eq | BinOp::Ne => {
                self.push(r(ROp::Sub));
                if op == BinOp::Eq {
                    self.push(MInst::Seqz { rd, rs: rd });
                } else {
                    self.push(MInst::Snez { rd, rs: rd });
                }
                return "相等比较：相减再判断是否为 0";
            }
            // && 和 || 在降级成三地址码时已经变成了跳转
            BinOp::And | BinOp::Or => self.push(r(ROp::Add)),
        }
        match op {
            BinOp::Lt | BinOp::Gt => "小于 / 大于 slt",
            _ => "寄存器运算 add / sub / mul / div / rem",
        }
    }
}
