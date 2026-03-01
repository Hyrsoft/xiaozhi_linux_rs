## armv7-unknown-linux-gnueabihf 交叉编译说明

### 混合链接策略

本脚本采用**混合链接**方式：

- **静态链接**：alsa-lib、opus、speexdsp 编译为静态库（`.a`）直接打入二进制
- **动态链接**：保持 libc (GLIBC) 和 libdl 的动态链接

优势：
- 部署时只需拷贝单个可执行文件，无需额外 `.so` 文件
- 支持 `dlopen` 动态加载板子上的 ALSA 插件（如 PulseAudio）
- `default` 音频设备名可正常工作

### CI 自动构建

本脚本会自动下载交叉编译工具链和 C 依赖源码，无需手动配置，可直接用于 GitHub Actions CI。

```bash
# 直接运行
bash scripts/armv7-unknown-linux-gnueabihf/build.sh

# 输出: target/armv7-unknown-linux-gnueabihf/release/xiaozhi_linux_rs
```

### 工具链说明

脚本使用 ARM 官方 GCC 8.3 工具链（`gcc-arm-8.3-2019.02-x86_64-arm-linux-gnueabihf`）。

> **GLIBC 版本兼容性**：编译时工具链的 GLIBC 版本决定了二进制能运行的最低系统版本。GCC 8.3 工具链提供较低的 GLIBC 版本，兼容性较好。

### 自定义工具链与编译参数

脚本支持通过环境变量自定义工具链和编译参数。未设置时按默认逻辑自动下载；设置后会验证工具链有效性，无效则报错退出。

| 环境变量 | 说明 | 默认值 |
|---|---|---|
| `CROSS_TOOLCHAIN_DIR` | 工具链根目录（需包含 `bin/<prefix>-gcc` 等） | 自动下载 |
| `CROSS_COMPILER_PREFIX` | 编译器前缀 | `arm-linux-gnueabihf` |
| `EXTRA_CFLAGS` | 额外 C 编译参数（追加到 `-fPIC` 之后） | 无 |
| `EXTRA_RUSTFLAGS` | 额外 Rust 链接参数（追加到默认 RUSTFLAGS 之后） | 无 |

```bash
# 示例：使用自己的 ARM GCC 工具链
CROSS_TOOLCHAIN_DIR=/opt/my-arm-gcc \
  bash scripts/armv7-unknown-linux-gnueabihf/build.sh

# 示例：追加自定义 CFLAGS 和 RUSTFLAGS
EXTRA_CFLAGS="-mfpu=neon-vfpv4" \
EXTRA_RUSTFLAGS="-C link-arg=-Wl,--strip-all" \
  bash scripts/armv7-unknown-linux-gnueabihf/build.sh
```

### 验证构建结果

```bash
# 应显示 'dynamically linked'
file target/armv7-unknown-linux-gnueabihf/release/xiaozhi_linux_rs

# NEEDED 中应仅出现 libc/libdl/libpthread，不应出现 libasound/libopus/libspeexdsp
readelf -d target/armv7-unknown-linux-gnueabihf/release/xiaozhi_linux_rs | grep NEEDED
```
