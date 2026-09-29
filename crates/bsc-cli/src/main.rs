use clap::{Parser, Subcommand};
use std::fs;
use std::path::Path;

#[derive(Parser)]
#[command(name = "bsc")]
#[command(author = "Bullshit Compiler Contributors")]
#[command(version = "0.2.0")]
#[command(about = "Bullshit Compiler System (BSc) - 双编译器系统：经典语言编译器 + 黑话文本文件编译器", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 编译大白话文本文件为高阶战略汇报黑话 (Compile Plain Text/File to Jargon Document)
    Compile {
        /// 输入白话文本或文件路径 (Source text or file path)
        #[arg(value_name = "INPUT")]
        input: Option<String>,

        /// 显式指定输入文件路径 (File path)
        #[arg(short, long)]
        file: Option<String>,

        /// 导出编译结果到文件 (Output file)
        #[arg(short, long)]
        output: Option<String>,

        /// 编译目标方言 (Supported: huawei, alibaba, state_owned, silicon_valley)
        #[arg(short, long, default_value = "alibaba")]
        target: String,

        /// 打印抽象语法树 (Dump AST)
        #[arg(long)]
        dump_ast: bool,

        /// 打印中间表示 (Dump Intent-IR)
        #[arg(long)]
        dump_ir: bool,
    },

    /// 编译并运行经典编程语言源文件 (Compile & Execute Classic Imperative Language Code)
    Classic {
        /// 源代码文件路径或代码字符串 (e.g. fib.lang, gcd.c)
        input: String,

        /// 在虚拟机中即时执行并输出计算结果 (Execute in VM)
        #[arg(short, long)]
        run: bool,

        /// 打印词法记号 (Dump Tokens)
        #[arg(long)]
        dump_tokens: bool,

        /// 打印抽象语法树 (Dump AST)
        #[arg(long)]
        dump_ast: bool,

        /// 打印 SSA 中间表示 (Dump SSA IR)
        #[arg(long)]
        dump_ir: bool,

        /// 优化级别: 0 (-O0 关闭), 1 (-O1 基础折叠), 2 (-O2 激进优化及DCE)
        #[arg(short = 'O', long = "opt-level", default_value_t = 2)]
        opt_level: u32,

        /// 打印 x86-64 目标汇编 (Dump Assembly)
        #[arg(long)]
        dump_asm: bool,
    },

    /// 反编译大厂黑话或招聘JD，揭露残酷真相 (Decompile Jargon to Brutal Plain Truth)
    Decompile {
        /// 输入黑话文本或文件路径
        input: String,
        /// 输出到文件
        #[arg(short, long)]
        output: Option<String>,
    },

    /// 列出所有受支持的后端目标平台
    Targets,

    /// 启动原生跨平台终端交互工作台 (Launch Native Cross-Platform Terminal TUI)
    Tui,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Compile { input, file, output, target, dump_ast, dump_ir } => {
            let source_text = if let Some(path_str) = file {
                match fs::read_to_string(&path_str) {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("❌ 无法读取输入文件 '{}': {}", path_str, e);
                        std::process::exit(1);
                    }
                }
            } else if let Some(in_str) = input {
                if Path::new(&in_str).is_file() {
                    fs::read_to_string(&in_str).unwrap_or(in_str)
                } else {
                    in_str
                }
            } else {
                eprintln!("❌ 请提供输入文本或通过 --file 指定输入文件路径！");
                std::process::exit(1);
            };

            println!("┌─────────────────────────────────────────────────────────────┐");
            println!("│  Bullshit Compiler (bsc) - 文本文件/多段落编译流水线启动    │");
            println!("└─────────────────────────────────────────────────────────────┘");
            println!("【源文本大小】: {} 字符\n", source_text.len());

            // 1. 前端词法与语法分析
            let (ast, mut ir) = match bsc_frontend::compile_source_to_ir(&source_text, "cli_doc_session") {
                Ok(res) => res,
                Err(err) => {
                    eprintln!("❌ 语法解析错误: {}", err);
                    std::process::exit(1);
                }
            };

            if dump_ast {
                println!("=== [Frontend: 抽象语法树 AST] ===");
                println!("{:#?}\n", ast);
            }

            if dump_ir {
                println!("=== [Middle-end: 原始 SSA Intent-IR] ===");
                println!("{}\n", ir.format_dump());
            }

            // 2. 中端 Pass 优化流水线
            let diffs = bsc_opt::optimize_module(&mut ir);

            // 3. 后端代码发射
            let emitted = match bsc_backend::emit_target(&target, &ir) {
                Ok(output) => output,
                Err(e) => {
                    eprintln!("❌ 后端发射失败: {}", e);
                    std::process::exit(1);
                }
            };

            println!("=== [编译流水线执行日志 (Compiler Pipeline Logs)] ===");
            println!("  [STAGE 1: FRONTEND] 词法与语法分析完成: 切分构建 {} 个语句/事件节点 (AST)", ast.statements.len());
            println!("  [STAGE 2: INTENT-IR] 初始意图三地址码降级完成: 分配 {} 个全局意图符号", ir.values.len());
            for (i, diff) in diffs.iter().enumerate() {
                println!("  [STAGE 3: PASS {}] {} - {} (重写生效: {})", i + 1, diff.pass_name, diff.description, diff.modified);
            }
            println!("  [STAGE 4: BACKEND] 目标方言 [{}] 代码生成发射完成 (产出 {} 字符)\n", target, emitted.len());

            println!("=== [Backend: 代码生成发射 (--target={})] ===", target);
            println!("{}\n", emitted);

            if let Some(out_path) = output {
                if let Err(e) = fs::write(&out_path, &emitted) {
                    eprintln!("❌ 写入输出文件 '{}' 失败: {}", out_path, e);
                } else {
                    println!("✓ 编译结果已成功保存至文件: {}", out_path);
                }
            }
        }

        Commands::Classic { input, run, dump_tokens, dump_ast, dump_ir, opt_level, dump_asm } => {
            let code = if Path::new(&input).is_file() {
                match fs::read_to_string(&input) {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("❌ 无法读取经典语言源码文件 '{}': {}", input, e);
                        std::process::exit(1);
                    }
                }
            } else {
                input
            };

            let opt = match opt_level {
                0 => bsc_classic::OptLevel::O0,
                1 => bsc_classic::OptLevel::O1,
                _ => bsc_classic::OptLevel::O2,
            };

            println!("┌─────────────────────────────────────────────────────────────┐");
            println!("│  Classic Language Compiler - 经典编译原理执行流水线启动      │");
            println!("└─────────────────────────────────────────────────────────────┘");

            let res = bsc_classic::compile_classic_pipeline_with_opt(&code, opt);
            println!("=== [编译流水线执行日志 (Compiler Pipeline Logs)] ===");
            for log_line in &res.logs {
                println!("  {}", log_line);
            }
            println!();

            if !res.success {
                eprintln!("❌ 编译失败:\n{}", res.error.unwrap_or_default());
                std::process::exit(1);
            }

            if dump_tokens {
                println!("=== [1. Lexer: Token Stream ({} tokens)] ===", res.tokens.len());
                for t in &res.tokens {
                    println!("  {:?} ({}) @ {}:{}", t.kind, t.raw, t.span.line, t.span.col);
                }
                println!();
            }

            if dump_ast {
                println!("=== [2. Parser: AST (JSON Dump)] ===");
                println!("{}\n", res.ast_json.as_deref().unwrap_or("// None"));
            }

            println!("=== [3. Semantic: 符号表与类型检查] ===");
            println!("{}\n", res.symbol_table_dump.as_deref().unwrap_or("// None"));

            if dump_ir {
                println!("=== [4. IR: 优化后 SSA 中间表示] ===");
                println!("{}\n", res.optimized_ir.as_deref().unwrap_or("// None"));
            }

            if dump_asm {
                println!("=== [5. Codegen: x86-64 目标汇编代码] ===");
                println!("{}\n", res.assembly_x86.as_deref().unwrap_or("// None"));
            }

            if run {
                println!("=== [6. Execution: 虚拟机运行时输出 (VM Execution)] ===");
                for line in &res.vm_stdout {
                    println!("[STDOUT] {}", line);
                }
                println!("返回值 (Return Value)   : {:?}", res.vm_return_val);
                println!("执行指令数 (Instructions): {}", res.instructions_executed);
                println!("✓ 虚拟机执行完毕。");
            }
        }

        Commands::Decompile { input, output } => {
            let text = if Path::new(&input).is_file() {
                fs::read_to_string(&input).unwrap_or(input)
            } else {
                input
            };

            println!("┌─────────────────────────────────────────────────────────────┐");
            println!("│  Bullshit Decompiler - 照妖镜反编译器启动                    │");
            println!("└─────────────────────────────────────────────────────────────┘");

            let decompiler = bsc_decompiler::Decompiler::new();
            let result = decompiler.decompile(&text);

            println!("=== [识别出的黑话套路] ===");
            for aspect in &result.extracted_key_aspects {
                println!("  • {}", aspect);
            }

            println!("\n=== [脱水后的大白话残酷真相] ===");
            println!("👉 {}\n", result.brutal_truth);

            if let Some(out_path) = output {
                let _ = fs::write(&out_path, format!("【套路】:\n{}\n\n【真相】:\n{}", result.extracted_key_aspects.join("\n"), result.brutal_truth));
                println!("✓ 反编译脱水结果已保存至: {}", out_path);
            }
        }

        Commands::Targets => {
            println!("Bullshit Compiler 支持的目标架构 (Targets):");
            println!("  • alibaba        : 阿里中台 / P8 敏捷 / 抓手赋能闭环风 (默认)");
            println!("  • huawei         : 华为鸿蒙 / 分布式微内核 / 自主可控护城河风");
            println!("  • state_owned    : 政务信创 / 国资红头文件 / 高位推进公文风");
            println!("  • silicon_valley : 硅谷英文科技布道 / Zero-Trust Fabric / AIoT Paradigm");
        }

        Commands::Tui => {
            let exe_dir = std::env::current_exe().ok().and_then(|p| p.parent().map(|p| p.to_path_buf()));
            let tui_path = exe_dir.map(|d| d.join("bsc-tui")).unwrap_or_else(|| "bsc-tui".into());
            let status = std::process::Command::new(tui_path).status();
            if let Err(e) = status {
                eprintln!("❌ 无法启动 bsc-tui 终端工作台: {}. 请确认 target/release/bsc-tui 存在或单独运行 bsc-tui", e);
            }
        }
    }
}
