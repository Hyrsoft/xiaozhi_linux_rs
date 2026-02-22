use crate::audio_bridge::{AudioBridge, AudioEvent};
use crate::config::Config;
use crate::gui_bridge::{GuiBridge, GuiEvent};
use crate::net_link::{NetCommand, NetEvent};
use crate::protocol::ServerMessage;
use crate::state_machine::SystemState;
use serde_json;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::process::Command;
use std::process::Stdio;

pub struct CoreController {
    state: SystemState,
    current_session_id: Option<String>,
    should_mute_mic: bool,
    config: Config,
    net_tx: mpsc::Sender<NetCommand>,
    audio_bridge: Arc<AudioBridge>,
    gui_bridge: Arc<GuiBridge>,
}

impl CoreController {
    pub fn new(
        config: Config,
        net_tx: mpsc::Sender<NetCommand>,
        audio_bridge: Arc<AudioBridge>,
        gui_bridge: Arc<GuiBridge>,
    ) -> Self {
        Self {
            state: SystemState::Idle,
            current_session_id: None,
            should_mute_mic: false,
            config,
            net_tx,
            audio_bridge,
            gui_bridge,
        }
    }

    // 处理来自 NetLink 的事件
    pub async fn handle_net_event(&mut self, event: NetEvent) {
        match event {
            NetEvent::Text(text) => self.process_server_text(text).await,
            NetEvent::Binary(data) => self.process_server_audio(data).await,
            NetEvent::Connected => {
                println!("WebSocket Connected");
                if let Err(e) = self.gui_bridge.send_message(r#"{"state": 3}"#).await {
                    eprintln!("Failed to send to GUI: {}", e);
                }
            }
            NetEvent::Disconnected => {
                println!("WebSocket Disconnected");
                self.state = SystemState::NetworkError;
                if let Err(e) = self.gui_bridge.send_message(r#"{"state": 4}"#).await {
                    eprintln!("Failed to send to GUI: {}", e);
                }
            }
        }
    }

    // 处理来自服务器的文本消息
    async fn process_server_text(&mut self, text: String) {
        println!("Received Text from Server: {}", text);

        let msg: ServerMessage = match serde_json::from_str(&text) {
            Ok(msg) => msg,
            Err(_) => {
                // 可能不是JSON，忽略
                return;
            }
        };

        if let Some(sid) = &msg.session_id {
            if self.current_session_id.as_deref() != Some(sid) {
                println!("New Session ID: {}", sid);
                self.current_session_id = Some(sid.clone());
            }
        }

        match msg.msg_type.as_str() {
            "hello" => {
                println!("Server Hello received. Starting listen mode...");
                // 使用正确的 session_id 发送 listen 命令
                self.send_auto_listen_command().await;
            }
            "iot" => {
                if let Some(cmd) = &msg.command {
                    println!("Processing IoT Command: {}", cmd);
                }
                
                // Fallback: 把接收到的完整 JSON 传递给外部脚本执行
                let fallback_script = "./scripts/mcp_iot_fallback.sh";
                let text_clone = text.clone();
                tokio::spawn(async move {
                    let mut child = match Command::new(fallback_script)
                        .stdin(Stdio::piped())
                        .stdout(Stdio::piped())
                        .stderr(Stdio::piped())
                        .spawn()
                    {
                        Ok(c) => c,
                        Err(e) => {
                            eprintln!("Failed to spawn IoT fallback script {}: {}", fallback_script, e);
                            return;
                        }
                    };
                    
                    if let Some(mut stdin) = child.stdin.take() {
                        use tokio::io::AsyncWriteExt;
                        if let Err(e) = stdin.write_all(text_clone.as_bytes()).await {
                            eprintln!("Failed to write to IoT fallback script stdin: {}", e);
                        }
                    }
                    
                    match child.wait_with_output().await {
                        Ok(output) => {
                            if !output.status.success() {
                                let err_str = String::from_utf8_lossy(&output.stderr);
                                eprintln!("IoT fallback script failed: {}", err_str);
                            } else {
                                let out_str = String::from_utf8_lossy(&output.stdout);
                                if !out_str.trim().is_empty() {
                                    println!("IoT fallback script output: {}", out_str);
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to wait for IoT fallback script: {}", e);
                        }
                    }
                });
            }
            "tts" => {
                if let Some(state) = &msg.state {
                    if state == "start" {
                        self.should_mute_mic = true;
                        println!("TTS Started, muting mic for AEC");
                    } else if state == "stop" {
                        self.should_mute_mic = false;
                        println!("TTS Stopped, unmuting mic");
                        self.send_auto_listen_command().await;
                    }
                }

                if let Some(t) = msg.text {
                    println!("TTS: {}", t);
                    // 仅在开启TTS显示开关时才将文本发送给GUI显示
                    if self.config.enable_tts_display {
                        if let Err(e) = self.gui_bridge.send_message(&text).await {
                            eprintln!("Failed to send TTS text to GUI: {}", e);
                        }
                    }
                }
            }
            "stt" => {
                if let Some(text_content) = msg.text {
                    println!("STT Result: {}", text_content);
                }
            }
            other => {
                println!("Unhandled message type: {}", other);
            }
        }
    }

    // 处理来自服务器的音频数据
    async fn process_server_audio(&mut self, data: Vec<u8>) {
        if self.state != SystemState::Speaking {
            self.state = SystemState::Speaking;
            if let Err(e) = self.gui_bridge.send_message(r#"{"state": 6}"#).await {
                eprintln!("Failed to send to GUI: {}", e);
            }
        }
        if let Err(e) = self.audio_bridge.send_audio(&data).await {
            eprintln!("Failed to send to Audio: {}", e);
        }
    }

    // 发送自动监听命令
    async fn send_auto_listen_command(&self) {
        let session_id = self.current_session_id.as_deref().unwrap_or("");
        let listen_cmd = format!(
            r#"{{"session_id":"{}","type":"listen","state":"start","mode":"auto"}}"#,
            session_id
        );
        if let Err(e) = self.net_tx.send(NetCommand::SendText(listen_cmd)).await {
            eprintln!("Failed to send loop listen command: {}", e);
        }
    }

    // 处理来自 AudioBridge 的事件
    pub async fn handle_audio_event(&mut self, event: AudioEvent) {
        match event {
            AudioEvent::AudioData(data) => {
                if self.should_mute_mic {
                    return;
                }
                if self.state != SystemState::Listening {
                    self.state = SystemState::Listening;
                    if let Err(e) = self.gui_bridge.send_message(r#"{"state": 5}"#).await {
                        eprintln!("Failed to send to GUI: {}", e);
                    }
                }
                if let Err(e) = self.net_tx.send(NetCommand::SendBinary(data)).await {
                    eprintln!("Failed to send audio to NetLink: {}", e);
                }
            }
        }
    }

    // 处理来自 GuiBridge 的事件
    pub async fn handle_gui_event(&mut self, event: GuiEvent) {
        let GuiEvent::Message(msg) = event;
        println!("Received Message from GUI: {}", msg);
        if let Err(e) = self.net_tx.send(NetCommand::SendText(msg)).await {
            eprintln!("Failed to send text to NetLink: {}", e);
        }
    }
}
