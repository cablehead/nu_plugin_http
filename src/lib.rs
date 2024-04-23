use nu_plugin::{Plugin, PluginCommand};

mod traits;
mod plugin;
mod commands;

pub use plugin::HTTPPlugin;

impl Plugin for HTTPPlugin {
    fn commands(&self) -> Vec<Box<dyn PluginCommand<Plugin = Self>>> {
        vec![Box::new(crate::commands::HTTPGet)]
    }
}

