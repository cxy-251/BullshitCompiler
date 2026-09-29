use bsc_calc::{Compilation, Stage, codegen};

#[test]
fn compiles_and_runs() {
    let cases = [
        ("1 + 2 * 3", 7),
        ("(1 + 2) * 3", 9),
        ("8 - 3 - 2", 3),
        ("-(2 + 3) * 4", -20),
        ("7 / 2", 3),
        ("-7 / 2", -3),
        ("  42  ", 42),
    ];
    for (src, want) in cases {
        let c = Compilation::new(src);
        assert_eq!(c.value(), Some(want), "{src}: {:?}", c.error());
    }
}

#[test]
fn bytecode_is_postorder() {
    let c = Compilation::new("1 + 2 * 3");
    assert_eq!(codegen::listing(c.code.as_ref().unwrap()), "push 1\npush 2\npush 3\nmul\nadd\n");
    let steps: Vec<_> = c.exec.unwrap().steps.into_iter().map(|s| s.stack).collect();
    assert_eq!(steps, vec![vec![1], vec![1, 2], vec![1, 2, 3], vec![1, 6], vec![7]]);
}

#[test]
fn errors_come_from_the_right_stage() {
    let stage = |src: &str| Compilation::new(src).error().map(|(s, d)| (s, d.code.unwrap()));
    assert_eq!(stage("1 + a"), Some((Stage::Lex, "E0101")));
    assert_eq!(stage("1 + "), Some((Stage::Parse, "E0201")));
    assert_eq!(stage("1 / (2 - 2)"), Some((Stage::Run, "E0301")));
    assert_eq!(stage("9223372036854775807 + 1"), Some((Stage::Run, "E0302")));
    assert_eq!(stage("1 + 1"), None);
}

#[test]
fn division_by_zero_diagnostic() {
    let src = "10 / (3 - 3)";
    let c = Compilation::new(src);
    let (_, d) = c.error().unwrap();
    let expected = "\
错误[E0301]: 除以零
 --> 输入:1:1
  |
1 | 10 / (3 - 3)
  | ^^^^^^^^^^^^ 这次除法的除数是 0
  |      ------- 它算出来是 0
  |
  = 注: 这是运行时错误：语法完全正确，要真正算到这一步才会发现问题
";
    assert_eq!(d.render(src, "输入"), expected);
}

/// 差分测试：随机生成表达式，"编译 + 虚拟机执行" 必须和 "直接在树上求值" 结果一致。
#[test]
fn vm_agrees_with_tree_interpreter() {
    let mut rng = Lcg(0x5eed);
    for _ in 0..2000 {
        let src = random_expr(&mut rng, 4);
        let c = Compilation::new(&src);
        let parse = c.parse.as_ref().expect("随机表达式总能通过词法分析");
        let root = *parse.root.as_ref().unwrap_or_else(|d| panic!("{src}: {d:?}"));
        assert_eq!(c.value(), parse.ast.eval(root), "{src}");
    }
}

struct Lcg(u64);

impl Lcg {
    fn next(&mut self, n: u64) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 33) % n
    }
}

fn random_expr(rng: &mut Lcg, depth: u32) -> String {
    if depth == 0 || rng.next(4) == 0 {
        return rng.next(100).to_string();
    }
    match rng.next(6) {
        0 => format!("-{}", random_expr(rng, depth - 1)),
        1 => format!("({})", random_expr(rng, depth - 1)),
        _ => {
            let op = ["+", "-", "*", "/"][rng.next(4) as usize];
            format!("{} {op} {}", random_expr(rng, depth - 1), random_expr(rng, depth - 1))
        }
    }
}
