## armv7-unknown-linux-uclibceabihf 交叉编译说明

### 概述

本脚本自动完成以下步骤，生成使用 uClibc 动态链接的 ARM 二进制文件：

1. **下载 uClibc 交叉编译工具链** — 从 GitHub Releases 下载（已有则跳过）
2. **下载并编译 alsa-lib 共享库** — 仅用于链接时符号解析，运行时使用设备系统库
3. **配置 Rust 交叉编译环境** — 设置 CC、pkg-config、链接标志
4. **编译 Rust 项目** — 使用 `cargo +nightly build -Z build-std` 输出混合链接二进制

### 链接策略

- **动态链接**: libc (uClibc) + libasound.so
- **静态链接**: opus + speexdsp（由 build.rs 自动从源码编译）

### 前置条件

- **Rust nightly 工具链** — `rustup toolchain install nightly`
- **rust-src 组件** — `rustup component add rust-src --toolchain nightly`
- **构建工具** — `wget` 或 `curl`、`make`、`tar`

### 使用方法

```bash
# 执行编译（工具链会自动下载）
bash scripts/armv7-unknown-linux-uclibceabihf/build.sh

# 输出文件
# target/armv7-unknown-linux-uclibceabihf/release/xiaozhi_linux_rs
```

### 目标设备

适用于 RV1106 (Luckfox Pico) 等使用 uClibc 的 ARM 设备。

### 缓存机制

所有下载和编译产物缓存在 `third_party/armv7-unknown-linux-uclibceabihf/` 目录下：
- `arm-rockchip830-linux-uclibcgnueabihf/` — uClibc 交叉编译工具链
- `alsa-shared/` — ALSA 共享库（仅链接时使用）
- `build/` — C 依赖库源码与编译中间产物

### uClibc 特别说明

uClibc 缺少 `getauxval` 函数的实现，因此在链接阶段会找不到函数定义。
解决方法是在 `auxval_stub.c` 中提供一个空的 `getauxval` 实现，
由 build.rs 自动编译为静态库并链接进可执行文件。