# MCP 音量调节工具示例

本示例演示如何通过小智 Linux 的 MCP 子进程工具，使用 `amixer` 命令调节系统音量。语音命令触发后，调用 `set_volume.py` 脚本设置音量百分比。

## 文件说明

- `set_volume.py`：音量控制脚本，解析 JSON 输入中的 `volume` 参数（0~100），执行 `amixer` 命令设置音量。

- `config.json`：MCP 工具配置示例，需合并到主配置文件 `xiaozhi_config.json` 中。

- `README.md`：本文档。

## 自定义声卡控制 ⚠️ 重要提示

**不同设备的声卡控制名称可能不同**，请根据你的系统修改脚本中的控制名。打开 `set_volume.py`，找到以下部分：

python

CARD = "0"
CONTROL = "DAC LINEOUT"

- `CARD`：声卡编号，默认为 `0`。可通过 `amixer cards` 查看。

- `CONTROL`：音量控制项名称。执行以下命令查看可用控制：
  
  bash
  
  amixer scontrols
  
  例如，如果你的设备输出为 `'Master'`，则将 `CONTROL` 改为 `'Master'`。

脚本使用的命令为：

bash

amixer -c 0 sset 'DAC LINEOUT' X%

请务必根据你的实际控制名称修改 `CONTROL` 变量。

## 集成到小智 Linux

1. 将 `set_volume.py` 复制到开发板（例如 `/root/` 目录），并赋予可执行权限：
   
   bash
   
   chmod +x /root/set_volume.py

2. 参考 `config.json` 中的 `mcp.tools` 数组内容，将其合并到你现有的 `xiaozhi_config.json` 中。  
   **注意**：不要直接覆盖原有配置文件，只需将 `set_volume` 工具对象添加到 `tools` 数组中。

3. 重启小智 Linux 主程序：
   
   bash
   
   ./xiaozhi_linux_rs-armv7-uclibceabih

## 使用方法

唤醒小智后，说出以下指令之一：

- “音量调到 50%”

- “把声音设置为 80%”

- “音量调到最大” （大模型可能将“最大”理解为 100，取决于大模型能力）

- “静音” （大模型可能将“静音”理解为 0）

机器人将执行 `amixer` 命令，脚本执行后会返回一条确认信息（如“音量已设置为 50%”），该信息会传递给大模型，大模型可能会据此生成语音回复。

## 测试脚本

在开发板上手动测试脚本是否正常工作：

bash

echo '{"volume": 50}' | python3 set_volume.py

预期输出：`音量已设置为 50%`，同时系统音量应实际变化（可通过播放音频验证）。

## 故障排除

- **amixer: command not found**：未安装 `alsa-utils`，请使用 `opkg install alsa-utils` 或 `apt-get install alsa-utils` 安装。

- **音量无变化**：检查 `CONTROL` 名称是否正确，以及声卡编号是否正确。

- **音量范围异常**：脚本内部做了 0~100 的校验，如果传递的值超出范围，会返回错误。

- **权限问题**：通常操作声卡不需要 root，但如果出现权限错误，可以尝试以 root 运行主程序。

---

本示例仅为参考，你可以根据实际硬件自由修改。欢迎为项目贡献更多实用工具！
