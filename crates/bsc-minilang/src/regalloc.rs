//! 寄存器分配：把无限多的虚拟寄存器装进有限的物理寄存器。
//!
//! 两个虚拟寄存器如果**同时活跃**（某一时刻两个值都还要用），就不能放进同一个物理寄存器。
//! 放不下的只好**溢出**（spill）到栈上：用之前从内存读出来，算完再写回去。
//!
//! - **线性扫描**（Poletto & Sarkar 1999）：把每个虚拟寄存器的活跃范围看成一条区间，按起点从左到右扫描，
//!   寄存器用完时溢出"结束得最晚"的那个。快，JIT 编译器爱用。
//! - **图着色**（Chaitin 1982，Briggs 的乐观改进）：同时活跃的虚拟寄存器之间连一条边，得到**干涉图**，
//!   用 K 种颜色（K 个寄存器）给图着色，相邻的点颜色不同。度数小于 K 的点总能着色，先把它们拿掉（简化），
//!   剩下的都 ≥ K 时挑一个可能溢出；最后倒着把点放回去着色。
//!
//! 分配用的是被调用者保存的 s1–s11：函数调用不会破坏它们（被调用的函数用到时会自己保存、恢复），
//! 所以跨调用活着的值也是安全的。t5、t6 留给溢出代码临时用。

use crate::dataflow::{Direction, Problem, solve};
use crate::rv::{MBlock, MFunc, MInst, Reg};

/// 可以分配的寄存器（按这个顺序挑）。
pub const POOL: [Reg; 11] = [
    Reg::S(1),
    Reg::S(2),
    Reg::S(3),
    Reg::S(4),
    Reg::S(5),
    Reg::S(6),
    Reg::S(7),
    Reg::S(8),
    Reg::S(9),
    Reg::S(10),
    Reg::S(11),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Loc {
    Reg(Reg),
    /// 栈上第几个溢出槽。
    Slot(usize),
}

/// 按线性顺序给指令编号后，每条指令执行之后活跃的虚拟寄存器。
pub struct Liveness {
    /// (块, 块内下标) → 线性编号 的起点。
    pub block_start: Vec<usize>,
    pub live_after: Vec<Vec<u32>>,
    pub n_insts: usize,
}

fn vregs_of(rs: Vec<Reg>) -> Vec<u32> {
    rs.into_iter()
        .filter_map(|r| match r {
            Reg::V(v) => Some(v),
            _ => None,
        })
        .collect()
}

pub fn liveness(f: &MFunc) -> Liveness {
    let mut gen_ = Vec::new();
    let mut kill = Vec::new();
    for b in &f.blocks {
        let (mut g, mut k): (Vec<usize>, Vec<usize>) = (vec![], vec![]);
        for i in &b.insts {
            for u in vregs_of(i.uses()) {
                let u = u as usize;
                if !k.contains(&u) && !g.contains(&u) {
                    g.push(u);
                }
            }
            for d in vregs_of(i.defs()) {
                if !k.contains(&(d as usize)) {
                    k.push(d as usize);
                }
            }
        }
        g.sort_unstable();
        k.sort_unstable();
        gen_.push(g);
        kill.push(k);
    }
    let succs: Vec<Vec<usize>> = f.blocks.iter().map(|b| b.succs.clone()).collect();
    let sol = solve(&succs, &Problem { dir: Direction::Backward, gen_, kill });
    let mut block_start = Vec::new();
    let mut live_after = Vec::new();
    for (bi, b) in f.blocks.iter().enumerate() {
        block_start.push(live_after.len());
        let mut live: Vec<u32> = sol.out[bi].iter().map(|&v| v as u32).collect();
        let mut rev = Vec::new();
        for i in b.insts.iter().rev() {
            rev.push(live.clone());
            let defs = vregs_of(i.defs());
            live.retain(|v| !defs.contains(v));
            for u in vregs_of(i.uses()) {
                if !live.contains(&u) {
                    live.push(u);
                }
            }
        }
        rev.reverse();
        live_after.extend(rev);
    }
    let n_insts = live_after.len();
    Liveness { block_start, live_after, n_insts }
}

/// 一个虚拟寄存器的活跃区间（线性编号，闭区间）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Interval {
    pub vreg: u32,
    pub start: usize,
    pub end: usize,
}

pub fn intervals(f: &MFunc, live: &Liveness) -> Vec<Interval> {
    let mut span: Vec<Option<(usize, usize)>> = vec![None; f.vregs.len()];
    let mut touch = |v: u32, i: usize| {
        let s = &mut span[v as usize];
        *s = Some(s.map_or((i, i), |(a, b)| (a.min(i), b.max(i))));
    };
    let mut idx = 0;
    for b in &f.blocks {
        for inst in &b.insts {
            for v in vregs_of(inst.uses()).into_iter().chain(vregs_of(inst.defs())) {
                touch(v, idx);
            }
            for &v in &live.live_after[idx] {
                touch(v, idx);
            }
            idx += 1;
        }
    }
    let mut out: Vec<Interval> = span
        .iter()
        .enumerate()
        .filter_map(|(v, s)| s.map(|(start, end)| Interval { vreg: v as u32, start, end }))
        .collect();
    out.sort_by_key(|i| (i.start, i.vreg));
    out
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScanStep {
    /// 处理下一条区间。
    Visit(Interval),
    /// 区间已经结束，归还寄存器。
    Expire {
        vreg: u32,
        reg: Reg,
    },
    Assign {
        vreg: u32,
        reg: Reg,
    },
    /// 寄存器用完：溢出当前区间。
    SpillSelf {
        vreg: u32,
    },
    /// 寄存器用完：溢出结束得更晚的 `victim`，把它的寄存器让给当前区间。
    SpillOther {
        vreg: u32,
        victim: u32,
        reg: Reg,
    },
}

#[derive(Clone, Debug)]
pub struct Allocation {
    pub loc: Vec<Option<Loc>>,
    pub slots: usize,
}

impl Allocation {
    fn new(n: usize) -> Self {
        Allocation { loc: vec![None; n], slots: 0 }
    }

    fn spill(&mut self, v: u32) {
        self.loc[v as usize] = Some(Loc::Slot(self.slots));
        self.slots += 1;
    }
}

/// 线性扫描的结果。
#[derive(Clone, Debug)]
pub struct LinearScan {
    pub intervals: Vec<Interval>,
    pub steps: Vec<ScanStep>,
    pub alloc: Allocation,
}

/// 线性扫描，最多用 k 个寄存器。
pub fn linear_scan(f: &MFunc, k: usize) -> LinearScan {
    let live = liveness(f);
    let ivs = intervals(f, &live);
    let mut steps = Vec::new();
    let mut alloc = Allocation::new(f.vregs.len());
    let mut free: Vec<Reg> = POOL[..k.clamp(1, POOL.len())].to_vec();
    let mut active: Vec<(Interval, Reg)> = Vec::new();
    for iv in &ivs {
        steps.push(ScanStep::Visit(*iv));
        active.sort_by_key(|(a, _)| a.end);
        while let Some((a, r)) = active.first().copied() {
            if a.end >= iv.start {
                break;
            }
            active.remove(0);
            free.push(r);
            free.sort_by_key(|r| POOL.iter().position(|p| p == r));
            steps.push(ScanStep::Expire { vreg: a.vreg, reg: r });
        }
        if free.is_empty() {
            let (victim, reg) = *active.last().unwrap();
            if victim.end > iv.end {
                active.pop();
                alloc.spill(victim.vreg);
                alloc.loc[iv.vreg as usize] = Some(Loc::Reg(reg));
                active.push((*iv, reg));
                steps.push(ScanStep::SpillOther { vreg: iv.vreg, victim: victim.vreg, reg });
            } else {
                alloc.spill(iv.vreg);
                steps.push(ScanStep::SpillSelf { vreg: iv.vreg });
            }
        } else {
            let reg = free.remove(0);
            alloc.loc[iv.vreg as usize] = Some(Loc::Reg(reg));
            active.push((*iv, reg));
            steps.push(ScanStep::Assign { vreg: iv.vreg, reg });
        }
    }
    LinearScan { intervals: ivs, steps, alloc }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ColorStep {
    /// 度数小于 K，拿掉压栈（它将来一定能着色）。
    Simplify { vreg: u32, degree: usize },
    /// 剩下的点度数都 ≥ K：挑度数最大的，作为"可能溢出"压栈（乐观：放回去时说不定还有颜色）。
    Potential { vreg: u32, degree: usize },
    /// 出栈着色。`None` 表示邻居用光了所有颜色，真的溢出。
    Select { vreg: u32, reg: Option<Reg> },
}

/// 干涉图的边（无向，小的编号在前）。
pub fn interference(f: &MFunc) -> Vec<(u32, u32)> {
    let live = liveness(f);
    let mut edges = Vec::new();
    let mut idx = 0;
    for b in &f.blocks {
        for inst in &b.insts {
            let skip = match inst {
                // mv a, b：a 和 b 值相同，不算冲突（这样它们有机会分到同一个寄存器）
                MInst::Mv { rs: Reg::V(s), .. } => Some(*s),
                _ => None,
            };
            for d in vregs_of(inst.defs()) {
                for &v in &live.live_after[idx] {
                    if v != d && Some(v) != skip {
                        let e = (d.min(v), d.max(v));
                        if !edges.contains(&e) {
                            edges.push(e);
                        }
                    }
                }
            }
            idx += 1;
        }
    }
    edges.sort_unstable();
    edges
}

/// 图着色的结果。
#[derive(Clone, Debug)]
pub struct Coloring {
    /// 参与着色的虚拟寄存器。
    pub nodes: Vec<u32>,
    pub edges: Vec<(u32, u32)>,
    pub steps: Vec<ColorStep>,
    pub alloc: Allocation,
}

/// 图着色（简化 → 乐观选择）。
pub fn color(f: &MFunc, k: usize) -> Coloring {
    let k = k.clamp(1, POOL.len());
    let edges = interference(f);
    // 只给真正出现过的虚拟寄存器着色
    let mut nodes: Vec<u32> = Vec::new();
    for b in &f.blocks {
        for i in &b.insts {
            for v in vregs_of(i.uses()).into_iter().chain(vregs_of(i.defs())) {
                if !nodes.contains(&v) {
                    nodes.push(v);
                }
            }
        }
    }
    nodes.sort_unstable();
    let neighbors = |v: u32| -> Vec<u32> {
        edges
            .iter()
            .filter_map(|&(a, b)| {
                if a == v {
                    Some(b)
                } else if b == v {
                    Some(a)
                } else {
                    None
                }
            })
            .collect()
    };
    let mut steps = Vec::new();
    let mut removed: Vec<u32> = Vec::new();
    while removed.len() < nodes.len() {
        let remaining: Vec<u32> = nodes.iter().copied().filter(|v| !removed.contains(v)).collect();
        let degree = |v: u32| neighbors(v).iter().filter(|n| remaining.contains(n)).count();
        if let Some(&v) = remaining.iter().find(|&&v| degree(v) < k) {
            steps.push(ColorStep::Simplify { vreg: v, degree: degree(v) });
            removed.push(v);
        } else {
            let &v = remaining.iter().max_by_key(|&&v| (degree(v), std::cmp::Reverse(v))).unwrap();
            steps.push(ColorStep::Potential { vreg: v, degree: degree(v) });
            removed.push(v);
        }
    }
    let mut alloc = Allocation::new(f.vregs.len());
    for &v in removed.iter().rev() {
        let taken: Vec<Reg> = neighbors(v)
            .iter()
            .filter_map(|n| match alloc.loc[*n as usize] {
                Some(Loc::Reg(r)) => Some(r),
                _ => None,
            })
            .collect();
        let reg = POOL[..k].iter().copied().find(|r| !taken.contains(r));
        match reg {
            Some(r) => alloc.loc[v as usize] = Some(Loc::Reg(r)),
            None => alloc.spill(v),
        }
        steps.push(ColorStep::Select { vreg: v, reg });
    }
    Coloring { nodes, edges, steps, alloc }
}

/// 栈帧布局。
#[derive(Clone, Debug)]
pub struct Frame {
    pub size: i64,
    /// 需要保存、恢复的被调用者保存寄存器，以及它们的位置（相对 sp）。
    pub saved: Vec<(Reg, i64)>,
    /// 函数里有调用时要保存返回地址 ra。
    pub ra: Option<i64>,
    /// 溢出槽的位置（相对 sp）。
    pub slots: Vec<i64>,
}

/// 分配之后的函数：只剩物理寄存器，加上了保存/恢复现场的代码。
#[derive(Clone, Debug)]
pub struct AsmFunc {
    pub name: String,
    pub blocks: Vec<MBlock>,
    pub frame: Frame,
    /// 删掉的"自己复制给自己"的 mv 条数。
    pub removed_moves: usize,
}

/// 按分配结果改写：虚拟寄存器换成物理寄存器，溢出的插入 ld/sd，加上序言和尾声。
pub fn finalize(f: &MFunc, alloc: &Allocation) -> AsmFunc {
    let scratch = [Reg::T(5), Reg::T(6)];
    let mut used_s: Vec<Reg> = alloc
        .loc
        .iter()
        .filter_map(|l| match l {
            Some(Loc::Reg(r)) => Some(*r),
            _ => None,
        })
        .collect();
    used_s.sort_by_key(|r| POOL.iter().position(|p| p == r));
    used_s.dedup();
    let has_call = f.blocks.iter().any(|b| b.insts.iter().any(|i| matches!(i, MInst::Call { .. })));
    let words = alloc.slots + used_s.len() + usize::from(has_call);
    let size = ((words * 8).div_ceil(16) * 16) as i64;
    let slots: Vec<i64> = (0..alloc.slots).map(|s| s as i64 * 8).collect();
    let saved: Vec<(Reg, i64)> = used_s.iter().enumerate().map(|(i, r)| (*r, (alloc.slots + i) as i64 * 8)).collect();
    let ra = has_call.then_some(size - 8);

    let mut removed_moves = 0;
    let mut blocks = Vec::new();
    for b in &f.blocks {
        let mut out = Vec::new();
        for inst in &b.insts {
            let mut inst = inst.clone();
            // 溢出的读：先从栈里读到临时寄存器
            let mut loads: Vec<(u32, Reg)> = Vec::new();
            for u in vregs_of(inst.uses()) {
                if let Some(Loc::Slot(s)) = alloc.loc[u as usize]
                    && !loads.iter().any(|(v, _)| *v == u)
                {
                    let r = scratch[loads.len()];
                    out.push(MInst::Ld { rd: r, off: slots[s], base: Reg::Sp });
                    loads.push((u, r));
                }
            }
            let mut store = None;
            let defs = vregs_of(inst.defs());
            inst.map_regs(|r| match r {
                Reg::V(v) => {
                    if defs.contains(&v)
                        && let Some(Loc::Slot(s)) = alloc.loc[v as usize]
                    {
                        store = Some(s);
                    }
                    match alloc.loc[v as usize] {
                        Some(Loc::Reg(p)) => p,
                        _ => loads.iter().find(|(x, _)| *x == v).map_or(scratch[0], |(_, r)| *r),
                    }
                }
                other => other,
            });
            // 写入溢出变量的指令：结果先写到 t5（上面 map_regs 已经把目标换成了 t5 或读入时用的临时寄存器）
            if let Some(s) = store {
                let rd = inst.defs()[0];
                if let MInst::Mv { rd: d, rs } = &inst
                    && d == rs
                {
                    removed_moves += 1;
                } else {
                    out.push(inst.clone());
                }
                out.push(MInst::Sd { rs: rd, off: slots[s], base: Reg::Sp });
                continue;
            }
            if let MInst::Mv { rd, rs } = &inst
                && rd == rs
            {
                removed_moves += 1;
                continue;
            }
            out.push(inst);
        }
        blocks.push(MBlock { label: b.label.clone(), insts: out, succs: b.succs.clone() });
    }
    // 最后一块末尾的 j 出口是多余的（出口就紧跟在后面）
    let exit = f.exit_label();
    if let Some(last) = blocks.last_mut()
        && matches!(last.insts.last(), Some(MInst::J { target }) if *target == exit)
    {
        last.insts.pop();
    }
    // 序言：开辟栈帧，保存 ra 和用到的 s 寄存器
    let mut pro = Vec::new();
    if size > 0 {
        pro.push(MInst::I { op: crate::rv::IOp::Addi, rd: Reg::Sp, rs1: Reg::Sp, imm: -size });
    }
    if let Some(off) = ra {
        pro.push(MInst::Sd { rs: Reg::Ra, off, base: Reg::Sp });
    }
    for (r, off) in &saved {
        pro.push(MInst::Sd { rs: *r, off: *off, base: Reg::Sp });
    }
    // 尾声：恢复，释放栈帧，返回
    let mut epi = Vec::new();
    for (r, off) in &saved {
        epi.push(MInst::Ld { rd: *r, off: *off, base: Reg::Sp });
    }
    if let Some(off) = ra {
        epi.push(MInst::Ld { rd: Reg::Ra, off, base: Reg::Sp });
    }
    if size > 0 {
        epi.push(MInst::I { op: crate::rv::IOp::Addi, rd: Reg::Sp, rs1: Reg::Sp, imm: size });
    }
    epi.push(MInst::Ret);
    let mut all = vec![MBlock { label: f.name.clone(), insts: pro, succs: vec![1] }];
    for mut b in blocks {
        b.succs = b.succs.iter().map(|s| s + 1).collect();
        all.push(b);
    }
    all.push(MBlock { label: exit, insts: epi, succs: vec![] });
    AsmFunc { name: f.name.clone(), blocks: all, frame: Frame { size, saved, ra, slots }, removed_moves }
}
