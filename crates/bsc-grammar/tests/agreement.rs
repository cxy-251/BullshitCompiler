//! 对照测试：同一个文法、同一串输入，三种完全不同的分析方法——Earley、LL(1) 表驱动、
//! SLR 移进-归约——必须对"是不是合法句子"给出相同的判断，并且得到同一棵语法树。

use bsc_grammar::derive::{generate, leftmost};
use bsc_grammar::first_follow::compute;
use bsc_grammar::lr::{Automaton, SlrTable};
use bsc_grammar::{Grammar, Sym, earley, ll1, lr};

/// 既是 LL(1) 又是 SLR(1) 的文法。
const GRAMMARS: &[&str] = &[
    "E -> T E'\nE' -> + T E' | ε\nT -> F T'\nT' -> * F T' | ε\nF -> ( E ) | id",
    "S -> a S b | c",
    "L -> [ Xs ]\nXs -> X Rest | ε\nRest -> , X Rest | ε\nX -> id | L",
];

#[test]
fn earley_ll1_and_slr_agree() {
    for src in GRAMMARS {
        let g = Grammar::parse(src).unwrap();
        let ll = ll1::Table::build(&g, &compute(&g));
        assert!(ll.conflicts().is_empty(), "{src} 应该是 LL(1) 文法");
        let auto = Automaton::build(&g);
        let slr = SlrTable::build(&auto);
        assert!(slr.conflicts().is_empty(), "{src} 应该是 SLR(1) 文法");
        let terms: Vec<Sym> = g.terminals().collect();

        for seed in 0..200u64 {
            let mut toks = generate(&g, seed, 12).expect("能生成句子");
            // 一半的输入随机改坏一个位置
            if seed % 2 == 1 && !toks.is_empty() {
                let i = (seed as usize * 7) % toks.len();
                toks[i] = terms[(seed as usize * 13) % terms.len()];
            }
            let e = earley::parse(&g, &toks, 2);
            let (_, ll_tree) = ll1::parse(&g, &ll, &toks);
            let (_, lr_tree) = lr::parse(&auto, &slr, &toks);
            assert_eq!(e.accepted, ll_tree.is_some(), "{src} / {}", g.seq_text(&toks));
            assert_eq!(e.accepted, lr_tree.is_some(), "{src} / {}", g.seq_text(&toks));
            if e.accepted {
                assert_eq!(e.trees.len(), 1, "无歧义文法只有一棵树");
                let want = e.trees[0].to_bracketed(&g);
                assert_eq!(ll_tree.unwrap().to_bracketed(&g), want);
                assert_eq!(lr_tree.unwrap().to_bracketed(&auto.g), want);
                // 最左推导的最后一步就是句子本身
                let last = leftmost(&e.trees[0]).pop().unwrap();
                assert_eq!(last.form.iter().map(|&(s, _)| s).collect::<Vec<_>>(), toks);
            }
        }
    }
}
