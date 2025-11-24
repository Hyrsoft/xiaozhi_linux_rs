use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use std::sync::Arc;
use crate::config::Config;

pub enum GuiEvent {
    Message(String),
}

pub struct GuiBridge {
    socket: Arc<UdpSocket>,
    target_addr: String,
    tx: mpsc::Sender<GuiEvent>,
}

impl GuiBridge {
    pub async fn new(config: &Config, tx: mpsc::Sender<GuiEvent>) -> anyhow::Result<Self> {
        let socket = UdpSocket::bind(format!("0.0.0.0:{}", config.ui_port_up)).await?;
        let target_addr = format!("127.0.0.1:{}", config.ui_port_down);
        
        Ok(Self {
            socket: Arc::new(socket),
            target_addr,
            tx,
        })
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        let mut buf = [0u8; 4096]; 
        loop {
            let (len, _) = self.socket.recv_from(&mut buf).await?;
            if len > 0 {
                if let Ok(msg) = std::str::from_utf8(&buf[..len]) {
                    if let Err(e) = self.tx.send(GuiEvent::Message(msg.to_string())).await {
                        eprintln!("Failed to send GUI event: {}", e);
                        break;
                    }
                }
            }
        }
        Ok(())
    }

    pub async fn send_message(&self, msg: &str) -> anyhow::Result<()> {
        self.socket.send_to(msg.as_bytes(), &self.target_addr).await?;
        Ok(())
    }
}
