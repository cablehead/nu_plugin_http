
use tokio::runtime::{Builder, Runtime};

pub struct HTTPPlugin {
    pub runtime: Runtime,
}

impl HTTPPlugin {
    pub fn new() -> Self {
        let runtime = Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Failed to create Tokio runtime");
        HTTPPlugin { runtime }
    }
}

impl Default for HTTPPlugin {
    fn default() -> Self {
        Self::new()
    }
}
