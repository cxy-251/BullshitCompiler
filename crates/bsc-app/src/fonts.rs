//! 字体：egui 自带的字体没有中文，需要自己提供中文字体。
//!
//! 为了控制体积，思源黑体 (Noto Sans SC) 裁剪到了 GB2312 的 6763 个常用汉字
//! 加常用符号，约 2.4 MB。裁剪方法见 `assets/fonts/README.md`。
//!
//! - 原生版：字体直接打包进程序。
//! - Web 版：字体是一个**单独的文件**（`fonts/NotoSansSC-Subset.ttf`，文件名固定），
//!   启动时下载。这样每次更新代码只需要重新下载程序本身，字体一直走浏览器缓存。
//!   如果打包进 wasm，每次发布新版本 wasm 的文件名都会变，字体就得跟着重新下载。

use std::sync::Arc;

use eframe::egui::{Context, FontData, FontDefinitions, FontFamily};

const JETBRAINS_MONO: &[u8] = include_bytes!("../assets/fonts/JetBrainsMono-Subset.ttf");

/// 原生版打包进程序的中文字体。
#[cfg(not(target_arch = "wasm32"))]
pub fn bundled_cjk() -> Option<FontData> {
    Some(FontData::from_static(include_bytes!("../assets/fonts/NotoSansSC-Subset.ttf")))
}

/// Web 版中文字体的地址（相对于页面）。`index.html` 里用 trunk 的 copy-file 把字体复制到这里。
#[cfg(target_arch = "wasm32")]
pub const CJK_FONT_URL: &str = "fonts/NotoSansSC-Subset.ttf";

/// 安装字体。`cjk` 为 `None`（Web 版下载失败）时中文会显示成方框，但程序照常可用。
pub fn install(ctx: &Context, cjk: Option<FontData>) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert("jetbrains_mono".to_owned(), Arc::new(FontData::from_static(JETBRAINS_MONO)));
    let mono = fonts.families.entry(FontFamily::Monospace).or_default();
    // 代码：英文用 JetBrains Mono，遇到中文退回思源黑体。
    mono.insert(0, "jetbrains_mono".to_owned());

    if let Some(cjk) = cjk {
        fonts.font_data.insert("noto_sans_sc".to_owned(), Arc::new(cjk));
        fonts.families.entry(FontFamily::Monospace).or_default().insert(1, "noto_sans_sc".to_owned());
        // 正文：优先用思源黑体（中英文风格统一），egui 自带字体作为后备（主要是图标）。
        fonts.families.entry(FontFamily::Proportional).or_default().insert(0, "noto_sans_sc".to_owned());
    }

    ctx.set_fonts(fonts);
}
