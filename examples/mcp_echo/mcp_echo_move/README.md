# MCP 机器人移动控制示例

本示例演示如何通过小智 Linux 的 MCP 子进程工具，使用 GPIO 控制机器人移动。语音命令触发后，调用 `robot_control.py` 脚本驱动电机。

## 文件说明

- `robot_control.py`：GPIO 控制脚本，解析 JSON 输入并设置对应引脚电平。
- `config.json`：MCP 工具配置示例，需合并到主配置文件 `xiaozhi_config.json` 中。
- `README.md`：本文档。

## 硬件接线

假设使用树莓派或类似 Linux 开发板，通过 sysfs 控制 GPIO。示例中默认引脚对应关系如下（可根据实际修改脚本）：

```python
GPIO_PINS = {
    'left1': 32,   # 左电机正向
    'left2': 33,   # 左电机反向
    'right1': 35,  # 右电机正向
    'right2': 36   # 右电机反向
}
```
