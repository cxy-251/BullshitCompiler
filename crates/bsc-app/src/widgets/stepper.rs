//! 步进播放器：教学演示的核心控件。
//!
//! 算法运行时记录下每一步，播放器负责在这些步骤之间前进、后退、自动播放。
//! `pos = k` 表示"前 k 步已经完成"，所以 `pos` 的取值范围是 `0..=len`。

use eframe::egui::{self, Slider, Ui};

pub struct Stepper {
    pub pos: usize,
    playing: bool,
    /// 自动播放速度：每秒几步。
    speed: f32,
    last_tick: f64,
}

impl Default for Stepper {
    fn default() -> Self {
        Self { pos: 0, playing: false, speed: 2.0, last_tick: 0.0 }
    }
}

impl Stepper {
    /// 输入变了、步骤序列重新生成时调用。
    pub fn reset(&mut self) {
        self.pos = 0;
        self.playing = false;
    }

    /// 跳到最后一步。实际长度在下一次 [`Stepper::ui`] 时截断。
    pub fn seek_end(&mut self) {
        self.pos = usize::MAX;
        self.playing = false;
    }

    /// 画出控制条，返回位置是否变化。
    pub fn ui(&mut self, ui: &mut Ui, len: usize) -> bool {
        let before = self.pos;
        self.pos = self.pos.min(len);

        ui.horizontal_wrapped(|ui| {
            if ui.add_enabled(self.pos > 0, egui::Button::new("◀◀ 开头")).clicked() {
                self.pos = 0;
                self.playing = false;
            }
            if ui.add_enabled(self.pos > 0, egui::Button::new("◀ 上一步")).clicked() {
                self.pos -= 1;
                self.playing = false;
            }
            let play_label = if self.playing { "■ 暂停" } else { "▶ 自动播放" };
            if ui.add_enabled(len > 0, egui::Button::new(play_label).selected(self.playing)).clicked() {
                self.playing = !self.playing;
                if self.playing && self.pos >= len {
                    self.pos = 0; // 播完了再点播放：从头开始
                }
                self.last_tick = ui.input(|i| i.time);
            }
            if ui.add_enabled(self.pos < len, egui::Button::new("下一步 ▶")).clicked() {
                self.pos += 1;
                self.playing = false;
            }
            if ui.add_enabled(self.pos < len, egui::Button::new("结尾 ▶▶")).clicked() {
                self.pos = len;
                self.playing = false;
            }

            ui.separator();
            ui.label(format!("第 {} / {} 步", self.pos, len));
            ui.add(Slider::new(&mut self.pos, 0..=len.max(1)).show_value(false));
            ui.separator();
            ui.label("速度");
            ui.add(Slider::new(&mut self.speed, 0.5..=8.0).suffix(" 步/秒").max_decimals(1));
        });
        self.pos = self.pos.min(len);

        if self.playing {
            let now = ui.input(|i| i.time);
            let interval = 1.0 / f64::from(self.speed);
            if now - self.last_tick >= interval {
                self.last_tick = now;
                if self.pos < len {
                    self.pos += 1;
                }
            }
            if self.pos >= len {
                self.playing = false;
            } else {
                ui.request_repaint_after(std::time::Duration::from_secs_f64(interval / 2.0));
            }
        }

        self.pos != before
    }
}
