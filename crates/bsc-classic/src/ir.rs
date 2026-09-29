use crate::ast::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Value {
    Const(i64),
    Bool(bool),
    Reg(usize),
}

impl Value {
    pub fn to_string(&self) -> String {
        match self {
            Value::Const(c) => c.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Reg(r) => format!("%v{}", r),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Instruction {
    Alloca { dest: usize, name: String },
    Store { src: Value, var_name: String },
    Load { dest: usize, var_name: String },
    BinOp { dest: usize, op: BinOp, left: Value, right: Value },
    UnaryOp { dest: usize, op: UnOp, src: Value },
    Call { dest: Option<usize>, func: String, args: Vec<Value> },
    Phi { dest: usize, incoming: Vec<(Value, String)> },
    Br { cond: Value, then_bb: String, else_bb: String },
    Jmp { target_bb: String },
    Ret { val: Option<Value> },
    Print { val: Value },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BasicBlock {
    pub label: String,
    pub instructions: Vec<Instruction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionIR {
    pub name: String,
    pub params: Vec<String>,
    pub blocks: Vec<BasicBlock>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleIR {
    pub name: String,
    pub functions: Vec<FunctionIR>,
}

impl ModuleIR {
    pub fn dump(&self) -> String {
        let mut out = format!("; --- SSA Intermediate Representation: {} ---\n\n", self.name);
        for func in &self.functions {
            let params_str = func.params.iter().map(|p| format!("%{}", p)).collect::<Vec<_>>().join(", ");
            out.push_str(&format!("define @{}({}) {{\n", func.name, params_str));
            for bb in &func.blocks {
                out.push_str(&format!("{}:\n", bb.label));
                for inst in &bb.instructions {
                    out.push_str("    ");
                    match inst {
                        Instruction::Alloca { dest, name } => {
                            out.push_str(&format!("%v{} = alloca i64, name \"{}\"\n", dest, name));
                        }
                        Instruction::Store { src, var_name } => {
                            out.push_str(&format!("store i64 {}, *{}\n", src.to_string(), var_name));
                        }
                        Instruction::Load { dest, var_name } => {
                            out.push_str(&format!("%v{} = load i64, *{}\n", dest, var_name));
                        }
                        Instruction::BinOp { dest, op, left, right } => {
                            let op_str = match op {
                                BinOp::Add => "add",
                                BinOp::Sub => "sub",
                                BinOp::Mul => "mul",
                                BinOp::Div => "sdiv",
                                BinOp::Mod => "srem",
                                BinOp::Eq => "icmp eq",
                                BinOp::Ne => "icmp ne",
                                BinOp::Lt => "icmp slt",
                                BinOp::Le => "icmp sle",
                                BinOp::Gt => "icmp sgt",
                                BinOp::Ge => "icmp sge",
                                BinOp::And => "and",
                                BinOp::Or => "or",
                            };
                            out.push_str(&format!("%v{} = {} {}, {}\n", dest, op_str, left.to_string(), right.to_string()));
                        }
                        Instruction::UnaryOp { dest, op, src } => {
                            let op_str = match op {
                                UnOp::Neg => "neg",
                                UnOp::Not => "not",
                            };
                            out.push_str(&format!("%v{} = {} {}\n", dest, op_str, src.to_string()));
                        }
                        Instruction::Call { dest, func, args } => {
                            let args_str = args.iter().map(|a| a.to_string()).collect::<Vec<_>>().join(", ");
                            if let Some(d) = dest {
                                out.push_str(&format!("%v{} = call @{}({})\n", d, func, args_str));
                            } else {
                                out.push_str(&format!("call @{}({})\n", func, args_str));
                            }
                        }
                        Instruction::Phi { dest, incoming } => {
                            let inc_str = incoming.iter().map(|(v, b)| format!("[ {}, %{} ]", v.to_string(), b)).collect::<Vec<_>>().join(", ");
                            out.push_str(&format!("%v{} = phi i64 {}\n", dest, inc_str));
                        }
                        Instruction::Br { cond, then_bb, else_bb } => {
                            out.push_str(&format!("br i1 {}, label %{}, label %{}\n", cond.to_string(), then_bb, else_bb));
                        }
                        Instruction::Jmp { target_bb } => {
                            out.push_str(&format!("jmp label %{}\n", target_bb));
                        }
                        Instruction::Ret { val } => {
                            if let Some(v) = val {
                                out.push_str(&format!("ret i64 {}\n", v.to_string()));
                            } else {
                                out.push_str("ret void\n");
                            }
                        }
                        Instruction::Print { val } => {
                            out.push_str(&format!("print i64 {}\n", val.to_string()));
                        }
                    }
                }
            }
            out.push_str("}\n\n");
        }
        out
    }
}

pub struct IRBuilder {
    reg_counter: usize,
    bb_counter: usize,
    current_blocks: Vec<BasicBlock>,
    current_bb_idx: usize,
}

impl IRBuilder {
    pub fn new() -> Self {
        Self {
            reg_counter: 0,
            bb_counter: 0,
            current_blocks: Vec::new(),
            current_bb_idx: 0,
        }
    }

    fn next_reg(&mut self) -> usize {
        let r = self.reg_counter;
        self.reg_counter += 1;
        r
    }

    fn new_label(&mut self, prefix: &str) -> String {
        let id = self.bb_counter;
        self.bb_counter += 1;
        format!("{}.{}", prefix, id)
    }

    fn append_bb(&mut self, label: String) -> usize {
        let idx = self.current_blocks.len();
        self.current_blocks.push(BasicBlock {
            label,
            instructions: Vec::new(),
        });
        idx
    }

    fn emit(&mut self, inst: Instruction) {
        if self.current_bb_idx < self.current_blocks.len() {
            self.current_blocks[self.current_bb_idx].instructions.push(inst);
        }
    }

    pub fn build_program(&mut self, prog: &Program) -> ModuleIR {
        let mut functions = Vec::new();

        for func in &prog.functions {
            functions.push(self.build_function(func));
        }

        ModuleIR {
            name: "classic_module".to_string(),
            functions,
        }
    }

    fn build_function(&mut self, func: &Function) -> FunctionIR {
        self.current_blocks.clear();
        self.current_bb_idx = self.append_bb("entry".to_string());

        // Allocate local parameters
        for (p_name, _) in &func.params {
            let reg = self.next_reg();
            self.emit(Instruction::Alloca { dest: reg, name: p_name.clone() });
        }

        for stmt in &func.body {
            self.build_stmt(stmt);
        }

        // Default ret if not terminated
        if let Some(last_bb) = self.current_blocks.last() {
            let is_term = last_bb.instructions.last().map(|i| matches!(i, Instruction::Ret { .. } | Instruction::Jmp { .. } | Instruction::Br { .. })).unwrap_or(false);
            if !is_term {
                self.emit(Instruction::Ret { val: None });
            }
        }

        FunctionIR {
            name: func.name.clone(),
            params: func.params.iter().map(|(p, _)| p.clone()).collect(),
            blocks: self.current_blocks.clone(),
        }
    }

    fn build_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let(name, _, init, _) => {
                let val = self.build_expr(init);
                let reg = self.next_reg();
                self.emit(Instruction::Alloca { dest: reg, name: name.clone() });
                self.emit(Instruction::Store { src: val, var_name: name.clone() });
            }
            Stmt::Assign(name, expr, _) => {
                let val = self.build_expr(expr);
                self.emit(Instruction::Store { src: val, var_name: name.clone() });
            }
            Stmt::If(cond, then_block, else_block, _) => {
                let cond_val = self.build_expr(cond);

                let then_lbl = self.new_label("if.then");
                let else_lbl = self.new_label("if.else");
                let merge_lbl = self.new_label("if.merge");

                self.emit(Instruction::Br {
                    cond: cond_val,
                    then_bb: then_lbl.clone(),
                    else_bb: if else_block.is_some() { else_lbl.clone() } else { merge_lbl.clone() },
                });

                // Then BB
                self.current_bb_idx = self.append_bb(then_lbl);
                for s in then_block { self.build_stmt(s); }
                self.emit(Instruction::Jmp { target_bb: merge_lbl.clone() });

                // Else BB
                if let Some(el) = else_block {
                    self.current_bb_idx = self.append_bb(else_lbl);
                    for s in el { self.build_stmt(s); }
                    self.emit(Instruction::Jmp { target_bb: merge_lbl.clone() });
                }

                // Merge BB
                self.current_bb_idx = self.append_bb(merge_lbl);
            }
            Stmt::While(cond, body, _) => {
                let cond_lbl = self.new_label("while.cond");
                let body_lbl = self.new_label("while.body");
                let exit_lbl = self.new_label("while.exit");

                self.emit(Instruction::Jmp { target_bb: cond_lbl.clone() });

                // Cond BB
                self.current_bb_idx = self.append_bb(cond_lbl.clone());
                let cond_val = self.build_expr(cond);
                self.emit(Instruction::Br {
                    cond: cond_val,
                    then_bb: body_lbl.clone(),
                    else_bb: exit_lbl.clone(),
                });

                // Body BB
                self.current_bb_idx = self.append_bb(body_lbl);
                for s in body { self.build_stmt(s); }
                self.emit(Instruction::Jmp { target_bb: cond_lbl });

                // Exit BB
                self.current_bb_idx = self.append_bb(exit_lbl);
            }
            Stmt::Return(expr, _) => {
                let val = expr.as_ref().map(|e| self.build_expr(e));
                self.emit(Instruction::Ret { val });
            }
            Stmt::Print(expr, _) => {
                let val = self.build_expr(expr);
                self.emit(Instruction::Print { val });
            }
            Stmt::Expr(expr, _) => {
                self.build_expr(expr);
            }
        }
    }

    fn build_expr(&mut self, expr: &Expr) -> Value {
        match expr {
            Expr::Literal(val, _) => Value::Const(*val),
            Expr::BoolLit(b, _) => Value::Bool(*b),
            Expr::Variable(name, _) => {
                let reg = self.next_reg();
                self.emit(Instruction::Load { dest: reg, var_name: name.clone() });
                Value::Reg(reg)
            }
            Expr::Binary(op, left, right, _) => {
                let l_val = self.build_expr(left);
                let r_val = self.build_expr(right);
                let reg = self.next_reg();
                self.emit(Instruction::BinOp { dest: reg, op: *op, left: l_val, right: r_val });
                Value::Reg(reg)
            }
            Expr::Unary(op, inner, _) => {
                let in_val = self.build_expr(inner);
                let reg = self.next_reg();
                self.emit(Instruction::UnaryOp { dest: reg, op: *op, src: in_val });
                Value::Reg(reg)
            }
            Expr::Call(name, args, _) => {
                let arg_vals: Vec<Value> = args.iter().map(|a| self.build_expr(a)).collect();
                let reg = self.next_reg();
                self.emit(Instruction::Call { dest: Some(reg), func: name.clone(), args: arg_vals });
                Value::Reg(reg)
            }
        }
    }
}
