pub mod config;
pub mod process;
pub mod protocol;
pub mod server;
pub mod tool;

pub use config::ExternalToolConfig;
pub use server::McpServer;

use process::ProcessSupervisor;
use tool::DynamicTool;

pub fn init_mcp_gateway(configs: Vec<ExternalToolConfig>) -> McpServer {
    let supervisor = ProcessSupervisor::default();
    let mut server = McpServer::new(supervisor.clone());
    for config in configs {
        let tool_name = config.name.clone();
        let tool = DynamicTool::new(config, supervisor.clone());
        server.register_tool(Box::new(tool));
        log::info!("Registered MCP Tool: {}", tool_name);
    }
    server
}
