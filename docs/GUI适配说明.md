## 示例 GUI 项目
- https://github.com/Hyrsoft/lvgl_xiaozhi_gui
- https://github.com/Hyrsoft/slint_xiaozhi_gui

## 一、配置 IP 与端口

GUI 作为独立的进程，和小智客户端核心进程（简称为 core）进行 UDP 通信，具体 IP 和端口可以在config.toml和xiaozhi_config.json中进行修改。

config.toml（修改后重新编译生效）

```toml
# GUI进程配置
[gui]
local_port = 5678
remote_port = 5679
local_ip = "0.0.0.0"
remote_ip = "127.0.0.1"
buffer_size = 4096

# 功能开关
[features]
enable_tts_display = true
```

xiaozhi_config.json（修改后重启 Core 生效）

```json
{
  "gui_local_port": 5678,
  "gui_remote_port": 5679,
  "gui_local_ip": "0.0.0.0",
  "gui_remote_ip": "127.0.0.1",
  "gui_buffer_size": 4096,
  "enable_tts_display": true
}
```

**其中，各个配置项的含义为：**

- `gui_local_ip`: Core 进程绑定的本地监听 IP 地址（通常为 `0.0.0.0`，表示监听所有网卡）。
- `gui_local_port`: Core 进程绑定的本地监听端口。**GUI 进程需要向该端口发送控制指令。**
- `gui_remote_ip`: GUI 进程所在的目标 IP 地址（如果同机运行，通常为 `127.0.0.1`）。
- `gui_remote_port`: GUI 进程监听的端口。**Core 进程会将状态信息和文本发送到该端口。**
- `gui_buffer_size`: UDP 接收缓冲区的大小（单位：字节）。
- `enable_tts_display`: 是否发送云端 TTS 文本给 GUI 用于字幕显示（必须为 `true` 才会向 GUI 发送 `"type": "tts"` 的数据包）。


GUI 进程需要做到：和core建立UDP通信后，接受从core发送的消息（json数据包），解析内容，并根据消息类型做出对应的行为，也可以向core发送控制指令（同样以json格式）。

---



## 二、消息类型



GUI 进程会接受的消息有

- 激活码信息

  ```json
  {"type": "activation", "code": "6位验证码"}
  ```

- 状态信息

  ```json
  {"state": 3}
  ```

  **其中，不同的 `state` 对应不同状态及含义：**

  - **`state: 3` (已连接 / 待机空闲)**：WebSocket 已成功连接到云端服务器。GUI 应当显示正常的待机表情或"在线"图标。
  - **`state: 4` (网络错误 / 断开连接)**：WebSocket 与服务器断开连接或连接失败。GUI 应当显示"断网提示"或相应的悲伤/重连表情。
  - **`state: 5` (正在倾听)**：设备的麦克风检测到声音（VAD 激活），系统正在收集音频并发送给服务器。GUI 应当切换为"正在听（录音中）"的动画特效（如声波纹、耳朵闪烁等）。
  - **`state: 6` (正在说话)**：设备收到了来自服务器的音频流，准备或正在播放语音（TTS）。GUI 应当切换为"正在说话"的动态表情或唇语动画。

  

- Toast 通知消息，Core 进程会发送 Toast 消息通知 GUI 显示临时信息（如"设备已激活"）

  ```json
  {"type": "toast", "text": "设备已激活"}
  ```

- TTS 文本播报，即服务端发送的TTS文本，用于在对话框中显示字幕（对话内容）

  Core 进程会将云端下发的原始 TTS JSON 直接透传给 GUI（仅在 `enable_tts_display` 配置为 `true` 时），格式如下

  ```json
  {
    "session_id": "xxxxx",
    "type": "tts",
    "state": "sentence_start",
    "text": "你好，我是小智"
  }
  ```

  **`state` 字段取值说明：**
  - `"sentence_start"`：一句话开始，Core 会同时向 GUI 发送 `{"state": 6}` 状态切换。
  - `"sentence_end"`：一句话结束，Core 会同时向 GUI 发送 `{"state": 3}` 状态切换。

  GUI 进程需要监听包含 `"type": "tts"` 的数据包，提取其中的 `"text"` 字段，将其渲染在屏幕的文本框或字幕区域中。可以根据 `"state"` 字段判断是 `"sentence_start"` (这句话的开始) 还是 `"sentence_end"` (这句话的结束)，从而决定是替换当前字幕还是保留字幕。

  

## 三、 GUI 进程可以发送的控制指令：

GUI 也可以作为输入设备（如果有触摸屏或键盘）主动向 Core 发起请求。

- **透传机制**：GUI 进程发送给 Core（目标端口 `gui_local_port`）的任何文本或 JSON 数据，**Core 都会直接将其作为网络命令通过 WebSocket 原封不动地转发给云端服务器**。

- **常见用法**：

  - **主动打断**：当用户点击屏幕时，GUI 可以发送特定的打断指令（如 `{"type":"abort"}` 视具体云端协议而定）来中断设备当前的说话状态。



