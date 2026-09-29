//! # 计算器语言：麻雀虽小，五脏俱全
//!
//! 这是整个项目里最小的一个**完整**编译器，只认识整数和 `+ - * / ( )`，
//! 但真实编译器的主干它都有：
//!
//! ```text
//!  源码 "1 + 2 * 3"
//!    │ 词法分析 lexer.rs    把一串字符切成一个个"记号"(token)
//!    ▼
//!  [1] [+] [2] [*] [3]
//!    │ 语法分析 parser.rs   按语法规则把记号组织成一棵树
//!    ▼
//!      +
//!     / \
//!    1   *
//!       / \
//!      2   3
//!    │ 代码生成 codegen.rs  把树翻译成一条条机器指令
//!    ▼
//!  push 1; push 2; push 3; mul; add
//!    │ 执行 vm.rs           一台只有"栈"的小机器照着指令算
//!    ▼
//!  7
//! ```
//!
//! 每个阶段除了产出结果，还会把**过程**记录下来（记号序列、语法分析事件、
//! 指令与语法树节点的对应、每一步的栈），界面用这些记录做逐步回放。

pub mod ast;
pub mod codegen;
pub mod lexer;
pub mod parser;
pub mod vm;

pub use bsc_core::{Diagnostic, Span};

use ast::Ast;
use codegen::Instr;
use lexer::Token;
use parser::ParseResult;
use vm::Execution;

/// 编译流水线的各个阶段。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Stage {
    Lex,
    Parse,
    Codegen,
    Run,
}

impl Stage {
    pub fn name(self) -> &'static str {
        match self {
            Stage::Lex => "词法分析",
            Stage::Parse => "语法分析",
            Stage::Codegen => "代码生成",
            Stage::Run => "执行",
        }
    }
}

/// 一次完整编译（加运行）的全部产物。前面的阶段出错时，后面的阶段为空。
#[derive(Clone, Debug)]
pub struct Compilation {
    pub source: String,
    pub tokens: Result<Vec<Token>, Diagnostic>,
    pub parse: Option<ParseResult>,
    pub code: Option<Vec<Instr>>,
    pub exec: Option<Execution>,
}

impl Compilation {
    pub fn new(source: &str) -> Self {
        let tokens = lexer::lex(source);
        let parse = tokens.as_ref().ok().map(|t| parser::parse(t));
        let code = parse.as_ref().and_then(|p| p.root.as_ref().ok().map(|&root| codegen::codegen(&p.ast, root)));
        let exec = match (&parse, &code) {
            (Some(p), Some(code)) => Some(vm::run(code, &p.ast)),
            _ => None,
        };
        Self { source: source.to_owned(), tokens, parse, code, exec }
    }

    pub fn ast(&self) -> Option<&Ast> {
        self.parse.as_ref().map(|p| &p.ast)
    }

    /// 第一个出错的阶段和对应的诊断信息。
    pub fn error(&self) -> Option<(Stage, &Diagnostic)> {
        if let Err(d) = &self.tokens {
            return Some((Stage::Lex, d));
        }
        if let Some(Err(d)) = self.parse.as_ref().map(|p| &p.root) {
            return Some((Stage::Parse, d));
        }
        if let Some(Err(d)) = self.exec.as_ref().map(|e| &e.result) {
            return Some((Stage::Run, d));
        }
        None
    }

    /// 计算结果（任何阶段出错时为 `None`）。
    pub fn value(&self) -> Option<i64> {
        self.exec.as_ref().and_then(|e| e.result.as_ref().ok().copied())
    }
}
