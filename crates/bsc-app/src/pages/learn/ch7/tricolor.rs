//! 7.3 三色标记与写屏障。

use bsc_runtime::Algorithm;
use eframe::egui::{RichText, Ui};

use super::GcDemo;
use crate::theme::Palette;
use crate::widgets::{CalloutKind, callout, card, prose, quiz};

const PRESETS: &[(&str, &str)] = &[
    ("丢失对象", "new A\nnew B\nnew C\nroot x = A\nA.0 = B\nB.0 = C\ngc begin\nmark 1\nA.1 = C\nB.0 = null\ngc end"),
    ("边标记边分配", "new A\nnew B\nroot x = A\nA.0 = B\ngc begin\nmark 1\nnew C\nB.0 = C\nmark 5\ngc end"),
    (
        "一步步标记",
        "new A\nnew B\nnew C\nnew D\nroot x = A\nA.0 = B\nA.1 = C\nC.0 = D\ngc begin\nmark 1\nmark 1\nmark 1\nmark 1\ngc end",
    ),
];

pub struct Lesson {
    demo: GcDemo,
    barrier: bool,
}

impl Default for Lesson {
    fn default() -> Self {
        Self { demo: GcDemo::new("gc_tri", PRESETS[0].1, Algorithm::TriColor { barrier: false }), barrier: false }
    }
}

impl Lesson {
    pub fn ui(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        ui.heading("7.3  三色标记与写屏障");
        ui.label(RichText::new("进阶 · 约 35 分钟").color(p.muted));

        ui.add_space(8.0);
        ui.label(RichText::new("一句话").size(20.0).strong());
        prose(
            ui,
            "7.2 课的回收要让程序完全停下来。堆有几个 GB 时，一次停顿可能长达几百毫秒——游戏会卡顿，服务器会超时。\
             **增量**（或**并发**）回收把标记拆成很多小步，和程序交替（或同时）进行。问题是：标记到一半时，程序还在改指针，\
             回收器会不会漏掉活着的对象？",
        );

        ui.add_space(8.0);
        ui.label(RichText::new("三色不变式").size(20.0).strong());
        card(ui, |ui| {
            prose(ui, "白 = 还没见到；灰 = 见到了、它的指针还没检查；黑 = 它的指针都检查过了，**不会再被扫描**。");
            prose(
                ui,
                "漏掉对象需要同时发生两件事：① 程序让一个**黑色**对象指向了一个**白色**对象；\
                 ② 从灰色对象到这个白色对象的所有路径都被切断了。这时白色对象只挂在黑色对象下面，而黑色对象不会再被扫描——\
                 回收器永远发现不了它，清除时把一个还在用的对象回收了。",
            );
            prose(
                ui,
                "**写屏障**是编译器在每条\"写指针\"指令旁边插入的一小段代码。这里用 Dijkstra 的插入屏障：标记进行时，\
                 往任何对象里写入一个指向白色对象的指针，就先把那个白色对象标成灰色。这样条件 ① 永远不会成立（\"黑色不指向白色\"，即三色不变式）。",
            );
        });

        ui.add_space(8.0);
        ui.label(RichText::new("单步观察").size(20.0).strong());
        prose(
            ui,
            "`gc begin` 开始一轮标记，`mark N` 处理 N 个灰色对象，中间可以穿插程序的操作，`gc end` 标记完剩下的并清除。\
             先**关掉**写屏障运行\"丢失对象\"例子，看看出了什么事；再打开写屏障运行一遍。",
        );
        ui.horizontal(|ui| {
            ui.label(RichText::new("写屏障：").strong());
            let mut changed = ui.selectable_value(&mut self.barrier, false, "关闭").changed();
            changed |= ui.selectable_value(&mut self.barrier, true, "打开").changed();
            if changed {
                self.demo.alg = Algorithm::TriColor { barrier: self.barrier };
                self.demo.rerun();
            }
        });
        self.demo.ui(ui, PRESETS);

        ui.add_space(8.0);
        callout(ui, CalloutKind::KeyPoint, "标记期间新分配的对象", |ui| {
            prose(
                ui,
                "选\"边标记边分配\"：C 是标记开始以后才分配的。这里直接把它标成黑色——这一轮不回收它。\
                 这会让少量本该回收的对象多活一轮（\"浮动垃圾\"），但保证了安全。增量回收总是在\"停顿短\"和\"回收得干净\"之间权衡。",
            );
        });

        ui.add_space(12.0);
        ui.label(RichText::new("小测验").size(20.0).strong());
        quiz(
            ui,
            "ch7-tri-1",
            "三色不变式说的是？",
            &["黑色对象不能直接指向白色对象", "灰色对象不能指向黑色对象", "白色对象必须被回收"],
            0,
            "只要这条成立，任何白色的活对象都能从某个灰色对象走到，标记就不会漏掉它。",
        );
        quiz(
            ui,
            "ch7-tri-2",
            "写屏障的代价是什么？",
            &["每次写指针都要多执行几条指令，程序整体变慢一点", "没有代价", "回收器会漏掉对象"],
            0,
            "这是用程序的吞吐量换回收器的停顿时间。Go 的回收器把停顿压到了亚毫秒级，代价就是写屏障和并发标记占用的 CPU。",
        );

        ui.add_space(12.0);
        callout(ui, CalloutKind::Deeper, "真实的并发回收器", |ui| {
            prose(
                ui,
                "另一种屏障是 Yuasa 的删除屏障：删除指针时把原来指向的对象标灰，破坏的是条件 ②。Go 1.8 起使用两者结合的混合屏障，\
                 省掉了标记结束时重新扫描栈的停顿。Java 的 ZGC、Shenandoah 还能在程序运行的同时**移动**对象，\
                 靠的是读屏障：每次读指针时检查对象是否已经搬走。",
            );
        });
        ui.add_space(24.0);
    }
}
