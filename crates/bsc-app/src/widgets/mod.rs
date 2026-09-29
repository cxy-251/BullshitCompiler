//! 可复用的界面组件。

mod graph_view;
mod source_view;
mod stepper;
mod tree_view;

pub use graph_view::{GEdge, GEdgeStyle, GNode, GNodeStyle, Graph, graph_view};
pub use source_view::{Mark, source_view};
pub use stepper::Stepper;
pub use tree_view::{NodeStyle, TreeNode, scrollable_tree};

use bsc_core::{Diagnostic, Severity, line_col};
use eframe::egui::{
    self, Color32, CornerRadius, FontId, Frame, Margin, Response, RichText, Sense, Stroke, StrokeKind, TextFormat, Ui,
    Vec2, text::LayoutJob,
};

use crate::theme::{Palette, mono};

/// 一段说明文字：用反引号包起来的部分（如 `1 + 2`）显示成代码样式。
pub fn prose(ui: &mut Ui, text: &str) -> Response {
    prose_sized(ui, text, 16.0)
}

pub fn prose_sized(ui: &mut Ui, text: &str, size: f32) -> Response {
    let color = Palette::of(ui).text;
    prose_colored(ui, text, size, color)
}

/// 同 [`prose`]，但指定文字颜色。
pub fn prose_colored(ui: &mut Ui, text: &str, size: f32, color: Color32) -> Response {
    let p = Palette::of(ui);
    let mut job = LayoutJob::default();
    let strong = ui.visuals().strong_text_color();
    // `代码` 显示成等宽字体加底色；**强调** 显示成更醒目的颜色（代码里的 ** 不算）。
    for (i, part) in text.split('`').enumerate() {
        if part.is_empty() {
            continue;
        }
        if i % 2 == 1 {
            let format = TextFormat { font_id: mono(size * 0.95), color, background: p.code_bg, ..Default::default() };
            job.append(part, 0.0, format);
            continue;
        }
        for (j, piece) in part.split("**").enumerate() {
            if piece.is_empty() {
                continue;
            }
            let c = if j % 2 == 1 { strong } else { color };
            let format = TextFormat { font_id: FontId::proportional(size), color: c, ..Default::default() };
            job.append(piece, 0.0, format);
        }
    }
    ui.label(job)
}

/// 圆角卡片容器。
pub fn card<R>(ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> R {
    let p = Palette::of(ui);
    Frame::new()
        .fill(p.card_bg)
        .stroke(Stroke::new(1.0, p.card_stroke))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::same(14))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add_contents(ui)
        })
        .inner
}

#[derive(Clone, Copy)]
pub enum CalloutKind {
    /// 打比方。
    Analogy,
    /// 要点。
    KeyPoint,
    /// 深入一点的补充。
    Deeper,
}

/// 左侧带色条的提示框：比喻、要点、补充说明。
pub fn callout(ui: &mut Ui, kind: CalloutKind, title: &str, body: impl FnOnce(&mut Ui)) {
    let p = Palette::of(ui);
    let (tag, color) = match kind {
        CalloutKind::Analogy => ("打个比方", p.operator),
        CalloutKind::KeyPoint => ("要点", p.accent),
        CalloutKind::Deeper => ("深入一点", p.paren),
    };
    let resp = Frame::new()
        .fill(p.card_bg)
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin { left: 16, right: 12, top: 10, bottom: 10 })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(RichText::new(tag).color(color).strong());
                if !title.is_empty() {
                    ui.label(RichText::new(title).strong());
                }
            });
            body(ui);
        });
    let r = resp.response.rect;
    ui.painter().rect_filled(
        egui::Rect::from_min_size(r.min, Vec2::new(4.0, r.height())),
        CornerRadius::same(2),
        color,
    );
}

/// 单选小测验。答题状态存在 egui 的临时内存里，调用方不用管。
pub fn quiz(ui: &mut Ui, id: &str, question: &str, options: &[&str], correct: usize, explanation: &str) {
    let p = Palette::of(ui);
    let state_id = egui::Id::new(("quiz", id));
    let mut chosen: Option<usize> = ui.data(|d| d.get_temp(state_id)).flatten();

    card(ui, |ui| {
        prose(ui, question);
        ui.add_space(4.0);
        for (i, opt) in options.iter().enumerate() {
            let mut text = RichText::new(format!("{}. {}", (b'A' + i as u8) as char, opt));
            if let Some(c) = chosen {
                if i == correct {
                    text = text.color(p.ok).strong();
                } else if i == c {
                    text = text.color(p.error).strikethrough();
                }
            }
            if ui.selectable_label(chosen == Some(i), text).clicked() {
                chosen = Some(i);
            }
        }
        if let Some(c) = chosen {
            ui.add_space(4.0);
            let verdict = if c == correct {
                RichText::new("答对了！").color(p.ok).strong()
            } else {
                RichText::new("不对哦，正确答案已经标成绿色。").color(p.error).strong()
            };
            ui.label(verdict);
            prose(ui, explanation);
            if ui.small_button("重新作答").clicked() {
                chosen = None;
            }
        }
    });
    ui.data_mut(|d| d.insert_temp(state_id, chosen));
}

/// 在界面上展示一条诊断信息：标题、带标注的源码、注释与修改建议。
pub fn diagnostic_view(ui: &mut Ui, source: &str, d: &Diagnostic) {
    let p = Palette::of(ui);
    Frame::new().fill(p.error_bg).corner_radius(CornerRadius::same(8)).inner_margin(Margin::same(12)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        let sev = match d.severity {
            Severity::Error => p.error,
            Severity::Warning | Severity::Note => p.paren,
        };
        let code = d.code.map(|c| format!("[{c}]")).unwrap_or_default();
        prose_colored(ui, &format!("{}{}：{}", d.severity.as_str(), code, d.message), 16.0, sev);

        let marks: Vec<Mark> = d
            .labels
            .iter()
            .map(|l| Mark { span: l.span, bg: if l.primary { p.error.gamma_multiply(0.35) } else { p.warn_bg } })
            .collect();
        source_view(ui, source, &[], &marks, 17.0);

        for l in &d.labels {
            let lc = line_col(source, l.span.start);
            let tag = if l.primary { "▲" } else { "△" };
            prose(ui, &format!("{tag} 第 {} 行第 {} 列：{}", lc.line, lc.col, l.message));
        }
        for n in &d.notes {
            prose_colored(ui, &format!("注：{n}"), 15.0, p.muted);
        }
        if let Some(h) = &d.help {
            prose_colored(ui, &format!("帮助：{h}"), 15.0, p.ok);
        }
        egui::CollapsingHeader::new(RichText::new("在终端里它长这样").small())
            .id_salt(("diag_term", d.message.as_str()))
            .show(ui, |ui| {
                ui.label(RichText::new(d.render(source, "输入")).font(mono(13.0)));
            });
    });
}

/// 栈的可视化：从下往上堆叠的格子，栈顶高亮。
pub fn stack_view(ui: &mut Ui, stack: &[i64], highlight_top: bool) {
    let p = Palette::of(ui);
    let cell = Vec2::new(120.0, 30.0);
    let rows = stack.len().max(1);
    let (resp, painter) =
        ui.allocate_painter(Vec2::new(cell.x + 70.0, rows as f32 * (cell.y + 4.0) + 24.0), Sense::hover());
    let left = resp.rect.left() + 4.0;
    let bottom = resp.rect.bottom() - 22.0;

    painter.text(
        egui::pos2(left + cell.x / 2.0, bottom + 12.0),
        egui::Align2::CENTER_CENTER,
        "栈底",
        egui::FontId::proportional(12.0),
        p.muted,
    );
    if stack.is_empty() {
        let r = egui::Rect::from_min_size(egui::pos2(left, bottom - cell.y), cell);
        painter.rect_stroke(r, CornerRadius::same(4), Stroke::new(1.0, p.card_stroke), StrokeKind::Inside);
        painter.text(r.center(), egui::Align2::CENTER_CENTER, "（空）", egui::FontId::proportional(13.0), p.muted);
        return;
    }
    for (i, v) in stack.iter().enumerate() {
        let y = bottom - (i as f32 + 1.0) * (cell.y + 4.0) + 4.0;
        let r = egui::Rect::from_min_size(egui::pos2(left, y), cell);
        let is_top = i + 1 == stack.len();
        let fill = if is_top && highlight_top { p.focus_bg } else { p.node_fill };
        painter.rect_filled(r, CornerRadius::same(4), fill);
        painter.rect_stroke(r, CornerRadius::same(4), Stroke::new(1.5, p.node_stroke), StrokeKind::Inside);
        painter.text(r.center(), egui::Align2::CENTER_CENTER, v.to_string(), mono(16.0), p.text);
        if is_top {
            painter.text(
                egui::pos2(r.right() + 8.0, r.center().y),
                egui::Align2::LEFT_CENTER,
                "← 栈顶",
                egui::FontId::proportional(13.0),
                p.muted,
            );
        }
    }
}

/// 一个"小卡片"：带底色和边框的一小段文字（记号、分组等）。
///
/// 用 `Button` 而不是 `Frame` 实现：egui 的自动换行布局只能在放置"单个控件"之前判断放不放得下，
/// `Frame` 是容器，放进 `horizontal_wrapped` 里不会换行，会把一行撑出界。
pub fn chip(ui: &mut Ui, text: RichText, fill: Color32, stroke: Stroke) -> Response {
    ui.add(egui::Button::new(text).fill(fill).stroke(stroke).corner_radius(CornerRadius::same(5)).sense(Sense::hover()))
}

/// 可点击的小卡片。
pub fn chip_button(ui: &mut Ui, text: RichText, fill: Color32, stroke: Stroke) -> Response {
    ui.add(egui::Button::new(text).fill(fill).stroke(stroke).corner_radius(CornerRadius::same(6)))
}

/// 小标签（"入门""进阶""规划中"等）。
pub fn tag(ui: &mut Ui, text: &str, color: Color32) {
    ui.label(RichText::new(format!("[{text}]")).small().color(color));
}
