//! 优化 Pass。每个 Pass 只改 IR 上的标记（词的说法、是否删除、块的句式），并记下每一处改动和理由。
//!
//! 顺序是：常量折叠 → 死代码消除 → 降级 → 公共子表达式消除。
//! 先折叠，固定搭配里的词就不会被当成修饰语删掉；先删修饰语，句式改写时看到的才是真正的内容；
//! 合并排比放在降级之后，这样"强化 A""加强 B"降级成同一个动词以后也能合并。

use bsc_core::Span;

use crate::emit::part_text;
use crate::ir::{ClauseIr, Part, Role, Template, is_glue, is_verb};
use crate::lexicon::{Cat, Func, Lexicon};

#[derive(Clone, Debug)]
pub struct Change {
    pub clause: usize,
    /// 涉及的原文位置（界面高亮用）。
    pub spans: Vec<Span>,
    pub before: String,
    pub after: String,
    pub why: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pass {
    Fold,
    Dce,
    Lower,
    Cse,
}

impl Pass {
    pub const ALL: [Pass; 4] = [Pass::Fold, Pass::Dce, Pass::Lower, Pass::Cse];

    pub fn name(self) -> &'static str {
        match self {
            Pass::Fold => "常量折叠",
            Pass::Dce => "死代码消除",
            Pass::Lower => "降级",
            Pass::Cse => "公共子表达式消除",
        }
    }

    pub fn what(self) -> &'static str {
        match self {
            Pass::Fold => "固定搭配（降本增效、形成合力……）的意思是固定的，像常量表达式一样，整体换成大白话。",
            Pass::Dce => {
                "膨胀修饰语（全面、深入、高质量……）不携带信息，删掉；\"形成闭环\"\"开创新局面\"这种动词和宾语都是虚的块，整块删掉；删完后悬空的\"的\"\"和\"一并删掉。"
            }
            Pass::Lower => {
                "空洞动词、虚指名词沿着词典里的上位词链一步步降到朴素说法（赋能 → 助力 → 帮助）；\"以 X 为抓手\"这类句式整体改写（从 X 入手）。"
            }
            Pass::Cse => {
                "动词堆叠（统筹推进、抓好落实）只留中心动词；相邻分句动词相同的排比（加强 A，强化 B）合并成一句（加强 A、B）。"
            }
        }
    }

    pub fn run(self, irs: &mut [ClauseIr], lex: &Lexicon) -> Vec<Change> {
        let mut out = Vec::new();
        match self {
            Pass::Fold => fold(irs, lex, &mut out),
            Pass::Dce => dce(irs, &mut out),
            Pass::Lower => lower(irs, lex, &mut out),
            Pass::Cse => cse(irs, &mut out),
        }
        out
    }
}

fn spans(ir: &ClauseIr, ws: &[usize]) -> Vec<Span> {
    ws.iter().map(|&w| ir.words[w].span).collect()
}

fn quote(ir: &ClauseIr, ws: &[usize]) -> String {
    ws.iter().map(|&w| format!("「{}」", ir.words[w].text)).collect()
}

/// 对一个块做一次改写，记下前后文字。
fn edit_part(
    irs: &mut [ClauseIr],
    c: usize,
    p: usize,
    out: &mut Vec<Change>,
    f: impl FnOnce(&mut ClauseIr, usize) -> Option<(Vec<usize>, String)>,
) {
    let before = part_text(&irs[c], &irs[c].parts[p]);
    if let Some((ws, why)) = f(&mut irs[c], p) {
        let ir = &irs[c];
        let after = if ir.parts[p].dead { String::new() } else { part_text(ir, &ir.parts[p]) };
        // 改动的词被句式改写"吃掉"了（比如"以 X 为抓手"里的抓手），输出没变，就不记了
        if after == before {
            return;
        }
        out.push(Change { clause: c, spans: spans(ir, &ws), before, after, why });
    }
}

fn live_parts(irs: &[ClauseIr]) -> Vec<(usize, usize)> {
    let mut v = Vec::new();
    for (c, ir) in irs.iter().enumerate() {
        if ir.merged {
            continue;
        }
        for (p, part) in ir.parts.iter().enumerate() {
            if !part.dead {
                v.push((c, p));
            }
        }
    }
    v
}

// ------------------------------------------------------------------ 常量折叠

fn fold(irs: &mut [ClauseIr], lex: &Lexicon, out: &mut Vec<Change>) {
    for (c, p) in live_parts(irs) {
        edit_part(irs, c, p, out, |ir, p| {
            let ws: Vec<usize> = ir.parts[p]
                .words
                .iter()
                .copied()
                .filter(|&w| ir.alive(w) && matches!(ir.words[w].cat, Cat::Fixed | Cat::FixedVerb))
                .filter(|&w| ir.words[w].entry.and_then(|e| lex.entries[e].target.as_ref()).is_some())
                .collect();
            if ws.is_empty() {
                return None;
            }
            let mut why = Vec::new();
            for &w in &ws {
                let e = &lex.entries[ir.words[w].entry.unwrap()];
                let t = e.target.clone().unwrap();
                why.push(format!("「{}」→「{t}」", e.word));
                ir.words[w].now = Some(t);
            }
            Some((ws, format!("固定搭配整体替换：{}", why.join("，"))))
        });
    }
}

// ------------------------------------------------------------------ 死代码消除

fn dce(irs: &mut [ClauseIr], out: &mut Vec<Change>) {
    for (c, p) in live_parts(irs) {
        // 1. 虚动词 + 虚宾语：整块没有信息（逐词处理的分句不知道结构，不做）
        edit_part(irs, c, p, out, |ir, p| {
            let part = &ir.parts[p];
            if part.role != Role::Action || ir.fallback {
                return None;
            }
            let verbs: Vec<usize> = part.verbs.iter().copied().filter(|&w| ir.alive(w)).collect();
            let nouns: Vec<usize> = part
                .obj
                .iter()
                .chain(&part.obj2)
                .copied()
                .filter(|&w| ir.alive(w) && !is_glue(ir.words[w].cat))
                .collect();
            let hollow_verbs = !verbs.is_empty() && verbs.iter().all(|&w| ir.words[w].cat == Cat::Filler);
            let vague_nouns = nouns.iter().any(|&w| ir.words[w].cat == Cat::Vague)
                && nouns.iter().all(|&w| matches!(ir.words[w].cat, Cat::Vague | Cat::Modifier));
            if !(hollow_verbs && vague_nouns) {
                return None;
            }
            let ws = part.words.clone();
            let why = format!(
                "{}的宾语只有虚指名词{}，整块不携带信息",
                quote(ir, &verbs),
                quote(ir, &nouns.iter().copied().filter(|&w| ir.words[w].cat == Cat::Vague).collect::<Vec<_>>())
            );
            ir.parts[p].dead = true;
            Some((ws, why))
        });
        if irs[c].parts[p].dead {
            continue;
        }
        // 2. 修饰语
        edit_part(irs, c, p, out, |ir, p| {
            let part = &ir.parts[p];
            let mut keep = Vec::new();
            // 名词组里只剩修饰语时（"实现高质量"），它其实是中心词，留下最后一个
            for slot in [&part.obj, &part.obj2] {
                let content: Vec<usize> =
                    slot.iter().copied().filter(|&w| ir.alive(w) && !is_glue(ir.words[w].cat)).collect();
                if !content.is_empty() && content.iter().all(|&w| ir.words[w].cat == Cat::Modifier) {
                    keep.push(*content.last().unwrap());
                }
            }
            let ws: Vec<usize> = part
                .words
                .iter()
                .copied()
                .filter(|&w| ir.alive(w) && ir.words[w].cat == Cat::Modifier && !keep.contains(&w))
                .collect();
            if ws.is_empty() {
                return None;
            }
            for &w in &ws {
                ir.words[w].dead = true;
            }
            let dangling = clean_glue(ir, p);
            let why = format!("删掉膨胀修饰语{}{}", quote(ir, &ws), glue_note(ir, &dangling));
            Some(([ws, dangling].concat(), why))
        });
    }
    // 前置里的修饰语（"我们要始终"）
    for (c, ir) in irs.iter_mut().enumerate() {
        let ws: Vec<usize> =
            ir.pre.iter().copied().filter(|&w| ir.alive(w) && ir.words[w].cat == Cat::Modifier).collect();
        if ws.is_empty() {
            continue;
        }
        let before = ir.text_of(&ir.pre);
        for &w in &ws {
            ir.words[w].dead = true;
        }
        out.push(Change {
            clause: c,
            spans: spans(ir, &ws),
            before,
            after: ir.text_of(&ir.pre),
            why: format!("删掉膨胀修饰语{}", quote(ir, &ws)),
        });
    }
}

/// 删掉悬空的"的"和并列连词：它的左边或右边已经没有实词了。返回删掉的词。
fn clean_glue(ir: &mut ClauseIr, p: usize) -> Vec<usize> {
    let mut removed = Vec::new();
    loop {
        let alive: Vec<usize> = ir.parts[p].words.iter().copied().filter(|&w| ir.alive(w)).collect();
        let bad = alive.iter().enumerate().find(|&(i, &w)| {
            let cat = ir.words[w].cat;
            if !is_glue(cat) {
                return false;
            }
            let solid = |x: Option<&usize>| {
                x.is_some_and(|&x| {
                    let c = ir.words[x].cat;
                    !is_glue(c) && !matches!(c, Cat::Func(_)) && !(cat == Cat::Func(Func::De) && is_verb(c))
                })
            };
            let left = if i == 0 { None } else { alive.get(i - 1) };
            !solid(left) || !solid(alive.get(i + 1))
        });
        match bad {
            Some((_, &w)) => {
                ir.words[w].dead = true;
                removed.push(w);
            }
            None => return removed,
        }
    }
}

fn glue_note(ir: &ClauseIr, ws: &[usize]) -> String {
    if ws.is_empty() { String::new() } else { format!("，以及删完后悬空的{}", quote(ir, ws)) }
}

// ------------------------------------------------------------------ 降级

/// 手段块里的 Y 降到这些说法时，改写成"从 X 入手"。
const START_WORDS: &[&str] = &["入手的地方", "切入点", "着力点"];
const DRIVE_WORDS: &[&str] = &["动力", "新动力"];

fn lower(irs: &mut [ClauseIr], lex: &Lexicon, out: &mut Vec<Change>) {
    for (c, p) in live_parts(irs) {
        // 1. 句式改写（看的是词的类别和降级链，不是具体的词，所以词典扩充后自动适用）
        edit_part(irs, c, p, out, |ir, p| rewrite_pattern(ir, p, lex));
        // 2. 沿上位词链降级
        edit_part(irs, c, p, out, |ir, p| {
            let mut ws = Vec::new();
            let mut why = Vec::new();
            for w in ir.parts[p].words.clone() {
                let word = &ir.words[w];
                if !ir.alive(w) || word.now.is_some() {
                    continue;
                }
                let canon = match word.cat {
                    Cat::Func(Func::WeiRao) => Some("围绕"),
                    Cat::Func(Func::TongGuo) => Some("通过"),
                    Cat::Func(Func::Rang) => Some("让"),
                    _ => None,
                };
                if let Some(canon) = canon {
                    if word.text != canon {
                        why.push(format!("{} → {canon}（功能词统一成最常用的说法）", word.text));
                        ir.words[w].now = Some(canon.to_owned());
                        ws.push(w);
                    }
                    continue;
                }
                if !matches!(word.cat, Cat::Hollow | Cat::Filler | Cat::Vague | Cat::Func(Func::Link)) {
                    continue;
                }
                let Some(e) = word.entry else { continue };
                let chain = lex.chain(e);
                if chain.len() < 2 {
                    continue;
                }
                why.push(chain.join(" → "));
                ir.words[w].now = Some(chain.last().unwrap().clone());
                ws.push(w);
            }
            if ws.is_empty() {
                return None;
            }
            Some((ws, format!("降级：{}", why.join("；"))))
        });
    }
}

fn rewrite_pattern(ir: &mut ClauseIr, p: usize, lex: &Lexicon) -> Option<(Vec<usize>, String)> {
    let part = &ir.parts[p];
    if part.template.is_some() || ir.fallback {
        return None;
    }
    let chain_has = |w: usize, set: &[&str]| {
        ir.words[w].entry.is_some_and(|e| lex.chain(e).iter().any(|s| set.contains(&s.as_str())))
    };
    let alive =
        |ws: &[usize]| ws.iter().copied().filter(|&w| ir.alive(w) && !is_glue(ir.words[w].cat)).collect::<Vec<_>>();
    match part.role {
        Role::Means => {
            let y = alive(&part.obj2);
            if !y.iter().any(|&w| ir.words[w].cat == Cat::Vague) {
                return None; // "以客户为中心"：没有黑话，不动
            }
            let (t, why) = if y.iter().any(|&w| chain_has(w, START_WORDS)) {
                (Template::StartFrom, format!("「以 X 为{}」是说从哪里入手，改写成「从 X 入手」", ir.text_of(&y)))
            } else if y.iter().any(|&w| chain_has(w, DRIVE_WORDS)) {
                (Template::DrivenBy, format!("「以 X 为{}」是说靠什么推动，改写成「靠 X 推动」", ir.text_of(&y)))
            } else {
                (Template::TreatAs, "「以 X 为 Y」改写成更口语的「把 X 当作 Y」".to_owned())
            };
            let ws = part.markers.clone();
            ir.parts[p].template = Some(t);
            Some((ws, why))
        }
        Role::Action => {
            // 空转动词 + "内容 + 虚指名词"：形成业务闭环 → 业务有完整流程
            let verbs: Vec<usize> = part.verbs.iter().copied().filter(|&w| ir.alive(w)).collect();
            if verbs.len() != 1 || ir.words[verbs[0]].cat != Cat::Filler || part.markers.iter().any(|&m| ir.alive(m)) {
                return None;
            }
            let obj2 = &part.obj2;
            let split = obj2.iter().rposition(|&w| ir.alive(w) && ir.words[w].cat != Cat::Vague).map_or(0, |i| i + 1);
            let tail = alive(&obj2[split..]);
            let head = alive(&[&part.obj[..], &obj2[..split]].concat());
            if tail.is_empty() || head.is_empty() {
                return None;
            }
            let why =
                format!("{}加上「内容 + 虚指名词」，意思是\"内容具备了某种状态\"，改写成「X 有 Y」", quote(ir, &verbs));
            ir.parts[p].template = Some(Template::Has(split));
            Some((verbs, why))
        }
        _ => None,
    }
}

// ------------------------------------------------------------------ 公共子表达式消除

fn cse(irs: &mut [ClauseIr], out: &mut Vec<Change>) {
    // 1. 动词堆叠：谓词里有好几个动词，前面的空洞动词都是多余的
    for (c, p) in live_parts(irs) {
        edit_part(irs, c, p, out, |ir, p| {
            let verbs: Vec<usize> = ir.parts[p].verbs.iter().copied().filter(|&w| ir.alive(w)).collect();
            if verbs.len() < 2 || ir.fallback {
                return None;
            }
            let (last, rest) = verbs.split_last().unwrap();
            // 三个以上、中心动词是朴素动词时（强化统筹协调），第一个动词是真正的谓语，留下它
            let keep_first = rest.len() >= 2 && ir.words[*last].cat == Cat::Verb;
            let ws: Vec<usize> = rest[usize::from(keep_first)..]
                .iter()
                .copied()
                .filter(|&w| {
                    matches!(ir.words[w].cat, Cat::Hollow | Cat::Filler)
                        || ir.words[w].current() == ir.words[*last].current()
                })
                .collect();
            if ws.is_empty() {
                return None;
            }
            for &w in &ws {
                ir.words[w].dead = true;
            }
            let dangling = clean_glue(ir, p);
            let why = format!(
                "动词堆叠：{}和中心动词「{}」重复，只留中心动词{}",
                quote(ir, &ws),
                ir.words[*last].text,
                glue_note(ir, &dangling)
            );
            Some(([ws, dangling].concat(), why))
        });
    }
    // 2. 排比合并：同一句里相邻的两个分句，都只有一个"动词 + 宾语"块、动词相同
    let mut i = 0;
    while i < irs.len() {
        if !mergeable(&irs[i]) {
            i += 1;
            continue;
        }
        // 同一句里的下一个还有内容的分句（跳过已合并的、整句被删光的）
        let mut j = i;
        while !irs[j].ends_sentence && j + 1 < irs.len() {
            j += 1;
            let empty = !irs[j].has_body() && irs[j].text_of(&irs[j].pre).is_empty();
            if !(irs[j].merged || empty) {
                break;
            }
        }
        if j == i || !mergeable(&irs[j]) || !irs[j].text_of(&irs[j].pre).is_empty() {
            i += 1;
            continue;
        }
        let (vi, oi) = verb_obj(&irs[i]);
        let (vj, oj) = verb_obj(&irs[j]);
        if vi != vj {
            i += 1;
            continue;
        }
        let before =
            format!("{}{}{}", crate::emit::clause_text(&irs[i]), irs[i].punct, crate::emit::clause_text(&irs[j]));
        let pi = irs[i].parts.iter().position(|p| !p.dead).unwrap();
        let pj = irs[j].parts.iter().position(|p| !p.dead).unwrap();
        let already = oi == oj || irs[i].parts[pi].appended.contains(&oj);
        if !already {
            irs[i].parts[pi].appended.push(oj.clone());
            let more = irs[j].parts[pj].appended.clone();
            irs[i].parts[pi].appended.extend(more);
        }
        irs[j].merged = true;
        irs[i].ends_sentence = irs[j].ends_sentence;
        irs[i].punct = irs[j].punct.clone();
        let after = crate::emit::clause_text(&irs[i]);
        let why = if already {
            format!("两个分句说的是同一件事「{vi}{oi}」，删掉重复的")
        } else {
            format!("排比：两个分句的动词都是「{vi}」，宾语并列成一句")
        };
        let mut sp = spans(&irs[i], &irs[i].parts[pi].words);
        sp.extend(spans(&irs[j], &irs[j].parts[pj].words));
        out.push(Change { clause: i, spans: sp, before, after, why });
        // 继续看 i 能不能和下一个分句合并
    }
    // 3. 同一句里重复的分句（常见于固定搭配折叠以后："提高质量和效率，提质增效"）
    let mut seen: Vec<String> = Vec::new();
    for (c, ir) in irs.iter_mut().enumerate() {
        if ir.has_body() {
            let body = crate::emit::body_text(ir);
            if seen.contains(&body) && ir.text_of(&ir.pre).is_empty() {
                let ws: Vec<usize> = (0..ir.words.len()).collect();
                out.push(Change {
                    clause: c,
                    spans: spans(ir, &ws),
                    before: body.clone(),
                    after: String::new(),
                    why: format!("和前面的分句重复（都是「{body}」），删掉"),
                });
                ir.merged = true;
            } else {
                seen.push(body);
            }
        }
        if ir.ends_sentence {
            seen.clear();
        }
    }
}

/// 可以参与排比合并：恰好一个活着的"谓词 + 宾语"动作块，没有句式改写。
fn mergeable(ir: &ClauseIr) -> bool {
    if ir.fallback || ir.merged {
        return false;
    }
    let live: Vec<&Part> = ir.parts.iter().filter(|p| !p.dead).collect();
    live.len() == 1 && {
        let p = live[0];
        p.role == Role::Action
            && p.template.is_none()
            && p.obj.iter().all(|&w| !ir.alive(w))
            && p.markers.iter().all(|&w| !ir.alive(w))
            && p.obj2.iter().any(|&w| ir.alive(w))
            && p.verbs.iter().any(|&w| ir.alive(w))
    }
}

fn verb_obj(ir: &ClauseIr) -> (String, String) {
    let p = ir.parts.iter().find(|p| !p.dead).unwrap();
    (ir.text_of(&p.verbs), ir.text_of(&p.obj2))
}
