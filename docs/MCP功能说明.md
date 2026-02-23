# MCP 功能说明

本程序采用了**解耦的 MCP (Model Context Protocol) 设计**，使外部工具的扩展和集成变得非常简单。程序**仅作为 MCP 网关**，负责与云端大模型的 JSON-RPC 消息交互、协议解析以及外部工具的拉起和生命周期管理。

## 核心架构

### 设计优势

1. **动态配置**：所有 MCP 工具设置通过 `xiaozhi_config.json` 完成，修改后**重启程序**即可生效，**无须重新编译**。
2. **多传输协议**：支持 **Subprocess（子进程）**、**HTTP** 和 **TCP Socket** 三种传输方式，通过配置切换。
3. **双执行模式**：
   - **sync**（默认）—— 对话级同步：等待工具执行完成，结果直接返回给大模型。
   - **background** —— 对话级异步：立刻返回"已启动"给大模型，后台执行任务。
4. **统一超时控制**：所有工具调用均有 `timeout_ms` 超时保护，防止外部脚本假死导致系统阻塞。
5. **纯异步非阻塞**：底层全部基于 `tokio` 异步运行时，无论哪种传输协议都不会阻塞系统线程。
6. **功能解耦**：每个外部脚本只负责完成一个具体任务，主程序无需了解脚本内部逻辑。

---

## 配置格式

### 完整的工具配置字段

```json
{
  "mcp": {
    "enabled": true,
    "tools": [
      {
        "name": "工具名称",
        "description": "工具描述（大模型据此判断何时调用）",
        "input_schema": { ... },
        "type": "subprocess | http | tcp",
        "mode": "sync | background",
        "timeout_ms": 5000,
        "notify": { "type": "disabled" }
      }
    ]
  }
}
```

| 字段 | 必填 | 说明 |
|------|------|------|
| `name` | 是 | 工具唯一名称 |
| `description` | 是 | 功能描述，大模型根据此文本判断调用时机 |
| `input_schema` | 是 | JSON Schema，定义工具接受的参数结构 |
| `type` | 是 | 传输协议：`subprocess`、`http`、`tcp` |
| `mode` | 否 | 执行模式：`sync`（默认）或 `background` |
| `timeout_ms` | 否 | 超时时间（毫秒），默认 5000 |
| `notify` | 否 | 异步任务完成通知方式，默认 `disabled`（仅对 `background` 模式有效） |

### 传输协议特有字段

**Subprocess 模式**（`"type": "subprocess"`）：

| 字段 | 说明 |
|------|------|
| `executable` | 可执行文件路径 |
| `args` | 命令行参数数组（可选） |

**HTTP 模式**（`"type": "http"`）：

| 字段 | 说明 |
|------|------|
| `url` | HTTP 接口地址 |
| `method` | 请求方法，默认 `POST` |

**TCP 模式**（`"type": "tcp"`）：

| 字段 | 说明 |
|------|------|
| `address` | TCP 地址，格式 `host:port` |

---

## 执行模式详解

### Sync 模式（对话级同步）

这是默认模式。大模型触发工具调用后，网关等待工具执行完成，将结果原样返回给大模型。适用于**执行时间短**（秒级）的工具。

```
用户说话 → 大模型决定调用工具 → 网关执行工具(等待) → 结果返回大模型 → 大模型回复用户
```

### Background 模式（对话级异步）

适用于**执行时间长**的工具。网关立刻向大模型返回 `{"status": "started"}`，后台用 `tokio::spawn` 异步执行任务。大模型会告知用户"任务已启动"，对话可以继续。

```
用户说话 → 大模型决定调用工具 → 网关立即返回"已启动" → 大模型回复"正在后台处理"
                                    ↓
                              后台异步执行任务
                                    ↓
                              完成后写入日志（通过 notify 接口处理）
```

**关于异步通知（notify 字段）**：

Background 模式的工具需要配置 `notify` 字段，决定任务完成后的通知方式。当前仅支持 `disabled`（打印日志），预留了 Webhook、LocalSocket、MQTT 等扩展接口。

由于云端大模型的语音对话被严格限制在"唤醒-倾听-思考-回复-待机"的刚性状态机中，端侧无法将异步结果注入回对话流。因此通知机制与对话流程完全解耦，后续可通过实现 `NotifyMethod` 的其他变体来接入 GUI 弹窗、硬件指示灯、蜂鸣器等外部反馈通道。

---

## 现有功能示例

### 示例 1: 获取系统状态（Sync + Subprocess）

一个 Bash 脚本，查询设备的负载、内存和磁盘信息。

**交互方式：** 对小智说"系统运行多久了？"或"现在负载大吗？"

```json
{
  "name": "get_system_status",
  "description": "获取当前设备的系统状态，包括 CPU 负载、内存使用率、磁盘空间和运行时间。",
  "type": "subprocess",
  "executable": "./test_tool.sh",
  "mode": "sync",
  "timeout_ms": 5000,
  "input_schema": {
    "type": "object",
    "properties": {}
  }
}
```

### 示例 2: 调节屏幕亮度（Sync + Subprocess）

一个 Python 脚本，接收 JSON 参数调节屏幕亮度。

**交互方式：** 对小智说"调亮一点"或"把亮度设置为80"

```json
{
  "name": "set_brightness",
  "description": "设置设备屏幕的亮度。",
  "type": "subprocess",
  "executable": "./set_brightness.py",
  "mode": "sync",
  "timeout_ms": 5000,
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
```

### 示例 3: 后台写入任务（Background + Subprocess）

一个耗时 10 秒的后台脚本，每秒写入一行进度到文件。

**交互方式：** 对小智说"启动后台写入任务"

```json
{
  "name": "long_time_write_task",
  "description": "启动一个耗时10秒的后台写入任务。任务会每秒向指定文件写入进度。",
  "type": "subprocess",
  "executable": "./incremental_writer.py",
  "mode": "background",
  "timeout_ms": 15000,
  "notify": { "type": "disabled" },
  "input_schema": {
    "type": "object",
    "properties": {
      "file_path": { "type": "string", "description": "要写入的文件路径" },
      "text": { "type": "string", "description": "要循环写入的文本内容" }
    }
  }
}
```

### 示例 4: 调用远程 HTTP 服务（Sync + HTTP）

```json
{
  "name": "remote_ai_tool",
  "description": "调用远程AI服务接口进行推理",
  "type": "http",
  "url": "http://192.168.1.100:8080/api/tool",
  "method": "POST",
  "mode": "sync",
  "timeout_ms": 10000,
  "input_schema": {
    "type": "object",
    "properties": {
      "prompt": { "type": "string", "description": "输入文本" }
    }
  }
}
```

### 示例 5: TCP Socket 控制内网设备（Sync + TCP）

```json
{
  "name": "iot_controller",
  "description": "通过Socket控制内网IoT设备",
  "type": "tcp",
  "address": "192.168.1.50:9999",
  "mode": "sync",
  "timeout_ms": 2000,
  "input_schema": {
    "type": "object",
    "properties": {
      "command": { "type": "string", "description": "设备控制命令" }
    }
  }
}
```

---

## 通信规范

### Subprocess 模式

网关与外部脚本之间采用 **stdin/stdout** 管道通信：
- **输入**：大模型提取的参数格式化为 JSON 字符串，通过 stdin 传递给脚本。
- **输出**：脚本执行结果（文本或 JSON）通过 stdout 打印，网关捕获后原样返回给大模型。

### HTTP 模式

- **POST**：参数作为 JSON Body 发送，响应 Body 作为结果返回。
- **GET**：参数作为 Query 参数发送。

### TCP 模式

- 参数序列化为 JSON + 换行符发送，读取响应文本作为结果。

---

## 扩展指南

### 添加新工具

1. 编写工具脚本（任意语言），确保能通过对应传输协议读入 JSON 参数并返回结果。
2. 在 `xiaozhi_config.json` 的 `mcp.tools` 数组中添加配置。
3. 重启程序即可生效。

### 添加新传输协议

在 `mcp_gateway.rs` 的 `ToolTransport` 枚举中新增变体，并在 `execute_inner` 中添加对应的 `match` 分支。

### 实现异步通知接口

在 `mcp_gateway.rs` 的 `NotifyMethod` 枚举中新增变体（如 `Webhook`、`LocalSocket`），并在 Background 任务完成后的 `match` 分支中实现对应的通知逻辑。核心对话流程无需任何修改。
