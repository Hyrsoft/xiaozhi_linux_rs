# CPAL 音频设备配置说明

> 本项目已从 ALSA 直接调用迁移至 [cpal](https://github.com/RustAudioGroup/cpal) 跨平台音频库。  
> 本文档说明迁移后的音频配置方式及与旧方案的差异。

---

## 一、架构变化概述

| | 旧方案（alsa crate） | 新方案（cpal） |
|---|---|---|
| **设备查找** | 用户传入 ALSA PCM 名称，直接 `PCM::new()` | 由 cpal Host 枚举设备，按名称匹配或使用系统默认 |
| **参数设置** | 用户必须指定采样率、通道数、period 等参数，参数不兼容则直接报错 | 播放端直接使用设备默认配置，录音端由协议参数驱动并自动协商，用户无需设置任何硬件参数 |
| **采样格式** | 用户指定或由 ALSA `plughw` 插件转换 | 代码内部优先选 I16，不支持时退化到 F32 并在回调中自动转换 |
| **缓冲区管理** | ALSA 阻塞式 `readi` / `writei` | cpal 回调驱动 + 无锁环形缓冲区（ringbuf） |
| **跨平台** | 仅支持 Linux | Linux（ALSA 后端）、macOS（CoreAudio）、Windows（WASAPI）等 |

**核心变化**：用户不再需要设置任何硬件参数——播放端直接使用 `device.default_output_config()` 获取设备原生配置，录音端通过协议参数驱动并自动协商。Speex 重采样器负责所有采样率/通道数的转换。

---

## 二、配置项说明

### 配置文件位置

编译时配置（首次编译写入默认值）：

```
config.toml        # 项目根目录
```

运行时可覆盖（无需重新编译）：

```
xiaozhi_config.json # 运行目录下，自动生成
```

### 当前保留的配置项

```toml
[audio]
capture_device  = "default"   # 录音设备名称
playback_device = "default"   # 播放设备名称
stream_format   = "opus"      # 网络流编码格式（与设备无关）
```

### 各项行为解析

| 配置项 | 是否必填 | 作用 |
|---|---|---|
| `capture_device` | 是 | 传给 `cpal::Host::input_devices()` 匹配；`"default"` 使用系统默认 |
| `playback_device` | 是 | 传给 `cpal::Host::output_devices()` 匹配；`"default"` 使用系统默认 |
| `stream_format` | 是 | 控制网络音频流的解码方式（`"opus"` / `"pcm"`），与设备硬件无关 |

### 已移除的配置项

以下参数在旧版（ALSA 直接调用）中用于手动适配不同硬件，迁移到 cpal 后已无必要，**已从配置文件中移除**：

| 已移除项 | 旧作用 | 移除原因 |
|---|---|---|
| `playback_sample_rate` | 指定播放采样率 | cpal 直接使用设备默认采样率，Speex 重采样器兜底 |
| `playback_channels` | 指定播放通道数 | cpal 直接使用设备默认通道数 |
| `playback_period_size` | 指定硬件缓冲区大小 | cpal 后端自行管理缓冲区 |

> **录音端参数**（采样率、通道数）由 `[hello_message]` 中的协议参数驱动（`sample_rate = 24000`，录音固定 2 通道），不在 `[audio]` 中暴露。cpal 会与设备协商最接近的配置，Speex 重采样器处理剩余差异。

---

## 三、设备名称格式变化

### 旧方案（ALSA 原生名称）

```
"default"        — ALSA 默认设备
"plughw:0,0"     — 带 plug 插件的硬件设备
"hw:0,0"         — 直接硬件设备
"hw:rv1106acodec,0" — 按声卡名称索引
```

### 新方案（cpal 设备名称）

cpal 在 Linux 上仍使用 ALSA 后端，但设备名称是 **ALSA 报告的描述性名称**，而不是 PCM 地址。

```
"default"                                    — 系统默认（推荐，大多数场景直接可用）
"HDA Intel PCH: ALC269VC Analog (hw:0,0)"   — 桌面声卡（完整描述名）
"USB Audio: USB Audio (hw:1,0)"              — USB 声卡
```

> cpal 在内部通过 `snd_device_name_hint()` 枚举设备，返回的名称格式为
> `"<card_name>: <device_desc> (hw:X,Y)"`。  
> **不能**直接填写 `"plughw:0,0"` 或 `"hw:0,0"` 等 ALSA PCM 名称。

---

## 四、如何查看可用设备

### 方法 1：查看程序日志

程序启动时会在日志中输出设备的默认配置和录音协商结果：

```
[INFO] CPAL supported config: channels=2, rate=[8000-192000], format=I16, buffer=...
[INFO] CPAL negotiated: rate=24000, ch=2, fmt=I16
[INFO] CPAL Capture opened: device="default", rate=24000, ch=2, ...
[INFO] CPAL playback device default: rate=48000, ch=2, fmt=I16
[INFO] CPAL Playback opened: device="default", rate=48000, ch=2, ...
```

录音端协商发生降级时，会输出 `WARN` 级别日志：

```
[WARN] CPAL rate fallback: requested 24000Hz, using 48000Hz (fmt=I16)
```

### 方法 2：命令行查询

在 Linux 上仍然可以用 ALSA 工具确认硬件存在：

```bash
# 查看播放设备
aplay -l

# 查看录音设备
arecord -l
```

但填写 `config.toml` 时，设备名称需要使用 cpal 格式（完整描述名或 `"default"`）。

### 方法 3：编写枚举小程序（高级）

```rust
use cpal::traits::{DeviceTrait, HostTrait};

fn main() {
    let host = cpal::default_host();
    
    println!("=== Input (Capture) Devices ===");
    if let Ok(devices) = host.input_devices() {
        for d in devices {
            println!("  \"{}\"", d.name().unwrap_or_default());
        }
    }
    
    println!("=== Output (Playback) Devices ===");
    if let Ok(devices) = host.output_devices() {
        for d in devices {
            println!("  \"{}\"", d.name().unwrap_or_default());
        }
    }
}
```

---

## 五、自动配置机制

### 播放端

播放端直接调用 `device.default_output_config()` 使用设备的原生默认配置（采样率、通道数、采样格式），**不做任何协商**。OpusDecoder 内置的 Speex 重采样器会将 Opus 解码输出（24000 Hz / 单声道）转换到设备实际使用的格式。

### 录音端

录音端使用三级降级协商，**期望值由协议参数驱动**（非用户配置）：

```
1. 精确匹配
   采样率(24000) + 通道数(2) + 采样格式(I16优先) 全部匹配
       ↓ 不满足
2. 采样率降级
   通道数 + 格式匹配，采样率选最近的设备支持值
       ↓ 不满足
3. 完全降级
   使用设备报告的第一个可用配置
```

**后续处理**：无论设备实际使用了什么采样率/通道数，工作线程都会通过 Speex 重采样器和通道混合将音频转换到 Opus 编码器要求的格式（24000 Hz / 单声道）。因此即使设备不支持 24000 Hz 也完全不影响功能。

---

## 六、典型配置示例

### 场景 1：绝大多数用户（推荐）

```toml
[audio]
capture_device  = "default"
playback_device = "default"
stream_format   = "opus"
```

使用系统默认设备，播放参数由 cpal 自动获取设备默认值。**绝大多数情况下不需要修改。**

### 场景 2：指定特定设备

先通过日志或枚举工具获取设备全名，然后填入：

```toml
[audio]
capture_device  = "USB Audio: USB Audio (hw:1,0)"
playback_device = "default"
stream_format   = "opus"
```

### 场景 3：嵌入式开发板

```toml
[audio]
capture_device  = "default"
playback_device = "default"
stream_format   = "opus"
```

在嵌入式场景下，设备可能只支持 16000 Hz 等较低采样率。无需任何手动适配——cpal 直接使用设备默认配置，Speex 重采样器自动处理与 Opus 编解码器之间的格式转换。

---

## 七、与旧文档的对照

旧文档 `docs/音频设备配置说明.md` 描述的是 ALSA 原生接口下的配置方式，迁移到 cpal 后主要差异如下：

| 旧文档建议 | 迁移后状态 |
|---|---|
| 使用 `"plughw:X,Y"` 格式 | ❌ 不再适用。使用 `"default"` 或 cpal 完整设备名 |
| 使用 `"hw:CARDNAME,Y"` 格式 | ❌ 不再适用 |
| `default` 与 `plughw` 的选择建议 | ⚠️ 简化为：`"default"` 即可覆盖绝大多数场景 |
| 采样率/格式必须与硬件严格匹配 | ✅ 已移除所有硬件参数配置。播放用设备默认，录音自动协商 + Speex 重采样兜底 |
| 设备名传给 `PCM::new()` | ✅ 已改为传给 `cpal::Host` 设备枚举匹配 |

---

## 八、故障排除

### "No default input/output device available"

系统未配置默认音频设备。检查：
- `aplay -l` / `arecord -l` 是否有设备列出
- ALSA 配置文件 `/etc/asound.conf` 或 `~/.asoundrc` 是否正确
- 嵌入式系统是否加载了声卡驱动（`lsmod | grep snd`）

### "Input/Output device 'xxx' not found"

指定的设备名称与 cpal 枚举结果不匹配。请：
1. 检查日志中输出的可用设备名称
2. 确保使用 cpal 格式的完整设备名（而非 ALSA `hw:X,Y` 格式）
3. 改用 `"default"` 测试

### 播放有杂音/卡顿

- 确认系统 CPU 负载不过高（Speex 预处理和 Opus 编解码需要一定算力）
- 检查日志中 `CPAL playback device default` 输出的实际采样率是否合理
- 如果设备默认采样率与 Opus 差异很大（如 192000 Hz），重采样开销会更高

### 在非 Linux 平台上使用

cpal 天然支持 macOS（CoreAudio）和 Windows（WASAPI）。设备名称格式因平台而异，但 `"default"` 在所有平台上都可用。具体设备名称请参考各平台的 cpal 文档。
