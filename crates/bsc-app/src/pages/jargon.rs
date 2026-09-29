//! 「黑话编译器」页。当前是设计预览，编译器本体在后续里程碑实现。

use eframe::egui::{self, RichText, ScrollArea, Ui};

use crate::theme::{Palette, mono};
use crate::widgets::{CalloutKind, callout, card, tag};

/// 编译原理概念 ↔ 黑话翻译中的对应物 ↔ 例子。
const MAPPING: &[(&str, &str, &str)] = &[
    ("词法分析", "切分出黑话词汇，并分类：空洞动词、虚指名词、膨胀修饰语", "赋能 / 抓手 / 全链路"),
    ("语法分析", "识别固定句式（黑话的\"文法\"）", "以 X 为抓手 · 围绕 X 做好 Y · 形成 X 闭环"),
    ("语义分析", "找出真正的动作、对象、目的、手段（语义角色）", "谁 · 做了什么 · 为了什么"),
    ("中间表示", "与措辞无关的\"意思\"：事件 + 语义角色", "推进(数字化)，目的：业务更好"),
    ("死代码消除", "删掉不携带信息的修饰语", "全方位 · 深层次 · 高质量"),
    ("常量折叠", "固定搭配直接折成简单说法", "降本增效 → 省钱"),
    ("公共子表达式消除", "排比句合并成一句", "强化 X、深化 X、优化 X → 做好 X"),
    ("降级 (lowering)", "沿\"上位词\"关系把抽象概念降到具体说法", "赋能 → 帮助 · 抓手 → 办法"),
    ("代码生成", "用朴素的句式重新说一遍", "我们打算……"),
    ("诊断信息", "解释每处翻译的理由，标出哪些词是\"水分\"", "这 4 个词删掉后意思不变"),
];

const EXAMPLE_INPUT: &str = "我们要以数字化转型为抓手，全面打通业务壁垒，形成全链路闭环，赋能业务高质量发展。";

const ROADMAP: &[(&str, &str)] = &[
    ("黑话词典与概念图", "收集词汇并建立\"上位词—下位词\"关系图，这是降级优化的依据。"),
    ("分词器", "Aho-Corasick 多模式匹配自动机 + 词图最大概率切分。对应第 1 章。"),
    ("句式文法与 Earley 分析器", "Earley 算法能处理有歧义的文法，适合自然语言。对应第 2 章。"),
    ("语义角色与中间表示", "从语法树提取\"谁、做什么、为了什么\"。对应第 3、4 章。"),
    ("优化 Pass", "死代码消除、常量折叠、公共子表达式消除、降级，每个 Pass 前后可对比。对应第 5 章。"),
    ("大白话生成与翻译理由", "生成朴素中文，并给出每处改动的解释。对应第 6 章。"),
    ("反向编译（娱乐）", "大白话 → 黑话。用同一套中间表示，换一个\"后端\"。"),
];

pub fn ui(ui: &mut Ui) {
    let p = Palette::of(ui);
    egui::CentralPanel::default().show(ui, |ui| {
        ScrollArea::vertical().show(ui, |ui| {
            ui.set_max_width(ui.available_width().min(980.0));
            ui.heading("黑话编译器");
            ui.horizontal(|ui| {
                tag(ui, "设计中", p.paren);
                ui.label(RichText::new("把故作高深的\"领导讲话\"编译成平凡质朴的大白话。").size(17.0));
            });

            ui.add_space(8.0);
            callout(ui, CalloutKind::KeyPoint, "为什么说这是一个编译问题", |ui| {
                ui.label(
                    "黑话看起来花样百出，其实高度公式化：词汇有限，句式固定，修饰语可以无限堆叠却几乎不携带信息。\
                     \"有固定词汇、有固定句式\"——这正是编译器擅长处理的\"语言\"。",
                );
                ui.label(
                    "所以黑话编译器不是查字典做替换，而是真正走一遍编译流水线：先理解句子结构和真实意思，\
                     再把没用的部分优化掉，最后用朴素的话重新说出来。",
                );
            });

            ui.add_space(8.0);
            ui.label(RichText::new("编译原理 ↔ 黑话翻译").size(20.0).strong());
            card(ui, |ui| {
                egui::Grid::new("jargon_mapping").num_columns(3).striped(true).spacing([16.0, 8.0]).show(ui, |ui| {
                    ui.label(RichText::new("编译器里的概念").strong());
                    ui.label(RichText::new("在黑话翻译里是什么").strong());
                    ui.label(RichText::new("例子").strong());
                    ui.end_row();
                    for (concept, meaning, example) in MAPPING {
                        ui.label(RichText::new(*concept).color(p.accent));
                        ui.label(*meaning);
                        ui.label(RichText::new(*example).color(p.muted));
                        ui.end_row();
                    }
                });
            });

            ui.add_space(8.0);
            ui.label(RichText::new("目标效果").size(20.0).strong());
            card(ui, |ui| {
                ui.label(
                    RichText::new("设计示意：以下内容是人工写出的目标效果，不是程序运行结果。编译器实现后，这里会换成真正的交互演示。")
                        .color(p.paren),
                );
                ui.add_space(4.0);
                ui.label(RichText::new("输入").strong());
                ui.label(RichText::new(EXAMPLE_INPUT).size(17.0));
                ui.add_space(4.0);
                ui.label(RichText::new("识别出的句式").strong());
                for line in [
                    "以【数字化转型】为抓手  →  手段：推进数字化",
                    "打通【业务壁垒】        →  动作：让各部门配合",
                    "形成【全链路闭环】      →  空转：没有新信息",
                    "赋能【业务高质量发展】  →  目的：把业务做好",
                ] {
                    ui.label(RichText::new(line).font(mono(14.0)));
                }
                ui.add_space(4.0);
                ui.label(RichText::new("被优化掉的修饰语").strong());
                ui.label(RichText::new("全面 · 全链路 · 闭环 · 高质量").color(p.muted).strikethrough());
                ui.add_space(4.0);
                ui.label(RichText::new("输出").strong());
                ui.label(RichText::new("我们打算推进数字化，让各部门配合好，把业务做好。").size(17.0).color(p.ok));
            });

            ui.add_space(8.0);
            ui.label(RichText::new("开发路线").size(20.0).strong());
            for (i, (title, detail)) in ROADMAP.iter().enumerate() {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(format!("{}. {title}", i + 1)).strong());
                    ui.label(RichText::new(*detail).color(p.muted));
                });
            }
            ui.add_space(8.0);
            ui.label(RichText::new("黑话编译器用到的每一项技术，都会先在课程里讲一遍。学完课程，你就能读懂它的全部源码。").color(p.muted));
            ui.add_space(24.0);
        });
    });
}
