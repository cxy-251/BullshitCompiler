use crate::span::Span;
use crate::types::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgramNode {
    pub statements: Vec<StatementNode>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StatementNode {
    Operation(OperationStmt),
    Incident(IncidentStmt),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationStmt {
    pub agent: Option<AgentNode>,
    pub medium: Option<MediumNode>,
    pub action: ActionNode,
    pub payload: Option<PayloadNode>,
    pub target: Option<EntityNode>,
    pub outcome: Option<OutcomeNode>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncidentStmt {
    pub target: EntityNode,
    pub action: ActionNode,
    pub outcome: Option<OutcomeNode>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentNode {
    pub kind: AgentKind,
    pub raw_text: String,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediumNode {
    pub kind: MediumKind,
    pub raw_text: String,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityNode {
    pub kind: EntityKind,
    pub raw_text: String,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayloadNode {
    pub kind: PayloadKind,
    pub raw_text: String,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionNode {
    pub kind: ActionKind,
    pub raw_text: String,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutcomeNode {
    pub kind: OutcomeKind,
    pub raw_text: String,
    pub span: Span,
}

pub trait ASTVisitor {
    type Output;
    fn visit_program(&mut self, node: &ProgramNode) -> Self::Output;
    fn visit_operation(&mut self, node: &OperationStmt) -> Self::Output;
    fn visit_incident(&mut self, node: &IncidentStmt) -> Self::Output;
}
