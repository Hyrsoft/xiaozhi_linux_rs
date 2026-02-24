use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::process::Stdio;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::time::{timeout, Duration};

// ==========================================
// 1. Protocol Definitions
// ==========================================

#[derive(Deserialize, Debug)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub method: String,
    pub params: Option<Value>,
    pub id: Option<Value>,
}

#[derive(Serialize, Debug)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<Value>,
}

// ==========================================
// 2. Configuration Types
// ==========================================

/// 执行模式 —— 对话语义层面的同步/异步
/// - Sync（默认）：等待执行完成，结果返回给大模型（对话级同步）
/// - Background：立刻返回，后台执行，完成后通过状态机通知队列告知用户（对话级异步）
#[derive(Clone, Debug, Deserialize, Serialize, Default, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ExecutionMode {
    #[default]
    Sync,
    Background,
}

/// 传输协议类型
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ToolTransport {
    /// 子进程 stdin/stdout 模式
    Subprocess {
        executable: String,
        #[serde(default)]
        args: Vec<String>,
    },
    /// HTTP/REST 调用
    Http {
        url: String,
        #[serde(default = "default_http_method")]
        method: String,
    },
    /// TCP Socket JSON 通信
    Tcp {
        address: String,
    },
}

fn default_http_method() -> String {
    "POST".to_string()
}

fn default_timeout() -> u64 {
    5000
}

/// 异步工具执行完成后的通知方式
/// 预留接口，当前仅支持 Disabled，后续可扩展 Webhook / LocalSocket / Mqtt 等
#[derive(Clone, Debug, Deserialize, Serialize, Default, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum NotifyMethod {
    #[default]
    Disabled,
    // 预留未来的接口：
    // Webhook { url: String },
    // LocalSocket { path: String },
    // Mqtt { topic: String },
}

/// 统一的工具配置
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ExternalToolConfig {
    pub name: String,
    pub description: String,
    pub input_schema: Value,

    /// 对话语义层面的执行模式，默认为 sync
    #[serde(default)]
    pub mode: ExecutionMode,

    /// 统一超时时间（毫秒），默认 5000ms
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,

    /// 传输协议配置（扁平化到同一层 JSON/TOML）
    #[serde(flatten)]
    pub transport: ToolTransport,

    /// 异步任务完成后的通知方式（仅对 background 模式有效），默认为 disabled
    #[serde(default)]
    pub notify: NotifyMethod,
}

// ==========================================
// 3. Tool Trait
// ==========================================

#[async_trait]
pub trait McpTool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> Value;
    async fn call(&self, params: Value) -> Result<Value, String>;
}

// ==========================================
// 4. DynamicTool — 多传输协议 + 多执行模式
// ==========================================

pub struct DynamicTool {
    config: ExternalToolConfig,
}

impl DynamicTool {
    pub fn new(config: ExternalToolConfig) -> Self {
        Self { config }
    }

    /// 根据传输协议类型分发执行（纯异步非阻塞）
    async fn execute_inner(config: &ExternalToolConfig, params: Value) -> Result<Value, String> {
        match &config.transport {
            ToolTransport::Subprocess { executable, args } => {
                Self::exec_subprocess(executable, args, params).await
            }
            ToolTransport::Http { url, method } => {
                Self::exec_http(url, method, params).await
            }
            ToolTransport::Tcp { address } => {
                Self::exec_tcp(address, params).await
            }
        }
    }

    /// 子进程执行（tokio::process，异步非阻塞）
    async fn exec_subprocess(
        executable: &str,
        args: &[String],
        params: Value,
    ) -> Result<Value, String> {
        let args_json = serde_json::to_string(&params).unwrap_or_default();
        log::info!("Executing subprocess tool: {}, args: {}", executable, args_json);

        let mut child = Command::new(executable)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to spawn {}: {}", executable, e))?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(args_json.as_bytes()).await.unwrap_or_default();
        }

        let output = child
            .wait_with_output()
            .await
            .map_err(|e| format!("Failed to wait for {}: {}", executable, e))?;

        if output.status.success() {
            let result_str = String::from_utf8_lossy(&output.stdout).to_string();
            Ok(json!(result_str))
        } else {
            let err_str = String::from_utf8_lossy(&output.stderr).to_string();
            Err(format!("Subprocess error: {}", err_str))
        }
    }

    /// HTTP 调用（reqwest 异步非阻塞）
    async fn exec_http(url: &str, method: &str, params: Value) -> Result<Value, String> {
        let client = reqwest::Client::new();

        let request = match method.to_uppercase().as_str() {
            "GET" => client.get(url),
            _ => client.post(url).json(&params),
        };

        let response = request
            .send()
            .await
            .map_err(|e| format!("HTTP request failed: {}", e))?;

        let text = response
            .text()
            .await
            .map_err(|e| format!("Failed to read HTTP response: {}", e))?;

        Ok(json!(text))
    }

    /// TCP Socket 调用（tokio::net，异步非阻塞）
    async fn exec_tcp(address: &str, params: Value) -> Result<Value, String> {
        use tokio::io::AsyncReadExt;
        use tokio::net::TcpStream;

        let mut stream = TcpStream::connect(address)
            .await
            .map_err(|e| format!("TCP connection to {} failed: {}", address, e))?;

        let mut payload = serde_json::to_vec(&params).unwrap_or_default();
        payload.push(b'\n');

        stream
            .write_all(&payload)
            .await
            .map_err(|e| format!("TCP write failed: {}", e))?;

        let mut buf = vec![0u8; 4096];
        let n = stream
            .read(&mut buf)
            .await
            .map_err(|e| format!("TCP read failed: {}", e))?;

        let result_str = String::from_utf8_lossy(&buf[..n]).to_string();
        Ok(json!(result_str))
    }
}

#[async_trait]
impl McpTool for DynamicTool {
    fn name(&self) -> &str {
        &self.config.name
    }

    fn description(&self) -> &str {
        &self.config.description
    }

    fn input_schema(&self) -> Value {
        self.config.input_schema.clone()
    }

    async fn call(&self, params: Value) -> Result<Value, String> {
        // ---- 后台模式（对话级异步）：立刻返回，后台执行，完成后仅打印日志 ----
        if self.config.mode == ExecutionMode::Background {
            let config_clone = self.config.clone();
            let timeout_ms = self.config.timeout_ms;

            tokio::spawn(async move {
                log::info!(">>> 后台任务已启动: {}", config_clone.name);
                log::info!(">>> 后台任务已启动: {}", config_clone.name);
                let timeout_duration = Duration::from_millis(timeout_ms);

                let result = match timeout(
                    timeout_duration,
                    Self::execute_inner(&config_clone, params),
                )
                .await
                {
                    Ok(Ok(value)) => {
                        let msg = value.as_str().unwrap_or(&value.to_string()).to_string();
                        let mcp_output = json!({
                            "content": [{
                                "type": "text",
                                "text": msg
                            }]
                        });
                        log::info!("✓ 后台任务 [{}] 执行完成 | MCP输出: {}", config_clone.name, mcp_output.to_string());
                        log::info!("✓ 后台任务 [{}] 执行完成 | 脚本输出: {}", config_clone.name, msg);
                        Ok(msg)
                    }
                    Ok(Err(err)) => {
                        log::error!("✗ 后台任务 [{}] 执行失败 | 错误信息: {}", config_clone.name, err);
                        log::error!("✗ 后台任务 [{}] 执行失败 | 错误信息: {}", config_clone.name, err);
                        Err(err)
                    }
                    Err(_) => {
                        log::error!("⏱ 后台任务 [{}] 执行超时 ({}ms)", config_clone.name, timeout_ms);
                        log::error!("⏱ 后台任务 [{}] 执行超时 ({}ms)", config_clone.name, timeout_ms);
                        Err(format!("后台任务超时 ({}ms)", timeout_ms))
                    }
                };

                // 根据 notify 配置处理完成通知（当前仅支持 Disabled，后续可扩展）
                match &config_clone.notify {
                    NotifyMethod::Disabled => {
                        // Disabled 模式：完成情况已在上方通过日志记录，此处无需额外操作
                        // 后续可扩展为 Webhook / LocalSocket / MQTT 等通知方式
                        log::info!("📝 后台任务 [{}] 完成结果已通过日志和标准错误输出记录", config_clone.name);
                    }
                    // 预留的防御性分支，防止未来加了配置但这里没实现
                    #[allow(unreachable_patterns)]
                    other => {
                        log::warn!("后台任务 [{}] 配置了未实现的通知方式: {:?}，已忽略", config_clone.name, other);
                        log::warn!("⚠️ 后台任务 [{}] 配置了未实现的通知方式: {:?}", config_clone.name, other);
                    }
                }
            });

            // 立刻返回，不阻塞大模型对话
            return Ok(json!({
                "status": "started",
                "message": format!("任务 '{}' 已在后台启动，完成后会通知您。", self.config.name)
            }));
        }

        // ---- 标准同步模式（对话级同步）：等待执行完成，结果返回给大模型 ----
        let timeout_duration = Duration::from_millis(self.config.timeout_ms);
        let config = &self.config;

        match timeout(timeout_duration, Self::execute_inner(config, params)).await {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(err)) => Err(err),
            Err(_) => Err(format!(
                "Tool '{}' execution timed out after {} ms",
                self.config.name, self.config.timeout_ms
            )),
        }
    }
}

// ==========================================
// 5. MCP Server Router
// ==========================================

pub struct McpServer {
    tools: HashMap<String, Box<dyn McpTool>>,
}

impl McpServer {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn register_tool(&mut self, tool: Box<dyn McpTool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    /// Handles an incoming WS text message. If it is a valid JSON-RPC for MCP,
    /// returns `Some(response_text)`. Otherwise returns `None`.
    pub async fn handle_message(&self, payload: &str) -> Option<String> {
        let req: JsonRpcRequest = match serde_json::from_str(payload) {
            Ok(r) => r,
            Err(_) => return None, // Ignore non-JSON-RPC payload
        };

        if req.jsonrpc != "2.0" {
            return None;
        }

        // 按照 JSON-RPC 2.0 规范，通知消息（没有 id 字段）不需要响应
        // 参考 xiaozhi-esp32: if (method_str.find("notifications") == 0) { return; }
        if req.id.is_none() || req.method.starts_with("notifications") {
            log::info!("MCP notification received (no response needed): {}", req.method);
            return Some(String::new()); // 返回空字符串表示已处理但不发送响应
        }

        let result = match req.method.as_str() {
            "initialize" => Ok(json!({
                "protocolVersion": "2024-11-05",
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "xiaozhi_linux_rs", "version": "1.0.0" }
            })),
            "tools/list" => {
                let tool_list: Vec<Value> = self.tools.values().map(|t| {
                    json!({
                        "name": t.name(),
                        "description": t.description(),
                        "inputSchema": t.input_schema()
                    })
                }).collect();
                Ok(json!({ "tools": tool_list }))
            },
            "tools/call" => self.handle_tool_call(req.params).await,
            // If it's a valid JSON-RPC but method is not found, we should still return an error response
            _ => Err(format!("Method not found: {}", req.method)),
        };

        let response = match result {
            Ok(res) => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: req.id,
                result: Some(res),
                error: None,
            },
            Err(err) => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: req.id,
                result: None,
                error: Some(json!({"code": -32601, "message": err})),
            },
        };

        Some(serde_json::to_string(&response).unwrap())
    }

    async fn handle_tool_call(&self, params: Option<Value>) -> Result<Value, String> {
        let params = params.ok_or("Missing parameters")?;
        let name = params.get("name").and_then(|n| n.as_str()).ok_or("Missing tool name")?;
        let args = params.get("arguments").cloned().unwrap_or(json!({}));

        if let Some(tool) = self.tools.get(name) {
            let exec_result = tool.call(args).await?;
            
            // Standard MCP Tool Output Format
            Ok(json!({
                "content": [{
                    "type": "text",
                    "text": exec_result.as_str().unwrap_or(&exec_result.to_string())
                }]
            }))
        } else {
            Err(format!("Tool {} not found", name))
        }
    }
}

// ==========================================
// 6. Setup helper
// ==========================================

pub fn init_mcp_gateway(
    configs: Vec<ExternalToolConfig>,
) -> McpServer {
    let mut server = McpServer::new();
    for config in configs {
        let tool_name = config.name.clone();
        let tool = DynamicTool::new(config);
        server.register_tool(Box::new(tool));
        log::info!("Registered MCP Tool: {}", tool_name);
    }
    server
}
