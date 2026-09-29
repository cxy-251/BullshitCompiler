use bsc_core::*;

pub struct IRBuilder {
    module: IntentModule,
}

impl IRBuilder {
    pub fn new(module_name: impl Into<String>) -> Self {
        Self {
            module: IntentModule::new(module_name),
        }
    }

    pub fn build(mut self, program: &ProgramNode) -> IntentModule {
        for stmt in &program.statements {
            match stmt {
                StatementNode::Operation(op) => self.lower_operation(op),
                StatementNode::Incident(inc) => self.lower_incident(inc),
            }
        }
        self.module
    }

    fn lower_operation(&mut self, op: &OperationStmt) {
        // 1. Allocate symbols
        let medium_id = op.medium.as_ref().map(|m| {
            self.module.alloc_value(
                "medium",
                IRValueKind::Medium(m.kind.clone()),
                m.span,
            )
        }).unwrap_or_else(|| {
            self.module.alloc_value(
                "default_channel",
                IRValueKind::Medium(MediumKind::WiFi),
                op.span,
            )
        });

        let target_id = op.target.as_ref().map(|t| {
            self.module.alloc_value(
                "target_node",
                IRValueKind::Entity(t.kind.clone()),
                t.span,
            )
        }).unwrap_or_else(|| {
            self.module.alloc_value(
                "default_node",
                IRValueKind::Entity(EntityKind::EmbeddedBoard),
                op.span,
            )
        });

        let payload_id = op.payload.as_ref().map(|p| {
            self.module.alloc_value(
                "payload",
                IRValueKind::Payload(p.kind.clone()),
                p.span,
            )
        });

        // 2. Opcode Emission
        match &op.action.kind {
            ActionKind::Connect => {
                let res = self.module.alloc_value("session", IRValueKind::AbstractSynthesized("SessionHandle".into()), op.span);
                self.module.push_instruction(IROpcode::EstablishSession {
                    result: res,
                    medium: medium_id,
                    target: target_id,
                    span: op.span,
                });
            }
            ActionKind::Transfer => {
                let pid = payload_id.unwrap_or_else(|| {
                    self.module.alloc_value(
                        "default_blob",
                        IRValueKind::Payload(PayloadKind::Document),
                        op.span,
                    )
                });

                let sess_id = self.module.alloc_value("virt_session", IRValueKind::AbstractSynthesized("ActiveChannel".into()), op.span);
                let tx_res = self.module.alloc_value("tx_ack", IRValueKind::AbstractSynthesized("DeliveryReceipt".into()), op.span);

                self.module.push_instruction(IROpcode::TransferPayload {
                    result: tx_res,
                    payload: pid,
                    target: target_id,
                    session: sess_id,
                    span: op.span,
                });
            }
            ActionKind::Restart | ActionKind::Process | ActionKind::Custom(_) => {
                let act_id = self.module.alloc_value("action_sym", IRValueKind::Action(op.action.kind.clone()), op.action.span);
                let mut_res = self.module.alloc_value("mutate_res", IRValueKind::AbstractSynthesized("StateChangeResult".into()), op.span);

                if let Some(pid) = payload_id {
                    let sess_id = self.module.alloc_value("domain_channel", IRValueKind::AbstractSynthesized("DomainChannel".into()), op.span);
                    self.module.push_instruction(IROpcode::TransferPayload {
                        result: mut_res,
                        payload: pid,
                        target: target_id,
                        session: sess_id,
                        span: op.span,
                    });
                } else {
                    self.module.push_instruction(IROpcode::MutateState {
                        result: mut_res,
                        target: target_id,
                        action: act_id,
                        span: op.span,
                    });
                }
            }
            ActionKind::Verify => {
                let exp_id = self.module.alloc_value("expected_health", IRValueKind::Outcome(OutcomeKind::SuccessNormal), op.span);
                let assert_res = self.module.alloc_value("audit_report", IRValueKind::AbstractSynthesized("VerificationReport".into()), op.span);

                self.module.push_instruction(IROpcode::AssertIntegrity {
                    result: assert_res,
                    target: target_id,
                    expected: exp_id,
                    span: op.span,
                });
            }
            _ => {}
        }

        // If there is an outcome assertion (e.g. "看了一下没问题")
        if let Some(out) = &op.outcome {
            let exp_id = self.module.alloc_value("outcome_assertion", IRValueKind::Outcome(out.kind.clone()), out.span);
            let assert_res = self.module.alloc_value("integrity_report", IRValueKind::AbstractSynthesized("SystemIntegrity".into()), out.span);
            self.module.push_instruction(IROpcode::AssertIntegrity {
                result: assert_res,
                target: target_id,
                expected: exp_id,
                span: out.span,
            });
        }
    }

    fn lower_incident(&mut self, inc: &IncidentStmt) {
        let target_id = self.module.alloc_value("target_entity", IRValueKind::Entity(inc.target.kind.clone()), inc.target.span);
        let act_id = self.module.alloc_value("incident_type", IRValueKind::Action(inc.action.kind.clone()), inc.action.span);
        let res = self.module.alloc_value("incident_record", IRValueKind::AbstractSynthesized("AnomalyIncident".into()), inc.span);

        self.module.push_instruction(IROpcode::MutateState {
            result: res,
            target: target_id,
            action: act_id,
            span: inc.span,
        });
    }
}
