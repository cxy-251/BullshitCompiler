use bsc_core::IntentModule;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PassDiffRecord {
    pub pass_name: String,
    pub description: String,
    pub ir_dump_before: String,
    pub ir_dump_after: String,
    pub modified: bool,
}

pub trait TransformPass {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn run(&mut self, module: &mut IntentModule) -> bool;
}

pub struct PassManager {
    passes: Vec<Box<dyn TransformPass>>,
    history: Vec<PassDiffRecord>,
}

impl PassManager {
    pub fn new() -> Self {
        Self {
            passes: Vec::new(),
            history: Vec::new(),
        }
    }

    pub fn add_pass(&mut self, pass: Box<dyn TransformPass>) {
        self.passes.push(pass);
    }

    pub fn run_pipeline(&mut self, module: &mut IntentModule) -> &[PassDiffRecord] {
        self.history.clear();
        for pass in &mut self.passes {
            let before = module.format_dump();
            let modified = pass.run(module);
            let after = module.format_dump();

            self.history.push(PassDiffRecord {
                pass_name: pass.name().to_string(),
                description: pass.description().to_string(),
                ir_dump_before: before,
                ir_dump_after: after,
                modified,
            });
        }
        &self.history
    }

    pub fn get_history(&self) -> &[PassDiffRecord] {
        &self.history
    }
}
