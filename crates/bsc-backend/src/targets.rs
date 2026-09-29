use crate::emitter::BackendEmitter;
use bsc_core::{ActionKind, AgentKind, EntityKind, IROpcode, IRValueKind, IntentModule, MediumKind, OutcomeKind, PayloadKind};

fn resolve_asset_name(module: &IntentModule, id: usize) -> String {
    if let Some(v) = module.values.get(id) {
        match &v.kind {
            IRValueKind::AbstractSynthesized(s) => s.clone(),
            IRValueKind::Payload(p) => match p {
                PayloadKind::BinaryCode => "高可用代码补丁与核心编译固件".into(),
                PayloadKind::Document => "体系化知识资产与汇报文档".into(),
                PayloadKind::LogStream => "全链路可观测性日志度量流".into(),
                PayloadKind::Image => "高维视觉多模态资产".into(),
                PayloadKind::Telemetry => "多维遥测感知数据报文".into(),
                PayloadKind::Custom(c) => c.clone(),
            },
            IRValueKind::Entity(e) => match e {
                EntityKind::ServerCluster => "弹性异构云原生集群底座".into(),
                EntityKind::Database => "分布式高性能关系型数据底座".into(),
                EntityKind::EmbeddedBoard => "边缘端智能异构嵌入式节点".into(),
                EntityKind::TerminalClient => "全场景触达用户终端入口".into(),
                EntityKind::PeripheralDevice => "高感知智能外设交互单元".into(),
                EntityKind::Custom(c) => c.clone(),
            },
            IRValueKind::Medium(m) => match m {
                MediumKind::WiFi => "无界高频无线软总线链路".into(),
                MediumKind::Ethernet => "低时延高吞吐千兆确定性网络通道".into(),
                MediumKind::SerialPort => "纳秒级硬同步串行通信信道".into(),
                MediumKind::Bluetooth => "超近场低功耗近感通信矩阵".into(),
                MediumKind::HttpRest => "标准化高维服务API治理网关".into(),
                MediumKind::Custom(c) => c.clone(),
            },
            IRValueKind::Action(a) => match a {
                ActionKind::Connect => "拉通协同连接与协议对齐".into(),
                ActionKind::Transfer => "高并发资产流转与吞吐调度".into(),
                ActionKind::Restart => "故障隔离与无感平滑热重载".into(),
                ActionKind::Verify => "多维质效常态化核验闭环".into(),
                ActionKind::Process => "全链路深度赋能与价值重塑".into(),
                ActionKind::Crash => "异构节点异常熔断与降级自愈".into(),
                ActionKind::Custom(c) => c.clone(),
            },
            IRValueKind::Outcome(o) => match o {
                OutcomeKind::SuccessNormal => "高标准交付并达成核心SLO指标".into(),
                OutcomeKind::FailureError => "异构风险触发熔断告警并完成自愈收敛".into(),
                OutcomeKind::Unknown => "常态化全链路质量基线".into(),
            },
            IRValueKind::Agent(ag) => match ag {
                AgentKind::FirstPersonSingular => "核心技术领军负责人".into(),
                AgentKind::FirstPersonPlural => "跨域敏捷攻坚特战先锋团队".into(),
                AgentKind::ThirdPerson => "跨域协同业务方矩阵".into(),
                AgentKind::Organization => "高绩效敏捷业务单元".into(),
                AgentKind::ImplicitSystem => "全自主自适应智能中台系统".into(),
            },
        }
    } else {
        "核心战略标的要素".to_string()
    }
}

fn get_primary_asset(module: &IntentModule) -> String {
    module.values.iter().find_map(|v| {
        if v.name == "payload" {
            if let IRValueKind::AbstractSynthesized(s) = &v.kind {
                return Some(s.clone());
            }
        }
        None
    }).or_else(|| {
        module.values.iter().find_map(|v| match &v.kind {
            IRValueKind::AbstractSynthesized(s) if !s.contains("网络") && !s.contains("信道") && !s.contains("通信") && !s.contains("软总线") && !s.contains("闭环") && !s.contains("节点") => Some(s.clone()),
            _ => None,
        })
    }).unwrap_or_else(|| "高维战略业务核心资产".to_string())
}

/// Target 1: 华为鸿蒙 / 山海经 / 军工宏大叙事后端
pub struct HuaweiEmitter;

impl BackendEmitter for HuaweiEmitter {
    fn target_name(&self) -> &'static str {
        "huawei"
    }

    fn description(&self) -> &'static str {
        "华为山海经 / 鸿蒙分布式微内核 / 自主可控军工风"
    }

    fn emit(&self, module: &IntentModule) -> String {
        let primary_asset = get_primary_asset(module);
        let mut total_insts = 0;
        for b in &module.blocks { total_insts += b.instructions.len(); }

        let is_doc = total_insts >= 4;

        let mut section1 = Vec::new();
        let mut section2 = Vec::new();
        let mut section3 = Vec::new();
        let mut section4 = Vec::new();

        section1.push(format!("依托深层微内核底座，打破【{}】物理交互边界，构筑全场景分布式互联互通护城河。", primary_asset));

        let transfer_templates = [
            "实现【{}】在异构计算环境中的毫秒级原子化流转",
            "聚力推进【{}】在星闪与软总线协议栈的高吞吐确定性分发",
            "打通【{}】端云协同硬核通道，实现根技术自主创新突破",
            "强化【{}】高并发并发管控，保障多模态无缝连续体验",
        ];
        let mut transfer_idx = 0;

        for block in &module.blocks {
            for inst in &block.instructions {
                match inst {
                    IROpcode::EstablishSession { target, .. } => {
                        let t_name = resolve_asset_name(module, *target);
                        section1.push(format!("高效拉通面向【{}】的端到端可信互联链路，筑牢软总线通信根基。", t_name));
                    }
                    IROpcode::TransferPayload { payload, .. } => {
                        let p_name = resolve_asset_name(module, *payload);
                        let tmpl = transfer_templates[transfer_idx % transfer_templates.len()];
                        transfer_idx += 1;
                        section2.push(format!("{}，保障高安全低时延交互。", tmpl.replace("{}", &p_name)));
                    }
                    IROpcode::MutateState { target, action, .. } => {
                        let t_name = resolve_asset_name(module, *target);
                        let a_name = resolve_asset_name(module, *action);
                        section3.push(format!("针对【{}】触发【{}】自愈机制，完成平滑热重载与底层容灾隔离。", t_name, a_name));
                    }
                    IROpcode::AssertIntegrity { target, expected, .. } => {
                        let t_name = resolve_asset_name(module, *target);
                        let e_name = resolve_asset_name(module, *expected);
                        section3.push(format!("对【{}】实施微内核级硬件安全基线核验，严密确证【{}】。", t_name, e_name));
                    }
                    IROpcode::SynthesizeStrategicLoop { strategic_domain, cadence, .. } => {
                        section4.push(format!("深度赋能【{}】，沉淀自主可控核心资产，形成长效【{}】军工级护城河！", strategic_domain, cadence));
                    }
                }
            }
        }

        if is_doc {
            let mut out = String::new();
            out.push_str("【全场景分布式微内核架构总览】\n");
            out.push_str(&section1.join(" "));
            out.push_str("\n\n【核心业务流原子化流转】\n");
            if section2.is_empty() {
                out.push_str(&format!("稳步驱动【{}】高效协同演进。\n", primary_asset));
            } else {
                for (i, item) in section2.iter().enumerate() {
                    out.push_str(&format!("{}. {}\n", i + 1, item));
                }
            }
            out.push_str("\n【容灾自愈与确定性质量基线】\n");
            if section3.is_empty() {
                out.push_str("全面构筑端云协同确定性可信安全底座。\n");
            } else {
                out.push_str(&section3.join(" "));
                out.push('\n');
            }
            if !section4.is_empty() {
                out.push_str("\n【战略远景与根技术护城河】\n");
                out.push_str(&section4.join(" "));
            }
            out
        } else {
            let mut all = section1;
            all.extend(section2);
            all.extend(section3);
            all.extend(section4);
            all.join("，")
        }
    }
}

/// Target 2: 阿里中台 / P8 敏捷抓手赋能后端
pub struct AlibabaEmitter;

impl BackendEmitter for AlibabaEmitter {
    fn target_name(&self) -> &'static str {
        "alibaba"
    }

    fn description(&self) -> &'static str {
        "阿里中台 / P8 敏捷架构 / 抓手对齐闭环风"
    }

    fn emit(&self, module: &IntentModule) -> String {
        let primary_asset = get_primary_asset(module);
        let mut total_insts = 0;
        for b in &module.blocks { total_insts += b.instructions.len(); }

        let is_doc = total_insts >= 4;

        let mut section1 = Vec::new();
        let mut section2 = Vec::new();
        let mut section3 = Vec::new();
        let mut section4 = Vec::new();

        section1.push(format!("围绕【{}】这一核心底层抓手，深耕全链路痛点打法，完成顶层心智透传与价值定位对齐。", primary_asset));

        let transfer_templates = [
            "打通端到端高维资产跨域交付全链路，实现【{}】边际效能跃迁",
            "聚力推进【{}】全生命周期精细化运营与效能深潜",
            "聚焦【{}】核心商业诉求，撬动业务确定性增长飞轮",
            "构建面向【{}】的自适应弹性调度引擎，打破数据孤岛",
        ];
        let mut transfer_idx = 0;

        for block in &module.blocks {
            for inst in &block.instructions {
                match inst {
                    IROpcode::EstablishSession { target, .. } => {
                        let t_name = resolve_asset_name(module, *target);
                        section1.push(format!("快速拉通面向【{}】的协同协议，强化组织心智与场景穿透力。", t_name));
                    }
                    IROpcode::TransferPayload { payload, .. } => {
                        let p_name = resolve_asset_name(module, *payload);
                        let tmpl = transfer_templates[transfer_idx % transfer_templates.len()];
                        transfer_idx += 1;
                        section2.push(format!("{}，形成飞轮效应。", tmpl.replace("{}", &p_name)));
                    }
                    IROpcode::MutateState { target, action, .. } => {
                        let t_name = resolve_asset_name(module, *target);
                        let a_name = resolve_asset_name(module, *action);
                        section3.push(format!("以敏捷演进机制倒逼【{}】架构韧性重构，针对【{}】打出快速响应组合拳。", t_name, a_name));
                    }
                    IROpcode::AssertIntegrity { target, expected, .. } => {
                        let t_name = resolve_asset_name(module, *target);
                        let e_name = resolve_asset_name(module, *expected);
                        section3.push(format!("针对【{}】构建可量化、可追溯的多维验证指标体系，牢牢锁定【{}】。", t_name, e_name));
                    }
                    IROpcode::SynthesizeStrategicLoop { strategic_domain, cadence, .. } => {
                        section4.push(format!("持续赋能【{}】，打出降本增效组合拳，驱动【{}】形成高维战略闭环！", strategic_domain, cadence));
                    }
                }
            }
        }

        if is_doc {
            let mut out = String::new();
            out.push_str("【业务战略大盘与核心打法聚焦】\n");
            out.push_str(&section1.join(" "));
            out.push_str("\n\n【关键链路抓手与交付落地】\n");
            if section2.is_empty() {
                out.push_str(&format!("深耕【{}】纵深场景，打造差异化护城河。\n", primary_asset));
            } else {
                for (i, item) in section2.iter().enumerate() {
                    out.push_str(&format!("{}. {}\n", i + 1, item));
                }
            }
            out.push_str("\n【架构韧性沉淀与质效闭环】\n");
            if section3.is_empty() {
                out.push_str("打通多维质效大盘，实现指标全局可视可控。\n");
            } else {
                out.push_str(&section3.join(" "));
                out.push('\n');
            }
            if !section4.is_empty() {
                out.push_str("\n【长效生态演进与价值跃迁】\n");
                out.push_str(&section4.join(" "));
            }
            out
        } else {
            let mut all = section1;
            all.extend(section2);
            all.extend(section3);
            all.extend(section4);
            all.join("，")
        }
    }
}

/// Target 3: 政企信创 / 政策红头文件后端
pub struct StateOwnedEmitter;

impl BackendEmitter for StateOwnedEmitter {
    fn target_name(&self) -> &'static str {
        "state_owned"
    }

    fn description(&self) -> &'static str {
        "政务信创 / 国资红头文件 / 高位推进公文风"
    }

    fn emit(&self, module: &IntentModule) -> String {
        let primary_asset = get_primary_asset(module);
        let mut total_insts = 0;
        for b in &module.blocks { total_insts += b.instructions.len(); }

        let is_doc = total_insts >= 4;

        let mut section1 = Vec::new();
        let mut section2 = Vec::new();
        let mut section3 = Vec::new();
        let mut section4 = Vec::new();

        section1.push(format!("提高政治站位，深入贯彻落实高质量发展总体要求，多措并举推进【{}】的高水平供给与科学调配。", primary_asset));

        let transfer_templates = [
            "稳步提升【{}】流通与业务协同保障效能",
            "全力推动【{}】规范化、制度化、数字化全过程管理",
            "着力优化【{}】资源配置结构，增强公共支撑服务能力",
            "统筹推进【{}】重点工程实施，切实发挥基础性战略性作用",
        ];
        let mut transfer_idx = 0;

        for block in &module.blocks {
            for inst in &block.instructions {
                match inst {
                    IROpcode::EstablishSession { target, .. } => {
                        let t_name = resolve_asset_name(module, *target);
                        section1.push(format!("统筹强化面向【{}】的基础支撑体系建设，确保协同渠道畅通高效。", t_name));
                    }
                    IROpcode::TransferPayload { payload, .. } => {
                        let p_name = resolve_asset_name(module, *payload);
                        let tmpl = transfer_templates[transfer_idx % transfer_templates.len()];
                        transfer_idx += 1;
                        section2.push(format!("{}，确保各项目标任务落到实处。", tmpl.replace("{}", &p_name)));
                    }
                    IROpcode::MutateState { target, action, .. } => {
                        let t_name = resolve_asset_name(module, *target);
                        let a_name = resolve_asset_name(module, *action);
                        section3.push(format!("高度重视【{}】运行安全，针对【{}】切实提升应急处置与韧性恢复能力，坚决守牢安全底线。", t_name, a_name));
                    }
                    IROpcode::AssertIntegrity { target, expected, .. } => {
                        let t_name = resolve_asset_name(module, *target);
                        let e_name = resolve_asset_name(module, *expected);
                        section3.push(format!("建立健全对【{}】的全流程常态化监管闭环机制，严肃确证【{}】。", t_name, e_name));
                    }
                    IROpcode::SynthesizeStrategicLoop { strategic_domain, cadence, .. } => {
                        section4.push(format!("以新质生产力持续赋能【{}】，坚持常态长效，稳步健全【{}】，筑牢高质量发展根基！", strategic_domain, cadence));
                    }
                }
            }
        }

        if is_doc {
            let mut out = String::new();
            out.push_str("一、 统一思想认识，明确总体推进方向\n");
            out.push_str(&section1.join(" "));
            out.push_str("\n\n二、 聚焦关键重点，扎实推进任务实施\n");
            if section2.is_empty() {
                out.push_str(&format!("坚持统筹兼顾，有力有序抓好【{}】重点领域攻坚。\n", primary_asset));
            } else {
                for (i, item) in section2.iter().enumerate() {
                    out.push_str(&format!("（{}）{}\n", i + 1, item));
                }
            }
            out.push_str("\n三、 坚决防范风险，筑牢安全监管底线\n");
            if section3.is_empty() {
                out.push_str("压紧压实各方责任，确保各环节安全可控稳定运行。\n");
            } else {
                out.push_str(&section3.join(" "));
                out.push('\n');
            }
            if !section4.is_empty() {
                out.push_str("\n四、 健全长效机制，持续深化发展成果\n");
                out.push_str(&section4.join(" "));
            }
            out
        } else {
            let mut all = section1;
            all.extend(section2);
            all.extend(section3);
            all.extend(section4);
            all.join("，")
        }
    }
}

/// Target 4: 硅谷英文科技布道后端 (Silicon Valley Tech-Bro)
pub struct SiliconValleyEmitter;

impl BackendEmitter for SiliconValleyEmitter {
    fn target_name(&self) -> &'static str {
        "silicon_valley"
    }

    fn description(&self) -> &'static str {
        "硅谷英文科技布道 / Zero-Trust Fabric / AIoT Paradigm"
    }

    fn emit(&self, module: &IntentModule) -> String {
        let primary_asset = get_primary_asset(module);
        let mut total_insts = 0;
        for b in &module.blocks { total_insts += b.instructions.len(); }

        let is_doc = total_insts >= 4;

        let mut section1 = Vec::new();
        let mut section2 = Vec::new();
        let mut section3 = Vec::new();
        let mut section4 = Vec::new();

        section1.push(format!("Leveraging an edge-native zero-trust fabric to dissolve physical compute boundaries around [{}].", primary_asset));

        let transfer_templates = [
            "Democratizes sub-millisecond ingestion and distributed replication of [{}] at hyper-scale",
            "Architects an autonomous, reactive event pipeline around [{}]",
            "Orchestrates dynamic load-shedding and multi-region consensus for [{}]",
            "Unlocks massive developer velocity and telemetry-driven elasticity for [{}]",
        ];
        let mut transfer_idx = 0;

        for block in &module.blocks {
            for inst in &block.instructions {
                match inst {
                    IROpcode::EstablishSession { target, .. } => {
                        let t_name = resolve_asset_name(module, *target);
                        section1.push(format!("We seamlessly orchestrate high-availability peer sessions targeting [{}].", t_name));
                    }
                    IROpcode::TransferPayload { payload, .. } => {
                        let p_name = resolve_asset_name(module, *payload);
                        let tmpl = transfer_templates[transfer_idx % transfer_templates.len()];
                        transfer_idx += 1;
                        section2.push(format!("{}.", tmpl.replace("{}", &p_name)));
                    }
                    IROpcode::MutateState { target, action, .. } => {
                        let t_name = resolve_asset_name(module, *target);
                        let a_name = resolve_asset_name(module, *action);
                        section3.push(format!("The autonomous self-healing control loop on [{}] seamlessly remediates [{}] under adversarial telemetry.", t_name, a_name));
                    }
                    IROpcode::AssertIntegrity { target, expected, .. } => {
                        let t_name = resolve_asset_name(module, *target);
                        let e_name = resolve_asset_name(module, *expected);
                        section3.push(format!("Continuously closing the observability loop on [{}] to guarantee [{}].", t_name, e_name));
                    }
                    IROpcode::SynthesizeStrategicLoop { strategic_domain, cadence, .. } => {
                        section4.push(format!("Driving an unprecedented paradigm shift across the {} ecosystem via a resilient {}!", strategic_domain, cadence));
                    }
                }
            }
        }

        if is_doc {
            let mut out = String::new();
            out.push_str("## 1. Architectural Vision & Edge-Native Paradigm\n");
            out.push_str(&section1.join(" "));
            out.push_str("\n\n## 2. Distributed Execution & High-Throughput Delivery\n");
            if section2.is_empty() {
                out.push_str(&format!("Unlocking real-time streaming velocity across [{}].\n", primary_asset));
            } else {
                for (i, item) in section2.iter().enumerate() {
                    out.push_str(&format!("- **P{}**: {}\n", i + 1, item));
                }
            }
            out.push_str("\n## 3. Autonomous Resilience & Observability SLOs\n");
            if section3.is_empty() {
                out.push_str("Continuous validation guarantees five-nines uptime across all clusters.\n");
            } else {
                out.push_str(&section3.join(" "));
                out.push('\n');
            }
            if !section4.is_empty() {
                out.push_str("\n## 4. Ecosystem Synergies & Long-term Trajectory\n");
                out.push_str(&section4.join(" "));
            }
            out
        } else {
            let mut all = section1;
            all.extend(section2);
            all.extend(section3);
            all.extend(section4);
            all.join(" ")
        }
    }
}
