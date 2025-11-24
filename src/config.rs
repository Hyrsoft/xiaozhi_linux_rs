use config::{Config as ConfigLoader, File, Environment};
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub audio_local_port: u16,
    pub audio_remote_port: u16,
    pub gui_local_port: u16,
    pub gui_remote_port: u16,
    pub ws_url: String,
    pub ws_token: String,
    pub device_id: String,
}

impl Config {
    pub fn new() -> Result<Self, config::ConfigError> {
        let s = ConfigLoader::builder()
            // 1. Default values
            .set_default("audio_local_port", 5676)?
            .set_default("audio_remote_port", 5677)?
            .set_default("gui_local_port", 5678)?
            .set_default("gui_remote_port", 5679)?
            .set_default("ws_url", "wss://api.xiaozhi.me/v1/ws")?
            .set_default("ws_token", "test-token")?
            .set_default("device_id", "unknown-device")?
            // 2. Read from config file (if exists) /etc/xiaozhi/config.json
            .add_source(File::with_name("/etc/xiaozhi/config").required(false))
            // 3. Read from environment variables (e.g. XIAOZHI_WS_TOKEN=...)
            .add_source(Environment::with_prefix("XIAOZHI"))
            .build()?;

        s.try_deserialize()
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| {
            // Fallback to hardcoded defaults if config loading fails completely
            // This is just to satisfy Default trait, but in main we should use Config::new()
             Self {
                audio_local_port: 5676,
                audio_remote_port: 5677,
                gui_local_port: 5678,
                gui_remote_port: 5679,
                ws_url: "wss://api.xiaozhi.me/v1/ws".to_string(),
                ws_token: "test-token".to_string(),
                device_id: "unknown-device".to_string(),
            }
        })
    }
}
