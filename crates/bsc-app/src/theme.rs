//! 配色与排版。所有颜色都区分亮色/暗色两套，保证两种主题下都清晰可读。

use bsc_calc::lexer::TokenClass;
use eframe::egui::{self, Color32, FontFamily, FontId, TextStyle, Theme, Ui};

/// 一套语义化的颜色：界面各处只用这里的名字，不直接写颜色值。
#[derive(Clone, Copy)]
pub struct Palette {
    pub number: Color32,
    pub operator: Color32,
    pub paren: Color32,
    pub keyword: Color32,
    pub muted: Color32,
    pub accent: Color32,
    /// 当前步骤的高亮背景。
    pub focus_bg: Color32,
    /// 已经处理过的部分的淡背景。
    pub done_bg: Color32,
    pub ok: Color32,
    pub error: Color32,
    pub error_bg: Color32,
    pub warn_bg: Color32,
    pub card_bg: Color32,
    /// 行内代码的背景。
    pub code_bg: Color32,
    pub card_stroke: Color32,
    pub node_fill: Color32,
    pub node_stroke: Color32,
    pub text: Color32,
}

impl Palette {
    pub fn of(ui: &Ui) -> Self {
        if ui.visuals().dark_mode { Self::DARK } else { Self::LIGHT }
    }

    /// 分类用的填充色（比如最小化时给不同的组上色），亮暗两套各 8 种，循环使用。
    pub fn category(&self, i: usize) -> Color32 {
        const LIGHT: [Color32; 8] = [
            Color32::from_rgb(0xd6, 0xe6, 0xfb),
            Color32::from_rgb(0xfc, 0xe0, 0xc8),
            Color32::from_rgb(0xd4, 0xf0, 0xd9),
            Color32::from_rgb(0xf3, 0xd9, 0xf0),
            Color32::from_rgb(0xfa, 0xf0, 0xbe),
            Color32::from_rgb(0xcf, 0xee, 0xee),
            Color32::from_rgb(0xe8, 0xdc, 0xf8),
            Color32::from_rgb(0xf6, 0xd4, 0xd4),
        ];
        const DARK: [Color32; 8] = [
            Color32::from_rgb(0x1f, 0x3a, 0x5c),
            Color32::from_rgb(0x5a, 0x3a, 0x1e),
            Color32::from_rgb(0x1f, 0x4a, 0x2c),
            Color32::from_rgb(0x4d, 0x28, 0x4a),
            Color32::from_rgb(0x4d, 0x45, 0x16),
            Color32::from_rgb(0x1c, 0x47, 0x47),
            Color32::from_rgb(0x3a, 0x2c, 0x5a),
            Color32::from_rgb(0x55, 0x24, 0x24),
        ];
        if self.text.r() > 128 { DARK[i % 8] } else { LIGHT[i % 8] }
    }

    pub fn token(&self, class: TokenClass) -> Color32 {
        match class {
            TokenClass::Number => self.number,
            TokenClass::Operator => self.operator,
            TokenClass::Paren => self.paren,
            TokenClass::End => self.muted,
        }
    }

    const LIGHT: Self = Self {
        number: Color32::from_rgb(0x1a, 0x6f, 0xc4),
        operator: Color32::from_rgb(0xb4, 0x3c, 0x8c),
        paren: Color32::from_rgb(0x8a, 0x6a, 0x00),
        keyword: Color32::from_rgb(0x8e, 0x3f, 0xc8),
        muted: Color32::from_rgb(0x70, 0x74, 0x7c),
        accent: Color32::from_rgb(0x2f, 0x6f, 0xeb),
        focus_bg: Color32::from_rgb(0xff, 0xe0, 0x8a),
        done_bg: Color32::from_rgb(0xe3, 0xee, 0xfb),
        ok: Color32::from_rgb(0x1e, 0x8a, 0x4c),
        error: Color32::from_rgb(0xc6, 0x28, 0x28),
        error_bg: Color32::from_rgb(0xfd, 0xe2, 0xe2),
        warn_bg: Color32::from_rgb(0xff, 0xef, 0xc9),
        card_bg: Color32::from_rgb(0xf6, 0xf7, 0xf9),
        code_bg: Color32::from_rgb(0xe8, 0xea, 0xee),
        card_stroke: Color32::from_rgb(0xdc, 0xdf, 0xe4),
        node_fill: Color32::from_rgb(0xff, 0xff, 0xff),
        node_stroke: Color32::from_rgb(0x9a, 0xa0, 0xa8),
        text: Color32::from_rgb(0x1f, 0x23, 0x28),
    };

    const DARK: Self = Self {
        number: Color32::from_rgb(0x6c, 0xb6, 0xff),
        operator: Color32::from_rgb(0xf0, 0x8c, 0xd0),
        paren: Color32::from_rgb(0xe8, 0xc5, 0x5c),
        keyword: Color32::from_rgb(0xc3, 0x9b, 0xff),
        muted: Color32::from_rgb(0x94, 0x98, 0xa0),
        accent: Color32::from_rgb(0x6c, 0x9c, 0xff),
        focus_bg: Color32::from_rgb(0x6b, 0x55, 0x10),
        done_bg: Color32::from_rgb(0x1d, 0x33, 0x4d),
        ok: Color32::from_rgb(0x5c, 0xd0, 0x8a),
        error: Color32::from_rgb(0xff, 0x7b, 0x72),
        error_bg: Color32::from_rgb(0x4d, 0x1f, 0x1f),
        warn_bg: Color32::from_rgb(0x4a, 0x3a, 0x12),
        card_bg: Color32::from_rgb(0x24, 0x27, 0x2c),
        code_bg: Color32::from_rgb(0x33, 0x37, 0x3e),
        card_stroke: Color32::from_rgb(0x3a, 0x3f, 0x46),
        node_fill: Color32::from_rgb(0x2c, 0x30, 0x36),
        node_stroke: Color32::from_rgb(0x6a, 0x70, 0x78),
        text: Color32::from_rgb(0xe6, 0xe8, 0xeb),
    };
}

/// 代码字体。
pub fn mono(size: f32) -> FontId {
    FontId::new(size, FontFamily::Monospace)
}

/// 全局排版：中文需要比 egui 默认值更大一点的字号才舒服。
pub fn install(ctx: &egui::Context) {
    for theme in [Theme::Light, Theme::Dark] {
        ctx.style_mut_of(theme, |style| {
            style.text_styles = [
                (TextStyle::Heading, FontId::proportional(24.0)),
                (TextStyle::Body, FontId::proportional(16.0)),
                (TextStyle::Button, FontId::proportional(15.0)),
                (TextStyle::Small, FontId::proportional(12.5)),
                (TextStyle::Monospace, FontId::monospace(15.0)),
            ]
            .into();
            style.spacing.item_spacing = egui::vec2(8.0, 8.0);
            style.spacing.button_padding = egui::vec2(10.0, 4.0);
            style.spacing.interact_size.y = 26.0;
        });
    }
}
