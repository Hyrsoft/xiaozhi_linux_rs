use crate::config::Config;
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Deserialize)]
pub struct ActivationResponse {
    pub mqtt: Option<MqttConfig>,
    pub activation: Option<ActivationInfo>,
}

#[derive(Debug, Deserialize)]
pub struct MqttConfig {
    pub endpoint: String,
    pub client_id: String,
}

#[derive(Debug, Deserialize)]
pub struct ActivationInfo {
    pub code: String,
    pub message: String,
}

pub enum ActivationResult {
    Activated,
    NeedActivation(String), // 包含 6 位验证码
    Error(String),
}

pub async fn check_device_activation(config: &Config) -> ActivationResult {
    // 构造 HTTP URL
    // 从配置文件读取
    let http_url = config.ota_url.as_ref();

    let client = Client::new();

    println!("Checking activation status via HTTP: {}", http_url);

    // 构造请求体
    let body = json!({
        "uuid": config.client_id,
        "application": {
            "name": env!("APP_NAME"),
            "version": env!("APP_VERSION")
        },
        "ota": {},
        "board": {
            "type": env!("BOARD_TYPE"),
            "name": env!("BOARD_NAME")
        }
    });

    // 构造请求
    // 参考 C++ control_center.cpp 中的 headers
    // 不包含 Authorization 和 Protocol-Version
    let response = client
        .post(http_url)
        .header("Device-Id", &config.device_id)
        .header("Content-Type", "application/json")
        .header("User-Agent", "weidongshan1")
        .header("Accept-Language", "zh-CN")
        .json(&body)
        .send()
        .await;

    match response {
        Ok(resp) => {
            if resp.status().is_success() {
                // 解析 JSON
                match resp.json::<serde_json::Value>().await {
                    Ok(json) => {
                        // 检查是否有 "activation" 字段
                        if let Some(activation) = json.get("activation") {
                            if let Some(code) = activation.get("code") {
                                let code_str = code.as_str().unwrap_or("").to_string();
                                return ActivationResult::NeedActivation(code_str);
                            }
                        }
                        // 如果没有 activation 字段，或者字段为空，视为已激活
                        return ActivationResult::Activated;
                    }
                    Err(e) => ActivationResult::Error(format!("JSON parse error: {}", e)),
                }
            } else {
                ActivationResult::Error(format!("HTTP Error: {}", resp.status()))
            }
        }
        Err(e) => ActivationResult::Error(format!("Request failed: {}", e)),
    }
}
