//! 1.2 正则表达式。

use bsc_automata::regex::RegexId;
use eframe::egui::{self, RichText, TextEdit, Ui};

use crate::automata_view::{RegexPipeline, regex_input, regex_tree};
use crate::theme::{Palette, mono};
use crate::widgets::{CalloutKind, Mark, NodeStyle, callout, card, prose, quiz, scrollable_tree, source_view};

const PRESETS: &[(&str, &str)] = &[
    ("[a-z_][a-z0-9_]*", "标识符"),
    ("[0-9]+", "整数"),
    ("-?[0-9]+(\\.[0-9]+)?", "小数"),
    ("(a|b)*abb", "龙书经典例子"),
    ("0x[0-9a-f]+", "十六进制"),
    ("ab|c", "优先级"),
];

pub struct Lesson {
    src: String,
    pipe: RegexPipeline,
    tests: String,
}

impl Default for Lesson {
    fn default() -> Self {
        let src = PRESETS[0].0.to_owned();
        Self { pipe: RegexPipeline::new(&src), src, tests: "count\n_tmp1\n9lives\nhello_world\nHello".to_owned() }
    }
}

impl Lesson {
    pub fn ui(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        ui.heading("1.2  正则表达式");
        ui.label(RichText::new("入门 · 约 20 分钟").color(p.muted));

        ui.add_space(8.0);
        ui.label(RichText::new("一句话").size(20.0).strong());
        ui.label(
            "词法分析器要知道每种记号\"长什么样\"：整数是一串数字，标识符是字母开头、后面跟字母或数字……\
             正则表达式就是专门用来描述这种\"形状\"的一门小语言。",
        );
        callout(ui, CalloutKind::Analogy, "寻人启事", |ui| {
            prose(
                ui,
                "寻人启事不会列出世界上所有符合条件的人，而是写\"男，身高 170 到 180，戴眼镜\"——\
                 用几条特征描述了无穷多个人。正则表达式也一样：`[0-9]+` 这短短 6 个字符，\
                 描述了所有的整数。",
            );
        });

        ui.add_space(8.0);
        ui.label(RichText::new("只需要三种运算").size(20.0).strong());
        ui.label("任何正则表达式都是由单个字符，用下面三种方式组合起来的：");
        card(ui, |ui| {
            egui::Grid::new("regex_core_ops").num_columns(4).striped(true).spacing([18.0, 8.0]).show(ui, |ui| {
                for h in ["运算", "写法", "意思", "例子"] {
                    ui.label(RichText::new(h).strong());
                }
                ui.end_row();
                for (name, syntax, meaning, example) in [
                    ("连接", "ab", "先 a 后 b", "`ab` 只匹配 ab"),
                    ("选择", "a|b", "a 或者 b", "`cat|dog` 匹配 cat 或 dog"),
                    ("重复", "a*", "a 出现 0 次或多次", "`ha*` 匹配 h、ha、haaa……"),
                ] {
                    ui.label(RichText::new(name).color(p.accent).strong());
                    ui.label(RichText::new(syntax).font(mono(16.0)));
                    ui.label(meaning);
                    prose(ui, example);
                    ui.end_row();
                }
            });
        });
        ui.label("其余的写法都只是简写，方便而已：");
        card(ui, |ui| {
            egui::Grid::new("regex_sugar").num_columns(3).striped(true).spacing([18.0, 8.0]).show(ui, |ui| {
                for h in ["写法", "意思", "等价于"] {
                    ui.label(RichText::new(h).strong());
                }
                ui.end_row();
                for (syntax, meaning, same) in [
                    ("a+", "至少出现 1 次", "aa*"),
                    ("a?", "可有可无", "a|ε（ε 表示空串）"),
                    ("[abc]", "其中任意一个字符", "a|b|c"),
                    ("[a-z]", "a 到 z 之间的任意字符", "a|b|c|…|z"),
                    ("[^0-9]", "除了数字以外的任意字符", ""),
                    (".", "除换行以外的任意字符", ""),
                    ("\\d \\w \\s", "数字 / 字母数字下划线 / 空白", "[0-9] / [a-zA-Z0-9_] / 空格制表换行"),
                    ("\\* \\. \\(", "符号本身（转义）", ""),
                ] {
                    ui.label(RichText::new(syntax).font(mono(16.0)));
                    ui.label(meaning);
                    ui.label(RichText::new(same).font(mono(14.0)).color(p.muted));
                    ui.end_row();
                }
            });
        });

        ui.add_space(8.0);
        ui.label(RichText::new("谁先结合").size(20.0).strong());
        prose(
            ui,
            "和算术的\"先乘除后加减\"一样，三种运算也有优先级：重复 `*` 最高，连接其次，选择 `|` 最低。\
             所以 `ab|c` 是 `(ab)|c`，而不是 `a(b|c)`；`ab*` 是 `a(b*)`，而不是 `(ab)*`。想改变顺序就加括号。",
        );
        ui.label("在下面输入正则，就能看到它被解析成的语法树——树的形状说明了谁先结合。");

        ui.add_space(8.0);
        ui.label(RichText::new("亲手试一试").size(20.0).strong());
        if regex_input(ui, &mut self.src, PRESETS) {
            self.pipe = RegexPipeline::new(&self.src);
        }
        if let Err(d) = &self.pipe.regex {
            ui.label("这条正则写得有问题：");
            crate::widgets::diagnostic_view(ui, &self.pipe.src, d);
        } else {
            self.playground(ui);
        }

        ui.add_space(12.0);
        ui.label(RichText::new("小测验").size(20.0).strong());
        quiz(
            ui,
            "ch1-regex-1",
            "下面哪个字符串能被 `(a|b)*abb` 匹配？",
            &["abba", "babb", "ab"],
            1,
            "`(a|b)*` 可以吃掉任意多个 a 或 b，但最后必须以 `abb` 结尾。babb = b + abb。",
        );
        quiz(
            ui,
            "ch1-regex-2",
            "`ab*` 能匹配哪个字符串？",
            &["abab", "abbb", "空串"],
            1,
            "`*` 的优先级比连接高，所以 `ab*` 是 a 后面跟任意多个 b，而不是把 ab 重复多次。",
        );
        quiz(
            ui,
            "ch1-regex-3",
            "想描述\"由若干个 0 和 1 组成、至少有一个字符\"的字符串，哪个写法对？",
            &["[01]*", "[01]+", "0|1+"],
            1,
            "`*` 允许 0 次，会匹配空串；`0|1+` 是\"0 或者 1+\"，不能匹配 01。",
        );

        ui.add_space(12.0);
        callout(ui, CalloutKind::Deeper, "正则语言与它的极限", |ui| {
            ui.label(
                "形式化地说，字母表 Σ 上的正则表达式是这样归纳定义的：ε 和 Σ 中的每个字符是正则表达式；\
                 如果 r、s 是正则表达式，那么 rs、r|s、r* 也是。一条正则描述的字符串集合叫\"正则语言\"。",
            );
            ui.label(
                "正则表达式有做不到的事：比如\"括号正确配对\"的字符串集合就不是正则语言。直觉上，要检查配对就得\"数\"\
                 左括号有多少个，而下一课会看到，正则对应的自动机只有有限个状态，没法数到任意大。\
                 （严格的证明用\"泵引理\"。）这正是第 2 章需要更强的工具——上下文无关文法——的原因。",
            );
        });
        ui.add_space(24.0);
    }

    fn playground(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        let Ok(regex) = &self.pipe.regex else { return };

        let hover_id = ui.id().with("regex_tree_hover");
        let hovered: Option<usize> = ui.data(|d| d.get_temp(hover_id)).flatten();
        let marks: Vec<Mark> =
            hovered.map(|i| Mark { span: regex.nodes[i].span, bg: p.focus_bg }).into_iter().collect();
        card(ui, |ui| {
            ui.label(RichText::new("正则（鼠标指向语法树的节点，这里会高亮对应的部分）").small().color(p.muted));
            source_view(ui, &self.pipe.src, &[], &marks, 22.0);
        });
        match hovered {
            Some(i) => prose(ui, &format!("这个节点：{}。", regex.explain(RegexId(i as u32)))),
            None => ui.label(RichText::new("把鼠标移到语法树的节点上，看看它表示什么。").color(p.muted)),
        };

        let mut new_hover = None;
        let tree = |ui: &mut Ui, new_hover: &mut Option<usize>| {
            ui.label(RichText::new("语法树").strong());
            *new_hover = scrollable_tree(ui, "regex_tree", &regex_tree(regex), |i| {
                let mut s = NodeStyle::normal(&p);
                if hovered == Some(i) {
                    s.fill = p.focus_bg;
                    s.stroke = p.accent;
                    s.emphasized = true;
                }
                s
            });
        };
        let tests = |ui: &mut Ui, text: &mut String, pipe: &RegexPipeline| {
            ui.label(RichText::new("测试字符串（每行一个，判断整行是否匹配）").strong());
            ui.add(TextEdit::multiline(text).font(mono(16.0)).desired_rows(5).desired_width(f32::INFINITY));
            for line in text.lines().take(30) {
                let ok = pipe.matches(line).unwrap_or(false);
                ui.horizontal(|ui| {
                    let (tag, color) = if ok { ("匹配", p.ok) } else { ("不匹配", p.error) };
                    ui.label(RichText::new(tag).color(color).strong());
                    let shown = if line.is_empty() { "（空串）" } else { line };
                    ui.label(RichText::new(shown).font(mono(16.0)));
                });
            }
        };
        if ui.available_width() > 820.0 {
            ui.columns(2, |cols| {
                tree(&mut cols[0], &mut new_hover);
                tests(&mut cols[1], &mut self.tests, &self.pipe);
            });
        } else {
            tree(ui, &mut new_hover);
            tests(ui, &mut self.tests, &self.pipe);
        }
        if new_hover != hovered {
            ui.data_mut(|d| d.insert_temp(hover_id, new_hover));
            ui.ctx().request_repaint();
        }
    }
}
