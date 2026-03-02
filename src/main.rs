mod activation;
mod audio;
mod audio_bridge;
mod config;
mod controller;
mod gui_bridge;
mod mcp_gateway;
mod net_link;
mod protocol;
mod state_machine;

use audio_bridge::{AudioBridge, AudioEvent};
use config::Config;
use controller::CoreController;
use gui_bridge::{GuiBridge, GuiEvent};

use mac_address::get_mac_address;
use net_link::{NetCommand, NetEvent, NetLink};
use std::sync::Arc;
use tokio::signal;
use tokio::sync::mpsc;
use uuid::Uuid;
use crate::mcp_gateway::init_mcp_gateway;

/// 打印帮助信息（含版本、用法、音频设备探测）
fn print_help() {
    let name = env!("APP_NAME");
    let version = env!("APP_VERSION");
    println!("{} v{}", name, version);
    println!();
    println!("用法: {} [选项]", name);
    println!();
    println!("选项:");
    println!("  -h, --help           显示帮助信息并列出可用音频设备");
    println!("  --list-devices       仅列出可用音频设备");
    println!();
    println!("配置文件:");
    println!("  编译时配置  config.toml        (修改后需重新编译)");
    println!("  运行时配置  xiaozhi_config.json (自动生成，可热修改)");
    println!();
    println!("──────────────────────────────────────────");
    println!("  可用音频设备");
    println!("──────────────────────────────────────────");
    println!();
    audio::AudioDevice::print_audio_devices();
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 解析命令行参数
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "-h" || a == "--help") {
        print_help();
        return Ok(());
    }
    if args.iter().any(|a| a == "--list-devices") {
        audio::AudioDevice::print_audio_devices();
        return Ok(());
    }

    // 初始化日志
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format(|buf, record| {
            use std::io::Write;
            writeln!(
                buf,
                "[{} {:<5}] {}",
                buf.timestamp(),
                record.level(),
                record.args()
            )
        })
        .init();

    // 加载配置（若不存在则根据编译时默认生成并持久化）
    let mut config = Config::load_or_create()?;

    // 立即进行严格校验 (Fail Fast)
    if let Err(e) = config.validate() {
        log::error!("🛑 程序启动失败：{}", e);
        std::process::exit(1);
    }

    // 设备id和客户端id的处理
    let mut config_dirty = false;
    if config.device_id == "unknown-device" {
        config.device_id = match get_mac_address() {
            Ok(Some(mac)) => mac.to_string().to_lowercase(),
            _ => Uuid::new_v4().to_string(),
        };
        config_dirty = true;
    }

    if config.client_id == "unknown-client" {
        config.client_id = Uuid::new_v4().to_string();
        log::info!("Generated new Client ID: {}", config.client_id);
        config_dirty = true;
    }

    if config_dirty {
        if let Err(e) = config.save() {
            log::error!("Failed to persist updated config: {}", e);
        }
    }

    // 初始化 MCP Gateway 工具箱
    let mcp_configs = if config.mcp.enabled {
        log::info!("MCP Gateway is enabled. Loaded {} tools from configuration.", config.mcp.tools.len());
        config.mcp.tools.clone()
    } else {
        log::info!("MCP Gateway is disabled.");
        vec![]
    };

    let mcp_server = Arc::new(init_mcp_gateway(mcp_configs));

    // 创建通道，用于组件间通信
    // 事件通道
    let (tx_net_event, mut rx_net_event) = mpsc::channel::<NetEvent>(100);

    // 命令通道
    let (tx_net_cmd, rx_net_cmd) = mpsc::channel::<NetCommand>(100);

    // 音频进程通道
    let (tx_audio_event, mut rx_audio_event) = mpsc::channel::<AudioEvent>(100);

    // GUI进程通道
    let (tx_gui_event, mut rx_gui_event) = mpsc::channel::<GuiEvent>(100);

    // 启动GUI桥，与GUI进程通信，优先启动，用于播报激活状态或者激活码
    let gui_bridge = Arc::new(GuiBridge::new(&config, tx_gui_event).await?);
    // clone一份，用于异步任务，还要用原始的gui_bridge在主循环中发送消息
    let gui_bridge_clone = gui_bridge.clone();
    tokio::spawn(async move {
        if let Err(e) = gui_bridge_clone.run().await {
            log::error!("GuiBridge error: {}", e);
        }
    });

    // 在启动 NetLink 前检查激活
    loop {
        match activation::check_device_activation(&config).await {
            activation::ActivationResult::Activated => {
                log::info!("Device is activated. Starting WebSocket...");
                if let Err(e) = gui_bridge
                    .send_message(r#"{"type":"toast", "text":"设备已激活"}"#)
                    .await
                {
                    log::error!("Failed to send GUI message: {}", e);
                }
                break; // 跳出循环，继续下面的 NetLink 启动
            }
            activation::ActivationResult::NeedActivation(code) => {
                log::info!("Device NOT activated. Code: {}", code);

                // GUI 显示验证码
                let gui_msg = format!(r#"{{"type":"activation", "code":"{}"}}"#, code);
                if let Err(e) = gui_bridge.send_message(&gui_msg).await {
                    log::error!("Failed to send GUI message: {}", e);
                }

                // TTS 播报
                // 如果支持的话，可以设置在这里
                // audio_bridge.speak_text(format!("请在手机输入验证码 {}", code)).await;

                // 等待几秒再轮询
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            }
            activation::ActivationResult::Error(e) => {
                log::error!("Activation check error: {}. Retrying in 5s...", e);
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            }
        }
    }

    // 启动网络链接，与小智服务器通信
    let net_link = NetLink::new(config.clone(), tx_net_event, rx_net_cmd, mcp_server);
    tokio::spawn(async move {
        net_link.run().await;
    });

    // 启动音频桥（内置音频系统，无需外部进程）
    let audio_bridge = Arc::new(AudioBridge::start(&config, tx_audio_event)?);

    // 初始化控制器
    let mut controller = CoreController::new(
        config.clone(),
        tx_net_cmd,
        audio_bridge,
        gui_bridge,
    );

    log::info!("Xiaozhi Core Started. Entering Event Loop...");

    loop {
        tokio::select! {
            _ = signal::ctrl_c() => {
                log::info!("Received Ctrl+C, shutting down...");
                break;
            }
            Some(event) = rx_net_event.recv() => controller.handle_net_event(event).await,
            Some(event) = rx_audio_event.recv() => controller.handle_audio_event(event).await,
            Some(event) = rx_gui_event.recv() => controller.handle_gui_event(event).await,
        }
    }
    Ok(())
}
