use crate::config::Config;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;

pub enum GuiEvent {
    Message(String),
}

pub struct GuiBridge {
    socket: Arc<UdpSocket>,
    target_addr: String,
    tx: mpsc::Sender<GuiEvent>,
    buffer_size: usize,
}

// GUI进程和Core进程通过本地UDP通信，端口在配置中指定
impl GuiBridge {
    pub async fn new(config: &Config, tx: mpsc::Sender<GuiEvent>) -> anyhow::Result<Self> {
        // 绑定本地UDP端口
        let socket =
            UdpSocket::bind(format!("{}:{}", config.gui_local_ip, config.gui_local_port)).await?;
        let target_addr = format!("{}:{}", config.gui_remote_ip, config.gui_remote_port);

        Ok(Self {
            socket: Arc::new(socket),
            target_addr,
            tx,
            buffer_size: config.gui_buffer_size,
        })
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        let mut buf = vec![0u8; self.buffer_size];
        loop {
            // 通过UDP socket接收消息
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
        self.socket
            .send_to(msg.as_bytes(), &self.target_addr)
            .await?;
        Ok(())
    }
}
