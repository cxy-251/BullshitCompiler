pub mod emitter;
pub mod targets;

pub use emitter::BackendEmitter;
pub use targets::{AlibabaEmitter, HuaweiEmitter, SiliconValleyEmitter, StateOwnedEmitter};

use bsc_core::IntentModule;

pub fn emit_target(target: &str, module: &IntentModule) -> Result<String, String> {
    match target.to_lowercase().as_str() {
        "huawei" => Ok(HuaweiEmitter.emit(module)),
        "alibaba" => Ok(AlibabaEmitter.emit(module)),
        "state_owned" | "gov" => Ok(StateOwnedEmitter.emit(module)),
        "silicon_valley" | "english" => Ok(SiliconValleyEmitter.emit(module)),
        other => Err(format!("Unknown compilation target '{}'. Supported: huawei, alibaba, state_owned, silicon_valley", other)),
    }
}
