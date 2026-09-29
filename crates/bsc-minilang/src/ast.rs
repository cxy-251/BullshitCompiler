//! mini-lang 的语法树。
//!
//! 所有节点放在一个数组里、用编号互相引用；每个节点的孩子按固定顺序存放
//! （见各个 `NodeKind` 的注释）。统一的结构让界面画树、后面的语义分析遍历树都更简单。

use bsc_core::Span;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(pub u32);

impl NodeId {
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

impl BinOp {
    pub fn symbol(self) -> &'static str {
        match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Rem => "%",
            BinOp::Eq => "==",
            BinOp::Ne => "!=",
            BinOp::Lt => "<",
            BinOp::Le => "<=",
            BinOp::Gt => ">",
            BinOp::Ge => ">=",
            BinOp::And => "&&",
            BinOp::Or => "||",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UnOp {
    Neg,
    Not,
}

impl UnOp {
    pub fn symbol(self) -> &'static str {
        match self {
            UnOp::Neg => "-",
            UnOp::Not => "!",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NodeKind {
    /// 整个程序。孩子：若干函数。
    Program,
    /// 函数定义。孩子：若干参数、[返回类型]、函数体。`has_ret` 表示有没有返回类型。
    Function {
        name: String,
        has_ret: bool,
    },
    /// 参数。孩子：类型。
    Param {
        name: String,
    },
    /// 类型名，比如 `int`。
    Type {
        name: String,
    },
    /// 代码块 `{ … }`。孩子：若干语句。
    Block,
    /// `let [mut] x [: 类型] = 初值;`。孩子：[类型]、初值。
    Let {
        name: String,
        mutable: bool,
        has_type: bool,
    },
    /// `if 条件 块 [else 块或 if]`。孩子：条件、then 块、[else 部分]。
    If,
    /// `while 条件 块`。孩子：条件、循环体。
    While,
    /// `return [值];`。孩子：[值]。
    Return,
    /// `表达式;`。孩子：表达式。
    ExprStmt,
    /// 二元运算。孩子：左、右。
    Binary(BinOp),
    /// 一元运算。孩子：操作数。
    Unary(UnOp),
    /// 赋值 `x = 值`。孩子：值。
    Assign {
        name: String,
    },
    /// 函数调用。孩子：若干实参。
    Call {
        name: String,
    },
    Int(i64),
    Bool(bool),
    Var(String),
    /// 语法错误处留下的占位节点（错误恢复用）。
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Node {
    pub kind: NodeKind,
    pub span: Span,
    pub children: Vec<NodeId>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Ast {
    pub nodes: Vec<Node>,
}

impl Ast {
    pub fn push(&mut self, kind: NodeKind, span: Span, children: Vec<NodeId>) -> NodeId {
        let id = NodeId(self.nodes.len() as u32);
        self.nodes.push(Node { kind, span, children });
        id
    }

    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id.index()]
    }

    /// 节点在树图里显示的文字。
    pub fn label(&self, id: NodeId) -> String {
        match &self.node(id).kind {
            NodeKind::Program => "程序".to_owned(),
            NodeKind::Function { name, .. } => format!("fn {name}"),
            NodeKind::Param { name } => format!("参数 {name}"),
            NodeKind::Type { name } => name.clone(),
            NodeKind::Block => "{ }".to_owned(),
            NodeKind::Let { name, mutable, .. } => format!("let {}{name}", if *mutable { "mut " } else { "" }),
            NodeKind::If => "if".to_owned(),
            NodeKind::While => "while".to_owned(),
            NodeKind::Return => "return".to_owned(),
            NodeKind::ExprStmt => "表达式语句".to_owned(),
            NodeKind::Binary(op) => op.symbol().to_owned(),
            NodeKind::Unary(op) => op.symbol().to_owned(),
            NodeKind::Assign { name } => format!("{name} ="),
            NodeKind::Call { name } => format!("{name}(…)"),
            NodeKind::Int(n) => n.to_string(),
            NodeKind::Bool(b) => b.to_string(),
            NodeKind::Var(v) => v.clone(),
            NodeKind::Error => "错误".to_owned(),
        }
    }

    /// 用 S 表达式写出一棵子树（测试和调试用），比如 `(+ 1 (* 2 3))`。
    pub fn sexpr(&self, id: NodeId) -> String {
        let n = self.node(id);
        let head = match &n.kind {
            NodeKind::Binary(op) => op.symbol().to_owned(),
            NodeKind::Unary(op) => format!("{}1", op.symbol()),
            NodeKind::Int(_) | NodeKind::Bool(_) | NodeKind::Var(_) | NodeKind::Type { .. } => return self.label(id),
            _ => self.label(id),
        };
        if n.children.is_empty() {
            return format!("({head})");
        }
        let kids: Vec<String> = n.children.iter().map(|&c| self.sexpr(c)).collect();
        format!("({head} {})", kids.join(" "))
    }
}
