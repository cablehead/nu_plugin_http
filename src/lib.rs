use nu_plugin::{Plugin, PluginCommand};

mod commands;
mod plugin;
mod bridge;

pub use plugin::HTTPPlugin;

impl Plugin for HTTPPlugin {
    fn commands(&self) -> Vec<Box<dyn PluginCommand<Plugin = Self>>> {
        vec![
            Box::new(crate::commands::HTTPGet),
            Box::new(crate::commands::HTTPServe),
        ]
    }
}
