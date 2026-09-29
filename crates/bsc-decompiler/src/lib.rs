use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecompileResult {
    pub raw_jargon: String,
    pub extracted_key_aspects: Vec<String>,
    pub brutal_truth: String,
}

pub struct Decompiler;

impl Decompiler {
    pub fn new() -> Self {
        Self
    }

    pub fn decompile(&self, jargon: &str) -> DecompileResult {
        let mut aspects = Vec::new();
        let mut truths = Vec::new();

        if jargon.contains("全栈") || jargon.contains("全链路") || jargon.contains("端到端") {
            aspects.push("【岗位职责/架构泛化】要求兼顾前后端与全局".into());
            truths.push("公司预算紧缺，不想招专职前端、后端和测试，企图用一个人的工资买下一个微型IT技术部。");
        }

        if jargon.contains("分布式软总线") || jargon.contains("异构互联") || jargon.contains("近场") {
            aspects.push("【底层通信概念】包装了点对点无线协议".into());
            truths.push("实际上就是普通的 Wi-Fi P2P 和蓝牙抓包广播，底层全是开源通信驱动。");
        }

        if jargon.contains("敏捷响应") || jargon.contains("高烈度") || jargon.contains("承压极限") || jargon.contains("主战场") {
            aspects.push("【工作节奏话术】强调拼搏与韧性".into());
            truths.push("天天强制加班到深夜，无加班费，业务频繁改需求且管理混乱。");
        }

        if jargon.contains("赋能") || jargon.contains("抓手") || jargon.contains("沉淀心智") {
            aspects.push("【管理学黑话】大厂八股汇报用语".into());
            truths.push("业务找不到突破口，PPT 只能狂造抽象大词忽悠领导和投资人。");
        }

        if jargon.contains("自主可控") || jargon.contains("安全底线") || jargon.contains("新质生产力") {
            aspects.push("【政策导向借势】信创与政务投标语境".into());
            truths.push("项目专门用来申报地方政府科技补贴或信创投标，做个演示原型拿验收款。");
        }

        if truths.is_empty() {
            truths.push("这段话本质上没有任何核心技术壁垒，主要是堆砌形容词掩饰简单的增删改查。");
        }

        DecompileResult {
            raw_jargon: jargon.to_string(),
            extracted_key_aspects: aspects,
            brutal_truth: truths.join(" "),
        }
    }
}
