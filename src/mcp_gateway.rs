use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::process::Stdio;
use tokio::process::Command;

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
// 2. Tool Trait definition
// ==========================================

#[async_trait]
pub trait McpTool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> Value;
    async fn call(&self, params: Value) -> Result<Value, String>;
}

// ==========================================
// 3. Subprocess External Tool
// ==========================================

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ExternalToolConfig {
    pub name: String,
    pub description: String,
    pub executable: String,
    pub input_schema: Value,
}

pub struct ExternalTool {
    config: ExternalToolConfig,
}

#[async_trait]
impl McpTool for ExternalTool {
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
        let args_json = serde_json::to_string(&params).unwrap_or_default();
        log::info!("Executing external tool: {}, args: {}", self.config.executable, args_json);

        let mut child = Command::new(&self.config.executable)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to spawn external program {}: {}", self.config.executable, e))?;

        if let Some(mut stdin) = child.stdin.take() {
            use tokio::io::AsyncWriteExt;
            stdin.write_all(args_json.as_bytes()).await.unwrap_or_default();
        }

        let output = child.wait_with_output().await
            .map_err(|e| format!("Failed to wait for external program: {}", e))?;

        if output.status.success() {
            let result_str = String::from_utf8_lossy(&output.stdout).to_string();
            // Return raw string output. We can map it to proper JSON array format for MCP later.
            Ok(json!(result_str))
        } else {
            let err_str = String::from_utf8_lossy(&output.stderr).to_string();
            Err(format!("External program error: {}", err_str))
        }
    }
}

// ==========================================
// 4. MCP Server router
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
// 5. Setup helper
// ==========================================

pub fn init_mcp_gateway(configs: Vec<ExternalToolConfig>) -> McpServer {
    let mut server = McpServer::new();
    for config in configs {
        let external_tool = ExternalTool { config };
        server.register_tool(Box::new(external_tool));
    }
    server
}
