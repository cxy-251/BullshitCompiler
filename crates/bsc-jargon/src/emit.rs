//! 后端：把语义块拼回中文。
//!
//! 没被改过的块按原词序拼接（所以对还没优化的 IR 生成，得到的就是原文——这是一个很好的自检）；
//! 被"句式改写"过的块按新句式生成。分句之间保留原来的标点；整个分句都被删掉时，它的主语、情态词
//! （"我们要"）转交给同一句里的下一个分句，免得后面的话丢了主语。

use crate::ir::{ClauseIr, Part, Template};
use crate::lexicon::{Cat, Func};

pub fn part_text(ir: &ClauseIr, p: &Part) -> String {
    let t = |ws: &[usize]| ir.text_of(ws);
    let mut s = match p.template {
        None => t(&p.words),
        Some(Template::StartFrom) => format!("从{}入手", t(&p.obj)),
        Some(Template::DrivenBy) => format!("靠{}推动", t(&p.obj)),
        Some(Template::TreatAs) => format!("把{}当作{}", t(&p.obj), t(&p.obj2)),
        Some(Template::Has(split)) => {
            // 话题在前："业务形成闭环" → "业务有完整流程"；在宾语里："形成业务闭环" → "让业务有完整流程"。
            // 情态词保留："要形成业务闭环" → "要让业务有完整流程"
            let modal: Vec<usize> =
                p.words.iter().copied().filter(|&w| ir.words[w].cat == Cat::Func(Func::Modal)).collect();
            let (m, head, tail) = (t(&modal), t(&p.obj2[..split]), t(&p.obj2[split..]));
            match t(&p.obj) {
                topic if !topic.is_empty() => format!("{topic}{m}有{head}{tail}"),
                _ => format!("{m}让{head}有{tail}"),
            }
        }
    };
    for a in &p.appended {
        s.push('、');
        s.push_str(a);
    }
    s
}

/// 分句的主体（不含前置的主语、情态词）。
pub fn body_text(ir: &ClauseIr) -> String {
    ir.parts.iter().filter(|p| !p.dead).map(|p| part_text(ir, p)).collect()
}

pub fn clause_text(ir: &ClauseIr) -> String {
    format!("{}{}{}", ir.marker, ir.text_of(&ir.pre), body_text(ir))
}

/// 整段输出。
pub fn render(irs: &[ClauseIr]) -> String {
    let mut out = String::new();
    let mut pieces: Vec<(String, &str)> = Vec::new();
    let mut carry: Option<String> = None;
    for ir in irs {
        if !ir.merged {
            let pre = ir.text_of(&ir.pre);
            if ir.words.is_empty() {
                // 只有编号的分句（"首先，"）
                pieces.push((ir.marker.clone(), &ir.punct));
            } else if !ir.has_body() {
                if !pre.is_empty() && carry.is_none() {
                    carry = Some(pre);
                }
            } else {
                let pre = if pre.is_empty() { carry.take().unwrap_or_default() } else { pre };
                carry = None;
                pieces.push((format!("{}{pre}{}", ir.marker, body_text(ir)), &ir.punct));
            }
        }
        if ir.ends_sentence {
            let n = pieces.len();
            for (i, (text, punct)) in pieces.drain(..).enumerate() {
                out.push_str(&text);
                if i + 1 < n {
                    out.push_str(if punct.is_empty() { "，" } else { punct });
                } else {
                    // 句末用这句最后一个分句的标点（它自己可能被删掉了），换行照原样保留
                    out.push_str(&ir.punct);
                }
            }
            carry = None;
        }
    }
    out
}
