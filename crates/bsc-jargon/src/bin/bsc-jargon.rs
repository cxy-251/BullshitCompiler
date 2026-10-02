//! 黑话编译器命令行工具：和网页用同一套编译器代码，读写普通文本文件，方便 AI 助手（agy）全程自己扩充词典。
//!
//! ```text
//! bsc-jargon todo  文章.txt… [-l 批次.txt]… [-n 80]   待办词表 + 给 AI 填词条的提示词
//! bsc-jargon check 批次.txt [文章.txt…] [-l 批次.txt]… 校验诊断、回归对比、参考译文对照、不变式
//! bsc-jargon run   文章.txt [-l 批次.txt]…            译文 + 每处改动的理由
//! ```
//!
//! 文件名写 `-` 表示从标准输入读。`check` 发现错误（E 开头的诊断、违反不变式）时退出码为 1。

use std::io::Read;
use std::process::ExitCode;

use bsc_core::Severity;
use bsc_jargon::corpus::{self, Kind};
use bsc_jargon::lexicon::{Layer, builtin_layers};
use bsc_jargon::{Compiler, PRESETS, ai_prompt, regression};

const USAGE: &str = "\
黑话编译器命令行工具

用法：
  bsc-jargon todo  文章.txt… [-l 批次.txt]… [-n 数量]
      找出文章里编译器不认识的词（按出现次数排序，附原文例句），后面跟着给 AI 填词条的提示词。
      -n 提示词里最多放多少个词（默认 80）。
  bsc-jargon check 批次.txt [文章.txt…] [-l 批次.txt]…
      把一批新词条加进词典，报告：词典校验、回归对比（示例讲话、参考译文、文章各段的译文变化）、
      参考译文对照、不变式检查、文章里还剩多少不认识的词。有错误时退出码为 1。
  bsc-jargon run   文章.txt [-l 批次.txt]…
      输出译文，以及每处改动的理由。

  -l 批次.txt   在内置词典后面再加一层（可以写多次，按顺序叠加）。
  文件名写 - 表示从标准输入读。
";

struct Args {
    cmd: String,
    files: Vec<String>,
    layers: Vec<String>,
    limit: usize,
}

fn parse_args() -> Result<Args, String> {
    let mut it = std::env::args().skip(1);
    let cmd = it.next().ok_or("缺少子命令")?;
    let mut args = Args { cmd, files: vec![], layers: vec![], limit: 80 };
    while let Some(a) = it.next() {
        match a.as_str() {
            "-l" | "--lexicon" => args.layers.push(it.next().ok_or("-l 后面要跟词典文件")?),
            "-n" => args.limit = it.next().and_then(|n| n.parse().ok()).ok_or("-n 后面要跟一个数字")?,
            "-h" | "--help" => return Err(String::new()),
            _ => args.files.push(a),
        }
    }
    Ok(args)
}

fn read(path: &str) -> Result<String, String> {
    let text = if path == "-" {
        let mut s = String::new();
        std::io::stdin().read_to_string(&mut s).map_err(|e| format!("读不了标准输入：{e}"))?;
        s
    } else {
        std::fs::read_to_string(path).map_err(|e| format!("读不了 {path}：{e}"))?
    };
    Ok(text.replace("\r\n", "\n"))
}

fn layer(path: &str) -> Result<Layer, String> {
    Ok(Layer { name: path.to_owned(), text: read(path)? })
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            if !e.is_empty() {
                eprintln!("错误：{e}\n");
            }
            eprint!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    let result = match args.cmd.as_str() {
        "todo" => todo(&args),
        "check" => check(&args),
        "run" => run(&args),
        other => Err(format!("不认识的子命令「{other}」")),
    };
    match result {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("错误：{e}");
            ExitCode::from(2)
        }
    }
}

fn extra_layers(args: &Args) -> Result<Vec<Layer>, String> {
    args.layers.iter().map(|p| layer(p)).collect()
}

/// 几篇文章连成一份语料。
fn corpus_text(paths: &[String]) -> Result<String, String> {
    let texts: Vec<String> = paths.iter().map(|p| read(p)).collect::<Result<_, _>>()?;
    Ok(texts.join("\n"))
}

fn todo(args: &Args) -> Result<bool, String> {
    if args.files.is_empty() {
        return Err("todo 需要至少一篇文章".to_owned());
    }
    let comp = Compiler::with_layers(extra_layers(args)?);
    let text = corpus_text(&args.files)?;
    let cands = corpus::analyze(&comp.lex, &text);
    println!("# 待办词表：{} 个不认识的词（次数 · 类型 · 词 · 例句）", cands.len());
    for c in &cands {
        let kind = match c.kind {
            Kind::Unknown => "未收录",
            Kind::Phrase => "拼合",
        };
        println!("{:>3}\t{kind}\t{}\t{}", c.count, c.text, c.example.replace('\n', " "));
    }
    let words: Vec<(String, String)> =
        cands.iter().take(args.limit).map(|c| (c.text.clone(), c.example.replace('\n', " "))).collect();
    println!("\n# ———————— 给 AI 的提示词（前 {} 个词） ————————\n", words.len());
    print!("{}", ai_prompt(&words));
    Ok(true)
}

fn run(args: &Args) -> Result<bool, String> {
    let [path] = args.files.as_slice() else { return Err("run 只接受一篇文章".to_owned()) };
    let comp = Compiler::with_layers(extra_layers(args)?);
    println!("{}", comp.compile(&read(path)?).report());
    Ok(true)
}

fn check(args: &Args) -> Result<bool, String> {
    let Some((batch_path, articles)) = args.files.split_first() else {
        return Err("check 需要一个批次文件".to_owned());
    };
    let extra = extra_layers(args)?;
    let batch = layer(batch_path)?;
    let mut layers = builtin_layers();
    layers.extend(extra);
    let base = Compiler::new(layers.clone());
    let batch_index = layers.len();
    layers.push(batch.clone());
    let comp = Compiler::new(layers);
    let mut ok = true;

    // 1. 词典校验
    let diags: Vec<_> = comp.lex.diags.iter().filter(|d| d.layer == batch_index).collect();
    let count = |s: Severity| diags.iter().filter(|d| d.diag.severity == s).count();
    let (errors, warnings, notes) = (count(Severity::Error), count(Severity::Warning), count(Severity::Note));
    println!(
        "# 1. 词典校验：这一批收录 {} 条；错误 {errors}，警告 {warnings}，提示 {notes}\n",
        comp.lex.count_by_layer(batch_index)
    );
    for d in &diags {
        println!("{}", d.diag.render(&batch.text, batch_path));
    }
    // 别的层因为这一批产生的新诊断（比如核心词典的映射现在成环了）
    let older: Vec<String> = comp
        .lex
        .diags
        .iter()
        .filter(|d| d.layer < batch_index)
        .map(|d| format!("{}: {}", comp.lex.layers[d.layer].name, d.diag.message))
        .filter(|m| {
            !base.lex.diags.iter().any(|d| format!("{}: {}", base.lex.layers[d.layer].name, d.diag.message) == *m)
        })
        .collect();
    for m in &older {
        println!("已有词典的新诊断 · {m}");
    }
    ok &= errors == 0;

    // 要重新编译的输入：示例讲话、参考译文的原文、文章的每一段
    let refs = corpus::parse_references(corpus::BUILTIN_REFERENCES);
    let mut inputs: Vec<(String, String)> =
        PRESETS.iter().map(|(n, t)| (format!("示例:{n}"), (*t).to_owned())).collect();
    inputs.extend(refs.iter().map(|r| (format!("参考:{}", r.name), r.source.clone())));
    let mut texts = Vec::new();
    for path in articles {
        let text = read(path)?;
        for (i, line) in text.lines().enumerate() {
            if !line.trim().is_empty() {
                inputs.push((format!("{path}:{}", i + 1), line.to_owned()));
            }
        }
        texts.push(text);
    }

    // 2. 回归对比
    let changed = regression(&base, &comp, &inputs);
    println!("\n# 2. 回归对比：{} 段输入里有 {} 段译文变了\n", inputs.len(), changed.len());
    for (name, before, after) in &changed {
        println!("## {name}\n  原来：{}\n  现在：{}\n", before.replace('\n', " / "), after.replace('\n', " / "));
    }

    // 3. 参考译文对照
    println!("# 3. 参考译文对照（{} 条）\n", refs.len());
    for r in &refs {
        let now = comp.compile(&r.source).output;
        let before = base.compile(&r.source).output;
        let mark = if now == before { "" } else { "（这一批改变了译文）" };
        println!("## {}{mark}\n  原文：{}", r.name, r.source);
        if now != before {
            println!("  原来：{before}");
        }
        println!("  现在：{now}\n  参考：{}\n", r.reference);
    }

    // 4. 不变式
    let violations: Vec<(String, Vec<String>)> = inputs
        .iter()
        .map(|(name, text)| (name.clone(), comp.invariant_violations(text)))
        .filter(|(_, v)| !v.is_empty())
        .collect();
    println!("# 4. 不变式：{} 段输入里有 {} 段不满足\n", inputs.len(), violations.len());
    for (name, v) in &violations {
        for m in v {
            println!("{name}: {m}");
        }
    }
    ok &= violations.is_empty();

    // 5. 文章里还剩多少不认识的词
    if !texts.is_empty() {
        let all = texts.join("\n");
        let (a, b) = (corpus::analyze(&base.lex, &all), corpus::analyze(&comp.lex, &all));
        let water = |c: &Compiler| c.compile(&all).stats.water() * 100.0;
        println!(
            "\n# 5. 文章：不认识的词 {} → {} 个；水分 {:.0}% → {:.0}%",
            a.len(),
            b.len(),
            water(&base),
            water(&comp)
        );
        let top: Vec<String> = b.iter().take(20).map(|c| format!("{}×{}", c.text, c.count)).collect();
        if !top.is_empty() {
            println!("还不认识的词（前 20）：{}", top.join("、"));
        }
    }

    println!("\n{}", if ok { "结论：通过" } else { "结论：有错误，见上面的「错误」和不变式" });
    Ok(ok)
}
