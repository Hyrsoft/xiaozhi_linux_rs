use crate::audio_bridge::{AudioBridge, AudioEvent};
use crate::config::Config;
use crate::gui_bridge::{GuiBridge, GuiEvent};
use crate::iot_bridge::{IotBridge, IotEvent};
use crate::net_link::{NetCommand, NetEvent};
use crate::protocol::ServerMessage;
use crate::state_machine::SystemState;
use serde_json;
use std::sync::Arc;
use tokio::sync::mpsc;

#[cfg(feature = "tui")]
use xiaozhi_tui::{TuiCommand, TuiState};

pub struct CoreController {
    state: SystemState,
    current_session_id: Option<String>,
    should_mute_mic: bool,
    config: Config,
    net_tx: mpsc::Sender<NetCommand>,
    audio_bridge: Arc<AudioBridge>,
    gui_bridge: Arc<GuiBridge>,
    iot_bridge: Arc<IotBridge>,
    #[cfg(feature = "tui")]
    tui_tx: Option<mpsc::Sender<TuiCommand>>,
}

impl CoreController {
    pub fn new(
        config: Config,
        net_tx: mpsc::Sender<NetCommand>,
        audio_bridge: Arc<AudioBridge>,
        gui_bridge: Arc<GuiBridge>,
        iot_bridge: Arc<IotBridge>,
        #[cfg(feature = "tui")] tui_tx: Option<mpsc::Sender<TuiCommand>>,
    ) -> Self {
        Self {
            state: SystemState::Idle,
            current_session_id: None,
            should_mute_mic: false,
            config,
            net_tx,
            audio_bridge,
            gui_bridge,
            iot_bridge,
            #[cfg(feature = "tui")]
            tui_tx,
        }
    }

    /// Send a command to the TUI (if enabled and connected).
    #[cfg(feature = "tui")]
    fn send_tui_cmd(&self, cmd: TuiCommand) {
        if let Some(tx) = &self.tui_tx {
            let _ = tx.try_send(cmd);
        }
    }

    /// Map SystemState to TUI FaceState and push it.
    #[cfg(feature = "tui")]
    fn update_tui_state(&self) {
        let tui_state = match self.state {
            SystemState::Idle => TuiState::Idle,
            SystemState::Listening => TuiState::Listening,
            SystemState::Processing => TuiState::Thinking,
            SystemState::Speaking => TuiState::Speaking,
            SystemState::NetworkError => TuiState::NetworkError,
        };
        self.send_tui_cmd(TuiCommand::SetState(tui_state));
    }

    // 处理来自 NetLink 的事件
    pub async fn handle_net_event(&mut self, event: NetEvent) {
        match event {
            NetEvent::Text(text) => self.process_server_text(text).await,
            NetEvent::Binary(data) => self.process_server_audio(data).await,
            NetEvent::Connected => {
                log::info!("WebSocket Connected");
                if let Err(e) = self.gui_bridge.send_message(r#"{"state": 3}"#).await {
                    log::error!("Failed to send to GUI: {}", e);
                }
                if let Err(e) = self
                    .iot_bridge
                    .send_message(r#"{"type":"network", "state":"connected"}"#)
                    .await
                {
                    log::error!("Failed to send to IoT: {}", e);
                }
            }
            NetEvent::Disconnected => {
                log::info!("WebSocket Disconnected");
                self.state = SystemState::NetworkError;
                #[cfg(feature = "tui")]
                self.update_tui_state();
                if let Err(e) = self.gui_bridge.send_message(r#"{"state": 4}"#).await {
                    log::error!("Failed to send to GUI: {}", e);
                }
                if let Err(e) = self
                    .iot_bridge
                    .send_message(r#"{"type":"network", "state":"disconnected"}"#)
                    .await
                {
                    log::error!("Failed to send to IoT: {}", e);
                }
            }
        }
    }

    // 处理来自服务器的文本消息
    async fn process_server_text(&mut self, text: String) {
        log::info!("Received Text from Server: {}", text);

        let msg: ServerMessage = match serde_json::from_str(&text) {
            Ok(msg) => msg,
            Err(_) => {
                // 可能不是JSON，忽略
                return;
            }
        };

        if let Some(sid) = &msg.session_id {
            if self.current_session_id.as_deref() != Some(sid) {
                log::info!("New Session ID: {}", sid);
                self.current_session_id = Some(sid.clone());
            }
        }

        match msg.msg_type.as_str() {
            "hello" => {
                log::info!("Server Hello received. Starting listen mode...");
                let listen_cmd =
                    r#"{"session_id":"","type":"listen","state":"start","mode":"auto"}"#;
                if let Err(e) = self
                    .net_tx
                    .send(NetCommand::SendText(listen_cmd.to_string()))
                    .await
                {
                    log::error!("Failed to send listen command: {}", e);
                }
            }
            "iot" => {
                if let Some(cmd) = &msg.command {
                    log::info!("Processing IoT Command: {}", cmd);
                }
                if let Err(e) = self.iot_bridge.send_message(&text).await {
                    log::error!("Failed to send to IoT: {}", e);
                }
            }
            "tts" => {
                if let Some(state) = &msg.state {
                    if state == "start" {
                        self.should_mute_mic = true;
                        log::info!("TTS Started, muting mic for AEC");
                    } else if state == "stop" {
                        self.should_mute_mic = false;
                        log::info!("TTS Stopped, unmuting mic");
                        self.send_auto_listen_command().await;
                    }
                }

                if let Some(t) = msg.text {
                    log::info!("TTS: {}", t);

                    // 发送字幕到 TUI
                    #[cfg(feature = "tui")]
                    self.send_tui_cmd(TuiCommand::SetSubtitle(t.clone()));

                    // 仅在开启TTS显示开关时才将文本发送给GUI显示
                    if self.config.enable_tts_display {
                        if let Err(e) = self.gui_bridge.send_message(&text).await {
                            log::error!("Failed to send TTS text to GUI: {}", e);
                        }
                    }
                }
            }
            "stt" => {
                if let Some(text_content) = msg.text {
                    log::info!("STT Result: {}", text_content);
                }
            }
            other => {
                log::info!("Unhandled message type: {}", other);
            }
        }
    }

    // 处理来自服务器的音频数据
    async fn process_server_audio(&mut self, data: Vec<u8>) {
        if self.state != SystemState::Speaking {
            self.state = SystemState::Speaking;
            #[cfg(feature = "tui")]
            self.update_tui_state();
            if let Err(e) = self.gui_bridge.send_message(r#"{"state": 6}"#).await {
                log::error!("Failed to send to GUI: {}", e);
            }
        }
        if let Err(e) = self.audio_bridge.send_audio(&data).await {
            log::error!("Failed to send to Audio: {}", e);
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
            log::error!("Failed to send loop listen command: {}", e);
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
                    #[cfg(feature = "tui")]
                    self.update_tui_state();
                    if let Err(e) = self.gui_bridge.send_message(r#"{"state": 5}"#).await {
                        log::error!("Failed to send to GUI: {}", e);
                    }
                }
                if let Err(e) = self.net_tx.send(NetCommand::SendBinary(data)).await {
                    log::error!("Failed to send audio to NetLink: {}", e);
                }
            }
        }
    }

    // 处理来自 GuiBridge 的事件
    pub async fn handle_gui_event(&mut self, event: GuiEvent) {
        let GuiEvent::Message(msg) = event;
        log::info!("Received Message from GUI: {}", msg);
        if let Err(e) = self.net_tx.send(NetCommand::SendText(msg)).await {
            log::error!("Failed to send text to NetLink: {}", e);
        }
    }

    // 处理来自 IotBridge 的事件
    pub async fn handle_iot_event(&mut self, event: IotEvent) {
        let IotEvent::Message(msg) = event;
        log::info!("Received Message from IoT: {}", msg);
        if let Err(e) = self.net_tx.send(NetCommand::SendText(msg)).await {
            log::error!("Failed to send text to NetLink: {}", e);
        }
    }
}
