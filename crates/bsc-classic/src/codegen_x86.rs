use crate::ast::BinOp;
use crate::ir::*;
use std::collections::HashMap;

pub struct X86Codegen;

impl X86Codegen {
    pub fn new() -> Self {
        Self
    }

    pub fn generate(&self, module: &ModuleIR) -> String {
        let mut asm = String::from("; --- Target Machine Code: x86-64 (System V AMD64 ABI) ---\n");
        asm.push_str(".intel_syntax noprefix\n");
        asm.push_str(".text\n\n");

        for func in &module.functions {
            asm.push_str(&self.generate_function(func));
        }

        asm.push_str("; --- Built-in runtime print helper ---\n");
        asm.push_str(".globl bsc_print_i64\nbsc_print_i64:\n");
        asm.push_str("    ret\n\n");

        asm
    }

    fn generate_function(&self, func: &FunctionIR) -> String {
        let mut out = format!(".globl {}\n{}:\n", func.name, func.name);
        out.push_str("    push rbp\n");
        out.push_str("    mov rbp, rsp\n");

        // Map virtual registers and named variables to stack slots: -8(%rbp), -16(%rbp), ...
        let mut stack_offset = 0;
        let mut reg_offsets: HashMap<usize, i32> = HashMap::new();
        let mut var_offsets: HashMap<String, i32> = HashMap::new();

        for bb in &func.blocks {
            for inst in &bb.instructions {
                match inst {
                    Instruction::Alloca { dest, name } => {
                        stack_offset += 8;
                        reg_offsets.insert(*dest, -stack_offset);
                        var_offsets.insert(name.clone(), -stack_offset);
                    }
                    Instruction::BinOp { dest, .. } | Instruction::UnaryOp { dest, .. } | Instruction::Load { dest, .. } | Instruction::Phi { dest, .. } => {
                        if !reg_offsets.contains_key(dest) {
                            stack_offset += 8;
                            reg_offsets.insert(*dest, -stack_offset);
                        }
                    }
                    Instruction::Call { dest: Some(dest), .. } => {
                        if !reg_offsets.contains_key(dest) {
                            stack_offset += 8;
                            reg_offsets.insert(*dest, -stack_offset);
                        }
                    }
                    _ => {}
                }
            }
        }

        // Align stack frame to 16 bytes
        let aligned_stack = (stack_offset + 15) & !15;
        if aligned_stack > 0 {
            out.push_str(&format!("    sub rsp, {}\n", aligned_stack));
        }

        for bb in &func.blocks {
            out.push_str(&format!(".L_{}_{}:\n", func.name, bb.label.replace('.', "_")));
            for inst in &bb.instructions {
                match inst {
                    Instruction::Store { src, var_name } => {
                        let offset = var_offsets.get(var_name).copied().unwrap_or(-8);
                        let src_op = self.val_to_op(src, &reg_offsets);
                        out.push_str(&format!("    mov rax, {}\n", src_op));
                        out.push_str(&format!("    mov [rbp{}], rax\n", self.fmt_offset(offset)));
                    }
                    Instruction::Load { dest, var_name } => {
                        let v_offset = var_offsets.get(var_name).copied().unwrap_or(-8);
                        let r_offset = reg_offsets.get(dest).copied().unwrap_or(-8);
                        out.push_str(&format!("    mov rax, [rbp{}]\n", self.fmt_offset(v_offset)));
                        out.push_str(&format!("    mov [rbp{}], rax\n", self.fmt_offset(r_offset)));
                    }
                    Instruction::BinOp { dest, op, left, right } => {
                        let d_offset = reg_offsets.get(dest).copied().unwrap_or(-8);
                        let l_op = self.val_to_op(left, &reg_offsets);
                        let r_op = self.val_to_op(right, &reg_offsets);

                        out.push_str(&format!("    mov rax, {}\n", l_op));
                        match op {
                            BinOp::Add => out.push_str(&format!("    add rax, {}\n", r_op)),
                            BinOp::Sub => out.push_str(&format!("    sub rax, {}\n", r_op)),
                            BinOp::Mul => out.push_str(&format!("    imul rax, {}\n", r_op)),
                            BinOp::Div => {
                                out.push_str("    cqo\n");
                                out.push_str(&format!("    mov rbx, {}\n", r_op));
                                out.push_str("    idiv rbx\n");
                            }
                            BinOp::Mod => {
                                out.push_str("    cqo\n");
                                out.push_str(&format!("    mov rbx, {}\n", r_op));
                                out.push_str("    idiv rbx\n");
                                out.push_str("    mov rax, rdx\n");
                            }
                            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                                out.push_str(&format!("    cmp rax, {}\n", r_op));
                                let set_cc = match op {
                                    BinOp::Eq => "sete",
                                    BinOp::Ne => "setne",
                                    BinOp::Lt => "setl",
                                    BinOp::Le => "setle",
                                    BinOp::Gt => "setg",
                                    BinOp::Ge => "setge",
                                    _ => "sete",
                                };
                                out.push_str(&format!("    {} al\n", set_cc));
                                out.push_str("    movzx rax, al\n");
                            }
                            BinOp::And => out.push_str(&format!("    and rax, {}\n", r_op)),
                            BinOp::Or => out.push_str(&format!("    or rax, {}\n", r_op)),
                        }
                        out.push_str(&format!("    mov [rbp{}], rax\n", self.fmt_offset(d_offset)));
                    }
                    Instruction::Br { cond, then_bb, else_bb } => {
                        let c_op = self.val_to_op(cond, &reg_offsets);
                        out.push_str(&format!("    cmp {}, 0\n", c_op));
                        out.push_str(&format!("    jne .L_{}_{}\n", func.name, then_bb.replace('.', "_")));
                        out.push_str(&format!("    jmp .L_{}_{}\n", func.name, else_bb.replace('.', "_")));
                    }
                    Instruction::Jmp { target_bb } => {
                        out.push_str(&format!("    jmp .L_{}_{}\n", func.name, target_bb.replace('.', "_")));
                    }
                    Instruction::Call { dest, func: target_fn, args } => {
                        // Pass first 4 args in rdi, rsi, rdx, rcx
                        let arg_regs = ["rdi", "rsi", "rdx", "rcx", "r8", "r9"];
                        for (i, arg) in args.iter().enumerate().take(6) {
                            let arg_op = self.val_to_op(arg, &reg_offsets);
                            out.push_str(&format!("    mov {}, {}\n", arg_regs[i], arg_op));
                        }
                        out.push_str(&format!("    call {}\n", target_fn));
                        if let Some(d) = dest {
                            let d_offset = reg_offsets.get(d).copied().unwrap_or(-8);
                            out.push_str(&format!("    mov [rbp{}], rax\n", self.fmt_offset(d_offset)));
                        }
                    }
                    Instruction::Ret { val } => {
                        if let Some(v) = val {
                            let v_op = self.val_to_op(v, &reg_offsets);
                            out.push_str(&format!("    mov rax, {}\n", v_op));
                        }
                        out.push_str("    leave\n");
                        out.push_str("    ret\n");
                    }
                    Instruction::Print { val } => {
                        let v_op = self.val_to_op(val, &reg_offsets);
                        out.push_str(&format!("    mov rdi, {}\n", v_op));
                        out.push_str("    call bsc_print_i64\n");
                    }
                    _ => {}
                }
            }
        }

        out.push_str("\n");
        out
    }

    fn val_to_op(&self, val: &Value, reg_offsets: &HashMap<usize, i32>) -> String {
        match val {
            Value::Const(c) => c.to_string(),
            Value::Bool(b) => (if *b { 1 } else { 0 }).to_string(),
            Value::Reg(r) => {
                let off = reg_offsets.get(r).copied().unwrap_or(-8);
                format!("[rbp{}]", self.fmt_offset(off))
            }
        }
    }

    fn fmt_offset(&self, offset: i32) -> String {
        if offset < 0 {
            format!("{}", offset)
        } else {
            format!("+{}", offset)
        }
    }
}
