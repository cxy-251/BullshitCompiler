//! 字体：egui 自带的字体没有中文，需要把中文字体打包进程序。
//!
//! 为了控制 Web 版体积，思源黑体 (Noto Sans SC) 裁剪到了 GB2312 的 6763 个常用汉字
//! 加常用符号，约 2.4 MB。裁剪方法见 `assets/fonts/README.md`。

use std::sync::Arc;

use eframe::egui::{Context, FontData, FontDefinitions, FontFamily};

const NOTO_SANS_SC: &[u8] = include_bytes!("../assets/fonts/NotoSansSC-Subset.ttf");
const JETBRAINS_MONO: &[u8] = include_bytes!("../assets/fonts/JetBrainsMono-Subset.ttf");

pub fn install(ctx: &Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert("noto_sans_sc".to_owned(), Arc::new(FontData::from_static(NOTO_SANS_SC)));
    fonts.font_data.insert("jetbrains_mono".to_owned(), Arc::new(FontData::from_static(JETBRAINS_MONO)));

    // 正文：优先用思源黑体（中英文风格统一），egui 自带字体作为后备（主要是图标）。
    let proportional = fonts.families.entry(FontFamily::Proportional).or_default();
    proportional.insert(0, "noto_sans_sc".to_owned());

    // 代码：英文用 JetBrains Mono，遇到中文退回思源黑体。
    let mono = fonts.families.entry(FontFamily::Monospace).or_default();
    mono.insert(0, "jetbrains_mono".to_owned());
    mono.insert(1, "noto_sans_sc".to_owned());

    ctx.set_fonts(fonts);
}
