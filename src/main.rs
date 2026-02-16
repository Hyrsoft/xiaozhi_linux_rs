mod activation;
mod audio_bridge;
mod config;
mod controller;
mod gui_bridge;
mod iot_bridge;
mod net_link;
mod protocol;
mod state_machine;

use audio_bridge::{AudioBridge, AudioEvent};
use config::Config;
use controller::CoreController;
use gui_bridge::{GuiBridge, GuiEvent};
use iot_bridge::{IotBridge, IotEvent};
use mac_address::get_mac_address;
use net_link::{NetCommand, NetEvent, NetLink};
use std::sync::Arc;
use tokio::signal;
use tokio::sync::mpsc;
use uuid::Uuid;

#[cfg(feature = "tui")]
use xiaozhi_tui::TuiApp;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 加载配置（若不存在则根据编译时默认生成并持久化）
    let mut config = Config::load_or_create()?;

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
        config_dirty = true;
    }

    if config_dirty {
        let _ = config.save();
    }

    // --- TUI 初始化 ---
    // 判断是否启用 TUI，如果启用则不初始化 env_logger（避免输出到终端干扰 TUI）
    #[cfg(feature = "tui")]
    let tui_active = config.enable_tui;
    #[cfg(not(feature = "tui"))]
    let tui_active = false;

    if !tui_active {
        // 仅在 TUI 关闭时初始化终端日志
        env_logger::init();
    }

    #[cfg(feature = "tui")]
    let tui_tx: Option<mpsc::Sender<xiaozhi_tui::TuiCommand>> = if config.enable_tui {
        let (tui_app, tx) = TuiApp::new();

        // 启动 TUI 事件循环
        tokio::spawn(async move {
            if let Err(e) = tui_app.run().await {
                log::error!("TUI error: {}", e);
            }
        });

        Some(tx)
    } else {
        None
    };

    #[cfg(not(feature = "tui"))]
    let _tui_placeholder = (); // 无 TUI 功能时的占位

    // 创建通道，用于组件间通信
    // 事件通道
    let (tx_net_event, mut rx_net_event) = mpsc::channel::<NetEvent>(100);

    // 命令通道
    let (tx_net_cmd, rx_net_cmd) = mpsc::channel::<NetCommand>(100);

    // 音频进程通道
    let (tx_audio_event, mut rx_audio_event) = mpsc::channel::<AudioEvent>(100);

    // GUI进程通道
    let (tx_gui_event, mut rx_gui_event) = mpsc::channel::<GuiEvent>(100);

    // IOT进程通道
    let (tx_iot_event, mut rx_iot_event) = mpsc::channel::<IotEvent>(100);

    // 启动GUI桥，与GUI进程通信，优先启动，用于播报激活状态或者激活码
    let gui_bridge = Arc::new(GuiBridge::new(&config, tx_gui_event).await?);
    // clone一份，用于异步任务，还要用原始的gui_bridge在主循环中发送消息
    let gui_bridge_clone = gui_bridge.clone();
    tokio::spawn(async move {
        if let Err(e) = gui_bridge_clone.run().await {
            log::error!("GuiBridge error: {}", e);
        }
    });

    // 启动IOT桥，与IOT进程通信
    let iot_bridge = Arc::new(IotBridge::new(&config, tx_iot_event).await?);
    let iot_bridge_clone = iot_bridge.clone();
    tokio::spawn(async move {
        if let Err(e) = iot_bridge_clone.run().await {
            log::error!("IotBridge error: {}", e);
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
    let net_link = NetLink::new(config.clone(), tx_net_event, rx_net_cmd);
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
        iot_bridge,
        #[cfg(feature = "tui")]
        tui_tx,
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
            Some(event) = rx_iot_event.recv() => controller.handle_iot_event(event).await,
        }
    }
    Ok(())
}
