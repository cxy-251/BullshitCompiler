//! 带高亮的源码视图：给指定区间上色、加背景，用来指出"现在处理到源码的哪一段"。

use bsc_core::Span;
use eframe::egui::{self, Color32, Response, Ui, text::LayoutJob};

use crate::theme::{Palette, mono};

/// 一段带背景色的高亮。空区间（比如"输入结束"的位置）画成一根竖线。
#[derive(Clone, Copy)]
pub struct Mark {
    pub span: Span,
    pub bg: Color32,
}

/// `colors`：给区间设置文字颜色（通常是记号着色）。
/// `marks`：给区间加背景色，后面的覆盖前面的。
pub fn source_view(ui: &mut Ui, source: &str, colors: &[(Span, Color32)], marks: &[Mark], size: f32) -> Response {
    let p = Palette::of(ui);
    let font = mono(size);

    let mut cuts = vec![0, source.len()];
    for (s, _) in colors {
        cuts.extend([s.start, s.end]);
    }
    for m in marks {
        cuts.extend([m.span.start, m.span.end]);
    }
    cuts.retain(|&c| c <= source.len());
    cuts.sort_unstable();
    cuts.dedup();

    let mut job = LayoutJob::default();
    let caret = |job: &mut LayoutJob, at: usize| {
        for m in marks.iter().filter(|m| m.span.is_empty() && m.span.start == at) {
            job.append("▏", 0.0, fmt(font.clone(), m.bg.gamma_multiply(3.0).to_opaque(), Color32::TRANSPARENT));
        }
    };

    for w in cuts.windows(2) {
        let (a, b) = (w[0], w[1]);
        caret(&mut job, a);
        if a == b {
            continue;
        }
        let seg = Span::new(a, b);
        let fg = colors.iter().rev().find(|(s, _)| s.contains(seg)).map_or(p.text, |(_, c)| *c);
        let bg = marks
            .iter()
            .rev()
            .find(|m| !m.span.is_empty() && m.span.contains(seg))
            .map_or(Color32::TRANSPARENT, |m| m.bg);
        job.append(&source[a..b], 0.0, fmt(font.clone(), fg, bg));
    }
    caret(&mut job, source.len());
    if source.is_empty() {
        job.append(" ", 0.0, fmt(font, p.muted, Color32::TRANSPARENT));
    }

    // 注意：`ui.columns` 里的布局是两端对齐（justify）的，egui 两端对齐时会去掉每行开头的空白，
    // 代码的缩进就没了。所以这里显式换成普通的左对齐布局。
    ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| ui.label(job)).inner
}

fn fmt(font_id: eframe::egui::FontId, color: Color32, background: Color32) -> eframe::egui::TextFormat {
    eframe::egui::TextFormat { font_id, color, background, ..Default::default() }
}
