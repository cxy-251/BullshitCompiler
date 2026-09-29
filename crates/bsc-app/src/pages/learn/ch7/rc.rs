//! 7.1 引用计数。

use bsc_runtime::Algorithm;
use eframe::egui::{RichText, Ui};

use super::GcDemo;
use crate::theme::Palette;
use crate::widgets::{CalloutKind, callout, card, prose, quiz};

const PRESETS: &[(&str, &str)] = &[
    ("基本释放", "new A\nnew B\nroot x = A\nA.0 = B\nA.0 = null"),
    ("连锁释放", "new A\nnew B\nnew C\nroot x = A\nA.0 = B\nB.0 = C\nroot x = null"),
    ("共享的对象", "new A\nnew B\nnew C\nroot x = A\nroot y = B\nA.0 = C\nB.0 = C\nroot x = null\nroot y = null"),
    ("环：内存泄漏", "new A\nnew B\nroot x = A\nA.0 = B\nB.0 = A\nroot x = null\ngc"),
];

pub struct Lesson {
    demo: GcDemo,
}

impl Default for Lesson {
    fn default() -> Self {
        Self { demo: GcDemo::new("gc_rc", PRESETS[1].1, Algorithm::RefCount) }
    }
}

impl Lesson {
    pub fn ui(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        ui.heading("7.1  引用计数");
        ui.label(RichText::new("入门 · 约 20 分钟").color(p.muted));

        ui.add_space(8.0);
        ui.label(RichText::new("为什么要垃圾回收").size(20.0).strong());
        prose(
            ui,
            "程序运行时在**堆**上创建对象（字符串、列表、类的实例……）。对象不再被用到时，它占的内存应该还给系统，否则内存会越用越多。\
             C 语言让程序员自己 `free`，忘了就泄漏，释放早了就是\"悬空指针\"——最常见的安全漏洞来源之一。\
             **垃圾回收**（GC）让运行时自动找出不再被用到的对象并回收。\"不再被用到\"的标准是：从**根**（栈上的变量、全局变量）出发，沿着指针走不到它。",
        );
        callout(ui, CalloutKind::Analogy, "气球和绳子", |ui| {
            prose(
                ui,
                "每个对象是一只氢气球，指针是系在上面的绳子，根是握在手里的绳子。一只气球只要还有一根绳子最终连到你手上，它就不会飘走；\
                 所有绳子都断了，它就飘走了（变成垃圾）。",
            );
        });

        ui.add_space(8.0);
        ui.label(RichText::new("引用计数").size(20.0).strong());
        prose(
            ui,
            "最直接的办法：每个对象记着\"有几根绳子系着我\"（**引用计数**）。多一个指针指向它，计数加 1；少一个，减 1；\
             **减到 0 立刻释放**，并把它指向的对象的计数也都减 1——可能引起一连串的释放。",
        );
        card(ui, |ui| {
            prose(
                ui,
                "下面的小脚本描述程序对堆做了什么：`new A` 分配对象，`root x = A` 让根变量指向 A，`A.0 = B` 让 A 的第 0 个指针域指向 B。\
                 每个对象下面标着它当前的引用计数。可以改脚本，也可以点例子。",
            );
        });
        self.demo.ui(ui, PRESETS);

        ui.add_space(8.0);
        callout(ui, CalloutKind::KeyPoint, "致命弱点：环", |ui| {
            prose(
                ui,
                "选\"环：内存泄漏\"：A 指向 B，B 指向 A。根不再指向 A 以后，两个对象都走不到了，可它们的计数都还是 1（互相指着），\
                 永远不会减到 0——内存泄漏。实际系统的对策：Python 在引用计数之外，定期用 7.2 课的方法专门找环；\
                 Swift 和 Rust 的 `Rc` 让程序员用**弱引用**（weak，不增加计数）打破环。",
            );
        });

        ui.add_space(12.0);
        ui.label(RichText::new("小测验").size(20.0).strong());
        quiz(
            ui,
            "ch7-rc-1",
            "引用计数最大的优点是？",
            &["对象一变成垃圾就立即回收，不需要让程序停下来", "能回收所有垃圾", "不占额外内存"],
            0,
            "回收的工作分散在每次指针赋值里，没有集中的停顿；缺点是每次赋值都要更新计数，而且回收不了环。",
        );
        quiz(
            ui,
            "ch7-rc-2",
            "多线程程序里用引用计数，额外的代价是什么？",
            &["计数的加减必须是原子操作，比普通加减慢得多", "没有额外代价", "不能用多线程"],
            0,
            "两个线程同时给同一个计数加 1，如果不是原子的，就可能少加一次，导致对象被提前释放。Rust 因此分成了 Rc（单线程）和 Arc（原子）。",
        );
        ui.add_space(24.0);
    }
}
