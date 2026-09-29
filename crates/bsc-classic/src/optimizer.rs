// crates/bsc-classic/src/optimizer.rs
use crate::ast::BinOp;
use crate::ir::*;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum OptLevel {
    O0, // No optimization
    O1, // Basic constant folding and static branch pruning
    O2, // Aggressive: Constant folding + Algebraic identity simplification + DCE
}

impl Default for OptLevel {
    fn default() -> Self {
        OptLevel::O2
    }
}

pub struct Optimizer {
    pub passes_run: usize,
    pub instructions_eliminated: usize,
}

impl Optimizer {
    pub fn new() -> Self {
        Self {
            passes_run: 0,
            instructions_eliminated: 0,
        }
    }

    pub fn optimize_module(&mut self, module: &mut ModuleIR) {
        self.optimize_module_with_level(module, OptLevel::O2);
    }

    pub fn optimize_module_with_level(&mut self, module: &mut ModuleIR, level: OptLevel) {
        if level == OptLevel::O0 {
            return;
        }
        for func in &mut module.functions {
            self.optimize_function_with_level(func, level);
        }
    }

    fn optimize_function_with_level(&mut self, func: &mut FunctionIR, level: OptLevel) {
        self.passes_run += 1;

        // 1. Constant Folding & Copy Propagation map: reg -> known value (Const, Bool, or Reg)
        let mut val_map: HashMap<usize, Value> = HashMap::new();

        for bb in &mut func.blocks {
            for inst in &mut bb.instructions {
                match inst {
                    Instruction::BinOp { dest, op, left, right } => {
                        let bin_op = *op;
                        // Substitute known constants or aliases
                        if let Value::Reg(r) = left {
                            if let Some(c) = val_map.get(r) { *left = c.clone(); }
                        }
                        if let Value::Reg(r) = right {
                            if let Some(c) = val_map.get(r) { *right = c.clone(); }
                        }

                        // Level O2: Algebraic identities
                        if level == OptLevel::O2 {
                            let identity_val = match (bin_op, &left, &right) {
                                (BinOp::Add, Value::Const(0), r) | (BinOp::Add, r, Value::Const(0)) => Some((*r).clone()),
                                (BinOp::Sub, r, Value::Const(0)) => Some((*r).clone()),
                                (BinOp::Mul, Value::Const(1), r) | (BinOp::Mul, r, Value::Const(1)) => Some((*r).clone()),
                                (BinOp::Mul, Value::Const(0), _) | (BinOp::Mul, _, Value::Const(0)) => Some(Value::Const(0)),
                                _ => None,
                            };
                            if let Some(v) = identity_val {
                                val_map.insert(*dest, v);
                                self.instructions_eliminated += 1;
                                continue;
                            }
                        }

                        // Fold if both operands are constant
                        if let (Value::Const(c1), Value::Const(c2)) = (&left, &right) {
                            let folded = match bin_op {
                                BinOp::Add => Some(Value::Const(c1 + c2)),
                                BinOp::Sub => Some(Value::Const(c1 - c2)),
                                BinOp::Mul => Some(Value::Const(c1 * c2)),
                                BinOp::Div if *c2 != 0 => Some(Value::Const(c1 / c2)),
                                BinOp::Mod if *c2 != 0 => Some(Value::Const(c1 % c2)),
                                BinOp::Eq => Some(Value::Bool(c1 == c2)),
                                BinOp::Ne => Some(Value::Bool(c1 != c2)),
                                BinOp::Lt => Some(Value::Bool(c1 < c2)),
                                BinOp::Le => Some(Value::Bool(c1 <= c2)),
                                BinOp::Gt => Some(Value::Bool(c1 > c2)),
                                BinOp::Ge => Some(Value::Bool(c1 >= c2)),
                                _ => None,
                            };
                            if let Some(val) = folded {
                                val_map.insert(*dest, val);
                                self.instructions_eliminated += 1;
                            }
                        }
                    }
                    Instruction::UnaryOp { dest, op, src } => {
                        if let Value::Reg(r) = src {
                            if let Some(c) = val_map.get(r) { *src = c.clone(); }
                        }
                        if let Value::Const(c) = src {
                            if *op == crate::ast::UnOp::Neg {
                                val_map.insert(*dest, Value::Const(-*c));
                                self.instructions_eliminated += 1;
                            }
                        } else if let Value::Bool(b) = src {
                            if *op == crate::ast::UnOp::Not {
                                val_map.insert(*dest, Value::Bool(!*b));
                                self.instructions_eliminated += 1;
                            }
                        }
                    }
                    Instruction::Store { src, .. } => {
                        if let Value::Reg(r) = src {
                            if let Some(c) = val_map.get(r) { *src = c.clone(); }
                        }
                    }
                    Instruction::Br { cond, then_bb, else_bb } => {
                        if let Value::Reg(r) = cond {
                            if let Some(c) = val_map.get(r) { *cond = c.clone(); }
                        }
                        // Fold static branch
                        if let Value::Bool(b) = cond {
                            let target = if *b { then_bb.clone() } else { else_bb.clone() };
                            *inst = Instruction::Jmp { target_bb: target };
                            self.instructions_eliminated += 1;
                        }
                    }
                    Instruction::Ret { val } => {
                        if let Some(Value::Reg(r)) = val {
                            if let Some(c) = val_map.get(r) { *val = Some(c.clone()); }
                        }
                    }
                    Instruction::Print { val } => {
                        if let Value::Reg(r) = val {
                            if let Some(c) = val_map.get(r) { *val = c.clone(); }
                        }
                    }
                    Instruction::Call { args, .. } => {
                        for a in args {
                            if let Value::Reg(r) = a {
                                if let Some(c) = val_map.get(r) { *a = c.clone(); }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        // 2. Dead Code Elimination (DCE) for Level O2
        if level == OptLevel::O2 {
            let mut used_regs = HashSet::new();
            for bb in &func.blocks {
                for inst in &bb.instructions {
                    match inst {
                        Instruction::Store { src, .. } => { if let Value::Reg(r) = src { used_regs.insert(*r); } }
                        Instruction::BinOp { left, right, .. } => {
                            if let Value::Reg(r) = left { used_regs.insert(*r); }
                            if let Value::Reg(r) = right { used_regs.insert(*r); }
                        }
                        Instruction::UnaryOp { src, .. } => { if let Value::Reg(r) = src { used_regs.insert(*r); } }
                        Instruction::Call { args, .. } => {
                            for a in args { if let Value::Reg(r) = a { used_regs.insert(*r); } }
                        }
                        Instruction::Phi { incoming, .. } => {
                            for (v, _) in incoming { if let Value::Reg(r) = v { used_regs.insert(*r); } }
                        }
                        Instruction::Br { cond, .. } => { if let Value::Reg(r) = cond { used_regs.insert(*r); } }
                        Instruction::Ret { val } => {
                            if let Some(Value::Reg(r)) = val { used_regs.insert(*r); }
                        }
                        Instruction::Print { val } => { if let Value::Reg(r) = val { used_regs.insert(*r); } }
                        _ => {}
                    }
                }
            }

            // Eliminate unused instructions
            for bb in &mut func.blocks {
                bb.instructions.retain(|inst| {
                    match inst {
                        Instruction::BinOp { dest, .. } | Instruction::UnaryOp { dest, .. } => {
                            if !used_regs.contains(dest) {
                                return false;
                            }
                        }
                        _ => {}
                    }
                    true
                });
            }
        }
    }
}
