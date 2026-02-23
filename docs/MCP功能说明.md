# MCP 功能说明

本程序采用了**解耦的 MCP (Model Context Protocol) 设计**，使外部工具的扩展和集成变得非常简单。在这个设计中，本程序**仅作为 MCP 网关**存在，专门负责处理与云端大模型的 JSON-RPC 消息交互、协议解析以及外部工具的拉起和生命周期管理。

这种设计的核心优势在于：
1. **动态配置**：所有的 MCP 工具设置都可以通过修改 `xiaozhi_config.json` 动态完成。只需要修改配置文件并**重启程序**即可立即生效，**无须重新编译**主程序代码。
2. **标准通信规范**：网关与外部脚本（各种工具）之间采用最基础的 **stdin 标准输入**和 **stdout 标准输出** 管道进行通信。
   - 输入：大模型提取的参数将被格式化为 **JSON 字符串**，并通过 stdin 传递给脚本。
   - 输出：脚本的执行结果（无论成功或错误信息）直接通过 stdout 打印，网关会捕获这些输出并原样返回给大模型。
3. **功能解耦**：每个外部脚本只负责完成一个具体的任务。主程序无需了解脚本的内部业务逻辑，脚本也无需了解复杂的网络和通信协议栈。

---

## 动态修改配置

你可以通过修改 `xiaozhi_config.json` 中的 `mcp` 字段来自由添加、删除或修改工具：

```json
{
  ...
  "mcp": {
    "enabled": true,
    "tools": [
      ...
    ]
  }
}
```

每次修改了 `tools` 列表后，重新运行程序即可生效，大模型将会立即感知到这些新的工具和参数约束。

---

## 现有功能示例

根据当前的 `xiaozhi_config.json` 配置文件，在 `scripts/` 目录下准备了两个示例脚本：

### 示例 1: 获取系统状态 (`test_tool.sh`)

这是一个 Bash 脚本，用于查询当前设备的负载、内存和磁盘信息。

**交互方式：**
- 当你对小智说：“现在系统压力大吗？”或“系统运行多久了？”
- **流程**：模型识别意图 -> 触发 `get_system_status` 工具 -> 网关拉起 `test_tool.sh` 脚本。脚本执行 `uptime`, `free`, `df` 等基础系统命令并将信息打印至 stdout。
- **结果**：小智会根据拿到的真实系统状态为你总结当前的运行情况。

```json
  "mcp": {
    "enabled": true,
    "tools": [
      {
        "name": "get_system_status",
        "description": "获取当前设备的系统状态，包括 CPU 负载、内存使用率、磁盘空间和运行时间。当用户询问系统压力、运行多久、负载情况时调用。",
        "executable": "./test_tool.sh",
        "input_schema": {
          "properties": {},
          "type": "object"
        }
      }
    ]
  }
```

### 示例 2: 调节屏幕亮度 (`set_brightness.py`)

这是一个 Python 脚本，用于调节开发板（如 Luckfox）的屏幕亮度，展示了 MCP 如何基于 JSON Schema 进行严格的参数校验和业务处理。

**交互方式：**
- 当你对小智说：“调亮一点”、“调暗一点”或“把亮度设置为80”等指令。
- **流程**：模型识别指令并计算目标亮度值 -> 触发 `set_brightness` 工具 -> 网关将 JSON 数据 `{"brightness": 80}` 通过 stdin 传给 `set_brightness.py`。
- 脚本内使用 Python 的 `json.loads` 解析传入数据，并对 0-100 的数值进行二次范围校验。成功校验后，脚本修改如 `/sys/class/backlight/` 的系统设备文件来真实调节亮度，最终将结果通过 stdout 输出。
- **结果**：小智告诉你：“没问题，亮度已经调整到 80% 啦。”

```json
  "mcp": {
    "enabled": true,
    "tools": [
      {
        "name": "set_brightness",
        "description": "设置设备屏幕的亮度。当用户要求‘调亮一点’、‘调暗一点’或指定具体亮度数值时调用。",
        "executable": "./set_brightness.py",
        "input_schema": {
          "type": "object",
          "properties": {
            "brightness": {
              "type": "integer",
              "minimum": 0,
              "maximum": 100,
              "description": "目标亮度值，范围从0（最暗）到100（最亮）"
            }
          },
          "required": ["brightness"]
        }
      }
    ]
  }
```

---

## 总结

通过这种解耦的网关机制，你可以**随时随地**使用 **任何你熟悉的语言** (Bash、Python、Node.js、C++等) 编写自定义的 MCP 工具脚本，只需要确保其能通过 stdin/stdout 读取和返回 JSON 或文本即可。
