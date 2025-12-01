use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Deserialize)]
struct Config {
    application: Application,
    board: Board,
    audio: Audio,
    gui: Gui,
    iot: Iot,
    network: Network,
    hello_message: HelloMessage,
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
    local_port: u16,
    remote_port: u16,
    local_ip: String,
    remote_ip: String,
    buffer_size: usize,
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
struct Iot {
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

    // 音频配置
    println!(
        "cargo:rustc-env=AUDIO_LOCAL_PORT={}",
        config.audio.local_port
    );
    println!(
        "cargo:rustc-env=AUDIO_REMOTE_PORT={}",
        config.audio.remote_port
    );
    println!("cargo:rustc-env=AUDIO_LOCAL_IP={}", config.audio.local_ip);
    println!("cargo:rustc-env=AUDIO_REMOTE_IP={}", config.audio.remote_ip);
    println!(
        "cargo:rustc-env=AUDIO_BUFFER_SIZE={}",
        config.audio.buffer_size
    );

    // GUI 配置
    println!("cargo:rustc-env=GUI_LOCAL_PORT={}", config.gui.local_port);
    println!("cargo:rustc-env=GUI_REMOTE_PORT={}", config.gui.remote_port);
    println!("cargo:rustc-env=GUI_LOCAL_IP={}", config.gui.local_ip);
    println!("cargo:rustc-env=GUI_REMOTE_IP={}", config.gui.remote_ip);
    println!("cargo:rustc-env=GUI_BUFFER_SIZE={}", config.gui.buffer_size);

    // IoT config
    println!("cargo:rustc-env=IOT_LOCAL_PORT={}", config.iot.local_port);
    println!("cargo:rustc-env=IOT_REMOTE_PORT={}", config.iot.remote_port);
    println!("cargo:rustc-env=IOT_LOCAL_IP={}", config.iot.local_ip);
    println!("cargo:rustc-env=IOT_REMOTE_IP={}", config.iot.remote_ip);
    println!("cargo:rustc-env=IOT_BUFFER_SIZE={}", config.iot.buffer_size);

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
}
