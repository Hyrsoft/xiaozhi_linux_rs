use crate::config::Config;
use futures_util::{SinkExt, StreamExt};
use mac_address::get_mac_address;
use serde::Serialize;
use tokio::sync::mpsc;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use url::Url;
use uuid::Uuid;

#[derive(Debug)]
pub enum NetEvent {
    Text(String),
    Binary(Vec<u8>),
    Connected,
    Disconnected,
}

#[derive(Debug)]
pub enum NetCommand {
    SendText(String),
    SendBinary(Vec<u8>),
}

// 音频参数结构体
#[derive(Serialize)]
struct AudioParams {
    format: String,
    sample_rate: u32,
    channels: u8,
    frame_duration: u32,
}

// Hello Message，用于初始化连接
#[derive(Serialize)]
struct HelloMessage {
    #[serde(rename = "type")]
    msg_type: String,
    version: u8,
    transport: String,
    audio_params: AudioParams,
    // device_id: String, // Removed to match C++ implementation
}

pub struct NetLink {
    config: Config,
    tx: mpsc::Sender<NetEvent>,
    rx_cmd: mpsc::Receiver<NetCommand>,
}

impl NetLink {
    pub fn new(
        config: Config,
        tx: mpsc::Sender<NetEvent>,
        rx_cmd: mpsc::Receiver<NetCommand>,
    ) -> Self {
        Self { config, tx, rx_cmd }
    }

    // 如果发生错误断开连接，5秒后重连
    pub async fn run(mut self) {
        // 重试机制，指数退避
        let mut retry_delay = 1;
        loop {
            if let Err(e) = self.connect_and_loop().await {
                eprintln!("Connection error: {}. Retrying in {}s...", e, retry_delay);
                let _ = self.tx.send(NetEvent::Disconnected).await;
                tokio::time::sleep(tokio::time::Duration::from_secs(retry_delay)).await;
                retry_delay = std::cmp::min(retry_delay * 2, 60);
            } else {
                // 如果连接和主循环正常退出，重置重试延迟
                // If it returns Ok, it might mean clean exit or just a disconnect that wasn't caught as Err?
                // In our case, connect_and_loop returns Err on disconnect.
                // If it returns Ok, it means we are shutting down (rx_cmd closed).
                break;
            }
        }
    }

    // 进入连接和主循环，处理WebSocket消息和发送命令
    async fn connect_and_loop(&mut self) -> anyhow::Result<()> {
        // 如果设备ID是unknown-device，则尝试获取MAC地址作为设备ID
        let device_id = if self.config.device_id == "unknown-device" {
            match get_mac_address() {
                Ok(Some(mac)) => mac.to_string().to_lowercase(), // Ensure lowercase to match typical Linux behavior 注意大小写一致，以匹配典型的Linux行为
                _ => Uuid::new_v4().to_string(), // 如果无法获取MAC地址，则生成新的UUID
            }
        } else {
            self.config.device_id.clone() // 使用配置中的设备ID
        };

        // 根据配置构建WebSocket请求
        let url = Url::parse(self.config.ws_url.as_ref())?;
        let host = url.host_str().unwrap_or("api.tenclass.net");

        let request = tokio_tungstenite::tungstenite::http::Request::builder()
            .method("GET")
            .uri(self.config.ws_url.as_ref())
            .header("Host", host)
            .header("Connection", "Upgrade")
            .header("Upgrade", "websocket")
            .header("Sec-WebSocket-Version", "13")
            .header(
                "Sec-WebSocket-Key",
                tokio_tungstenite::tungstenite::handshake::client::generate_key(),
            )
            .header("Authorization", format!("Bearer {}", self.config.ws_token))
            .header("Device-Id", &device_id)
            .header("Client-Id", &self.config.client_id)
            .header("Protocol-Version", "1")
            .body(())?;

        println!("Connecting to {}...", self.config.ws_url);
        println!("Headers: {:?}", request.headers()); // Debug headers
        let (ws_stream, _) = connect_async(request).await?;
        println!("Connected!");

        let (mut write, mut read) = ws_stream.split();

        self.tx.send(NetEvent::Connected).await?;

        // 发送Hello消息进行初始化链接
        let hello_msg = HelloMessage {
            msg_type: "hello".to_string(),
            version: 1,
            transport: "websocket".to_string(),
            audio_params: AudioParams {
                format: self.config.hello_format.to_string(),
                sample_rate: self.config.hello_sample_rate,
                channels: self.config.hello_channels,
                frame_duration: self.config.hello_frame_duration,
            },
        };
        let hello_json = serde_json::to_string(&hello_msg)?;

        println!("Sending Hello: {}", hello_json);
        write.send(Message::Text(hello_json.into())).await?;

        // 主循环，处理读取和写入
        loop {
            tokio::select! {
                msg = read.next() => {
                    match msg {
                        Some(Ok(msg)) => {
                            match msg {
                                Message::Text(text) => {
                                    println!("Received Text: {}", text);
                                    self.tx.send(NetEvent::Text(text.to_string())).await?;
                                }
                                Message::Binary(data) => {
                                    self.tx.send(NetEvent::Binary(data.to_vec())).await?;
                                }
                                Message::Close(frame) => {
                                    println!("Server closed connection: {:?}", frame);
                                    return Err(anyhow::anyhow!("Connection closed"));
                                }
                                _ => {}
                            }
                        }
                        Some(Err(e)) => return Err(e.into()),
                        None => return Err(anyhow::anyhow!("Connection closed")),
                    }
                }
                Some(cmd) = self.rx_cmd.recv() => {
                    match cmd {
                        NetCommand::SendText(text) => {
                            write.send(Message::Text(text.into())).await?;
                        }
                        NetCommand::SendBinary(data) => {
                            write.send(Message::Binary(data.into())).await?;
                        }
                    }
                }
                else => break,
            }
        }
        Ok(())
    }
}
