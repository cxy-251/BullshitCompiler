//! 应用主框架：顶栏导航 + 三个页面。

use eframe::egui::{self, RichText, Ui};
use serde::{Deserialize, Serialize};

use crate::pages::{jargon::JargonPage, lab::LabPage, learn::LearnPage};
use crate::theme::Palette;
use crate::{fonts, theme};

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
enum Tab {
    #[default]
    Learn,
    Lab,
    Jargon,
}

impl Tab {
    const ALL: [Tab; 3] = [Tab::Learn, Tab::Lab, Tab::Jargon];

    fn title(self) -> &'static str {
        match self {
            Tab::Learn => "学习",
            Tab::Lab => "实验台",
            Tab::Jargon => "黑话编译器",
        }
    }
}

/// 需要跨次启动保存的状态（原生版存在配置目录，Web 版存在 localStorage）。
#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct Saved {
    tab: Tab,
    lesson: Option<String>,
    /// 实验台里的 mini-lang 程序（旧版本存的是计算器算式，字段名不同，自动忽略）。
    lab_program: Option<String>,
    /// 黑话编译器的输入和用户粘贴的扩充词条。
    jargon_input: Option<String>,
    jargon_batch: Option<String>,
}

pub struct BscApp {
    tab: Tab,
    learn: LearnPage,
    lab: LabPage,
    jargon: JargonPage,
}

impl BscApp {
    pub fn new(cc: &eframe::CreationContext<'_>, cjk_font: Option<eframe::egui::FontData>) -> Self {
        fonts::install(&cc.egui_ctx, cjk_font);
        theme::install(&cc.egui_ctx);

        let saved: Saved = cc.storage.and_then(|s| eframe::get_value(s, eframe::APP_KEY)).unwrap_or_default();
        let mut learn = LearnPage::default();
        if let Some(lesson) = saved.lesson {
            learn.current = lesson;
        }
        let lab = saved.lab_program.map(LabPage::with_input).unwrap_or_default();
        let jargon = match saved.jargon_input {
            Some(input) => JargonPage::with_saved(input, saved.jargon_batch.unwrap_or_default()),
            None => JargonPage::default(),
        };
        Self { tab: saved.tab, learn, lab, jargon }
    }
}

impl eframe::App for BscApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        let saved = Saved {
            tab: self.tab,
            lesson: Some(self.learn.current.clone()),
            lab_program: Some(self.lab.input.clone()),
            jargon_input: Some(self.jargon.input.clone()),
            jargon_batch: Some(self.jargon.batch.clone()),
        };
        eframe::set_value(storage, eframe::APP_KEY, &saved);
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        egui::Panel::top("top_bar").show(ui, |ui| self.top_bar(ui));
        match self.tab {
            Tab::Learn => self.learn.ui(ui),
            Tab::Lab => self.lab.ui(ui),
            Tab::Jargon => self.jargon.ui(ui),
        }
    }
}

impl BscApp {
    fn top_bar(&mut self, ui: &mut Ui) {
        let p = Palette::of(ui);
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("BSc").size(20.0).strong().color(p.accent));
            ui.label(RichText::new("编译原理实验室").size(17.0).strong());
            ui.separator();
            for tab in Tab::ALL {
                ui.selectable_value(&mut self.tab, tab, RichText::new(tab.title()).size(16.0));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // 自己画切换按钮：egui 自带的切换按钮用的是太阳/月亮图标，我们没有打包那套图标字体。
                let dark = ui.visuals().dark_mode;
                if ui.button(if dark { "浅色" } else { "深色" }).on_hover_text("切换亮色/暗色主题").clicked()
                {
                    ui.ctx().set_theme(if dark { egui::Theme::Light } else { egui::Theme::Dark });
                }
                ui.hyperlink_to("源码", "https://github.com/cxy-251/BullshitCompiler");
                ui.label(RichText::new(concat!("v", env!("CARGO_PKG_VERSION"))).small().color(p.muted));
            });
        });
        ui.add_space(2.0);
    }
}
