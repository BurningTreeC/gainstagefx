use std::sync::OnceLock;

use crate::config::{Au2Config, four_cc_string};

use crate::instance::NicePluginInstance;

pub type PluginFactory = fn() -> Box<dyn NicePluginInstance>;

static PLUGIN_FACTORY: OnceLock<PluginFactory> = OnceLock::new();

static FACTORY_CONFIG: OnceLock<Au2Config> = OnceLock::new();

pub fn register_factory(factory: PluginFactory, config: Au2Config) {
    if PLUGIN_FACTORY.get().is_some() {
        return;
    }

    let _ = PLUGIN_FACTORY.set(factory);

    let _ = FACTORY_CONFIG.set(config);

    if let Some(c) = FACTORY_CONFIG.get() {
        log::debug!(
            "AU factory registered: {} ({} {})",
            c.name,
            four_cc_string(c.manufacturer),
            four_cc_string(c.sub_type)
        );
    }
}

pub fn create_instance() -> Option<Box<dyn NicePluginInstance>> {
    PLUGIN_FACTORY.get().map(|factory| factory())
}

pub fn plugin_config() -> Option<&'static Au2Config> {
    FACTORY_CONFIG.get()
}

pub fn is_registered() -> bool {
    PLUGIN_FACTORY.get().is_some()
}
