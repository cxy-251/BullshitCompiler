// crates/bsc-classic/src/lib.rs
pub mod token;
pub mod lexer;
pub mod ast;
pub mod parser;
pub mod semantic;
pub mod ir;
pub mod optimizer;
pub mod codegen_x86;
pub mod vm;
pub mod diagnostic;

pub use optimizer::OptLevel;
pub use diagnostic::DiagnosticRenderer;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassicCompileResponse {
    pub success: bool,
    pub error: Option<String>,
    pub tokens: Vec<token::Token>,
    pub ast_json: Option<String>,
    pub symbol_table_dump: Option<String>,
    pub raw_ir: Option<String>,
    pub optimized_ir: Option<String>,
    pub assembly_x86: Option<String>,
    pub vm_stdout: Vec<String>,
    pub vm_return_val: Option<i64>,
    pub instructions_executed: usize,
    pub logs: Vec<String>,
}

pub fn compile_classic_pipeline(source: &str) -> ClassicCompileResponse {
    compile_classic_pipeline_with_opt(source, OptLevel::O2)
}

pub fn compile_classic_pipeline_with_opt(source: &str, opt_level: OptLevel) -> ClassicCompileResponse {
    let mut logs = Vec::new();
    logs.push(format!("[STAGE 0: INIT] 启动经典语言编译流水线, 源码长度: {} 字符, 优化等级: {:?}", source.len(), opt_level));

    // 1. Lexer
    let mut lexer = lexer::Lexer::new(source);
    let tokens = match lexer.tokenize() {
        Ok(t) => {
            logs.push(format!("[STAGE 1: LEXER] 词法扫描完成: 识别出 {} 个记号单元 (Tokens), 过滤注释与空白字符", t.len()));
            t
        }
        Err(e) => {
            logs.push(format!("[STAGE 1: LEXER ERROR] 词法分析异常: {}", e));
            let diag = DiagnosticRenderer::render(source, &e, "Lexer");
            return ClassicCompileResponse {
                success: false,
                error: Some(diag),
                tokens: vec![],
                ast_json: None,
                symbol_table_dump: None,
                raw_ir: None,
                optimized_ir: None,
                assembly_x86: None,
                vm_stdout: vec![],
                vm_return_val: None,
                instructions_executed: 0,
                logs,
            };
        }
    };

    // 2. Parser
    let mut parser = parser::Parser::new(tokens.clone());
    let prog = match parser.parse_program() {
        Ok(p) => {
            logs.push(format!("[STAGE 2: PARSER] 递归下降语法分析完成: 识别 {} 个顶级函数定义, 构建 AST 抽象语法树", p.functions.len()));
            p
        }
        Err(e) => {
            logs.push(format!("[STAGE 2: PARSER ERROR] 语法解析异常: {}", e));
            let diag = DiagnosticRenderer::render(source, &e, "Parser");
            return ClassicCompileResponse {
                success: false,
                error: Some(diag),
                tokens,
                ast_json: None,
                symbol_table_dump: None,
                raw_ir: None,
                optimized_ir: None,
                assembly_x86: None,
                vm_stdout: vec![],
                vm_return_val: None,
                instructions_executed: 0,
                logs,
            };
        }
    };
    let ast_json = serde_json::to_string_pretty(&prog).ok();

    // 3. Semantic Analysis & Type Checking
    let mut checker = semantic::TypeChecker::new();
    if let Err(e) = checker.check_program(&prog) {
        logs.push(format!("[STAGE 3: SEMANTIC ERROR] 语义检查/类型不相容:\n{}", e));
        let diag = DiagnosticRenderer::render(source, &e, "Semantic");
        return ClassicCompileResponse {
            success: false,
            error: Some(diag),
            tokens,
            ast_json,
            symbol_table_dump: Some(checker.dump_symbol_table()),
            raw_ir: None,
            optimized_ir: None,
            assembly_x86: None,
            vm_stdout: vec![],
            vm_return_val: None,
            instructions_executed: 0,
            logs,
        };
    }
    logs.push("[STAGE 3: SEMANTIC] 作用域树与符号表校验通过: 0 个类型错误, 函数签名及形参相容".to_string());
    let symbol_table_dump = Some(checker.dump_symbol_table());

    // 4. Intermediate Representation (SSA IR)
    let mut ir_builder = ir::IRBuilder::new();
    let raw_ir_module = ir_builder.build_program(&prog);
    let mut total_insts = 0;
    for f in &raw_ir_module.functions {
        for b in &f.blocks {
            total_insts += b.instructions.len();
        }
    }
    logs.push(format!("[STAGE 4: IR-GEN] SSA 静态单赋值中间代码生成: {} 个函数, 累计生成 {} 条 3-地址码指令", raw_ir_module.functions.len(), total_insts));
    let raw_ir_dump = Some(raw_ir_module.dump());

    // 5. Optimization Passes
    let mut opt = optimizer::Optimizer::new();
    let mut opt_module = raw_ir_module.clone();
    opt.optimize_module_with_level(&mut opt_module, opt_level);
    logs.push(format!("[STAGE 5: OPTIMIZER] 机器无关优化流水线完成 (级别: {:?}): 常量折叠与代数化简消除 {} 处冗余", opt_level, opt.instructions_eliminated));
    let opt_ir_dump = Some(opt_module.dump());

    // 6. x86-64 Machine Code Generation
    let codegen = codegen_x86::X86Codegen::new();
    let asm = Some(codegen.generate(&opt_module));
    logs.push("[STAGE 6: CODEGEN] 目标代码生成完成: 发射 Intel 语法 x86-64 汇编 (System V AMD64 ABI)".to_string());

    // 7. Virtual Machine Execution
    logs.push("[STAGE 7: VM-EXEC] 启动内部 SSA 控制流解释虚拟机执行入口 @main...".to_string());
    let mut vm = vm::VirtualMachine::new(&opt_module);
    let vm_report = vm.run_main();

    if vm_report.success {
        logs.push(format!("[STAGE 7: VM-EXEC] 虚拟机正常执行完毕: 累计执行 {} 条指令, 返回值: {:?}", vm_report.instructions_executed, vm_report.return_value));
    } else {
        logs.push(format!("[STAGE 7: VM-EXEC ERROR] 虚拟机运行时异常: {}", vm_report.error.as_deref().unwrap_or("Unknown")));
    }

    let final_error = vm_report.error.as_ref().map(|e| DiagnosticRenderer::render(source, e, "VM"));

    ClassicCompileResponse {
        success: vm_report.success,
        error: final_error,
        tokens,
        ast_json,
        symbol_table_dump,
        raw_ir: raw_ir_dump,
        optimized_ir: opt_ir_dump,
        assembly_x86: asm,
        vm_stdout: vm_report.stdout_lines,
        vm_return_val: vm_report.return_value,
        instructions_executed: vm_report.instructions_executed,
        logs,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fib_pipeline() {
        let code = r#"
        fn fib(n: int) -> int {
            if n <= 1 {
                return n;
            }
            return fib(n - 1) + fib(n - 2);
        }
        fn main() -> int {
            let r: int = fib(10);
            print(r);
            return r;
        }
        "#;
        let res = compile_classic_pipeline(code);
        assert!(res.success);
        assert_eq!(res.vm_return_val, Some(55));
        assert_eq!(res.vm_stdout, vec!["55"]);
    }

    #[test]
    fn test_diagnostic_formatting() {
        let bad_code = "fn main() -> i64 { return 0; }";
        let res = compile_classic_pipeline(bad_code);
        assert!(!res.success);
        let err = res.error.unwrap();
        assert!(err.contains("error[E0201]"));
        assert!(err.contains("--> input.lang:1:14"));
        assert!(err.contains("^"));
        assert!(err.contains("= help:"));
    }

    #[test]
    fn test_opt_levels() {
        let code = r#"
        fn main() -> int {
            let a: int = 10 + 20;
            let b: int = a * 0;
            return b;
        }
        "#;
        let res_o0 = compile_classic_pipeline_with_opt(code, OptLevel::O0);
        assert!(res_o0.success);
        assert_eq!(res_o0.vm_return_val, Some(0));

        let res_o2 = compile_classic_pipeline_with_opt(code, OptLevel::O2);
        assert!(res_o2.success);
        assert_eq!(res_o2.vm_return_val, Some(0));
    }
}
