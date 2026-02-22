use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::{borrow::Cow, fs, path::Path};
use uuid::Uuid;
use crate::mcp_gateway::ExternalToolConfig;

const CONFIG_FILE_NAME: &str = "xiaozhi_config.json";

/// 网络下发流的编码格式（源格式）
#[derive(Debug, Deserialize, Serialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AudioStreamFormat {
    Opus,
    Mp3,
    Pcm,
}

impl AudioStreamFormat {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Opus => "opus",
            Self::Mp3 => "mp3",
            Self::Pcm => "pcm",
        }
    }
}

impl std::fmt::Display for AudioStreamFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct McpConfig {
    pub enabled: bool,
    #[serde(default)]
    pub tools: Vec<ExternalToolConfig>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    // 音频设备配置
    pub capture_device: Cow<'static, str>,
    pub playback_device: Cow<'static, str>,
    pub stream_format: AudioStreamFormat,
    pub playback_sample_rate: u32,
    pub playback_channels: u32,
    pub playback_period_size: usize,

    // GUI进程配置
    pub gui_local_port: u16,
    pub gui_remote_port: u16,
    pub gui_local_ip: Cow<'static, str>,
    pub gui_remote_ip: Cow<'static, str>,
    pub gui_buffer_size: usize,

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

    // 功能开关
    pub enable_tts_display: bool,

    // MCP配置
    pub mcp: McpConfig,
}

impl Config {
    /// 返回配置文件路径
    fn config_path() -> &'static Path {
        Path::new(CONFIG_FILE_NAME)
    }

    /// 从编译时设置的环境变量创建配置
    /// 所有参数都在编译时从 config.toml 中读取
    fn default_from_build() -> Result<Self, &'static str> {
        // 解析编译时嵌入的 stream_format 字符串为枚举
        let stream_format = match env!("AUDIO_STREAM_FORMAT") {
            "opus" => AudioStreamFormat::Opus,
            "mp3" => AudioStreamFormat::Mp3,
            "pcm" => AudioStreamFormat::Pcm,
            _ => return Err("Invalid AUDIO_STREAM_FORMAT value"),
        };

        Ok(Self {
            // 音频设备配置
            capture_device: Cow::Borrowed(env!("AUDIO_CAPTURE_DEVICE")),
            playback_device: Cow::Borrowed(env!("AUDIO_PLAYBACK_DEVICE")),
            stream_format,
            playback_sample_rate: env!("AUDIO_PLAYBACK_SAMPLE_RATE")
                .parse()
                .map_err(|_| "Failed to parse AUDIO_PLAYBACK_SAMPLE_RATE")?,
            playback_channels: env!("AUDIO_PLAYBACK_CHANNELS")
                .parse()
                .map_err(|_| "Failed to parse AUDIO_PLAYBACK_CHANNELS")?,
            playback_period_size: env!("AUDIO_PLAYBACK_PERIOD_SIZE")
                .parse()
                .map_err(|_| "Failed to parse AUDIO_PLAYBACK_PERIOD_SIZE")?,

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

            // 功能开关
            enable_tts_display: env!("ENABLE_TTS_DISPLAY")
                .parse()
                .map_err(|_| "Failed to parse ENABLE_TTS_DISPLAY")?,

            // MCP配置
            mcp: serde_json::from_str(env!("MCP_CONFIG_JSON"))
                .map_err(|_| "Failed to parse MCP_CONFIG_JSON")?,
        })
    }

    /// 校验配置参数的合法性（Fail Fast）
    pub fn validate(&self) -> anyhow::Result<()> {
        // 校验音频格式支持情况
        match self.stream_format {
            AudioStreamFormat::Opus => {
                log::info!("音频流格式校验通过: opus");
            }
            AudioStreamFormat::Mp3 => {
                anyhow::bail!(
                    "配置错误：当前版本尚未支持 mp3 格式解码，请改回 opus"
                );
            }
            AudioStreamFormat::Pcm => {
                log::info!("音频流格式校验通过: pcm (原始PCM直通)");
            }
        }

        // 校验采样率是否在合理范围
        if self.hello_sample_rate < 8000 || self.hello_sample_rate > 48000 {
            anyhow::bail!(
                "配置错误：hello采样率 {}Hz 不合法 (支持 8000-48000)",
                self.hello_sample_rate
            );
        }

        if self.playback_sample_rate < 8000 || self.playback_sample_rate > 192000 {
            anyhow::bail!(
                "配置错误：播放采样率 {}Hz 不合法 (支持 8000-192000)",
                self.playback_sample_rate
            );
        }

        Ok(())
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

// 为 Config 实现 Default trait，使用编译时环境变量的默认值
impl Default for Config {
    fn default() -> Self {
        Self::default_from_build()
            .expect("Failed to create default Config from build-time environment variables")
    }
}
