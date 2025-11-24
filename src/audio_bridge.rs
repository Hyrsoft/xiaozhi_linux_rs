use crate::config::Config;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;

use serde::Deserialize;

pub enum AudioEvent {
    AudioData(Vec<u8>),
    Command(AudioMessage),
}

#[derive(Debug, Deserialize)]
pub struct AudioMessage {
    pub session_id: Option<String>,
    pub text: Option<String>,
    // Add other fields as needed
}

pub struct AudioBridge {
    socket: Arc<UdpSocket>,
    target_addr: String,
    tx: mpsc::Sender<AudioEvent>,
}

impl AudioBridge {
    pub async fn new(config: &Config, tx: mpsc::Sender<AudioEvent>) -> anyhow::Result<Self> {
        let socket = UdpSocket::bind(format!("0.0.0.0:{}", config.audio_local_port)).await?;
        let target_addr = format!("127.0.0.1:{}", config.audio_remote_port);

        Ok(Self {
            socket: Arc::new(socket),
            target_addr,
            tx,
        })
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        let mut buf = [0u8; 2048]; // Adjust buffer size as needed
        loop {
            let (len, _) = self.socket.recv_from(&mut buf).await?;
            if len > 0 {
                let data = &buf[..len];
                
                // Treat as audio data
                // Filter out very small packets which might be noise or keep-alives
                if len > 10 {
                    if let Err(e) = self.tx.send(AudioEvent::AudioData(data.to_vec())).await {
                        eprintln!("Failed to send audio event: {}", e);
                        break;
                    }
                }
            }
        }
        Ok(())
    }

    pub async fn send_audio(&self, data: &[u8]) -> anyhow::Result<()> {
        self.socket.send_to(data, &self.target_addr).await?;
        Ok(())
    }
}
