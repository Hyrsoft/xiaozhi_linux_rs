# Xiaozhi Linux

![logo](./docs/images/logo.png)

English | [简体中文](./README.md) 

---



## Project Introduction

This project is a complete implementation of the Xiaozhi AI client on the Linux platform, integrating **network interaction, audio processing, and business logic control**. Through a unified Rust application, it consolidates audio, GUI interaction, and cloud communication to provide a modern and efficient AI client solution.

> ***GUI Design***: Since Rust currently lacks mature and open-source-friendly embedded GUI libraries, this project **does not integrate GUI functionality**. Instead, it communicates with independent GUI processes through inter-process communication (IPC). This decoupled design allows flexible selection of graphics libraries such as LVGL, Qt, Slint, or TUI based on specific device requirements. The project functions completely even without a GUI process.

***Why Choose Rust?***

Not to ride the wave of "**Rewrite It In Rust**", but rather for personal interest and practice. Rust's modern package management, cross-compilation friendliness, and type safety characteristics provide a relatively unified development experience for embedded Linux devices. This helps overcome the ecosystem fragmentation caused by different SDKs, toolchains, and kernel differences, improving project maintainability.

This project builds upon the excellent design and valuable experience of [虾哥's Xiaozhi ESP32 version](https://github.com/78/xiaozhi-esp32) and [100askTeam's Xiaozhi Linux version](https://github.com/100askTeam/xiaozhi-linux). We pay tribute to their work.

QQ Group：695113129

---

## System Architecture

```mermaid
graph TD
    Config[Configuration File<br/>xiaozhi_config.json]

    subgraph External [External Services]
        Cloud[Xiaozhi Cloud Server WebSocket/HTTP]
        MCP_Ext[External MCP Service<br/>Process/HTTP/TCP]
    end

    subgraph "Xiaozhi Linux App (This Project)"
        Net[Network Module]
        Audio[Audio Processing<br/>ALSA + Opus + SpeexDSP]
        Logic[State Machine & Business Logic]
        MCP[MCP Gateway<br/>Dynamically Loads Multi-Protocol]
        
        Net <--> Logic
        Audio <--> Logic
        Logic <--> MCP
    end

    subgraph "Independent GUI Process (Optional)"
        GUI[GUI Interface<br/>LVGL/Qt/Slint/TUI]
    end

    subgraph Hardware [Hardware]
        Mic[Microphone]
        Speaker[Speaker]
        Screen[Screen]
        Touch[Touchscreen]
    end

    Config -.->|Read at startup<br/>Dynamic Load Parameters| Logic
    Config -.->|Dynamically Load Tools| MCP

    Net <-->|WSS / HTTP| Cloud
    MCP <-->|Multi-Protocol Interaction| MCP_Ext
    Audio <--> Mic
    Audio <--> Speaker
    Logic <-->|IPC<br/>UDP Events| GUI
    GUI <--> Screen
    GUI <--> Touch
    
    style Audio fill:#ace,stroke:#888,stroke-width:2px
    style GUI fill:#fcc,stroke:#888,stroke-width:2px
    style Config fill:#eef,stroke:#888,stroke-width:2px,stroke-dasharray: 5 5
    style MCP fill:#efe,stroke:#888,stroke-width:2px
```

## ✨ Features

### Implemented Features

- ✓ **Audio Processing**
  - Support for I2S and USB audio cards
  - ALSA real-time audio capture and playback
  - Opus audio encoding (16kHz, PCM16) and decoding
  - SpeexDSP real-time processing (noise reduction, AGC, resampling)
  - Support for custom audio device configuration, see [Audio Device Configuration Guide](./docs/音频设备配置说明.md)

- ✓ **Cloud Interaction and Protocol**
  - WebSocket full-duplex long connection with heartbeat keepalive
  - Device authentication and Hello handshake
  - TTS (Text-to-Speech), STT (Speech-to-Text), IoT control commands

- ✓ **Device Management**
  - Automatic device activation and binding
  - Device identity persistence (Client ID, Device ID)
  - State machine management (Idle, Listening, Processing, Speaking, Network Error)

- ✓ **Configuration System**
  - TOML file configuration loading
  - Runtime parameter persistence
  - Environment variable override

- ✓ **MCP Extension Capabilities**
  - Decoupled MCP gateway design supporting dynamic tool integration
  - Standard JSON-RPC message processing and tool lifecycle management
  - Communication with external scripts via stdin/stdout
  - Dynamic tool configuration without recompilation, see [MCP Function Documentation](./docs/MCP功能说明.md)

### Features To Be Implemented


- ☐ **IoT and Smart Home Integration**

- ☐ **Local Offline Wake-up and Audio Front-end Processing (AFE)**


---


> Note: In the ESP32 environment, Xiaozhi typically serves as the sole firmware program and must comprehensively manage all logic from low-level Wi-Fi drivers, provisioning protocols (BluFi/AP), and system self-updates (OTA) to auto-startup on boot. In Linux systems, however, Xiaozhi exists as an independent system process. Therefore, many features that must be built into the embedded version, such as provisioning, hardware drivers, and startup management, are delegated to more specialized components of the operating system in the Linux version. Similarly, OTA functionality will not be built into this project but will be implemented by other projects. See [OTA Documentation](./docs/OTA功能说明.md) for details.

## Quick Start

### Dependencies

- **Rust Toolchain** (Stable 1.75+)

- **Linux Development Environment**

- **C Development Toolchain** (gcc, make, pkg-config)

- **Embedded Linux Device SDK, or custom sysroot** (for dynamic linking libc and audio-related C libraries)

- **Dynamic Libraries**:

  - `libasound2-dev` / `alsa-lib-devel` (ALSA audio library)
  - `libopus-dev` / `opus-devel` (Opus codec library)
  - `libspeexdsp-dev` / `speexdsp-devel` (SpeexDSP processing library)

### Verified Target Devices (Development Boards)

> Running this project requires the target device to have audio input and output capabilities.

- **armv7-unknow-linux-uclibceabihf**
  - [Luckfox Pico series](https://wiki.luckfox.com/en/Luckfox-Pico-RV1106/) (Rockchip RV1106, Buildroot, I2S Sound Card)
  - [Echo-Mate Desktop Robot](https://github.com/No-Chicken/Echo-Mate) (Rockchip RV1106, Buildroot, I2S Sound Card)
- **armv7-unknow-linux-gnueabihf**
  - [Luckfox Lyra series](https://wiki.luckfox.com/en/Luckfox-Lyra/Introduction) (Rockchip RK3506, Buildroot, USB Sound Card)
- **aarch64-unknown-linux-gnu**
  - [Dshanpi-A1](https://wiki.dshanpi.org/docs/DshanPi-A1/intro/) (Rockchip RK3576, Armbian, Both I2S and USB Sound Card)
  - Redmi 2 (Qualcomm Snapdragon 410, Armbian, I2S Sound Card)
  - N1 Box (Amlogic S905D, Armbian, USB Sound Card)
- **x86_64-unknown-linux-gnu**
  - Laptop with Arch Linux installed

Other Linux devices on different target platforms (including x86 virtual machines) have not been verified yet, but are theoretically supported. For specific cross-compilation procedures, refer to [Rust Book](https://doc.rust-lang.org/beta/rustc/platform-support.html) and [RV1106 build script](./boards/rv1106_uclibceabihf/armv7_uclibc_build.sh).

**Testing and Pull Requests are welcome** (for build scripts in scripts, current sections of README, and sample code in the `examples` directory).

---

### Local Build and Run

```bash
# Clone the repository
git clone https://github.com/Hyrsoft/xiaozhi_linux_rs.git
cd xiaozhi_linux_rs

# Install dependencies (Ubuntu/Debian)
sudo apt-get install -y \
    libasound2-dev \
    libopus-dev \
    libspeexdsp-dev \
    pkg-config

# Build
cargo build --release

# Run (requires network connection and configuration file)
cargo run --release
```

### Cross-compilation to Embedded Devices

#### Example: Compiling for Luckfox Pico (RV1106)

```bash
# No need to prepare the sdk environment, just use the cross-compilation script directly, it will automatically download the cross-compilation toolchain and dependency libraries, and compile and link them

# Add support for the target
rustup target add armv7-unknown-linux-uclibceabihf
rustup toolchain install nightly
rustup component add rust-src --toolchain nightly

# Use the provided build script
./scripts/armv7-unknown-linux-uclibceabihf/build.sh

# Build output: target/armv7-unknown-linux-uclibceabihf/release/xiaozhi_linux_rs
```

#### Verify Build Results

```bash
[root@luckfox root]# ldd xiaozhi_linux_rs 
        libasound.so.2 => /usr/lib/libasound.so.2 (0xa6d72000)
        libgcc_s.so.1 => /lib/libgcc_s.so.1 (0xa6d43000)
        libc.so.0 => /lib/libc.so.0 (0xa6cb4000)
        ld-uClibc.so.1 => /lib/ld-uClibc.so.0 (0xa6efc000)
```

---

## Open Source License and Distribution Notice

The core code of this project is open-sourced under the MIT License. The audio components that this project depends on (ALSA-related libraries) are subject to the LGPL License.

Given the open source license restrictions, the statically linked binaries distributed by this project are recommended for testing and evaluation purposes only. If you plan to further develop or commercially distribute this project, please ensure compliance with the LGPL License (e.g., use dynamic linking, or open-source your derivative works). Developers bear full responsibility for any legal risks arising from violations of open source licenses; this project assumes no liability.

---




## Contributing

If you're interested in embedded Rust and Linux network programming, we welcome you to submit Issues or Pull Requests!

---

## Acknowledgments

- [78/xiaozhi-esp32](https://github.com/78/xiaozhi-esp32)
- [100askTeam/xiaozhi-linux](https://github.com/100askTeam/xiaozhi-linux)
- [xinnan-tech/xiaozhi-esp32-server](https://github.com/xinnan-tech/xiaozhi-esp32-server)
