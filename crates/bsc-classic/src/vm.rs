use crate::ast::{BinOp, UnOp};
use crate::ir::*;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct VMExecutionReport {
    pub success: bool,
    pub return_value: Option<i64>,
    pub stdout_lines: Vec<String>,
    pub instructions_executed: usize,
    pub call_count: usize,
    pub error: Option<String>,
}

#[derive(Clone)]
#[allow(dead_code)]
struct Frame {
    pub function_name: String,
    pub registers: HashMap<usize, i64>,
    pub variables: HashMap<String, i64>,
    pub current_bb: String,
    pub inst_idx: usize,
}

#[allow(dead_code)]
pub struct VirtualMachine<'a> {
    module: &'a ModuleIR,
    fn_map: HashMap<String, &'a FunctionIR>,
    frames: Vec<Frame>,
    stdout: Vec<String>,
    instructions_executed: usize,
    max_instructions: usize,
}

impl<'a> VirtualMachine<'a> {
    pub fn new(module: &'a ModuleIR) -> Self {
        let mut fn_map = HashMap::new();
        for func in &module.functions {
            fn_map.insert(func.name.clone(), func);
        }
        Self {
            module,
            fn_map,
            frames: Vec::new(),
            stdout: Vec::new(),
            instructions_executed: 0,
            max_instructions: 100_000,
        }
    }

    pub fn run_main(&mut self) -> VMExecutionReport {
        let main_fn = match self.fn_map.get("main") {
            Some(f) => *f,
            None => {
                return VMExecutionReport {
                    success: false,
                    return_value: None,
                    stdout_lines: vec![],
                    instructions_executed: 0,
                    call_count: 0,
                    error: Some("No 'main()' entry point found in module".to_string()),
                };
            }
        };

        match self.call_function(main_fn, vec![]) {
            Ok(ret) => VMExecutionReport {
                success: true,
                return_value: ret,
                stdout_lines: self.stdout.clone(),
                instructions_executed: self.instructions_executed,
                call_count: self.frames.len(),
                error: None,
            },
            Err(e) => VMExecutionReport {
                success: false,
                return_value: None,
                stdout_lines: self.stdout.clone(),
                instructions_executed: self.instructions_executed,
                call_count: self.frames.len(),
                error: Some(e),
            },
        }
    }

    fn call_function(&mut self, func: &FunctionIR, args: Vec<i64>) -> Result<Option<i64>, String> {
        let mut frame = Frame {
            function_name: func.name.clone(),
            registers: HashMap::new(),
            variables: HashMap::new(),
            current_bb: "entry".to_string(),
            inst_idx: 0,
        };

        // Populate params
        for (i, p_name) in func.params.iter().enumerate() {
            if i < args.len() {
                frame.variables.insert(p_name.clone(), args[i]);
            }
        }

        self.frames.push(frame);

        let bb_map: HashMap<String, &BasicBlock> = func.blocks.iter().map(|b| (b.label.clone(), b)).collect();
        let mut current_bb_label = "entry".to_string();

        while let Some(bb) = bb_map.get(&current_bb_label) {
            let mut i = 0;
            while i < bb.instructions.len() {
                self.instructions_executed += 1;
                if self.instructions_executed > self.max_instructions {
                    return Err(format!("Execution step limit exceeded ({} instructions) - possible infinite loop", self.max_instructions));
                }

                let inst = &bb.instructions[i];
                match inst {
                    Instruction::Alloca { .. } => {}
                    Instruction::Store { src, var_name } => {
                        let val = self.eval_value(src)?;
                        self.frames.last_mut().unwrap().variables.insert(var_name.clone(), val);
                    }
                    Instruction::Load { dest, var_name } => {
                        let val = self.frames.last().unwrap().variables.get(var_name).copied().unwrap_or(0);
                        self.frames.last_mut().unwrap().registers.insert(*dest, val);
                    }
                    Instruction::BinOp { dest, op, left, right } => {
                        let l = self.eval_value(left)?;
                        let r = self.eval_value(right)?;
                        let res = match op {
                            BinOp::Add => l.wrapping_add(r),
                            BinOp::Sub => l.wrapping_sub(r),
                            BinOp::Mul => l.wrapping_mul(r),
                            BinOp::Div => if r != 0 { l / r } else { return Err("Division by zero".to_string()); },
                            BinOp::Mod => if r != 0 { l % r } else { return Err("Modulo by zero".to_string()); },
                            BinOp::Eq => if l == r { 1 } else { 0 },
                            BinOp::Ne => if l != r { 1 } else { 0 },
                            BinOp::Lt => if l < r { 1 } else { 0 },
                            BinOp::Le => if l <= r { 1 } else { 0 },
                            BinOp::Gt => if l > r { 1 } else { 0 },
                            BinOp::Ge => if l >= r { 1 } else { 0 },
                            BinOp::And => if l != 0 && r != 0 { 1 } else { 0 },
                            BinOp::Or => if l != 0 || r != 0 { 1 } else { 0 },
                        };
                        self.frames.last_mut().unwrap().registers.insert(*dest, res);
                    }
                    Instruction::UnaryOp { dest, op, src } => {
                        let v = self.eval_value(src)?;
                        let res = match op {
                            UnOp::Neg => -v,
                            UnOp::Not => if v == 0 { 1 } else { 0 },
                        };
                        self.frames.last_mut().unwrap().registers.insert(*dest, res);
                    }
                    Instruction::Call { dest, func: callee_name, args } => {
                        let mut evaluated_args = Vec::new();
                        for a in args {
                            evaluated_args.push(self.eval_value(a)?);
                        }

                        if callee_name == "print" {
                            let p_val = evaluated_args.first().copied().unwrap_or(0);
                            self.stdout.push(p_val.to_string());
                        } else {
                            let callee = self.fn_map.get(callee_name).copied().ok_or_else(|| {
                                format!("Unknown function '{}' called in VM", callee_name)
                            })?;
                            let ret = self.call_function(callee, evaluated_args)?;
                            if let Some(d) = dest {
                                if let Some(v) = ret {
                                    self.frames.last_mut().unwrap().registers.insert(*d, v);
                                }
                            }
                        }
                    }
                    Instruction::Phi { dest, incoming } => {
                        // In basic linear execution, choose first available
                        if let Some((v, _)) = incoming.first() {
                            let val = self.eval_value(v)?;
                            self.frames.last_mut().unwrap().registers.insert(*dest, val);
                        }
                    }
                    Instruction::Br { cond, then_bb, else_bb } => {
                        let c = self.eval_value(cond)?;
                        current_bb_label = if c != 0 { then_bb.clone() } else { else_bb.clone() };
                        break;
                    }
                    Instruction::Jmp { target_bb } => {
                        current_bb_label = target_bb.clone();
                        break;
                    }
                    Instruction::Ret { val } => {
                        let ret_val = match val {
                            Some(v) => Some(self.eval_value(v)?),
                            None => None,
                        };
                        self.frames.pop();
                        return Ok(ret_val);
                    }
                    Instruction::Print { val } => {
                        let v = self.eval_value(val)?;
                        self.stdout.push(v.to_string());
                    }
                }
                i += 1;
            }
        }

        self.frames.pop();
        Ok(None)
    }

    fn eval_value(&self, val: &Value) -> Result<i64, String> {
        match val {
            Value::Const(c) => Ok(*c),
            Value::Bool(b) => Ok(if *b { 1 } else { 0 }),
            Value::Reg(r) => {
                let frame = self.frames.last().ok_or("No active stack frame")?;
                Ok(frame.registers.get(r).copied().unwrap_or(0))
            }
        }
    }
}
