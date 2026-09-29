//! SSA（静态单赋值）形式的构造。
//!
//! SSA 要求**每个变量在程序文本里只被赋值一次**。做法是给每次赋值换一个新名字（x.1、x.2……），
//! 并在控制流汇合的地方插入 φ 函数：`x.3 = φ(B1: x.1, B2: x.2)` 表示"从 B1 来就取 x.1，从 B2 来就取 x.2"。
//!
//! 构造分两步（Cytron 等人 1991 年的经典算法）：
//!
//! 1. **插入 φ**：变量 x 在块 b 里被赋值，那么 b 的支配边界 DF(b) 上的每个块都需要一个 x 的 φ；
//!    φ 本身也是一次赋值，所以对新插了 φ 的块重复这个过程。
//!    只给"跨块使用"的变量插 φ（某个块里先用后赋值的变量），这叫半剪枝（semi-pruned）SSA。
//! 2. **重命名**：沿支配树深度优先遍历，每个变量维护一个"当前版本"的栈：
//!    遇到赋值就压入新版本，遇到使用就换成栈顶；处理完一个块，把它的后继块里 φ 的对应参数填上；
//!    离开块时弹出这个块压入的版本。

use crate::cfg::Cfg;
use crate::dom::{Dominators, dominators};
use crate::ir::{Inst, Operand, Var, VarId};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    /// 需要考虑 φ 的变量：在某个块里先被使用、后被赋值（或者根本没赋值）。
    Globals(Vec<VarId>),
    /// 变量在哪些块里被赋值。
    DefSites { var: VarId, blocks: Vec<usize> },
    /// 因为 block ∈ DF(from)，在 block 开头插入 var 的 φ。
    Phi { var: VarId, block: usize, from: usize },
    /// 重命名：进入一个块。
    Enter(usize),
    /// 这条指令改写完成（`inst` 是 SSA 形式里的序号，φ 排在最前面）。
    Rewrite { block: usize, inst: usize },
    /// 变量 var 有了新版本 new，压入它的栈。
    Push { var: VarId, new: VarId },
    /// 填上后继块 block 里第 inst 条（φ）的第 arg 个参数。
    PhiArg { block: usize, inst: usize, arg: usize },
    /// 离开块，弹出这个块压入的版本。
    Exit { block: usize, popped: Vec<VarId> },
}

#[derive(Clone, Debug)]
pub struct Ssa {
    /// 构造前（已删除不可达块）。
    pub before: Cfg,
    /// SSA 形式。变量表是原来的变量加上各个版本。
    pub after: Cfg,
    pub dom: Dominators,
    /// 被重命名的原变量（只赋值一次、也不需要 φ 的变量保持原名）。
    pub renamed: Vec<VarId>,
    /// 每个块开头的 φ 各是哪个原变量的。
    pub phi_vars: Vec<Vec<VarId>>,
    pub steps: Vec<Step>,
}

pub fn build(cfg: &Cfg) -> Ssa {
    let before = cfg.clone();
    let n = before.blocks.len();
    let nvars = before.func.vars.len();
    let dom = dominators(&before.succs());
    let mut steps = Vec::new();

    // 1. 跨块使用的变量、每个变量的赋值位置
    let mut globals = Vec::new();
    let mut def_sites: Vec<Vec<usize>> = vec![Vec::new(); nvars];
    for (b, block) in before.blocks.iter().enumerate() {
        let mut killed = vec![false; nvars];
        for inst in &block.insts {
            for u in inst.uses() {
                if let Operand::Var(v) = u
                    && !killed[v]
                    && !globals.contains(&v)
                {
                    globals.push(v);
                }
            }
            if let Some(d) = inst.def() {
                killed[d] = true;
                if !def_sites[d].contains(&b) {
                    def_sites[d].push(b);
                }
            }
        }
    }
    globals.sort_unstable();
    steps.push(Step::Globals(globals.clone()));

    // 2. 插入 φ
    let mut phi_vars: Vec<Vec<VarId>> = vec![Vec::new(); n];
    for &v in &globals {
        steps.push(Step::DefSites { var: v, blocks: def_sites[v].clone() });
        let mut work = def_sites[v].clone();
        let mut has_def = def_sites[v].clone();
        while let Some(b) = work.pop() {
            for &d in &dom.df[b] {
                if !phi_vars[d].contains(&v) {
                    phi_vars[d].push(v);
                    steps.push(Step::Phi { var: v, block: d, from: b });
                    if !has_def.contains(&d) {
                        has_def.push(d);
                        work.push(d);
                    }
                }
            }
        }
    }

    // 需要重命名的：静态赋值不止一次（φ 也算一次赋值）
    let mut def_count = vec![0usize; nvars];
    for block in &before.blocks {
        for inst in &block.insts {
            if let Some(d) = inst.def() {
                def_count[d] += 1;
            }
        }
    }
    for pv in &phi_vars {
        for &v in pv {
            def_count[v] += 1;
        }
    }
    let renamed: Vec<VarId> = (0..nvars).filter(|&v| def_count[v] >= 2).collect();

    let mut after = before.clone();
    for (b, block) in after.blocks.iter_mut().enumerate() {
        let phis: Vec<Inst> = phi_vars[b]
            .iter()
            .map(|&v| Inst::Phi { dst: v, args: block.preds.iter().map(|&p| (p, Operand::Undef)).collect() })
            .collect();
        let spans = vec![block.origin.first().copied().unwrap_or_default(); phis.len()];
        block.insts.splice(0..0, phis);
        block.origin.splice(0..0, spans);
    }

    // 3. 重命名
    let mut r = Rename {
        cfg: &mut after,
        dom: &dom,
        is_renamed: (0..nvars).map(|v| renamed.contains(&v)).collect(),
        stacks: vec![Vec::new(); nvars],
        counter: vec![0; nvars],
        phi_vars: &phi_vars,
        steps: &mut steps,
    };
    r.block(0);
    Ssa { before, after, dom, renamed, phi_vars, steps }
}

struct Rename<'a> {
    cfg: &'a mut Cfg,
    dom: &'a Dominators,
    is_renamed: Vec<bool>,
    stacks: Vec<Vec<VarId>>,
    counter: Vec<u32>,
    phi_vars: &'a [Vec<VarId>],
    steps: &'a mut Vec<Step>,
}

impl Rename<'_> {
    fn top(&self, v: VarId) -> Operand {
        self.stacks[v].last().map_or(Operand::Undef, |&x| Operand::Var(x))
    }

    fn fresh(&mut self, v: VarId) -> VarId {
        self.counter[v] += 1;
        let k = self.counter[v];
        let base = &self.cfg.func.vars[v];
        let var = Var { name: format!("{}.{k}", base.name), temp: base.temp, ssa: Some((v, k)) };
        self.cfg.func.vars.push(var);
        self.cfg.func.vars.len() - 1
    }

    fn block(&mut self, b: usize) {
        self.steps.push(Step::Enter(b));
        let mut pushed = Vec::new();
        for i in 0..self.cfg.blocks[b].insts.len() {
            let mut inst = self.cfg.blocks[b].insts[i].clone();
            if !matches!(inst, Inst::Phi { .. }) {
                for u in inst.uses_mut() {
                    if let Operand::Var(v) = *u
                        && self.is_renamed[v]
                    {
                        *u = self.top(v);
                    }
                }
            }
            if let Some(d) = inst.def()
                && self.is_renamed[d]
            {
                let new = self.fresh(d);
                *inst.def_mut().unwrap() = new;
                self.stacks[d].push(new);
                pushed.push(d);
                self.steps.push(Step::Push { var: d, new });
            }
            self.cfg.blocks[b].insts[i] = inst;
            self.steps.push(Step::Rewrite { block: b, inst: i });
        }
        let mut succs = self.cfg.blocks[b].succs.clone();
        succs.dedup();
        for s in succs {
            let j = self.cfg.blocks[s].preds.iter().position(|&p| p == b).expect("b 是 s 的前驱");
            for (pi, &v) in self.phi_vars[s].iter().enumerate() {
                let val = self.top(v);
                if let Inst::Phi { args, .. } = &mut self.cfg.blocks[s].insts[pi] {
                    args[j].1 = val;
                }
                self.steps.push(Step::PhiArg { block: s, inst: pi, arg: j });
            }
        }
        for c in self.dom.children[b].clone() {
            self.block(c);
        }
        for &v in &pushed {
            self.stacks[v].pop();
        }
        self.steps.push(Step::Exit { block: b, popped: pushed });
    }
}
