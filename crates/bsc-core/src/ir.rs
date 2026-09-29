use crate::span::Span;
use crate::types::*;
use serde::{Deserialize, Serialize};

pub type ValueId = usize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IRValueKind {
    Agent(AgentKind),
    Medium(MediumKind),
    Entity(EntityKind),
    Payload(PayloadKind),
    Action(ActionKind),
    Outcome(OutcomeKind),
    AbstractSynthesized(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IRValue {
    pub id: ValueId,
    pub name: String,
    pub kind: IRValueKind,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum IROpcode {
    /// %res = EstablishSession(%medium, %target)
    EstablishSession {
        result: ValueId,
        medium: ValueId,
        target: ValueId,
        span: Span,
    },
    /// %res = TransferPayload(%payload, %target, via: %session)
    TransferPayload {
        result: ValueId,
        payload: ValueId,
        target: ValueId,
        session: ValueId,
        span: Span,
    },
    /// %res = MutateState(%target, %action)
    MutateState {
        result: ValueId,
        target: ValueId,
        action: ValueId,
        span: Span,
    },
    /// %res = AssertIntegrity(%target, %outcome)
    AssertIntegrity {
        result: ValueId,
        target: ValueId,
        expected: ValueId,
        span: Span,
    },
    /// %res = SynthesizeStrategicLoop(domain, cadence)
    SynthesizeStrategicLoop {
        result: ValueId,
        strategic_domain: String,
        cadence: String,
        span: Span,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BasicBlock {
    pub label: String,
    pub instructions: Vec<IROpcode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentModule {
    pub name: String,
    pub values: Vec<IRValue>,
    pub blocks: Vec<BasicBlock>,
}

impl IntentModule {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            values: Vec::new(),
            blocks: vec![BasicBlock {
                label: "entry".to_string(),
                instructions: Vec::new(),
            }],
        }
    }

    pub fn alloc_value(&mut self, name: impl Into<String>, kind: IRValueKind, span: Span) -> ValueId {
        let id = self.values.len();
        self.values.push(IRValue {
            id,
            name: name.into(),
            kind,
            span,
        });
        id
    }

    pub fn push_instruction(&mut self, inst: IROpcode) {
        if let Some(entry) = self.blocks.first_mut() {
            entry.instructions.push(inst);
        }
    }

    pub fn format_dump(&self) -> String {
        let mut out = format!("; Intent-IR Module: {}\n", self.name);
        out.push_str("; Declarations:\n");
        for val in &self.values {
            out.push_str(&format!("  %{} = {:?}\n", val.name, val.kind));
        }
        out.push_str("\n");
        for block in &self.blocks {
            out.push_str(&format!("{}:\n", block.label));
            for inst in &block.instructions {
                match inst {
                    IROpcode::EstablishSession { result, medium, target, .. } => {
                        out.push_str(&format!("    %{} = EstablishSession(%{}, %{})\n", result, medium, target));
                    }
                    IROpcode::TransferPayload { result, payload, target, session, .. } => {
                        out.push_str(&format!("    %{} = TransferPayload(%{}, dest: %{}, via: %{})\n", result, payload, target, session));
                    }
                    IROpcode::MutateState { result, target, action, .. } => {
                        out.push_str(&format!("    %{} = MutateState(%{}, action: %{})\n", result, target, action));
                    }
                    IROpcode::AssertIntegrity { result, target, expected, .. } => {
                        out.push_str(&format!("    %{} = AssertIntegrity(%{}, expect: %{})\n", result, target, expected));
                    }
                    IROpcode::SynthesizeStrategicLoop { result, strategic_domain, cadence, .. } => {
                        out.push_str(&format!("    %{} = SynthesizeStrategicLoop(domain: \"{}\", cadence: \"{}\")\n", result, strategic_domain, cadence));
                    }
                }
            }
        }
        out
    }
}
