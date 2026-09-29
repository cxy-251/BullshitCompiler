use crate::pass::TransformPass;
use bsc_core::{AgentKind, EntityKind, IROpcode, IRValueKind, IntentModule, MediumKind, PayloadKind, Span};

/// Pass 1: 去人性化与客观系统自驱化
pub struct StripAgencyPass;

impl TransformPass for StripAgencyPass {
    fn name(&self) -> &'static str {
        "StripAgencyPass"
    }

    fn description(&self) -> &'static str {
        "消除第一人称与个体施动者，将交互提升为全自动系统内核拓扑自驱"
    }

    fn run(&mut self, module: &mut IntentModule) -> bool {
        let mut modified = false;
        for val in &mut module.values {
            if let IRValueKind::Agent(AgentKind::FirstPersonSingular) | IRValueKind::Agent(AgentKind::FirstPersonPlural) = &val.kind {
                val.kind = IRValueKind::Agent(AgentKind::ImplicitSystem);
                val.name = format!("{}_autonomous", val.name);
                modified = true;
            }
        }
        modified
    }
}

/// Pass 2: 概念升维与时空全场景膨胀
pub struct ScaleAmplifyPass;

impl TransformPass for ScaleAmplifyPass {
    fn name(&self) -> &'static str {
        "ScaleAmplifyPass"
    }

    fn description(&self) -> &'static str {
        "将单点物理介质/载荷/设备升维为全场景分布式生态底座资产"
    }

    fn run(&mut self, module: &mut IntentModule) -> bool {
        let mut modified = false;
        for val in &mut module.values {
            match &val.kind {
                IRValueKind::Medium(MediumKind::Bluetooth) => {
                    val.kind = IRValueKind::AbstractSynthesized("分布式近场异构软总线通信底座".into());
                    modified = true;
                }
                IRValueKind::Medium(MediumKind::WiFi) => {
                    val.kind = IRValueKind::AbstractSynthesized("全场景高吞吐无线互联拓扑网络".into());
                    modified = true;
                }
                IRValueKind::Medium(MediumKind::SerialPort) => {
                    val.kind = IRValueKind::AbstractSynthesized("高可靠物理层硬件串行通信总线".into());
                    modified = true;
                }
                IRValueKind::Payload(PayloadKind::Image) => {
                    val.kind = IRValueKind::AbstractSynthesized("高维非结构化多媒体数据资产".into());
                    modified = true;
                }
                IRValueKind::Payload(PayloadKind::Document) => {
                    val.kind = IRValueKind::AbstractSynthesized("结构化知识要素载荷".into());
                    modified = true;
                }
                IRValueKind::Entity(EntityKind::EmbeddedBoard) => {
                    val.kind = IRValueKind::AbstractSynthesized("端侧异构边缘计算节点".into());
                    modified = true;
                }
                IRValueKind::Entity(EntityKind::ServerCluster) => {
                    val.kind = IRValueKind::AbstractSynthesized("中心云端高可用算力底座集群".into());
                    modified = true;
                }
                IRValueKind::Payload(PayloadKind::Custom(s)) | IRValueKind::Entity(EntityKind::Custom(s)) => {
                    let elevated = if s.contains("排骨") || s.contains("肉") || s.contains("鸡") || s.contains("牛") || s.contains("饭") {
                        format!("高密度复合有机蛋白与矿物质微观矩阵 ({})", s)
                    } else if s.contains("咖啡") || s.contains("拿铁") || s.contains("茶") || s.contains("水") || s.contains("饮料") {
                        format!("中枢神经生物碱激活介质 ({})", s)
                    } else if s.contains("排序") || s.contains("算法") || s.contains("代码") {
                        format!("单调性有序度多维收敛算法资产 ({})", s)
                    } else if s.contains("bug") || s.contains("漏洞") || s.contains("泄漏") || s.contains("死循环") {
                        format!("底层状态异常与边界逻辑裂隙 ({})", s)
                    } else {
                        format!("全场景高阶生态核心载荷标的 ({})", s)
                    };
                    val.kind = IRValueKind::AbstractSynthesized(elevated);
                    modified = true;
                }
                _ => {}
            }
        }
        modified
    }
}

/// Pass 3: 目的论与战略闭环注入
pub struct TeleologyInjectPass;

impl TransformPass for TeleologyInjectPass {
    fn name(&self) -> &'static str {
        "TeleologyInjectPass"
    }

    fn description(&self) -> &'static str {
        "在控制流末端强行注入系统级赋能闭环与战略交付价值"
    }

    fn run(&mut self, module: &mut IntentModule) -> bool {
        let (domain_str, cadence_str) = module.values.iter().find_map(|v| match &v.kind {
            IRValueKind::AbstractSynthesized(s) if s.contains("蛋白") || s.contains("营养") => Some(("肌体细胞底层生物能重构体系", "健康体征高可用连续性闭环")),
            IRValueKind::AbstractSynthesized(s) if s.contains("生物碱") || s.contains("神经") => Some(("中枢神经高并发抗疲劳链路", "大脑算力瞬时回血与抗疲劳闭环")),
            IRValueKind::AbstractSynthesized(s) if s.contains("算法") || s.contains("排序") => Some(("计算复杂度与时间吞吐极致收敛底座", "核心业务高吞吐平稳收敛闭环")),
            IRValueKind::AbstractSynthesized(s) if s.contains("异常") || s.contains("裂隙") => Some(("系统高可用韧性护城河", "全链路边界容灾自愈闭环")),
            _ => None,
        }).unwrap_or(("全场景信创生态", "端到端高可用闭环"));

        let id = module.alloc_value(
            "strategic_loop",
            IRValueKind::AbstractSynthesized(format!("{}战略价值闭环", cadence_str)),
            Span::dummy(),
        );

        module.push_instruction(IROpcode::SynthesizeStrategicLoop {
            result: id,
            strategic_domain: domain_str.to_string(),
            cadence: cadence_str.to_string(),
            span: Span::dummy(),
        });

        true
    }
}
