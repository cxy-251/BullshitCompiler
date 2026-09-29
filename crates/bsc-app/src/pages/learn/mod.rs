//! 「学习」页：课程目录 + 课程内容。

mod ch0;
mod ch1;
mod ch2;
mod ch3;
mod ch4;
mod ch5;

use eframe::egui::{self, RichText, ScrollArea, Ui};

use crate::theme::Palette;
use crate::widgets::{card, tag};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Level {
    /// 面向零基础。
    Basic,
    /// 教材级的深入内容，建立在入门内容之上。
    Advanced,
}

pub struct LessonMeta {
    pub id: &'static str,
    pub title: &'static str,
    pub level: Level,
    /// 是否已经上线。
    pub ready: bool,
    /// 这一课会讲什么（规划中的课程显示这段话）。
    pub preview: &'static str,
}

pub struct Chapter {
    pub title: &'static str,
    pub lessons: &'static [LessonMeta],
}

const fn ready(id: &'static str, title: &'static str, level: Level) -> LessonMeta {
    LessonMeta { id, title, level, ready: true, preview: "" }
}

const fn planned(id: &'static str, title: &'static str, level: Level, preview: &'static str) -> LessonMeta {
    LessonMeta { id, title, level, ready: false, preview }
}

pub const COURSE: &[Chapter] = &[
    Chapter {
        title: "第 0 章 · 开篇",
        lessons: &[LessonMeta {
            id: "0.1",
            title: "编译器是什么",
            level: Level::Basic,
            ready: true,
            preview: "用一个完整的计算器编译器，走一遍词法分析、语法分析、代码生成、执行。",
        }],
    },
    Chapter {
        title: "第 1 章 · 词法分析：让机器认字",
        lessons: &[
            ready("1.1", "手写一个扫描器", Level::Basic),
            ready("1.2", "正则表达式", Level::Basic),
            ready("1.3", "NFA：非确定有限自动机", Level::Basic),
            ready("1.4", "DFA 与子集构造", Level::Advanced),
            ready("1.5", "DFA 最小化", Level::Advanced),
            ready("1.6", "从正则到词法分析器", Level::Basic),
        ],
    },
    Chapter {
        title: "第 2 章 · 语法分析：理解句子结构",
        lessons: &[
            ready("2.1", "文法与推导", Level::Basic),
            ready("2.2", "歧义与优先级", Level::Basic),
            ready("2.3", "递归下降", Level::Basic),
            ready("2.4", "Pratt 分析法", Level::Basic),
            ready("2.5", "LL(1) 分析表", Level::Advanced),
            ready("2.6", "LR 分析", Level::Advanced),
        ],
    },
    Chapter {
        title: "第 3 章 · 语义分析：检查意思对不对",
        lessons: &[
            ready("3.1", "作用域与符号表", Level::Basic),
            ready("3.2", "类型检查", Level::Basic),
            ready("3.3", "类型推导", Level::Advanced),
        ],
    },
    Chapter {
        title: "第 4 章 · 中间表示：编译器的内部语言",
        lessons: &[
            ready("4.1", "三地址码", Level::Basic),
            ready("4.2", "基本块与控制流图", Level::Basic),
            ready("4.3", "支配树", Level::Advanced),
            ready("4.4", "SSA 形式", Level::Advanced),
        ],
    },
    Chapter {
        title: "第 5 章 · 优化：让程序更快更小",
        lessons: &[
            ready("5.1", "常量折叠与代数化简", Level::Basic),
            ready("5.2", "数据流分析", Level::Basic),
            ready("5.3", "死代码消除", Level::Basic),
            ready("5.4", "格与不动点", Level::Advanced),
            ready("5.5", "稀疏条件常量传播", Level::Advanced),
        ],
    },
    Chapter {
        title: "第 6 章 · 后端：生成机器指令",
        lessons: &[
            planned("6.1", "栈式虚拟机", Level::Basic, "字节码的设计与解释执行；和 JVM、WebAssembly 的对照。"),
            planned("6.2", "指令选择", Level::Advanced, "树模式匹配：把中间表示覆盖成目标机器指令。"),
            planned("6.3", "寄存器分配", Level::Advanced, "活跃区间、线性扫描、干涉图着色的动画演示。"),
            planned("6.4", "函数调用约定", Level::Advanced, "栈帧、参数传递、调用者/被调用者保存寄存器。"),
        ],
    },
    Chapter {
        title: "第 7 章 · 运行时（选修）",
        lessons: &[planned("7.1", "垃圾回收", Level::Advanced, "引用计数、标记-清除、三色标记与写屏障。")],
    },
];

pub struct LearnPage {
    pub current: String,
    ch0: ch0::Lesson,
    ch1: ch1::Chapter,
    ch2: ch2::Chapter,
    ch3: ch3::Chapter,
    ch4: ch4::Chapter,
    ch5: ch5::Chapter,
}

impl Default for LearnPage {
    fn default() -> Self {
        Self {
            current: "0.1".to_owned(),
            ch0: ch0::Lesson::default(),
            ch1: ch1::Chapter::default(),
            ch2: ch2::Chapter::default(),
            ch3: ch3::Chapter::default(),
            ch4: ch4::Chapter::default(),
            ch5: ch5::Chapter::default(),
        }
    }
}

impl LearnPage {
    pub fn ui(&mut self, ui: &mut Ui) {
        let wide = ui.available_width() > 900.0;
        if wide {
            egui::Panel::left("course_catalog").default_size(250.0).size_range(200.0..=360.0).show(ui, |ui| {
                ScrollArea::vertical().show(ui, |ui| self.catalog(ui));
            });
        }
        egui::CentralPanel::default().show(ui, |ui| {
            ScrollArea::vertical().id_salt("lesson_scroll").show(ui, |ui| {
                if !wide {
                    egui::CollapsingHeader::new("课程目录").show(ui, |ui| self.catalog(ui));
                    ui.separator();
                }
                ui.set_max_width(ui.available_width().min(980.0));
                self.lesson(ui);
            });
        });
    }

    fn catalog(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        for ch in COURSE {
            ui.add_space(6.0);
            ui.label(RichText::new(ch.title).strong());
            for l in ch.lessons {
                ui.horizontal(|ui| {
                    let mut text = RichText::new(format!("{}  {}", l.id, l.title));
                    if !l.ready {
                        text = text.color(p.muted);
                    }
                    if ui.selectable_label(self.current == l.id, text).clicked() {
                        self.current = l.id.to_owned();
                    }
                    if l.level == Level::Advanced {
                        tag(ui, "进阶", p.paren);
                    }
                });
            }
        }
    }

    fn lesson(&mut self, ui: &mut Ui) {
        let Some(meta) = COURSE.iter().flat_map(|c| c.lessons).find(|l| l.id == self.current) else {
            self.current = "0.1".to_owned();
            return;
        };
        match meta.id {
            "0.1" => self.ch0.ui(ui),
            "1.1" => self.ch1.scanner.ui(ui),
            "1.2" => self.ch1.regex.ui(ui),
            "1.3" => self.ch1.nfa.ui(ui),
            "1.4" => self.ch1.dfa.ui(ui),
            "1.5" => self.ch1.minimize.ui(ui),
            "1.6" => self.ch1.lexgen.ui(ui),
            "2.1" => self.ch2.grammar.ui(ui),
            "2.2" => self.ch2.ambiguity.ui(ui),
            "2.3" => self.ch2.rd.ui(ui),
            "2.4" => self.ch2.pratt.ui(ui),
            "2.5" => self.ch2.ll1.ui(ui),
            "2.6" => self.ch2.lr.ui(ui),
            "3.1" => self.ch3.scope.ui(ui),
            "3.2" => self.ch3.typeck.ui(ui),
            "3.3" => self.ch3.infer.ui(ui),
            "4.1" => self.ch4.tac.ui(ui),
            "4.2" => self.ch4.cfg.ui(ui),
            "4.3" => self.ch4.dom.ui(ui),
            "4.4" => self.ch4.ssa.ui(ui),
            "5.1" => self.ch5.fold.ui(ui),
            "5.2" => self.ch5.dataflow.ui(ui),
            "5.3" => self.ch5.dce.ui(ui),
            "5.4" => self.ch5.lattice.ui(ui),
            "5.5" => self.ch5.sccp.ui(ui),
            _ => planned_lesson(ui, meta),
        }
    }
}

fn planned_lesson(ui: &mut Ui, meta: &LessonMeta) {
    let p = Palette::of(ui);
    ui.heading(format!("{}  {}", meta.id, meta.title));
    ui.horizontal(|ui| {
        tag(ui, "规划中", p.muted);
        if meta.level == Level::Advanced {
            tag(ui, "进阶", p.paren);
        } else {
            tag(ui, "入门", p.ok);
        }
    });
    card(ui, |ui| {
        ui.label(RichText::new("这一课会讲：").strong());
        ui.label(meta.preview);
    });
    ui.label(RichText::new("每一课的演示都会由真实运行的编译器代码驱动，不是预先录好的动画。").color(p.muted));
}
