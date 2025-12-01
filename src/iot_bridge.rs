use crate::config::Config;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;

pub enum IotEvent {
    Message(String),
}

pub struct IotBridge {
    socket: Arc<UdpSocket>,
    target_addr: String,
    tx: mpsc::Sender<IotEvent>,
    buffer_size: usize,
}

impl IotBridge {
    pub async fn new(config: &Config, tx: mpsc::Sender<IotEvent>) -> anyhow::Result<Self> {
        let socket =
            UdpSocket::bind(format!("{}:{}", config.iot_local_ip, config.iot_local_port)).await?;
        let target_addr = format!("{}:{}", config.iot_remote_ip, config.iot_remote_port);

        Ok(Self {
            socket: Arc::new(socket),
            target_addr,
            tx,
            buffer_size: config.iot_buffer_size,
        })
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        let mut buf = vec![0u8; self.buffer_size];
        loop {
            let (len, _) = self.socket.recv_from(&mut buf).await?;
            if len > 0 {
                if let Ok(msg) = std::str::from_utf8(&buf[..len]) {
                    if let Err(e) = self.tx.send(IotEvent::Message(msg.to_string())).await {
                        eprintln!("Failed to send IoT event: {}", e);
                        break;
                    }
                }
            }
        }
        Ok(())
    }

    pub async fn send_message(&self, msg: &str) -> anyhow::Result<()> {
        self.socket
            .send_to(msg.as_bytes(), &self.target_addr)
            .await?;
        Ok(())
    }
}
