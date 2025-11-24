use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub audio_port_up: u16,
    pub audio_port_down: u16,
    pub ui_port_up: u16,
    pub ui_port_down: u16,
    pub ws_url: String,
    pub ws_token: String,
    pub device_id: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            audio_port_up: 5676,
            audio_port_down: 5677,
            ui_port_up: 5678,
            ui_port_down: 5679,
            // Default values, should be overridden by config file or env vars
            ws_url: "wss://api.xiaozhi.me/v1/ws".to_string(),
            ws_token: "test-token".to_string(),
            device_id: "unknown-device".to_string(),
        }
    }
}
