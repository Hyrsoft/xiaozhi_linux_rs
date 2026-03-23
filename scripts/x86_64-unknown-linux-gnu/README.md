## x86_64-unknown-linux-gnu 编译说明

### 混合链接策略

本脚本采用**混合链接**方式：

- **动态链接**：libc (GLIBC) + libasound.so（运行时由系统提供）
- **静态链接**：opus、speexdsp 直接嵌入二进制

优势：
- 部署时只需拷贝单个可执行文件，无需额外 `.so` 文件
- 支持 `dlopen` 动态加载系统 ALSA 插件（如 PulseAudio）
- `default` 音频设备名可正常工作

### 快速开始

```bash
# 直接运行（无需安装额外工具链）
bash scripts/x86_64-unknown-linux-gnu/build.sh

# 输出: target/release/xiaozhi_linux_rs
```

### 前置依赖

需要安装以下系统包（Ubuntu/Debian）：

```bash
sudo apt-get install -y build-essential pkg-config curl
```

脚本会自动从源码下载并编译 alsa-lib、opus、speexdsp，无需手动安装对应的 `-dev` 包。

### 编译原理

1. **alsa-lib**：从源码编译为共享库（`.so`），仅用于链接时符号解析；运行时由系统的 `libasound.so.2` 提供
2. **opus / speexdsp**：由 `build.rs` 自动从源码编译为静态库（`.a`），直接打入二进制

### 验证构建结果

```bash
# 应显示 'dynamically linked'
file target/release/xiaozhi_linux_rs

# NEEDED 中应包含 libasound.so.2 和 libc.so.6，不应出现 libopus/libspeexdsp
readelf -d target/release/xiaozhi_linux_rs | grep NEEDED
```
