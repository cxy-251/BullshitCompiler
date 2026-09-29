//! 抽象语法树 (AST)。
//!
//! 节点统一存放在一个数组里，用下标 [`NodeId`] 互相引用（"arena" 写法）。
//! 这比 `Box` 连成的树更方便：界面可以用编号高亮某个节点，
//! 指令也可以用编号记住自己是从哪个节点翻译来的。

use bsc_core::Span;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(pub u32);

impl NodeId {
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
}

impl BinOp {
    pub fn symbol(self) -> &'static str {
        match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            BinOp::Add => "加",
            BinOp::Sub => "减",
            BinOp::Mul => "乘",
            BinOp::Div => "除",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeKind {
    /// 整数，比如 `42`。
    Num(i64),
    /// 取负，比如 `-x`。
    Neg(NodeId),
    /// 二元运算，比如 `a + b`。
    Binary(BinOp, NodeId, NodeId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Node {
    pub kind: NodeKind,
    /// 这个节点覆盖的源码范围（`1 + 2` 的加法节点覆盖整个 `1 + 2`）。
    pub span: Span,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Ast {
    pub nodes: Vec<Node>,
}

impl Ast {
    pub fn push(&mut self, kind: NodeKind, span: Span) -> NodeId {
        let id = NodeId(u32::try_from(self.nodes.len()).expect("语法树节点过多"));
        self.nodes.push(Node { kind, span });
        id
    }

    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id.index()]
    }

    pub fn children(&self, id: NodeId) -> Vec<NodeId> {
        match self.node(id).kind {
            NodeKind::Num(_) => vec![],
            NodeKind::Neg(x) => vec![x],
            NodeKind::Binary(_, l, r) => vec![l, r],
        }
    }

    /// 节点在树图里显示的文字。
    pub fn label(&self, id: NodeId) -> String {
        match self.node(id).kind {
            NodeKind::Num(n) => n.to_string(),
            NodeKind::Neg(_) => "负".to_owned(),
            NodeKind::Binary(op, _, _) => op.symbol().to_owned(),
        }
    }

    /// 用 S 表达式写出整棵树，例如 `(+ 1 (* 2 3))`。测试和命令行输出用。
    pub fn sexpr(&self, id: NodeId) -> String {
        match self.node(id).kind {
            NodeKind::Num(n) => n.to_string(),
            NodeKind::Neg(x) => format!("(neg {})", self.sexpr(x)),
            NodeKind::Binary(op, l, r) => format!("({} {} {})", op.symbol(), self.sexpr(l), self.sexpr(r)),
        }
    }

    /// 直接在树上求值（"树遍历解释器"）。
    ///
    /// 编译器走的是"生成指令 → 虚拟机执行"这条路；这个函数是另一条更直接的路，
    /// 测试里用它来核对编译结果是否正确。溢出或除以零时返回 `None`。
    pub fn eval(&self, id: NodeId) -> Option<i64> {
        match self.node(id).kind {
            NodeKind::Num(n) => Some(n),
            NodeKind::Neg(x) => self.eval(x)?.checked_neg(),
            NodeKind::Binary(op, l, r) => {
                let (a, b) = (self.eval(l)?, self.eval(r)?);
                match op {
                    BinOp::Add => a.checked_add(b),
                    BinOp::Sub => a.checked_sub(b),
                    BinOp::Mul => a.checked_mul(b),
                    BinOp::Div => a.checked_div(b),
                }
            }
        }
    }
}
