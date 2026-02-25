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
        let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
        let stub_c = format!("{}/scripts/armv7-unknown-linux-uclibceabihf/auxval_stub.c", manifest_dir);
        
        cc::Build::new()
            .file(&stub_c)
            .compile("auxval_stub");
    }

    // 处理 C 依赖构建 (opus, speexdsp)
    // 交叉编译时从源码构建或通过 pkg-config 查找静态库
    // 本地编译时通过 pkg-config 查找系统动态库
    let host = env::var("HOST").unwrap_or_default();
    if target != host {
        build_or_probe_c_deps(&target);
    } else {
        pkg_config::Config::new()
            .probe("speexdsp")
            .expect("Failed to find speexdsp. Please install libspeexdsp-dev.");
        pkg_config::Config::new()
            .probe("opus")
            .expect("Failed to find opus. Please install libopus-dev.");
    }
}

fn build_or_probe_c_deps(target: &str) {
    let out_dir = env::var("OUT_DIR").unwrap();
    let out_path = Path::new(&out_dir);

    // 1. SpeexDSP
    if pkg_config::Config::new().statik(true).probe("speexdsp").is_err() {
        println!("cargo:warning=speexdsp not found via pkg-config for target {}. Building from source...", target);
        let speexdsp_version = "1.2.1";
        let speexdsp_url = format!("https://github.com/Hyrsoft/xiaozhi_linux_rs/releases/download/Source_Mirror/speexdsp-{}.tar.gz", speexdsp_version);
        let src_dir = download_and_extract(&speexdsp_url, "speexdsp", speexdsp_version, out_path);
        
        let mut config = autotools::Config::new(src_dir);
        config.enable_static().disable_shared();
            
        if target.contains("arm-linux-gnueabihf") || target == "armv7-unknown-linux-gnueabihf" {
            config.config_option("host", Some("arm-linux-gnueabihf"));
        } else if target.contains("arm-rockchip830-linux-uclibcgnueabihf") || target == "armv7-unknown-linux-uclibceabihf" {
            config.config_option("host", Some("arm-rockchip830-linux-uclibcgnueabihf"));
        } else if target.contains("aarch64-linux-gnu") || target == "aarch64-unknown-linux-gnu" {
            config.config_option("host", Some("aarch64-linux-gnu"));
        }
            
        let dst = config.build();
            
        println!("cargo:rustc-link-search=native={}/lib", dst.display());
        println!("cargo:rustc-link-lib=static=speexdsp");
    }

    // 2. Opus
    if pkg_config::Config::new().statik(true).probe("opus").is_err() {
        println!("cargo:warning=opus not found via pkg-config for target {}. Building from source...", target);
        let opus_version = "1.5.2";
        let opus_url = format!("https://github.com/Hyrsoft/xiaozhi_linux_rs/releases/download/Source_Mirror/opus-{}.tar.gz", opus_version);
        let src_dir = download_and_extract(&opus_url, "opus", opus_version, out_path);
        
        let mut config = autotools::Config::new(src_dir);
        config.enable_static()
            .disable_shared()
            .config_option("disable-doc", None)
            .config_option("disable-extra-programs", None);
            
        if target.contains("arm-linux-gnueabihf") || target == "armv7-unknown-linux-gnueabihf" {
            config.config_option("host", Some("arm-linux-gnueabihf"));
        } else if target.contains("arm-rockchip830-linux-uclibcgnueabihf") || target == "armv7-unknown-linux-uclibceabihf" {
            config.config_option("host", Some("arm-rockchip830-linux-uclibcgnueabihf"));
        } else if target.contains("aarch64-linux-gnu") || target == "aarch64-unknown-linux-gnu" {
            config.config_option("host", Some("aarch64-linux-gnu"));
        }
        
        let dst = config.build();
            
        println!("cargo:rustc-link-search=native={}/lib", dst.display());
        println!("cargo:rustc-link-lib=static=opus");
    }
}

fn download_and_extract(url: &str, name: &str, version: &str, out_path: &Path) -> std::path::PathBuf {
    let extract_dir = out_path.join(format!("{}-{}", name, version));
    
    // 如果目录已经存在并且不是空的，就假设已经解压好了
    if extract_dir.exists() && extract_dir.read_dir().map(|mut d| d.next().is_some()).unwrap_or(false) {
        return extract_dir;
    }

    let tarball_path = out_path.join(format!("{}-{}.tar.gz", name, version));

    if !tarball_path.exists() {
        println!("cargo:warning=Downloading {} from {}", name, url);
        let response = reqwest::blocking::get(url).unwrap_or_else(|e| panic!("Failed to download {}: {}", name, e));
        let bytes = response.bytes().unwrap_or_else(|e| panic!("Failed to read bytes for {}: {}", name, e));
        std::fs::write(&tarball_path, bytes).unwrap_or_else(|e| panic!("Failed to save tarball for {}: {}", name, e));
    }

    println!("cargo:warning=Extracting {}...", name);
    let tar_gz = std::fs::File::open(&tarball_path).unwrap();
    let tar = flate2::read::GzDecoder::new(tar_gz);
    let mut archive = tar::Archive::new(tar);
    archive.unpack(out_path).unwrap_or_else(|e| panic!("Failed to unpack archive for {}: {}", name, e));

    extract_dir
}
