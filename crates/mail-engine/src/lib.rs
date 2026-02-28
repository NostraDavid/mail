use anyhow::Result;
use tracing::info;

pub struct Engine {
    app_name: String,
}

impl Engine {
    pub fn new(app_name: impl Into<String>) -> Self {
        Self {
            app_name: app_name.into(),
        }
    }

    pub async fn start(&self) -> Result<()> {
        info!("engine start: {}", self.app_name);
        Ok(())
    }
}
