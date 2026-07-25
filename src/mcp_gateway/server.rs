use serde_json::{Value, json};
use std::collections::HashMap;

use super::process::ProcessSupervisor;
use super::protocol::{JsonRpcRequest, JsonRpcResponse};
use super::tool::McpTool;

pub struct McpServer {
    tools: HashMap<String, Box<dyn McpTool>>,
    supervisor: ProcessSupervisor,
}

impl McpServer {
    pub fn new(supervisor: ProcessSupervisor) -> Self {
        Self {
            tools: HashMap::new(),
            supervisor,
        }
    }

    pub async fn shutdown(&self) {
        self.supervisor.shutdown().await;
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
        if req.id.is_none() || req.method.starts_with("notifications") {
            log::info!(
                "MCP notification received (no response needed): {}",
                req.method
            );
            return Some(String::new()); // 返回空字符串表示已处理但不发送响应
        }

        let result = match req.method.as_str() {
            "initialize" => Ok(json!({
                "protocolVersion": "2024-11-05",
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "xiaozhi_linux_rs", "version": "1.0.0" }
            })),
            "tools/list" => {
                let tool_list: Vec<Value> = self
                    .tools
                    .values()
                    .map(|t| {
                        json!({
                            "name": t.name(),
                            "description": t.description(),
                            "inputSchema": t.input_schema()
                        })
                    })
                    .collect();
                Ok(json!({ "tools": tool_list }))
            }
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
        let name = params
            .get("name")
            .and_then(|n| n.as_str())
            .ok_or("Missing tool name")?;
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
