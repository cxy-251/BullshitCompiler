pub mod ir_builder;
pub mod lexer;
pub mod parser;

pub use ir_builder::IRBuilder;
pub use lexer::{Lexer, Token, TokenKind};
pub use parser::Parser;

use bsc_core::{CompileError, IntentModule, ProgramNode};

/// One-stop frontend compilation from natural language to AST and unoptimized Intent-IR
pub fn compile_source_to_ir(source: &str, module_name: &str) -> Result<(ProgramNode, IntentModule), CompileError> {
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize();
    let mut parser = Parser::new(tokens);
    let ast = parser.parse()?;
    let ir = IRBuilder::new(module_name).build(&ast);
    Ok((ast, ir))
}
