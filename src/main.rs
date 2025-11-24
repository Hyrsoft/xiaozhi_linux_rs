mod audio_bridge;
mod config;
mod gui_bridge;
mod net_link;
mod state_machine;

use audio_bridge::{AudioBridge, AudioEvent};
use config::Config;
use gui_bridge::{GuiBridge, GuiEvent};
use net_link::{NetCommand, NetEvent, NetLink};
use state_machine::SystemState;
use std::sync::Arc;
use tokio::sync::mpsc;
use serde::Deserialize;

#[derive(Deserialize)]
struct ServerMessage {
    #[serde(rename = "type")]
    msg_type: String,
    // For "iot" type
    command: Option<String>,
    // For "tts" type
    text: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志
    env_logger::init();

    // 加载配置
    let config = Config::new().unwrap_or_default();

    // 创建通道，用于组件间通信

    // 事件通道
    let (tx_net_event, mut rx_net_event) = mpsc::channel::<NetEvent>(100);

    // 命令通道
    let (tx_net_cmd, rx_net_cmd) = mpsc::channel::<NetCommand>(100);

    // 音频通道
    let (tx_audio_event, mut rx_audio_event) = mpsc::channel::<AudioEvent>(100);

    // GUI通道
    let (tx_gui_event, mut rx_gui_event) = mpsc::channel::<GuiEvent>(100);

    // 启动网络链接，与小智服务器通信
    let net_link = NetLink::new(config.clone(), tx_net_event, rx_net_cmd);
    tokio::spawn(async move {
        net_link.run().await;
    });

    // 启动音频桥，与音频进程通信
    let audio_bridge = Arc::new(AudioBridge::new(&config, tx_audio_event).await?);
    let audio_bridge_clone = audio_bridge.clone();
    tokio::spawn(async move {
        if let Err(e) = audio_bridge_clone.run().await {
            eprintln!("AudioBridge error: {}", e);
        }
    });

    // 启动GUI桥，与GUI进程通信
    let gui_bridge = Arc::new(GuiBridge::new(&config, tx_gui_event).await?);
    let gui_bridge_clone = gui_bridge.clone();
    tokio::spawn(async move {
        if let Err(e) = gui_bridge_clone.run().await {
            eprintln!("GuiBridge error: {}", e);
        }
    });

    // 主事件循环，处理各组件事件
    // 监听来自NetLink、AudioBridge和GuiBridge的事件，并进行相应处理
    let mut current_state = SystemState::Idle;
    println!("Xiaozhi Core Started. State: {:?}", current_state);

    loop {
        tokio::select! {

            // 监听与服务器的网络事件
            Some(event) = rx_net_event.recv() => {
                match event {

                    // 如果接收到服务器的文本消息，就转发给GUI
                    NetEvent::Text(text) => {
                        println!("Received Text from Server: {}", text);
                        
                        // Try to parse as JSON to handle specific message types
                        if let Ok(msg) = serde_json::from_str::<ServerMessage>(&text) {
                            match msg.msg_type.as_str() {
                                "iot" => {
                                    if let Some(cmd) = msg.command {
                                        println!("Received IoT Command: {}", cmd);
                                        // TODO: Handle IoT command (e.g. call HomeAssistant API)
                                    }
                                }
                                "tts" => {
                                    // Forward TTS text to GUI if needed, or just log
                                    if let Some(t) = msg.text {
                                        println!("TTS: {}", t);
                                    }
                                }
                                _ => {}
                            }
                        }

                        if let Err(e) = gui_bridge.send_message(&text).await {
                            eprintln!("Failed to send to GUI: {}", e);
                        }
                    }

                    // 如果接收到服务器的二进制音频数据，就转发给音频桥播放
                    NetEvent::Binary(data) => {
                        // println!("Received Audio from Server: {} bytes", data.len());
                        if current_state != SystemState::Speaking {
                            current_state = SystemState::Speaking;
                            // Notify GUI: kDeviceStateSpeaking = 6
                            let _ = gui_bridge.send_message(r#"{"state": 6}"#).await;
                        }
                        // Forward to Audio
                        if let Err(e) = audio_bridge.send_audio(&data).await {
                            eprintln!("Failed to send to Audio: {}", e);
                        }
                    }

                    // 连接状态变化
                    NetEvent::Connected => {
                        println!("WebSocket Connected");
                        // Notify GUI: kDeviceStateIdle = 3
                        let _ = gui_bridge.send_message(r#"{"state": 3}"#).await;
                    }
                    NetEvent::Disconnected => {
                        println!("WebSocket Disconnected");
                        current_state = SystemState::NetworkError;
                        // Notify GUI: kDeviceStateConnecting = 4 (or Error = 9)
                        let _ = gui_bridge.send_message(r#"{"state": 4}"#).await;
                        
                        // TODO: Clear audio buffer if any
                    }
                }
            }

            // 监听来自音频桥的音频事件
            Some(event) = rx_audio_event.recv() => {
                match event {
                    AudioEvent::AudioData(data) => {
                        // println!("Received Audio from Mic: {} bytes", data.len());
                        
                        // Always forward audio to server to support barge-in (interruption)
                        // The server VAD will decide if user is speaking and send stop command
                        
                        if current_state != SystemState::Listening {
                             current_state = SystemState::Listening;
                             // Notify GUI: kDeviceStateListening = 5
                             let _ = gui_bridge.send_message(r#"{"state": 5}"#).await;
                        }
                        // Forward to Server
                        let _ = tx_net_cmd.send(NetCommand::SendBinary(data)).await;
                    }
                    AudioEvent::Command(cmd) => {
                        println!("Received Command from AudioBridge: {:?}", cmd);
                        // Handle commands from sound_app if any
                    }
                }
            }

            // 监听来自GUI桥的GUI事件
            Some(event) = rx_gui_event.recv() => {
                match event {
                    GuiEvent::Message(msg) => {
                        println!("Received Message from GUI: {}", msg);
                        // Forward to Server
                        let _ = tx_net_cmd.send(NetCommand::SendText(msg)).await;
                    }
                }
            }
        }
    }
}
