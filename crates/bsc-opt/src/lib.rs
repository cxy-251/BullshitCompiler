pub mod pass;
pub mod passes;

pub use pass::{PassDiffRecord, PassManager, TransformPass};
pub use passes::{ScaleAmplifyPass, StripAgencyPass, TeleologyInjectPass};

use bsc_core::IntentModule;

/// Prepares the standard default optimization pipeline
pub fn create_default_pipeline() -> PassManager {
    let mut pm = PassManager::new();
    pm.add_pass(Box::new(StripAgencyPass));
    pm.add_pass(Box::new(ScaleAmplifyPass));
    pm.add_pass(Box::new(TeleologyInjectPass));
    pm
}

/// Optimizes an IntentModule and returns step-by-step diff records
pub fn optimize_module(module: &mut IntentModule) -> Vec<PassDiffRecord> {
    let mut pm = create_default_pipeline();
    pm.run_pipeline(module).to_vec()
}
