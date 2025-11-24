mod config;
mod state_machine;
mod net_link;
mod audio_bridge;
mod gui_bridge;

use tokio::sync::mpsc;
use std::sync::Arc;
use config::Config;
use state_machine::SystemState;
use net_link::{NetLink, NetEvent, NetCommand};
use audio_bridge::{AudioBridge, AudioEvent};
use gui_bridge::{GuiBridge, GuiEvent};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    env_logger::init();

    // Load config
    let config = Config::default(); // TODO: Load from file

    // Create channels
    let (tx_net_event, mut rx_net_event) = mpsc::channel::<NetEvent>(100);
    let (tx_net_cmd, rx_net_cmd) = mpsc::channel::<NetCommand>(100);
    
    let (tx_audio_event, mut rx_audio_event) = mpsc::channel::<AudioEvent>(100);
    let (tx_gui_event, mut rx_gui_event) = mpsc::channel::<GuiEvent>(100);

    // Spawn NetLink
    let net_link = NetLink::new(config.clone(), tx_net_event, rx_net_cmd);
    tokio::spawn(async move {
        net_link.run().await;
    });

    // Spawn AudioBridge
    let audio_bridge = Arc::new(AudioBridge::new(&config, tx_audio_event).await?);
    let audio_bridge_clone = audio_bridge.clone();
    tokio::spawn(async move {
        if let Err(e) = audio_bridge_clone.run().await {
            eprintln!("AudioBridge error: {}", e);
        }
    });

    // Spawn GuiBridge
    let gui_bridge = Arc::new(GuiBridge::new(&config, tx_gui_event).await?);
    let gui_bridge_clone = gui_bridge.clone();
    tokio::spawn(async move {
        if let Err(e) = gui_bridge_clone.run().await {
            eprintln!("GuiBridge error: {}", e);
        }
    });

    let mut current_state = SystemState::Idle;
    println!("Xiaozhi Core Started. State: {:?}", current_state);

    loop {
        tokio::select! {
            Some(event) = rx_net_event.recv() => {
                match event {
                    NetEvent::Text(text) => {
                        println!("Received Text from Server: {}", text);
                        // Forward to GUI
                        if let Err(e) = gui_bridge.send_message(&text).await {
                            eprintln!("Failed to send to GUI: {}", e);
                        }
                        
                        // Simple state update based on server messages (example)
                        // You might want to parse the JSON here to update state
                    }
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
                    }
                }
            }
            Some(event) = rx_audio_event.recv() => {
                match event {
                    AudioEvent::AudioData(data) => {
                        // println!("Received Audio from Mic: {} bytes", data.len());
                        if current_state != SystemState::Speaking {
                             if current_state != SystemState::Listening {
                                 current_state = SystemState::Listening;
                                 // Notify GUI: kDeviceStateListening = 5
                                 let _ = gui_bridge.send_message(r#"{"state": 5}"#).await;
                             }
                             // Forward to Server
                             let _ = tx_net_cmd.send(NetCommand::SendBinary(data)).await;
                        }
                    }
                }
            }
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
