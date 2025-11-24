#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SystemState {
    Idle,           // Waiting for wake word
    Listening,      // Recording audio (VAD Active)
    Processing,     // Audio sent, waiting for server response
    Speaking,       // Playing TTS
    NetworkError,   // Reconnecting
}
