use crate::lexer::{Token, TokenKind};
use bsc_core::*;

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    pub fn parse(&mut self) -> Result<ProgramNode, CompileError> {
        let mut statements = Vec::new();
        let start_span = self.tokens.first().map(|t| t.span).unwrap_or_else(Span::dummy);

        while self.pos < self.tokens.len() {
            // Skip pure punctuation at statement boundaries
            if let Some(Token { kind: TokenKind::Punctuation(_), .. }) = self.peek() {
                self.pos += 1;
                continue;
            }

            if let Some(stmt) = self.parse_statement()? {
                statements.push(stmt);
            } else {
                self.pos += 1;
            }
        }

        let end_span = self.tokens.last().map(|t| t.span).unwrap_or(start_span);
        Ok(ProgramNode {
            statements,
            span: start_span.merge(&end_span),
        })
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn parse_statement(&mut self) -> Result<Option<StatementNode>, CompileError> {
        let mut agent: Option<AgentNode> = None;
        let mut medium: Option<MediumNode> = None;
        let mut action: Option<ActionNode> = None;
        let mut payload: Option<PayloadNode> = None;
        let mut target: Option<EntityNode> = None;
        let mut outcome: Option<OutcomeNode> = None;

        let start_span = self.peek().map(|t| t.span).unwrap_or_else(Span::dummy);

        while let Some(token) = self.peek() {
            match &token.kind {
                TokenKind::Punctuation(_) => {
                    self.pos += 1;
                    break; // End of clause/statement
                }
                TokenKind::Agent(kind) => {
                    agent = Some(AgentNode {
                        kind: kind.clone(),
                        raw_text: token.raw.clone(),
                        span: token.span,
                    });
                    self.pos += 1;
                }
                TokenKind::Prep(_) => {
                    // Preposition e.g. "用", skip prep and look for Medium
                    self.pos += 1;
                    if let Some(next) = self.peek() {
                        if let TokenKind::Medium(m) = &next.kind {
                            medium = Some(MediumNode {
                                kind: m.clone(),
                                raw_text: next.raw.clone(),
                                span: next.span,
                            });
                            self.pos += 1;
                        }
                    }
                }
                TokenKind::Medium(kind) => {
                    medium = Some(MediumNode {
                        kind: kind.clone(),
                        raw_text: token.raw.clone(),
                        span: token.span,
                    });
                    self.pos += 1;
                }
                TokenKind::Payload(kind) => {
                    payload = Some(PayloadNode {
                        kind: kind.clone(),
                        raw_text: token.raw.clone(),
                        span: token.span,
                    });
                    self.pos += 1;
                }
                TokenKind::Action(kind) => {
                    action = Some(ActionNode {
                        kind: kind.clone(),
                        raw_text: token.raw.clone(),
                        span: token.span,
                    });
                    self.pos += 1;
                }
                TokenKind::Entity(kind) => {
                    target = Some(EntityNode {
                        kind: kind.clone(),
                        raw_text: token.raw.clone(),
                        span: token.span,
                    });
                    self.pos += 1;
                }
                TokenKind::Outcome(kind) => {
                    outcome = Some(OutcomeNode {
                        kind: kind.clone(),
                        raw_text: token.raw.clone(),
                        span: token.span,
                    });
                    self.pos += 1;
                }
                TokenKind::Text(word) => {
                    if payload.is_none() && target.is_none() && word.chars().count() >= 2 && !word.contains("的") && !word.contains("在") {
                        payload = Some(PayloadNode {
                            kind: PayloadKind::Custom(word.clone()),
                            raw_text: word.clone(),
                            span: token.span,
                        });
                    }
                    self.pos += 1;
                }
            }
        }

        let end_span = self.tokens.get(self.pos.saturating_sub(1)).map(|t| t.span).unwrap_or(start_span);

        if let Some(act) = action {
            Ok(Some(StatementNode::Operation(OperationStmt {
                agent,
                medium,
                action: act,
                payload,
                target,
                outcome,
                span: start_span.merge(&end_span),
            })))
        } else if let Some(ent) = target {
            // Incident without active verb (e.g. "开发板 挂了")
            let fallback_action = ActionNode {
                kind: ActionKind::Crash,
                raw_text: "异常".into(),
                span: start_span,
            };
            Ok(Some(StatementNode::Incident(IncidentStmt {
                target: ent,
                action: fallback_action,
                outcome,
                span: start_span.merge(&end_span),
            })))
        } else if let Some(p) = payload {
            let fallback_action = ActionNode {
                kind: ActionKind::Process,
                raw_text: "处理".into(),
                span: start_span,
            };
            Ok(Some(StatementNode::Operation(OperationStmt {
                agent,
                medium,
                action: fallback_action,
                payload: Some(p),
                target,
                outcome,
                span: start_span.merge(&end_span),
            })))
        } else {
            Ok(None)
        }
    }
}
