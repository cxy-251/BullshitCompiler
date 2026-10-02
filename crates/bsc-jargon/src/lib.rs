//! # 黑话编译器
//!
//! 把故作高深的"领导讲话"编译成大白话。它分成两层：
//!
//! - **词典**（`lexicon`）：黑话词语 → 类别 + 上位词 / 大白话的映射。这是纯文本数据，可以让 AI 批量生成、
//!   随时追加新的一批；加载时像编译器检查源码一样检查它（冲突、成环、映射里还有黑话、单字词条……），
//!   所以词典膨胀到上万条也不会悄悄变坏。
//! - **编译流水线**：只依赖词的类别，不依赖具体的词，所以词典怎么扩充都不用改：
//!   1. 分句、分词（`segment`）：Aho-Corasick 找出所有候选词，词图上选代价最小的切分；
//!   2. 句式分析（`earley` + `grammar`）：加权 Earley 在有歧义的句式文法里找代价最小的语法树；
//!   3. 语义角色 IR（`ir`）：手段 / 焦点 / 途径 / 目的 / 动作；
//!   4. 优化 Pass（`passes`）：常量折叠、死代码消除、降级（含句式改写）、公共子表达式消除；
//!   5. 生成（`gen`）：拼回中文；最后再检查一遍输出里有没有残留黑话。
//!
//! 逐词替换（包括让普通 AI 直接改写）做不好的事，都在流水线里：句式整体改写（"以 X 为抓手"→"从 X 入手"）、
//! 整块删除没有信息的话（"形成全链路闭环"）、动词堆叠合并、排比合并、删完修饰语后清理悬空的"的"，
//! 以及每一处改动都能说出理由、同一个词在全文里的译法始终一致。

pub mod ac;
pub mod corpus;
pub mod earley;
pub mod emit;
pub mod grammar;
pub mod ir;
pub mod lexicon;
pub mod passes;
pub mod segment;

use std::collections::HashMap;

use bsc_core::{Diagnostic, Span};
use bsc_grammar::Sym;
use bsc_grammar::grammar::Grammar;

use crate::ir::ClauseIr;
use crate::lexicon::{Layer, Lexicon, builtin_layers};
use crate::passes::{Change, Pass};
use crate::segment::Clause;

pub struct Compiler {
    pub lex: Lexicon,
    pub grammar: Grammar,
    /// 每个词类别对应的终结符。
    term_of: Vec<(&'static str, Sym)>,
    /// 套话模板字面部分的终结符：(字面文字 → 终结符)。
    lit_term: HashMap<String, Sym>,
    /// 每条套话模板（`lex.patterns` 的下标）对应的产生式。
    pattern_prods: Vec<usize>,
}

#[derive(Clone, Debug)]
pub struct ClauseResult {
    pub seg: Clause,
    /// 送进句式分析的终结符序列。
    pub syms: Vec<Sym>,
    pub parse: earley::Parse,
}

#[derive(Clone, Debug)]
pub struct PassRun {
    pub pass: Pass,
    pub changes: Vec<Change>,
    /// 这个 Pass 之后的整段输出。
    pub output: String,
}

#[derive(Clone, Debug, Default)]
pub struct Stats {
    /// 原文、译文的字数（不算标点和空白）。
    pub chars_in: usize,
    pub chars_out: usize,
    pub words: usize,
    pub jargon_words: usize,
    /// 句式没认出来、退回逐词处理的分句数。
    pub fallback: usize,
}

impl Stats {
    /// 水分：删掉的字占原文的比例（译文更长时为 0）。
    pub fn water(&self) -> f32 {
        if self.chars_in == 0 { 0.0 } else { 1.0 - (self.chars_out as f32 / self.chars_in as f32).min(1.0) }
    }
}

#[derive(Clone, Debug)]
pub struct Compilation {
    pub clauses: Vec<ClauseResult>,
    /// 优化前的 IR。
    pub ir0: Vec<ClauseIr>,
    /// 优化后的 IR。
    pub irs: Vec<ClauseIr>,
    /// 优化前的 IR 生成的文字（应该和原文一致）。
    pub unoptimized: String,
    pub passes: Vec<PassRun>,
    pub output: String,
    pub diags: Vec<Diagnostic>,
    pub stats: Stats,
    /// 输出里残留的黑话。
    pub residual: Vec<String>,
}

impl Compiler {
    pub fn new(layers: Vec<Layer>) -> Compiler {
        let lex = Lexicon::build(layers);
        let grammar = Grammar::parse(&grammar::clause_grammar(&lex)).expect("句式文法写错了");
        let term_of = ["修饰", "动", "名"]
            .into_iter()
            .chain(lexicon::Func::ALL.iter().map(|f| f.name()))
            .map(|name| (name, grammar.symbol(name).filter(|&s| grammar.is_terminal(s)).expect(name)))
            .collect();
        let mut lit_term = HashMap::new();
        for pat in &lex.patterns {
            for piece in &pat.src {
                if let lexicon::Piece::Lit(l) = piece
                    && let Some(sym) = grammar.symbol(&grammar::lit_terminal(l))
                {
                    lit_term.insert(l.clone(), sym);
                }
            }
        }
        // 套话的产生式按模板的顺序排在一起
        let pattern_prods =
            (0..grammar.productions.len()).filter(|&p| grammar.name(grammar.productions[p].lhs) == "套话").collect();
        Compiler { lex, grammar, term_of, lit_term, pattern_prods }
    }

    /// 内置词典。
    pub fn builtin() -> Compiler {
        Compiler::new(builtin_layers())
    }

    /// 内置词典再加上若干层（本地词典文件夹里的批次、用户粘贴的一批新词条……）。
    pub fn with_layers(extra: Vec<Layer>) -> Compiler {
        let mut layers = builtin_layers();
        layers.extend(extra.into_iter().filter(|l| !l.text.trim().is_empty()));
        Compiler::new(layers)
    }

    /// 内置词典再加上一层（比如用户粘贴的一批新词条）。
    pub fn with_extra(name: &str, text: &str) -> Compiler {
        let mut layers = builtin_layers();
        if !text.trim().is_empty() {
            layers.push(Layer { name: name.to_owned(), text: text.to_owned() });
        }
        Compiler::new(layers)
    }

    fn sym(&self, name: &str) -> Sym {
        self.term_of.iter().find(|(n, _)| *n == name).unwrap().1
    }

    pub fn compile(&self, input: &str) -> Compilation {
        let lex = &self.lex;
        let g = &self.grammar;
        let mut clauses = Vec::new();
        let mut ir0 = Vec::new();
        let mut diags = Vec::new();
        let mut stats = Stats::default();
        for seg in segment::split(input, lex) {
            let syms: Vec<Sym> = seg.tokens.iter().map(|t| self.sym(t.cat(lex).terminal())).collect();
            // 每个词的候选终结符：类别，以及（如果它是某个套话模板的字面部分）模板的终结符
            let opts: Vec<Vec<Sym>> = seg
                .tokens
                .iter()
                .zip(&syms)
                .map(|(t, &s)| std::iter::once(s).chain(self.lit_term.get(&t.text).copied()).collect())
                .collect();
            let parse = earley::parse(g, &opts, &|p| grammar::cost(g, p));
            let words = ir::words_of(&seg.tokens, lex);
            for w in &words {
                stats.words += 1;
                if w.cat.is_jargon() {
                    stats.jargon_words += 1;
                }
            }
            let mut ir = match &parse.tree {
                _ if words.is_empty() => ir::fallback(words, seg.punct.clone(), seg.ends_sentence),
                Some(tree) => ir::lower_tree(g, tree, words, seg.punct.clone(), seg.ends_sentence, &self.pattern_prods),
                None => {
                    stats.fallback += 1;
                    diags.push(self.parse_error(&seg, &syms, parse.fail_at.unwrap_or(0)));
                    ir::fallback(words, seg.punct.clone(), seg.ends_sentence)
                }
            };
            ir.fallback &= !seg.tokens.is_empty();
            ir.marker = seg.marker.clone();
            ir0.push(ir);
            clauses.push(ClauseResult { seg, syms, parse });
        }
        let unoptimized = emit::render(&ir0);
        let mut irs = ir0.clone();
        let mut passes = Vec::new();
        for pass in Pass::ALL {
            let changes = pass.run(&mut irs, lex);
            passes.push(PassRun { pass, changes, output: emit::render(&irs) });
        }
        let output = emit::render(&irs);
        stats.chars_in = count_chars(input);
        stats.chars_out = count_chars(&output);

        // 自检：把输出再分一次词，不应该还有黑话
        let mut residual = Vec::new();
        for seg in segment::split(&output, lex) {
            for t in &seg.tokens {
                if t.cat(lex).is_jargon() && !residual.contains(&t.text) {
                    residual.push(t.text.clone());
                }
            }
        }
        if !residual.is_empty() {
            diags.push(
                Diagnostic::warning(format!("译文里还有黑话：「{}」", residual.join("」「")))
                    .with_code("W3102")
                    .with_note("通常是因为词典里某个词的映射本身还是黑话，或者词条缺了映射")
                    .with_help("看看词典校验里的 W3005 / W3008 / E3007，把相应词条的映射改成朴素说法"),
            );
        }
        Compilation { clauses, ir0, irs, unoptimized, passes, output, diags, stats, residual }
    }

    fn parse_error(&self, seg: &Clause, syms: &[Sym], at: usize) -> Diagnostic {
        let words: Vec<String> =
            seg.tokens.iter().zip(syms).map(|(t, &s)| format!("{}〔{}〕", t.text, self.grammar.name(s))).collect();
        let (span, label) = match seg.tokens.get(at) {
            Some(t) => (t.span, "从这里开始接不上任何句式"),
            None => {
                let end = seg.span.end;
                (seg.tokens.last().map_or(Span::new(end, end), |t| t.span), "说到这里，句子还没说完")
            }
        };
        Diagnostic::warning("这个分句的句式没认出来，只做了词语级的处理")
            .with_code("W3101")
            .with_primary(span, label)
            .with_note(format!("分词和类别：{}", words.join(" ")))
            .with_note(format!("支持的句式：{}", grammar::PATTERNS))
            .with_help("修饰语照样删、固定搭配照样换、黑话词照样降级，但不做句式改写和排比合并。在词典里补上没认出来的词，常常就能认出句式")
    }
}

impl Compilation {
    /// 译文 + 每处改动的说明（保存成 txt、命令行 `run` 用）。
    pub fn report(&self) -> String {
        let mut s = self.output.clone();
        s.push_str(&format!(
            "\n\n———— 黑话编译器报告 ————\n原文 {} 字 → 译文 {} 字，水分 {:.0}%\n",
            self.stats.chars_in,
            self.stats.chars_out,
            self.stats.water() * 100.0
        ));
        for run in &self.passes {
            for ch in &run.changes {
                let after = if ch.after.is_empty() { "（删掉）" } else { &ch.after };
                s.push_str(&format!("[{}] {} → {}：{}\n", run.pass.name(), ch.before, after, ch.why));
            }
        }
        for d in &self.diags {
            s.push_str(&format!("[提醒] {}\n", d.message));
        }
        s
    }
}

impl Compiler {
    /// 检查一段输入是否满足编译器的不变式，返回违反的条目（空 = 全部满足）：
    /// - 没优化的 IR 生成回来就是原文（前端没丢东西）；
    /// - 输出里没有残留黑话；
    /// - 未收录的普通词（讲话的实际内容）一个不少；
    /// - 编译是幂等的：把输出再编译一遍，结果不变。
    pub fn invariant_violations(&self, text: &str) -> Vec<String> {
        let r = self.compile(text);
        let mut out = Vec::new();
        let squeeze = |s: &str| s.chars().filter(|c| !c.is_whitespace()).collect::<String>();
        if squeeze(&r.unoptimized) != squeeze(text) {
            out.push(format!("未优化的 IR 没有还原原文：{}", r.unoptimized));
        }
        if !r.residual.is_empty() {
            out.push(format!("译文里还有黑话：「{}」", r.residual.join("」「")));
        }
        for cl in &r.clauses {
            for t in cl.seg.tokens.iter().filter(|t| t.entry.is_none()) {
                if !r.output.contains(&t.text) {
                    out.push(format!("丢了未收录的内容词「{}」", t.text));
                }
            }
        }
        let again = self.compile(&r.output).output;
        if again != r.output {
            out.push(format!("不幂等：再编译一遍变成了「{again}」"));
        }
        out
    }
}

/// 回归对比：同样的输入，在 `base` 和 `comp` 两套词典下译文不同的那些，返回 (名字, 原来的译文, 现在的译文)。
pub fn regression(base: &Compiler, comp: &Compiler, inputs: &[(String, String)]) -> Vec<(String, String, String)> {
    inputs
        .iter()
        .filter_map(|(name, text)| {
            let (a, b) = (base.compile(text).output, comp.compile(text).output);
            (a != b).then(|| (name.clone(), a, b))
        })
        .collect()
}

fn count_chars(s: &str) -> usize {
    s.chars().filter(|c| c.is_alphanumeric()).count()
}

/// 让 AI 批量生成词条时用的提示词。`words` 是（词, 例句）。
pub fn ai_prompt(words: &[(String, String)]) -> String {
    let mut s = String::from(
        "你是中文\"官话、套话、互联网黑话\"的词典编纂者。请为下面的词语各写一行词条，只输出词条，不要解释。\n\
         \n\
         格式：词 | 类别 | 映射 | 备注（备注可省略）\n\
         类别只能是下面之一：\n\
         - 修饰：不携带信息的膨胀修饰语（全面、深度、高质量），不写映射\n\
         - 空动：空洞动词，映射写更朴素的近义说法（赋能 | 空动 | 帮助）\n\
         - 空转：和虚指名词搭配时没有信息的动词（形成、实现），映射写朴素动词\n\
         - 虚名：虚指名词，映射写朴素说法（抓手 | 虚名 | 入手的地方）\n\
         - 固定 / 固定动：名词性 / 动词性的固定搭配，映射写整个搭配的大白话（降本增效 | 固定 | 省钱又提效）\n\
         - 实动：本来就朴素的动词\n\
         - 内容：普通词，不是黑话\n\
         - 缩略：压缩过的说法，背后有具体内容，映射写展开后的完整说法（放管服 | 缩略 | 简化审批、加强监管、改进服务）\n\
         - 模板：带槽位的套话句式，{X}、{Y} 是槽位（坚持{X}不动摇 | 模板 | 一直坚持{X}）。映射里每个槽位正好用一次；\
           字面部分写两个字以上的词，不要用单个字\n\
         要求：\n\
         - 先看例句判断这个词在这里是不是空话；例句里它表达实在意思的（具体的事、具体的对象），写成\"内容\"类。\n\
         - 有歧义、拿不准的词写成\"内容\"，不要猜。\n\
         - 映射优先写上位词（更朴素的近义说法），不要写整句解释；映射里不能再含黑话。\n\
         - 映射必须是完全朴素的说法；不要写单个字的词条。\n\
         - 只输出词条行，不要把例句抄进输出。\n",
    );
    if !words.is_empty() {
        s.push_str("\n词语：\n");
        for (w, ex) in words {
            s.push_str(w);
            if !ex.is_empty() {
                s.push_str("\t〔例句：");
                s.push_str(ex);
                s.push('〕');
            }
            s.push('\n');
        }
    }
    s
}

/// 界面上的示例讲话：(名字, 内容)。
pub const PRESETS: &[(&str, &str)] = &[
    ("数字化转型", "我们要以数字化转型为抓手，全面打通业务壁垒，形成全链路闭环，赋能业务高质量发展。"),
    (
        "年度动员",
        "各部门要紧紧围绕年度目标，统筹推进各项工作，切实把责任压实，确保落地见效。\
         要加强队伍建设，强化作风建设，提升工作能力，打好组合拳。",
    ),
    (
        "中台战略",
        "通过打造数智化中台，沉淀方法论，形成合力，进而实现降本增效。\
         要以用户为中心，打造极致体验，形成用户心智，构建护城河。",
    ),
    (
        "开创新局面",
        "我们要坚定不移地深化改革，开创高质量发展新局面，谱写新篇章。全体员工要真抓实干，奋力开创工作新局面。",
    ),
    ("对齐颗粒度", "对齐颗粒度，拉通对齐，沉淀可复用的方法论。这个方案的底层逻辑是什么，抓手在哪里，颗粒度够不够细。"),
    ("排比与重复", "我们要提高质量和效率，提质增效。要以改革为引擎，激活发展新动能，补齐短板，夯实基础。"),
    (
        "一份通知",
        "关于推进数字化转型工作的通知\n各部门：\n为深入贯彻落实公司战略部署，全面推进数字化转型，现就有关工作通知如下。\n\
         一是提高政治站位。各部门要充分认识数字化转型的重要意义，切实增强责任感和紧迫感。\n\
         二是强化统筹协调。要建立健全工作机制，形成上下联动、协同推进的工作格局。\n\
         三是狠抓工作落实。要以钉钉子精神抓好各项任务，确保取得实效。",
    ),
    ("句式之外", "和兄弟单位一起，共建共享。在新的历史起点上，我们要守正创新，勇毅前行。"),
    ("大白话", "昨天下雨了，我没带伞。"),
    ("套话与缩略", "我们要坚持改革开放不动摇，把各项任务落到实处，在质量上下功夫。要守住安全底线，抓好三农工作。"),
];

/// 界面上"扩充词典"的示例：一批 AI 生成的词条，故意混进了词典膨胀时常见的各种问题。
pub const EXAMPLE_BATCH: &str = "\
# AI 扩充第 002 批（示例：好词条和各种有问题的词条混在一起）
勇毅前行 | 固定动 | 大胆往前走
历史起点 | 虚名 | 时候
数据孤岛 | 虚名 | 互不相通的数据
问题导向 | 固定 | 盯着问题
兄弟单位 | 内容
# 和核心词典冲突：保留核心词典的定义
赋能 | 空动 | 支持
# 和第 1 批重复（定义相同）
护城河 | 虚名 | 优势
# 单字词条：会把普通词切碎
化 | 修饰
# 功能词只能在核心词典里定义
基于 | 通过
# 类别写错了
顶层 | 高大上
# 这一行格式不对
这一行没有竖线
# 两个词互相映射，绕成了圈
心智模型 | 虚名 | 认知框架
认知框架 | 虚名 | 心智模型
# 映射到的说法里还有黑话
颠覆式创新 | 固定 | 全面创新
破壁 | 空动 | 打破壁垒
# 空洞词没写映射
矩阵式打法 | 虚名
# 映射到了自己
拉齐 | 空动 | 拉齐
# 链太长
超级抓手 | 虚名 | 核心抓手
核心抓手 | 虚名 | 关键抓手
# 缩略语和套话模板
一网通办 | 缩略 | 在一个网站上办完各种手续
推动{X}落地生根 | 模板 | 让{X}扎下根来
# 模板的映射把槽位丢了（槽位里是实际内容）
坚持{X}不放松 | 模板 | 一直坚持
# 槽位只能写 {X}、{Y}
以{内容}为纲 | 模板 | 抓住{内容}
# 模板的字面部分有没收录的单字
向{X}要效益 | 模板 | 从{X}里找效益
关键抓手 | 虚名 | 抓手
# 映射比原词长太多，应该放进备注
内卷 | 虚名 | 大家都在拼命但是谁也没有占到便宜
";

#[cfg(test)]
mod tests {
    use super::*;

    /// 端到端对照：典型的讲话 → 期望的大白话。
    #[test]
    fn golden() {
        let cases = [
            (0, "我们要从数字化转型入手，打通业务隔阂，帮助业务发展。"),
            (
                1,
                "各部门要围绕年度目标，推动工作，把责任落实，确保做出效果。要加强队伍建设、作风建设，提高工作能力，几个办法一起用。",
            ),
            (3, "我们要加深改革，让发展有新进展。全体员工要认真干，让工作有新进展。"),
            (5, "我们要提高质量和效率。要靠改革推动，调动发展新动力，把弱项补上，打牢基础。"),
        ];
        let c = Compiler::builtin();
        assert_eq!(c.lex.errors(), 0, "{:?}", c.lex.diags);
        for (i, want) in cases {
            let r = c.compile(PRESETS[i].1);
            assert_eq!(r.output, want);
        }
        // 分词代价相同时不把普通词切碎（提|高效|益 → "提益"）
        assert_eq!(c.compile("要提高效益，提高效果。").output, "要提高效益、效果。");
    }

    /// 对所有示例和参考译文集的原文检查不变式（见 `Compiler::invariant_violations`）。
    #[test]
    fn invariants() {
        let c = Compiler::builtin();
        let check = |c: &Compiler, name: &str, text: &str| {
            let v = c.invariant_violations(text);
            assert!(v.is_empty(), "{name}: {v:?}");
        };
        for (name, text) in PRESETS {
            check(&c, name, text);
        }
        // 句式改写不能吃掉触发词后面的内容
        check(&c, "抓手之后", "我们要以数字化转型为抓手进行全网营销。");
        // 参考译文集的每条原文也跑一遍同样的不变式
        for r in corpus::parse_references(corpus::BUILTIN_REFERENCES) {
            check(&c, &format!("参考:{}", r.name), &r.source);
        }
        // 示例批次：错误都被拦下，词典仍然可用
        let ext = Compiler::with_extra("示例", EXAMPLE_BATCH);
        let codes: Vec<&str> = ext.lex.diags.iter().filter(|d| d.layer == 2).filter_map(|d| d.diag.code).collect();
        for code in
            ["E3001", "E3002", "E3003", "E3010", "E3007", "E3014", "E3015", "E3016", "W3004", "W3005", "W3008", "W3013"]
                .into_iter()
                .chain(["N3004", "N3009", "N3011"])
        {
            assert!(codes.contains(&code), "{code} 没有报出来：{codes:?}");
        }
        let outside = PRESETS.iter().find(|(n, _)| *n == "句式之外").unwrap().1;
        assert!(ext.compile(outside).output.contains("大胆往前走"));
        assert!(
            ext.compile("要推动改革落地生根。").output.contains("让改革扎下根来"),
            "{}",
            ext.compile("要推动改革落地生根。").output
        );
    }

    /// 套话模板和缩略语：模板整体换掉、槽位里的内容保留；缩略语展开。
    #[test]
    fn patterns_and_abbrevs() {
        let c = Compiler::builtin();
        let text = PRESETS.iter().find(|(n, _)| *n == "套话与缩略").unwrap().1;
        let out = c.compile(text).output;
        for want in ["一直坚持改革开放", "落实", "任务", "花力气抓质量", "不出安全问题", "农业、农村、农民"]
        {
            assert!(out.contains(want), "缺「{want}」：{out}");
        }
        for gone in ["不动摇", "落到实处", "上下功夫", "底线", "三农"] {
            assert!(!out.contains(gone), "还有「{gone}」：{out}");
        }
    }
}
