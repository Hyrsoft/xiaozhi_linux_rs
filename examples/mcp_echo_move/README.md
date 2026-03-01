# 油炸机开源 echo 机器人 - GPIO 移动控制示例

本示例为 **油炸机开源的 echo 机器人**（基于 [Xiaozhi Linux](https://github.com/%E4%BD%A0%E7%9A%84%E6%9C%8B%E5%8F%8B%E7%9A%84%E9%A1%B9%E7%9B%AE%E5%9C%B0%E5%9D%80) 项目）提供了一个通过 GPIO 控制机器人移动的 MCP 工具。  
你可以通过语音指令让机器人前进、后退、左转、右转或停止，每次移动一小段距离后自动停下，避免持续运行。

## 文件说明

- `robot_control.py` – Python 脚本，负责解析 JSON 参数并操作 GPIO。

- `xiaozhi_config.robot.json` – MCP 工具配置示例，展示如何将该工具添加到主配置文件 `xiaozhi_config.json` 中。

## 硬件要求

- RV1106 或任何支持 Linux GPIO 控制的开发板。

- 两个直流电机（左轮和右轮），通过 GPIO 控制（例如使用 L298N 或 TB6612 驱动）。

- 接线方式（默认）：
  
  - GPIO32 → 左轮 IN1
  
  - GPIO33 → 左轮 IN2
  
  - GPIO35 → 右轮 IN1
  
  - GPIO36 → 右轮 IN2

## 自定义接线 ⚠️ 重要提示

**每个人的硬件接线可能不同**，请务必根据你的实际连接修改脚本中的引脚定义。  
打开 `robot_control.py`，找到以下部分：

python

GPIO_PINS = {
    'left1': 32,
    'left2': 33,
    'right1': 35,
    'right2': 36
}

将数字改为你实际使用的 GPIO 编号。  
如果电机转向与实际要求相反（例如语音“左转”后机器人向右转），可以调整 `ACTIONS` 字典中的元组：

python

ACTIONS = {
    'forward':  (0, 1, 1, 0),   # (左1, 左2, 右1, 右2)
    'backward': (1, 0, 0, 1),
    'left':     (0, 1, 0, 1),   # 左转（原地）
    'right':    (1, 0, 1, 0),   # 右转（原地）
    'stop':     (0, 0, 0, 0)
}

每个元组的四个值分别对应 `left1`, `left2`, `right1`, `right2` 的输出电平。你可以根据自己的电机驱动逻辑修改这些值。

## 移动时长调整

默认每次移动持续 **0.8 秒** 后自动停止。如需改变，修改脚本中的 `MOVE_DURATION` 变量（单位秒）：

python

MOVE_DURATION = 0.8   # 修改为你想要的时长

## 集成到 Xiaozhi Linux

1. 将 `robot_control.py` 复制到你的开发板（例如 `/root/` 目录），并赋予可执行权限：
   
   bash
   
   chmod +x /root/robot_control.py

2. 参考 `xiaozhi_config.robot.json` 中的 `mcp.tools` 数组内容，将其合并到你现有的 `xiaozhi_config.json` 中。  
   **注意**：不要直接覆盖原有配置文件，只需将 `control_robot` 工具对象添加到 `tools` 数组中。

3. 重启 Xiaozhi Linux 主程序：
   
   bash
   
   ./xiaozhi_linux_rs-armv7-uclibceabih

## 使用方法

唤醒小智后，说出以下指令之一：

- “小车前进” / “向前走”

- “后退”

- “左转”

- “右转”

- “停止”

机器人将执行相应动作，移动一小段距离后自动停止。

## 故障排除

- **日志中出现编码错误**：确保 `robot_control.py` 第一行包含 `# -*- coding: utf-8 -*-`，且文件保存为 UTF-8 格式。

- **GPIO 操作无反应**：检查引脚号是否正确，以及程序是否以 root 权限运行（GPIO 通常需要 root）。

- **方向相反**：调整 `ACTIONS` 字典中的元组，或交换电机接线。

---

本示例仅为参考，你可以根据实际硬件自由修改。欢迎为项目贡献更多实用工具！
