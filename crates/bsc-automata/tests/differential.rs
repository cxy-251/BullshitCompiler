//! 差分测试：随机生成正则和字符串，四种判断方法必须给出相同的答案——
//! 语法树上的参考匹配器、NFA 模拟、子集构造出的 DFA、最小 DFA。

use bsc_automata::dfa::subset_construction;
use bsc_automata::minimize::minimize;
use bsc_automata::nfa::thompson;
use bsc_automata::regex::Regex;
use bsc_automata::scanner::{Lexer, Rule};

struct Lcg(u64);

impl Lcg {
    fn next(&mut self, n: u64) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 33) % n
    }
}

fn random_regex(rng: &mut Lcg, depth: u32) -> String {
    if depth == 0 || rng.next(3) == 0 {
        return match rng.next(6) {
            0 => "[ab]".to_owned(),
            1 => ".".to_owned(),
            2 => "()".to_owned(),
            n => ["a", "b", "c"][(n - 3) as usize].to_owned(),
        };
    }
    let a = random_regex(rng, depth - 1);
    match rng.next(6) {
        0 => format!("({a})*"),
        1 => format!("({a})+"),
        2 => format!("({a})?"),
        3 => format!("({a}|{})", random_regex(rng, depth - 1)),
        _ => format!("{a}{}", random_regex(rng, depth - 1)),
    }
}

fn random_string(rng: &mut Lcg) -> String {
    let len = rng.next(7);
    (0..len).map(|_| ['a', 'b', 'c'][rng.next(3) as usize]).collect()
}

#[test]
fn nfa_dfa_and_min_dfa_agree_with_reference() {
    let mut rng = Lcg(0xC0FFEE);
    for _ in 0..600 {
        let src = random_regex(&mut rng, 4);
        let re = Regex::parse(&src).unwrap_or_else(|d| panic!("{src}: {d:?}"));
        let nfa = thompson(&re).nfa;
        let (dfa, _) = subset_construction(&nfa);
        let min = minimize(&dfa);
        assert!(min.dfa.states.len() <= dfa.states.len(), "{src}");
        // 最小 DFA 再最小化一次，状态数不应再变少。
        assert_eq!(minimize(&min.dfa).dfa.states.len(), min.dfa.states.len(), "{src}");

        for _ in 0..25 {
            let s = random_string(&mut rng);
            let want = re.matches(&s);
            assert_eq!(nfa.accepts_str(&s), want, "NFA: /{src}/ on {s:?}");
            assert_eq!(dfa.accepts_str(&s), want, "DFA: /{src}/ on {s:?}");
            assert_eq!(min.dfa.accepts_str(&s), want, "最小 DFA: /{src}/ on {s:?}");
        }
    }
}

/// 暴力参考：在每个位置，从最长的前缀往短里试，第一个能被某条规则匹配的前缀就是记号，
/// 同样长时取编号最小的规则。
fn reference_scan(rules: &[Regex], input: &str) -> Option<Vec<(usize, String)>> {
    let chars: Vec<char> = input.chars().collect();
    let mut out = Vec::new();
    let mut pos = 0;
    while pos < chars.len() {
        let (len, rule) = (1..=chars.len() - pos).rev().find_map(|len| {
            let piece: String = chars[pos..pos + len].iter().collect();
            rules.iter().position(|r| r.matches(&piece)).map(|r| (len, r))
        })?;
        out.push((rule, chars[pos..pos + len].iter().collect()));
        pos += len;
    }
    Some(out)
}

#[test]
fn generated_lexer_agrees_with_brute_force() {
    let mut rng = Lcg(0xBEEF);
    let mut checked = 0;
    while checked < 200 {
        let n = 1 + rng.next(3) as usize;
        let patterns: Vec<String> = (0..n).map(|_| random_regex(&mut rng, 3)).collect();
        let rules: Vec<Rule> = patterns.iter().enumerate().map(|(i, p)| Rule::new(&format!("R{i}"), p)).collect();
        let Ok(lexer) = Lexer::build(rules) else { continue }; // 能匹配空串的规则会被拒绝
        checked += 1;
        for _ in 0..10 {
            let s = random_string(&mut rng);
            let scan = lexer.scan(&s);
            let got = scan
                .error
                .is_none()
                .then(|| scan.tokens.iter().map(|t| (t.rule, t.span.text(&s).to_owned())).collect::<Vec<_>>());
            assert_eq!(got, reference_scan(&lexer.regexes, &s), "rules {patterns:?} on {s:?}");
        }
    }
}
