//! 7.2 标记-清除。

use bsc_runtime::Algorithm;
use eframe::egui::{RichText, Ui};

use super::GcDemo;
use crate::theme::Palette;
use crate::widgets::{CalloutKind, callout, prose, quiz};

const PRESETS: &[(&str, &str)] = &[
    ("环也能回收", "new A\nnew B\nroot x = A\nA.0 = B\nB.0 = A\nroot x = null\ngc"),
    (
        "一次完整回收",
        "new A\nnew B\nnew C\nnew D\nnew E\nroot x = A\nA.0 = B\nA.1 = C\nC.0 = D\nD.0 = C\nroot y = E\nroot y = null\ngc",
    ),
    ("回收两次", "new A\nnew B\nnew C\nroot x = A\nA.0 = B\nB.0 = C\ngc\nA.0 = null\ngc"),
];

pub struct Lesson {
    demo: GcDemo,
}

impl Default for Lesson {
    fn default() -> Self {
        Self { demo: GcDemo::new("gc_ms", PRESETS[1].1, Algorithm::MarkSweep) }
    }
}

impl Lesson {
    pub fn ui(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        ui.heading("7.2  标记-清除");
        ui.label(RichText::new("入门 · 约 20 分钟").color(p.muted));

        ui.add_space(8.0);
        ui.label(RichText::new("一句话").size(20.0).strong());
        prose(
            ui,
            "既然\"垃圾\"的定义是\"从根走不到\"，那就直接去走：平时什么都不做，内存快不够时让程序停下来，\
             **标记**阶段从根出发把能走到的对象都做上记号，**清除**阶段把没记号的全部回收。这类方法叫**追踪式**回收，\
             Java、Go、JavaScript、C# 的垃圾回收都属于这一类。",
        );
        callout(ui, CalloutKind::Analogy, "大扫除", |ui| {
            prose(
                ui,
                "平时乱放东西不管；大扫除那天，从门口出发，把所有还在用的东西贴上标签（标记），最后没贴标签的统统扔掉（清除）。\
                 缺点是大扫除那天什么事都干不了（\"停下全世界\"，stop-the-world）。",
            );
        });

        ui.add_space(8.0);
        ui.label(RichText::new("单步观察").size(20.0).strong());
        prose(
            ui,
            "`gc` 一行触发一次完整的回收。标记时用三种颜色记录进度：**白**（还没见到）、**灰**（见到了，还没看它指向谁）、\
             **黑**（它指向的对象都已经见到了）。标记从根指向的对象变灰开始，每次取一个灰色对象，把它指向的白色对象变灰、自己变黑，\
             直到没有灰色对象；剩下的白色对象就是垃圾。",
        );
        self.demo.ui(ui, PRESETS);

        ui.add_space(8.0);
        callout(ui, CalloutKind::KeyPoint, "环不再是问题", |ui| {
            prose(
                ui,
                "选\"环也能回收\"：A、B 互相指着，但从根走不到它们，标记阶段根本不会碰到它们，清除时一起回收。\
                 追踪式回收只关心\"能不能走到\"，不关心\"有几个指针\"，所以天然能处理环。",
            );
        });

        ui.add_space(12.0);
        ui.label(RichText::new("小测验").size(20.0).strong());
        quiz(
            ui,
            "ch7-ms-1",
            "标记-清除一次回收的工作量主要和什么成正比？",
            &["标记：活着的对象数量；清除：整个堆的大小", "只和垃圾的数量有关", "和程序的代码长度有关"],
            0,
            "标记只走活对象，清除要把整个堆扫一遍找没标记的。所以活对象很多时，标记很慢。",
        );
        quiz(
            ui,
            "ch7-ms-2",
            "\"分代回收\"基于什么观察？",
            &["大多数对象都死得很早（刚创建不久就没用了）", "老对象更容易变成垃圾", "对象越大越容易变成垃圾"],
            0,
            "所以把新对象放在一小块\"新生代\"里频繁地回收，很便宜；活得久的才搬到\"老年代\"，偶尔才回收一次。",
        );

        ui.add_space(12.0);
        callout(ui, CalloutKind::Deeper, "清除之后：碎片与整理", |ui| {
            prose(
                ui,
                "清除后空闲内存东一块西一块（碎片），大对象可能找不到连续的空间。**标记-整理**把活对象挪到一头；\
                 **复制式**回收（Cheney 算法）把堆分成两半，每次把活对象复制到另一半，顺便就整理好了，代价是只能用一半内存。\
                 现代的回收器（Java 的 G1、ZGC）把这些技术和分代、并发组合在一起。",
            );
        });
        ui.add_space(24.0);
    }
}
