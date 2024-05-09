use nu_plugin::{Plugin, PluginCommand};

mod bridge;
mod commands;
mod plugin;

pub use plugin::HTTPPlugin;

impl Plugin for HTTPPlugin {
    fn commands(&self) -> Vec<Box<dyn PluginCommand<Plugin = Self>>> {
        vec![
            Box::new(crate::commands::HTTPGet),
            Box::new(crate::commands::HTTPServe),
        ]
    }
}
