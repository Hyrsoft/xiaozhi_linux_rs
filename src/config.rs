use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::{borrow::Cow, fs, path::Path};
use uuid::Uuid;

const CONFIG_FILE_NAME: &str = "xiaozhi_config.json";

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    // 音频进程配置
    pub audio_local_port: u16,
    pub audio_remote_port: u16,
    pub audio_local_ip: Cow<'static, str>,
    pub audio_remote_ip: Cow<'static, str>,
    pub audio_buffer_size: usize,

    // GUI进程配置
    pub gui_local_port: u16,
    pub gui_remote_port: u16,
    pub gui_local_ip: Cow<'static, str>,
    pub gui_remote_ip: Cow<'static, str>,
    pub gui_buffer_size: usize,

    // IoT进程配置
    pub iot_local_port: u16,
    pub iot_remote_port: u16,
    pub iot_local_ip: Cow<'static, str>,
    pub iot_remote_ip: Cow<'static, str>,
    pub iot_buffer_size: usize,

    // 网络配置（静态部分）
    pub ws_url: Cow<'static, str>,
    pub ota_url: Cow<'static, str>,
    pub ws_token: Cow<'static, str>,

    // 设备标识（动态部分，可在运行时修改）
    pub device_id: String,
    pub client_id: String,

    // Hello消息参数
    pub hello_format: Cow<'static, str>,
    pub hello_sample_rate: u32,
    pub hello_channels: u8,
    pub hello_frame_duration: u32,
}

impl Config {
    /// 返回配置文件路径
    fn config_path() -> &'static Path {
        Path::new(CONFIG_FILE_NAME)
    }

    /// 从编译时设置的环境变量创建配置
    /// 所有参数都在编译时从 config.toml 中读取
    fn default_from_build() -> Result<Self, &'static str> {
        Ok(Self {
            // 音频进程配置
            audio_local_port: env!("AUDIO_LOCAL_PORT")
                .parse()
                .map_err(|_| "Failed to parse AUDIO_LOCAL_PORT")?,
            audio_remote_port: env!("AUDIO_REMOTE_PORT")
                .parse()
                .map_err(|_| "Failed to parse AUDIO_REMOTE_PORT")?,
            audio_local_ip: Cow::Borrowed(env!("AUDIO_LOCAL_IP")),
            audio_remote_ip: Cow::Borrowed(env!("AUDIO_REMOTE_IP")),
            audio_buffer_size: env!("AUDIO_BUFFER_SIZE")
                .parse()
                .map_err(|_| "Failed to parse AUDIO_BUFFER_SIZE")?,

            // GUI进程配置
            gui_local_port: env!("GUI_LOCAL_PORT")
                .parse()
                .map_err(|_| "Failed to parse GUI_LOCAL_PORT")?,
            gui_remote_port: env!("GUI_REMOTE_PORT")
                .parse()
                .map_err(|_| "Failed to parse GUI_REMOTE_PORT")?,
            gui_local_ip: Cow::Borrowed(env!("GUI_LOCAL_IP")),
            gui_remote_ip: Cow::Borrowed(env!("GUI_REMOTE_IP")),
            gui_buffer_size: env!("GUI_BUFFER_SIZE")
                .parse()
                .map_err(|_| "Failed to parse GUI_BUFFER_SIZE")?,

            // IoT进程配置
            iot_local_port: env!("IOT_LOCAL_PORT")
                .parse()
                .map_err(|_| "Failed to parse IOT_LOCAL_PORT")?,
            iot_remote_port: env!("IOT_REMOTE_PORT")
                .parse()
                .map_err(|_| "Failed to parse IOT_REMOTE_PORT")?,
            iot_local_ip: Cow::Borrowed(env!("IOT_LOCAL_IP")),
            iot_remote_ip: Cow::Borrowed(env!("IOT_REMOTE_IP")),
            iot_buffer_size: env!("IOT_BUFFER_SIZE")
                .parse()
                .map_err(|_| "Failed to parse IOT_BUFFER_SIZE")?,

            // 网络配置
            ws_url: Cow::Borrowed(env!("WS_URL")),
            ota_url: Cow::Borrowed(env!("OTA_URL")),
            ws_token: Cow::Borrowed(env!("WS_TOKEN")),

            // 设备标识初始化为config.toml中的值
            device_id: env!("DEVICE_ID").to_string(),
            client_id: env!("CLIENT_ID").to_string(),

            // Hello消息参数
            hello_format: Cow::Borrowed(env!("HELLO_FORMAT")),
            hello_sample_rate: env!("HELLO_SAMPLE_RATE")
                .parse()
                .map_err(|_| "Failed to parse HELLO_SAMPLE_RATE")?,
            hello_channels: env!("HELLO_CHANNELS")
                .parse()
                .map_err(|_| "Failed to parse HELLO_CHANNELS")?,
            hello_frame_duration: env!("HELLO_FRAME_DURATION")
                .parse()
                .map_err(|_| "Failed to parse HELLO_FRAME_DURATION")?,
        })
    }

    /// 加载现有配置或使用默认值创建新文件
    pub fn load_or_create() -> anyhow::Result<Self> {
        let path = Self::config_path();
        if path.exists() {
            let content = fs::read_to_string(path)
                .with_context(|| format!("Failed to read {}", path.display()))?;
            let mut config: Config = serde_json::from_str(&content)
                .with_context(|| format!("Failed to parse {}", path.display()))?;

            if config.client_id.trim().is_empty() || config.client_id == "unknown-client" {
                config.client_id = Uuid::new_v4().to_string();
                config.save()?;
            }

            Ok(config)
        } else {
            let mut config = Self::default_from_build().map_err(anyhow::Error::msg)?;

            if config.client_id.trim().is_empty() || config.client_id == "unknown-client" {
                config.client_id = Uuid::new_v4().to_string();
            }

            config.save()?;
            Ok(config)
        }
    }

    /// 将当前配置写回磁盘
    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::config_path();
        let json = serde_json::to_string_pretty(self)?;
        fs::write(path, json).with_context(|| format!("Failed to write {}", path.display()))
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::default_from_build()
            .expect("Failed to create default Config from build-time environment variables")
    }
}
