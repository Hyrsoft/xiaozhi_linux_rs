use async_trait::async_trait;
use serde_json::{Value, json};
use std::time::Duration;
use tokio::io::AsyncWriteExt;

use super::config::{ExecutionMode, ExternalToolConfig, NotifyMethod, ToolTransport};
use super::process::{ProcessRequest, ProcessSupervisor};

#[async_trait]
pub trait McpTool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> Value;
    async fn call(&self, params: Value) -> Result<Value, String>;
}

pub struct DynamicTool {
    config: ExternalToolConfig,
    supervisor: ProcessSupervisor,
}

impl DynamicTool {
    pub fn new(config: ExternalToolConfig, supervisor: ProcessSupervisor) -> Self {
        Self { config, supervisor }
    }

    /// 根据传输协议类型分发执行（纯异步非阻塞）
    async fn execute_inner(
        supervisor: &ProcessSupervisor,
        config: &ExternalToolConfig,
        params: Value,
    ) -> Result<Value, String> {
        match &config.transport {
            ToolTransport::Subprocess { executable, args } => {
                Self::exec_subprocess(
                    supervisor,
                    executable,
                    args,
                    params,
                    Duration::from_millis(config.timeout_ms),
                )
                .await
            }
            ToolTransport::Http { url, method } => tokio::time::timeout(
                Duration::from_millis(config.timeout_ms),
                Self::exec_http(url, method, params),
            )
            .await
            .map_err(|_| {
                format!(
                    "Tool '{}' execution timed out after {} ms",
                    config.name, config.timeout_ms
                )
            })?,
            ToolTransport::Tcp { address } => tokio::time::timeout(
                Duration::from_millis(config.timeout_ms),
                Self::exec_tcp(address, params),
            )
            .await
            .map_err(|_| {
                format!(
                    "Tool '{}' execution timed out after {} ms",
                    config.name, config.timeout_ms
                )
            })?,
        }
    }

    /// 子进程执行（tokio::process，异步非阻塞）
    async fn exec_subprocess(
        supervisor: &ProcessSupervisor,
        executable: &str,
        args: &[String],
        params: Value,
        timeout: Duration,
    ) -> Result<Value, String> {
        let args_json = serde_json::to_string(&params).unwrap_or_default();
        log::info!(
            "Executing subprocess tool: {}, args: {}",
            executable,
            args_json
        );

        let output = supervisor
            .run(ProcessRequest {
                executable: executable.to_owned(),
                args: args.to_vec(),
                stdin: args_json.into_bytes(),
                timeout,
            })
            .await?;

        if output.status.success() {
            let result_str = String::from_utf8_lossy(&output.stdout).to_string();
            Ok(json!(result_str))
        } else {
            let err_str = String::from_utf8_lossy(&output.stderr);
            Err(format!(
                "Subprocess exited with {}: {}",
                output.status, err_str
            ))
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
        // ---- 后台模式（对话级异步） ----
        if self.config.mode == ExecutionMode::Background {
            let config_clone = self.config.clone();
            let supervisor = self.supervisor.clone();

            self.supervisor.spawn(async move {
                log::info!(">>> 后台任务已启动: {}", config_clone.name);
                let result = Self::execute_inner(&supervisor, &config_clone, params).await;
                let _result = match result {
                    Ok(value) => {
                        let msg = value.as_str().unwrap_or(&value.to_string()).to_string();
                        let mcp_output = json!({
                            "content": [{
                                "type": "text",
                                "text": msg
                            }]
                        });
                        log::info!(
                            "✓ 后台任务 [{}] 执行完成 | MCP输出: {}",
                            config_clone.name,
                            mcp_output
                        );
                        log::info!(
                            "✓ 后台任务 [{}] 执行完成 | 脚本输出: {}",
                            config_clone.name,
                            msg
                        );
                        Ok(msg)
                    }
                    Err(err) => {
                        log::error!(
                            "✗ 后台任务 [{}] 执行失败 | 错误信息: {}",
                            config_clone.name,
                            err
                        );
                        Err(err)
                    }
                };

                match &config_clone.notify {
                    NotifyMethod::Disabled => {
                        log::info!(
                            "📝 后台任务 [{}] 完成结果已通过日志和标准错误输出记录",
                            config_clone.name
                        );
                    }
                    #[allow(unreachable_patterns)]
                    other => {
                        log::warn!(
                            "⚠️ 后台任务 [{}] 配置了未实现的通知方式: {:?}",
                            config_clone.name,
                            other
                        );
                    }
                }
            });

            return Ok(json!({
                "status": "started",
                "message": format!("任务 '{}' 已在后台启动，完成后会通知您。", self.config.name)
            }));
        }

        // ---- 标准同步模式（对话级同步） ----
        Self::execute_inner(&self.supervisor, &self.config, params).await
    }
}
