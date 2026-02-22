use serde::Deserialize;
use std::fs;
use std::path::Path;
use std::env;

#[derive(Deserialize)]
struct Config {
    application: Application,
    board: Board,
    audio: Audio,
    gui: Gui,
    network: Network,
    hello_message: HelloMessage,
    features: Features,
    mcp: serde_json::Value,
}

#[derive(Deserialize)]
struct Application {
    name: String,
    version: String,
}

#[derive(Deserialize)]
struct Board {
    #[serde(rename = "type")]
    type_: String,
    name: String,
}

#[derive(Deserialize)]
struct Audio {
    capture_device: String,
    playback_device: String,
    stream_format: String,
    playback_sample_rate: u32,
    playback_channels: u32,
    playback_period_size: usize,
}

#[derive(Deserialize)]
struct Gui {
    local_port: u16,
    remote_port: u16,
    local_ip: String,
    remote_ip: String,
    buffer_size: usize,
}

#[derive(Deserialize)]
struct Network {
    ws_url: String,
    ota_url: String,
    ws_token: String,
    device_id: String,
    client_id: String,
}

#[derive(Deserialize)]
struct HelloMessage {
    format: String,
    sample_rate: u32,
    channels: u8,
    frame_duration: u32,
}

#[derive(Deserialize)]
struct Features {
    enable_tts_display: bool,
}

// 在编译时读取 config.toml 并设置环境变量
fn main() {
    println!("cargo:rerun-if-changed=config.toml");

    let config_path = Path::new("config.toml");
    if !config_path.exists() {
        panic!("config.toml not found!");
    }

    let config_str = fs::read_to_string(config_path).expect("Failed to read config.toml");
    let config: Config = toml::from_str(&config_str).expect("Failed to parse config.toml");

    // 应用和板子信息
    println!("cargo:rustc-env=APP_NAME={}", config.application.name);
    println!("cargo:rustc-env=APP_VERSION={}", config.application.version);
    println!("cargo:rustc-env=BOARD_TYPE={}", config.board.type_);
    println!("cargo:rustc-env=BOARD_NAME={}", config.board.name);

    // 音频设备配置
    println!(
        "cargo:rustc-env=AUDIO_CAPTURE_DEVICE={}",
        config.audio.capture_device
    );
    println!(
        "cargo:rustc-env=AUDIO_PLAYBACK_DEVICE={}",
        config.audio.playback_device
    );
    println!(
        "cargo:rustc-env=AUDIO_STREAM_FORMAT={}",
        config.audio.stream_format
    );
    println!(
        "cargo:rustc-env=AUDIO_PLAYBACK_SAMPLE_RATE={}",
        config.audio.playback_sample_rate
    );
    println!(
        "cargo:rustc-env=AUDIO_PLAYBACK_CHANNELS={}",
        config.audio.playback_channels
    );
    println!(
        "cargo:rustc-env=AUDIO_PLAYBACK_PERIOD_SIZE={}",
        config.audio.playback_period_size
    );

    // GUI 配置
    println!("cargo:rustc-env=GUI_LOCAL_PORT={}", config.gui.local_port);
    println!("cargo:rustc-env=GUI_REMOTE_PORT={}", config.gui.remote_port);
    println!("cargo:rustc-env=GUI_LOCAL_IP={}", config.gui.local_ip);
    println!("cargo:rustc-env=GUI_REMOTE_IP={}", config.gui.remote_ip);
    println!("cargo:rustc-env=GUI_BUFFER_SIZE={}", config.gui.buffer_size);

    // 网络配置
    println!("cargo:rustc-env=WS_URL={}", config.network.ws_url);
    println!("cargo:rustc-env=OTA_URL={}", config.network.ota_url);
    println!("cargo:rustc-env=WS_TOKEN={}", config.network.ws_token);
    println!("cargo:rustc-env=DEVICE_ID={}", config.network.device_id);
    println!("cargo:rustc-env=CLIENT_ID={}", config.network.client_id);

    // Hello 消息配置
    println!(
        "cargo:rustc-env=HELLO_FORMAT={}",
        config.hello_message.format
    );
    println!(
        "cargo:rustc-env=HELLO_SAMPLE_RATE={}",
        config.hello_message.sample_rate
    );
    println!(
        "cargo:rustc-env=HELLO_CHANNELS={}",
        config.hello_message.channels
    );
    println!(
        "cargo:rustc-env=HELLO_FRAME_DURATION={}",
        config.hello_message.frame_duration
    );

    // 功能开关
    println!(
        "cargo:rustc-env=ENABLE_TTS_DISPLAY={}",
        config.features.enable_tts_display
    );

    // MCP配置
    let mcp_json = serde_json::to_string(&config.mcp).expect("Failed to serialize mcp config");
    println!("cargo:rustc-env=MCP_CONFIG_JSON={}", mcp_json);

    // 交叉编译配置
    let target = env::var("TARGET").unwrap_or_default();
    
    // 只在交叉编译到 uclibc 目标时链接 auxval_stub
    if target.contains("uclibc") {
        // 获取项目根目录
        let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
        let stub_dir = format!("{}/uclibc_stub", manifest_dir);
        
        // 使用 link-arg 确保在最后链接，解决链接顺序问题
        println!("cargo:rustc-link-arg=-L{}", stub_dir);
        println!("cargo:rustc-link-arg=-Wl,--push-state,--whole-archive");
        println!("cargo:rustc-link-arg=-lauxval_stub");
        println!("cargo:rustc-link-arg=-Wl,--pop-state");
    }

    if target.contains("musl") {
        // musl 目标：使用手动编译的静态库，不依赖 pkg-config
        if let Ok(sysroot) = std::env::var("MUSL_SYSROOT") {
            println!("cargo:rustc-link-search=native={}/usr/lib", sysroot);
        }
        println!("cargo:rustc-link-lib=static=speexdsp");
    } else {
        // 动态链接或其他目标：通过 pkg-config 查找 libspeexdsp
        pkg_config::Config::new()
            .probe("speexdsp")
            .expect("Failed to find speexdsp. Please install libspeexdsp-dev.");
    }
}
