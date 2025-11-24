#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SystemState {
    Idle,         // 等待唤醒词
    Listening,    // 录音中（VAD激活）
    Processing,   // 音频已发送，等待服务器响应
    Speaking,     // 播放TTS
    NetworkError, // 重新连接中
}
