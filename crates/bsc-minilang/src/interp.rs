//! 控制流图的解释器：直接执行三地址码（包括 SSA 形式的 φ）。
//!
//! 用来做对照测试——同一个程序，降级前后、转成 SSA 前后，运行结果必须一样。
//! 以后讲优化时，也用它验证"优化没有改变程序的行为"。

use crate::ast::{BinOp, UnOp};
use crate::cfg::Cfg;
use crate::ir::{Inst, Operand};

/// 执行 `main`，返回 `print` 打印出的所有数。
pub fn run(funcs: &[Cfg]) -> Result<Vec<i64>, String> {
    let mut m = Machine { funcs, output: Vec::new(), fuel: 1_000_000, depth: 0 };
    let main = funcs.iter().position(|f| f.func.name == "main").ok_or("没有 main 函数")?;
    m.call(main, &[])?;
    Ok(m.output)
}

struct Machine<'a> {
    funcs: &'a [Cfg],
    output: Vec<i64>,
    /// 剩余可执行的指令数（防止死循环）。
    fuel: usize,
    depth: usize,
}

/// 二元运算的语义（常量折叠也用它，保证编译时算的和运行时一样）。
pub fn binop(op: BinOp, a: i64, b: i64) -> Result<i64, String> {
    use BinOp::*;
    Ok(match op {
        Add => a.wrapping_add(b),
        Sub => a.wrapping_sub(b),
        Mul => a.wrapping_mul(b),
        Div | Rem if b == 0 => return Err("除以零".to_owned()),
        Div => a.wrapping_div(b),
        Rem => a.wrapping_rem(b),
        Eq => i64::from(a == b),
        Ne => i64::from(a != b),
        Lt => i64::from(a < b),
        Le => i64::from(a <= b),
        Gt => i64::from(a > b),
        Ge => i64::from(a >= b),
        And => i64::from(a != 0 && b != 0),
        Or => i64::from(a != 0 || b != 0),
    })
}

impl Machine<'_> {
    fn call(&mut self, f: usize, args: &[i64]) -> Result<Option<i64>, String> {
        self.depth += 1;
        if self.depth > 200 {
            return Err("递归太深".to_owned());
        }
        let cfg = &self.funcs[f];
        let mut env: Vec<Option<i64>> = vec![None; cfg.func.vars.len()];
        let read = |env: &[Option<i64>], o: Operand| match o {
            Operand::Const(c) => Ok(c),
            Operand::Var(v) => env[v].ok_or_else(|| format!("读取了没有值的变量 {}", cfg.func.vars[v].name)),
            Operand::Undef => Err("读取了未定义的值".to_owned()),
        };
        let (mut b, mut prev) = (0usize, None::<usize>);
        loop {
            let block = &cfg.blocks[b];
            // 块开头的 φ 同时取值：先全部读出来，再一起写
            let mut phi_vals = Vec::new();
            for inst in &block.insts {
                if let Inst::Phi { dst, args } = inst {
                    let p = prev.ok_or("入口块不能有 φ")?;
                    let (_, o) = args.iter().find(|(pb, _)| *pb == p).ok_or("φ 缺少来自前驱的参数")?;
                    let val = match o {
                        Operand::Undef => None,
                        o => Some(read(&env, *o)?),
                    };
                    phi_vals.push((*dst, val));
                }
            }
            for (d, v) in phi_vals {
                env[d] = v;
            }
            let mut next = None;
            for inst in &block.insts {
                self.fuel = self.fuel.checked_sub(1).ok_or("执行的指令太多（死循环？）")?;
                match inst {
                    Inst::Phi { .. } | Inst::Label(_) => {}
                    Inst::Param { dst, index } => env[*dst] = Some(args[*index]),
                    Inst::Copy { dst, src } => env[*dst] = Some(read(&env, *src)?),
                    Inst::Bin { dst, op, a, b } => env[*dst] = Some(binop(*op, read(&env, *a)?, read(&env, *b)?)?),
                    Inst::Un { dst, op, a } => {
                        let x = read(&env, *a)?;
                        env[*dst] = Some(match op {
                            UnOp::Neg => x.wrapping_neg(),
                            UnOp::Not => i64::from(x == 0),
                        });
                    }
                    Inst::Call { dst, func, args } => {
                        let vals = args.iter().map(|a| read(&env, *a)).collect::<Result<Vec<_>, _>>()?;
                        let ret = if func == "print" {
                            self.output.push(vals[0]);
                            None
                        } else {
                            let g = self.funcs.iter().position(|c| c.func.name == *func).ok_or("调用了不存在的函数")?;
                            self.call(g, &vals)?
                        };
                        if let Some(d) = dst {
                            env[*d] = Some(ret.ok_or("函数没有返回值")?);
                        }
                    }
                    Inst::Jump(_) => next = Some(block.succs[0]),
                    Inst::Branch { cond, .. } => {
                        next = Some(if read(&env, *cond)? != 0 { block.succs[0] } else { block.succs[1] })
                    }
                    Inst::Return(v) => {
                        self.depth -= 1;
                        return v.map(|v| read(&env, v)).transpose();
                    }
                }
            }
            // 没有跳转指令：顺序执行到下一块
            let n = next.or_else(|| block.succs.first().copied()).ok_or("执行到了函数末尾却没有 return")?;
            prev = Some(b);
            b = n;
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{bytecode, cfg, dataflow, interp, ir, lexer, opt, parser, regalloc, rv, sccp, sema, sim, ssa};

    /// 降级、转 SSA 前后，程序的运行结果都必须和预期一致；SSA 里每个变量只赋值一次。
    #[test]
    fn lowering_and_ssa_preserve_behavior() {
        let cases: &[(&str, &[i64])] = &[
            (
                "fn main() { print(sum(10)); }\n\
                 fn sum(n: int) -> int { let mut s = 0; let mut i = 1; while i <= n { s = s + i; i = i + 1; } return s; }",
                &[55],
            ),
            (
                "fn main() { let mut i = 0; while i < 8 { print(fib(i)); i = i + 1; } }\n\
                 fn fib(n: int) -> int { if n < 2 { return n; } return fib(n - 1) + fib(n - 2); }",
                &[0, 1, 1, 2, 3, 5, 8, 13],
            ),
            (
                // 短路求值、遮蔽、else if、return 之后的不可达代码
                "fn main() {\n  let x = 3;\n  { let x = x * 10; print(x); }\n  let x = x + 1;\n  print(x);\n  \
                 let mut k = 0;\n  while k < 5 && !(k == 3 || k == 4) { k = k + 1; }\n  print(k);\n  \
                 print(sign(0 - 7)); print(sign(0)); print(sign(9));\n}\n\
                 fn sign(v: int) -> int {\n  if v > 0 { return 1; } else if v < 0 { return -1; } else { return 0; }\n  \
                 print(999);\n}",
                &[30, 4, 3, -1, 0, 1],
            ),
            (
                // 函数一开头就是循环；循环体里声明的变量
                "fn main() { while false { let y = 1; print(y); } let mut a = 1; let mut b = 1;\n  \
                 while a < 50 { let t = a + b; a = b; b = t; if a % 2 == 0 { print(a); } } }",
                &[2, 8, 34],
            ),
        ];
        for (src, expect) in cases {
            let toks = lexer::lex(src).tokens;
            let p = parser::parse_program(src, &toks);
            assert!(p.errors.is_empty(), "{:?}", p.errors);
            let a = sema::analyze(src, &p.ast, p.root);
            assert!(a.errors.is_empty(), "{:?}", a.errors);
            let low = ir::lower(src, &p.ast, p.root, &a);
            let cfgs: Vec<_> = low.funcs.iter().map(|f| cfg::build(f).compact()).collect();
            assert_eq!(interp::run(&cfgs).as_deref(), Ok(*expect), "三地址码：{src}");
            let ssas: Vec<_> = cfgs.iter().map(ssa::build).collect();
            for s in &ssas {
                let mut defs = vec![0; s.after.func.vars.len()];
                for b in &s.after.blocks {
                    for i in &b.insts {
                        if let Some(d) = i.def() {
                            defs[d] += 1;
                        }
                    }
                }
                assert!(defs.iter().all(|&d| d <= 1), "SSA 里有变量被赋值了不止一次：{src}");
            }
            let after: Vec<_> = ssas.into_iter().map(|s| s.after).collect();
            assert_eq!(interp::run(&after).as_deref(), Ok(*expect), "SSA：{src}");

            // 栈式虚拟机
            let bc = bytecode::run(&bytecode::compile(&p.ast, p.root, &a), 0);
            assert_eq!((bc.output.as_slice(), bc.error), (*expect, None), "字节码：{src}");

            // RISC-V：两种指令选择 × 两种寄存器分配 × 不同的寄存器个数（少的时候大量溢出）
            for smart in [true, false] {
                for k in [1, 2, 3, 11] {
                    for scan in [true, false] {
                        let asm: Vec<_> = cfgs
                            .iter()
                            .map(|c| {
                                let (m, _) = rv::select(c, smart);
                                let alloc = if scan {
                                    regalloc::linear_scan(&m, k).alloc
                                } else {
                                    regalloc::color(&m, k).alloc
                                };
                                regalloc::finalize(&m, &alloc)
                            })
                            .collect();
                        let r = sim::run(&asm, 0);
                        assert_eq!(
                            (r.output.as_slice(), r.error),
                            (*expect, None),
                            "RISC-V（smart={smart} k={k} 线性扫描={scan}）：{src}"
                        );
                    }
                }
            }

            // 优化前后行为不变
            let opt: Vec<_> = cfgs.iter().map(|c| opt::dce(&opt::fold(c).output).output).collect();
            assert_eq!(interp::run(&opt).as_deref(), Ok(*expect), "折叠 + 死代码消除：{src}");
            for conditional in [true, false] {
                let sc: Vec<_> = after.iter().map(|c| sccp::sccp(c, conditional).output).collect();
                assert_eq!(interp::run(&sc).as_deref(), Ok(*expect), "SCCP({conditional})：{src}");
            }
            // 工作表算法和逐轮迭代的结果一致
            for c in &cfgs {
                for p in [dataflow::liveness(c), dataflow::reaching_definitions(c).0] {
                    let w = dataflow::solve(&c.succs(), &p);
                    assert_eq!((w.inn, w.out), dataflow::solve_round_robin(&c.succs(), &p), "{src}");
                }
            }
        }
    }

    /// 经典例子：x 只在一个永远不会执行的分支里被改成 2。
    /// 条件常量传播能证明 x 始终是 1；不追踪可执行边的常量传播做不到。
    #[test]
    fn sccp_sees_through_dead_branches() {
        let src = "fn main() { print(f()); }\n\
                   fn f() -> int { let mut x = 1; let mut i = 0; while i < 10 { if x != 1 { x = 2; } i = i + 1; } return x; }";
        let toks = lexer::lex(src).tokens;
        let p = parser::parse_program(src, &toks);
        let a = sema::analyze(src, &p.ast, p.root);
        let low = ir::lower(src, &p.ast, p.root, &a);
        let f = ssa::build(&cfg::build(&low.funcs[1]).compact()).after;
        let ret = |c: &cfg::Cfg| {
            c.blocks.iter().flat_map(|b| &b.insts).find_map(|i| match i {
                ir::Inst::Return(Some(o)) => Some(*o),
                _ => None,
            })
        };
        assert_eq!(ret(&sccp::sccp(&f, true).output), Some(ir::Operand::Const(1)));
        assert!(matches!(ret(&sccp::sccp(&f, false).output), Some(ir::Operand::Var(_))));
    }
}
