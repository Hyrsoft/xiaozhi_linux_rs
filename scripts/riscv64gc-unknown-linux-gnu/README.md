## riscv64gc-unknown-linux-gnu 交叉编译说明

### 混合链接策略

本脚本采用**混合链接**方式：

- **静态链接**：opus、speexdsp 编译为静态库（`.a`）直接打入二进制
- **动态链接**：ALSA (`libasound.so.2`)、libc (GLIBC)、libdl 以及 libpthread 保持动态链接

优势：
- opus 和 speexdsp 静态链接，减少部署时对音视频编解码额外 `.so` 依赖的烦恼
- 动态链接 `libasound.so`，从而支持 `dlopen` 动态加载板子上的 ALSA 插件（如 PulseAudio/PipeWire 插件及各类硬件混音器）
- `default` 音频设备名可顺畅工作

### ⚠️ RISC-V 生态与兼容性说明

由于目前 **RISC-V 生态尚在快速发展期，存在较为明显的碎片化现象**，各家芯片厂商（如全志、平头哥、阿里、赛昉等）的底层 BSP 实现、工具链版本、指令集扩展（各种定制的 Z 扩展支持）以及音频驱动的完善程度可能存在较大差异。

- **兼容性暂不确定**：目前交叉编译链基于通用 GNU Linux 目标构建（GCC 8 且忽略指令集扩展 mismatched 告警）。这能否完美兼容市面上繁杂的各类 RISC-V 单板机/开发板，**暂时不能完全确定**。
- **音频驱动潜在风险**：不同 RISC-V 长期支持内核对 ALSA 以及上层音频插件的支持差异明显，部分板子可能遇到 `arecord`/`aplay` 及底层设备的诡异问题，导致找不到设备、回声消除失效或播放杂音。
- **亟待更多测试**：当前该 Target 的产物仍处于**试验阶段**，强烈建议您在手头的硬件平台上进行充分测试。如果运行正常或遇到链接、段错误、总线错误、音频失效等问题，十分欢迎提交 Issue 或 PR 反馈/修补兼容性情况。

### CI 自动构建 / 本地构建

基于本仓库，自动化构建脚本会自动下载一套 `riscv64-wangzai-linux-gnu-gcc` 交叉工具链及 C 依赖。无需手动配置繁复的环境。

```bash
# 直接在仓库根目录执行构建脚本
bash scripts/riscv64gc-unknown-linux-gnu/build.sh

# 成功后产物输出至: target/riscv64gc-unknown-linux-gnu/release/xiaozhi_linux_rs
```

### 工具链与构建细节说明

本脚本默认使用定制的 RISC-V GCC 工具链。

> **链接器警告说明**：由于较老版本的 GCC 工具链 (如 GNU ld 2.3x) 不识别 Rust 较新版本注入的一些新 RISC-V Z 扩展属性，这将会抛出属性合并警告。**脚本已经默认启用了 `-Wl,--no-warn-mismatch` 屏蔽该误报**，这通常不影响运行时的正确性。
>
> 如果您的 RISC-V 系统提供的 GLIBC 版本甚至比编译器自带预期的还要老，或指令集无法兼容本编译产物（通常表现为 `Illegal instruction` 或类似找不到 GLIBC_XXX 符号），您可能需要修改 `build.sh` 替换为硬件厂商（BSP）原生提供的专有 GCC 工具链重新执行构建。

### 验证构建结果

将产物上传至您的 RISC-V 主板，或者在您的构建宿主环境验证依赖：

```bash
# 应显示 'dynamically linked' 和 'RISC-V' 架构
file target/riscv64gc-unknown-linux-gnu/release/xiaozhi_linux_rs

# NEEDED 中应包含 libasound.so.2、libc.so.6、libdl.so 等系统提供库
# 不应出现 libopus 或 libspeexdsp (因为它们已被静态链入)
readelf -d target/riscv64gc-unknown-linux-gnu/release/xiaozhi_linux_rs | grep NEEDED
```
