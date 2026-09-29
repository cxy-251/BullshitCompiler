//! BSc 编译原理实验室：同一份代码既编译成原生桌面程序，也编译成 WebAssembly 在浏览器里运行。

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // Windows 发布版不弹控制台窗口

mod app;
mod calc_view;
mod fonts;
mod pages;
mod theme;
mod widgets;

use app::BscApp;

#[cfg(not(target_arch = "wasm32"))]
const APP_TITLE: &str = "BSc 编译原理实验室";

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    env_logger::init();
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title(APP_TITLE)
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([480.0, 360.0]),
        ..Default::default()
    };
    eframe::run_native(APP_TITLE, options, Box::new(|cc| Ok(Box::new(BscApp::new(cc)))))
}

#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;

    eframe::WebLogger::init(log::LevelFilter::Info).ok();

    wasm_bindgen_futures::spawn_local(async {
        let document = eframe::web_sys::window().expect("没有 window 对象").document().expect("没有 document 对象");
        let canvas = document
            .get_element_by_id("bsc_canvas")
            .expect("页面里找不到 id 为 bsc_canvas 的元素")
            .dyn_into::<eframe::web_sys::HtmlCanvasElement>()
            .expect("bsc_canvas 不是 <canvas> 元素");

        let result = eframe::WebRunner::new()
            .start(canvas, eframe::WebOptions::default(), Box::new(|cc| Ok(Box::new(BscApp::new(cc)))))
            .await;

        if let Some(loading) = document.get_element_by_id("loading") {
            match result {
                Ok(()) => loading.remove(),
                Err(e) => {
                    loading.set_inner_html("<p>启动失败，请打开浏览器控制台查看详情。</p>");
                    panic!("启动 eframe 失败: {e:?}");
                }
            }
        }
    });
}
