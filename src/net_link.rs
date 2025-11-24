use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use futures_util::{StreamExt, SinkExt};
use tokio::sync::mpsc;
use serde::Serialize;
use crate::config::Config;
use url::Url;

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

#[derive(Serialize)]
struct AudioParams {
    format: String,
    sample_rate: u32,
    channels: u8,
    frame_duration: u32,
}

#[derive(Serialize)]
struct HelloMessage {
    #[serde(rename = "type")]
    msg_type: String,
    version: u8,
    transport: String,
    audio_params: AudioParams,
}

pub struct NetLink {
    config: Config,
    tx: mpsc::Sender<NetEvent>,
    rx_cmd: mpsc::Receiver<NetCommand>,
}

impl NetLink {
    pub fn new(config: Config, tx: mpsc::Sender<NetEvent>, rx_cmd: mpsc::Receiver<NetCommand>) -> Self {
        Self {
            config,
            tx,
            rx_cmd,
        }
    }

    pub async fn run(mut self) {
        loop {
            if let Err(e) = self.connect_and_loop().await {
                eprintln!("Connection error: {}. Retrying in 5s...", e);
                let _ = self.tx.send(NetEvent::Disconnected).await;
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            }
        }
    }

    async fn connect_and_loop(&mut self) -> anyhow::Result<()> {
        let url = Url::parse(&self.config.ws_url)?;
        let mut request = url.into_client_request()?;
        
        // Add headers
        let headers = request.headers_mut();
        headers.insert("Authorization", format!("Bearer {}", self.config.ws_token).parse()?);
        headers.insert("Device-Id", self.config.device_id.parse()?);
        headers.insert("Protocol-Version", "1".parse()?);

        println!("Connecting to {}...", self.config.ws_url);
        let (ws_stream, _) = connect_async(request).await?;
        println!("Connected!");
        
        let (mut write, mut read) = ws_stream.split();
        
        self.tx.send(NetEvent::Connected).await?;

        // Send Hello
        let hello = HelloMessage {
            msg_type: "hello".to_string(),
            version: 1,
            transport: "websocket".to_string(),
            audio_params: AudioParams {
                format: "opus".to_string(),
                sample_rate: 16000,
                channels: 1,
                frame_duration: 60,
            },
        };
        let hello_json = serde_json::to_string(&hello)?;
        write.send(Message::Text(hello_json)).await?;

        loop {
            tokio::select! {
                Some(msg) = read.next() => {
                    match msg? {
                        Message::Text(text) => {
                            self.tx.send(NetEvent::Text(text)).await?;
                        }
                        Message::Binary(data) => {
                            self.tx.send(NetEvent::Binary(data)).await?;
                        }
                        Message::Close(_) => {
                            return Err(anyhow::anyhow!("Connection closed"));
                        }
                        _ => {}
                    }
                }
                Some(cmd) = self.rx_cmd.recv() => {
                    match cmd {
                        NetCommand::SendText(text) => {
                            write.send(Message::Text(text)).await?;
                        }
                        NetCommand::SendBinary(data) => {
                            write.send(Message::Binary(data)).await?;
                        }
                    }
                }
                else => break,
            }
        }
        Ok(())
    }
}
