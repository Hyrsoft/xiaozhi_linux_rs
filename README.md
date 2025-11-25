# Xiaozhi Linux Core 

## 项目简介 

本项目专注于小智 AI 客户端整个系统的**网络交互**与**业务逻辑控制**部分。通过 IPC协议与音频服务和 GUI 服务交互，实现业务逻辑与硬件 BSP的解耦。



## 系统架构

通过多进程解耦设计，将系统划分为不同的职责域。

```mermaid
graph TD
    subgraph External Services [外部服务]
        Cloud[小智云端服务器 WebSocket/HTTP]
    end

    subgraph "Xiaozhi Linux Core "
        Core[控制中枢]
        Net[网络模块]
        Logic[状态机 & 业务逻辑]
        
        Core --> Net
        Core --> Logic
    end

    subgraph "Peripherals BSP Dependent"
        AudioApp[音频服务]
        GUIApp[GUI 界面服务]
    end

    %% Connections
    Net <-->|WSS / HTTP| Cloud
    Core <-->|UDP IPC / Audio Data| AudioApp
    Core <-->|UDP IPC / JSON Events| GUIApp
    
    style Core fill:#dea,stroke:#888,stroke-width:2px
```

- **xiaozhi_linux_core :** 负责与云端通信、设备状态管理、OTA 激活逻辑以及指令分发。
- **Audio Service:** 负责底层的 ALSA/PulseAudio 录音与播放（本项目不包含，通过 UDP 交互）。
- **GUI Service:** 负责屏幕显示与触控交互（本项目不包含，通过 UDP 交互）。

## ✨ 目前实现的功能 

- **网络通信栈**：
  - [x] 完整的 WebSocket 客户端实现（基于 `tokio-tungstenite`）。
  - [x] HTTP 激活接口与 OTA 检查（基于 `reqwest`）。
  - [x] 强类型的 JSON 消息序列化/反序列化（基于 `serde`）。
- **进程间通信 (IPC)**：
  - [x] UDP Bridge，用于接收/发送音频 PCM 数据。
  - [x] 异步 UDP 通道，用于与 GUI 进程交换控制指令。
- **配置管理**：
  - [x] 支持分层配置加载（默认值 -> 配置文件 `/etc/xiaozhi/config` -> 环境变量）。
  - [x] 自动生成或持久化设备 UUID/MAC 标识。
- **业务逻辑**：
  - [x] 基础状态机（Idle, Listening, Speaking, Connecting）。
  - [x] 激活流程控制（检测激活状态、显示验证码）。
  - [x] 音频流的全双工透传。

## 快速开始 

### 依赖环境

- Rust Toolchain (Stable)
- Linux 环境 (或 macOS/Windows + WSL)

### 编译与运行

**本地运行:**

```bash
# 克隆项目
git clone https://github.com/haoruanwn/xiaozhi_linux_core.git
cd xiaozhi_linux_core

# 运行 (需确保本地没有占用对应 UDP 端口)
cargo run
```

**交叉编译 (推荐用cross编译musl的版本):**

``` bash
# 安装目标架构支持
cargo install cross

# cross需要docker或者podman来运行
# 例如，编译为 armv7 musleabihf 目标 (静态链接)
cross build \
   --target=armv7-unknown-linux-musleabihf \
   --release \
   --config 'target.armv7-unknown-linux-musleabihf.rustflags=["-C", "target-feature=+crt-static"]'
```

------

## 贡献

如果你对嵌入式 Rust、Linux 网络编程感兴趣，欢迎提交 Issue 或 Pull Request！

## 致谢

- [78/xiaozhi-esp32](https://github.com/78/xiaozhi-esp32)
- [100askTeam/xiaozhi-linux](https://github.com/100askTeam/xiaozhi-linux)
- [xinnan-tech/xiaozhi-esp32-server](https://github.com/xinnan-tech/xiaozhi-esp32-server)

