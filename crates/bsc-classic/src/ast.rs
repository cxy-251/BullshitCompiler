use crate::token::Span;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Type {
    Int,
    Bool,
    Void,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnOp {
    Neg,
    Not,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Expr {
    Literal(i64, Span),
    BoolLit(bool, Span),
    Variable(String, Span),
    Binary(BinOp, Box<Expr>, Box<Expr>, Span),
    Unary(UnOp, Box<Expr>, Span),
    Call(String, Vec<Expr>, Span),
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::Literal(_, s) => *s,
            Expr::BoolLit(_, s) => *s,
            Expr::Variable(_, s) => *s,
            Expr::Binary(_, _, _, s) => *s,
            Expr::Unary(_, _, s) => *s,
            Expr::Call(_, _, s) => *s,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Stmt {
    Let(String, Type, Expr, Span),
    Assign(String, Expr, Span),
    If(Expr, Vec<Stmt>, Option<Vec<Stmt>>, Span),
    While(Expr, Vec<Stmt>, Span),
    Return(Option<Expr>, Span),
    Print(Expr, Span),
    Expr(Expr, Span),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Function {
    pub name: String,
    pub params: Vec<(String, Type)>,
    pub ret_type: Type,
    pub body: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Program {
    pub functions: Vec<Function>,
}
