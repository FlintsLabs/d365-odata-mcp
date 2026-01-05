//! D365 OData MCP Server
//!
//! Entry point for the MCP server binary.
//! Implements MCP protocol over stdio using JSON-RPC 2.0.

use d365_odata_mcp::auth::AzureAdAuth;
use d365_odata_mcp::config::Config;
use d365_odata_mcp::mcp::{
    CallToolParams, CallToolResult, D365McpServer, InitializeResult, JsonRpcRequest,
    JsonRpcResponse, ListToolsResult, ServerCapabilities, ServerInfo, ToolsCapability,
};
use d365_odata_mcp::odata::ODataClient;
use std::io::{self, BufRead, Write};
use std::sync::Arc;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging to stderr (MCP uses stdout for protocol)
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")))
        .with_writer(io::stderr)
        .init();

    tracing::info!("Starting D365 OData MCP Server...");

    // Load configuration
    let config = Config::load_default()?;
    let runtime_config = config.to_runtime()?;

    tracing::info!(
        "Configured for {:?} at {}",
        runtime_config.product,
        runtime_config.endpoint
    );

    // Initialize authentication
    let auth = Arc::new(AzureAdAuth::new(
        runtime_config.tenant_id.clone(),
        runtime_config.client_id.clone(),
        runtime_config.client_secret.clone(),
    ));

    // Initialize OData client
    let client = Arc::new(ODataClient::new(
        auth,
        runtime_config.endpoint.clone(),
        runtime_config.product.clone(),
        runtime_config.max_retries,
        runtime_config.retry_delay_ms,
    ));

    // Create MCP server
    let server = D365McpServer::new(client, Arc::new(runtime_config));

    tracing::info!("MCP Server ready, listening on stdio...");

    // Run stdio message loop
    run_stdio_loop(server).await
}

async fn run_stdio_loop(server: D365McpServer) -> Result<(), Box<dyn std::error::Error>> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        tracing::debug!("Received: {}", line);

        let request: JsonRpcRequest = match serde_json::from_str(&line) {
            Ok(req) => req,
            Err(e) => {
                let error_response = JsonRpcResponse::error(None, -32700, &format!("Parse error: {}", e));
                send_response(&mut stdout, &error_response)?;
                continue;
            }
        };

        let response = handle_request(&server, request).await;
        send_response(&mut stdout, &response)?;
    }

    Ok(())
}

async fn handle_request(server: &D365McpServer, request: JsonRpcRequest) -> JsonRpcResponse {
    let id = request.id.clone();

    match request.method.as_str() {
        "initialize" => {
            let result = InitializeResult {
                protocol_version: "2024-11-05".to_string(),
                capabilities: ServerCapabilities {
                    tools: Some(ToolsCapability {
                        list_changed: Some(false),
                    }),
                },
                server_info: ServerInfo {
                    name: "d365-odata-mcp".to_string(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                },
            };
            JsonRpcResponse::success(id, serde_json::to_value(result).unwrap())
        }

        "initialized" => {
            // Notification, no response needed but we'll acknowledge
            JsonRpcResponse::success(id, serde_json::json!({}))
        }

        "tools/list" => {
            let tools = server.get_tools();
            let result = ListToolsResult { tools };
            JsonRpcResponse::success(id, serde_json::to_value(result).unwrap())
        }

        "tools/call" => {
            let params: CallToolParams = match request.params {
                Some(p) => match serde_json::from_value(p) {
                    Ok(params) => params,
                    Err(e) => {
                        return JsonRpcResponse::error(id, -32602, &format!("Invalid params: {}", e));
                    }
                },
                None => {
                    return JsonRpcResponse::error(id, -32602, "Missing params");
                }
            };

            let args = params.arguments.unwrap_or_default();
            let result: CallToolResult = server.call_tool(&params.name, &args).await;
            JsonRpcResponse::success(id, serde_json::to_value(result).unwrap())
        }

        "ping" => {
            JsonRpcResponse::success(id, serde_json::json!({}))
        }

        _ => {
            JsonRpcResponse::error(id, -32601, &format!("Method not found: {}", request.method))
        }
    }
}

fn send_response(stdout: &mut io::Stdout, response: &JsonRpcResponse) -> io::Result<()> {
    let json = serde_json::to_string(response)?;
    tracing::debug!("Sending: {}", json);
    writeln!(stdout, "{}", json)?;
    stdout.flush()?;
    Ok(())
}
