use bsc_core::IntentModule;

pub trait BackendEmitter {
    fn target_name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn emit(&self, module: &IntentModule) -> String;
}
