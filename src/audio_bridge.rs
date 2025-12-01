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
    buffer_size: usize,
}

impl AudioBridge {
    pub async fn new(config: &Config, tx: mpsc::Sender<AudioEvent>) -> anyhow::Result<Self> {
        let socket = UdpSocket::bind(format!(
            "{}:{}",
            config.audio_local_ip, config.audio_local_port
        ))
        .await?;
        let target_addr = format!("{}:{}", config.audio_remote_ip, config.audio_remote_port);

        Ok(Self {
            socket: Arc::new(socket),
            target_addr,
            tx,
            buffer_size: config.audio_buffer_size,
        })
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        let mut buf = vec![0u8; self.buffer_size];
        loop {
            let (len, _) = self.socket.recv_from(&mut buf).await?;
            if len > 0 {
                let data = &buf[..len];

                // 如果数据包长度大于10字节则认为是有效音频数据
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
