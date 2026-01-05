//! MCP Server implementation for D365 OData
//!
//! Exposes tools for querying and interacting with Dynamics 365 data

use crate::config::RuntimeConfig;
use crate::mcp::protocol::*;
use crate::odata::{ODataClient, QueryOptions};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

/// MCP Server for D365 OData
pub struct D365McpServer {
    client: Arc<ODataClient>,
    config: Arc<RuntimeConfig>,
}

impl D365McpServer {
    /// Create a new MCP server instance
    pub fn new(client: Arc<ODataClient>, config: Arc<RuntimeConfig>) -> Self {
        Self { client, config }
    }

    /// Get list of available tools
    pub fn get_tools(&self) -> Vec<Tool> {
        vec![
            Tool {
                name: "list_entities".to_string(),
                description: "List all available D365 entities/tables that can be queried".to_string(),
                input_schema: create_tool_schema(vec![]),
            },
            Tool {
                name: "query_entity".to_string(),
                description: "Query data from a D365 entity. Returns records matching the criteria.".to_string(),
                input_schema: create_tool_schema(vec![
                    ("entity", "Entity set name, e.g., 'contacts', 'accounts'", true),
                    ("select", "Comma-separated fields to select, e.g., 'name,email'", false),
                    ("filter", "OData filter expression, e.g., \"status eq 'active'\"", false),
                    ("top", "Maximum records to return (default: 50, max: 500)", false),
                ]),
            },
            Tool {
                name: "get_entity_schema".to_string(),
                description: "Get entity schema by fetching a sample record. Shows available fields.".to_string(),
                input_schema: create_tool_schema(vec![
                    ("entity", "Entity set name, e.g., 'contacts'", true),
                ]),
            },
            Tool {
                name: "get_record".to_string(),
                description: "Get a single record by its ID/primary key".to_string(),
                input_schema: create_tool_schema(vec![
                    ("entity", "Entity set name, e.g., 'contacts'", true),
                    ("id", "Record ID/GUID", true),
                ]),
            },
            Tool {
                name: "get_environment_info".to_string(),
                description: "Get information about the connected D365 environment".to_string(),
                input_schema: create_tool_schema(vec![]),
            },
        ]
    }

    /// Handle a tool call
    pub async fn call_tool(&self, name: &str, args: &HashMap<String, Value>) -> CallToolResult {
        match name {
            "list_entities" => self.list_entities().await,
            "query_entity" => self.query_entity(args).await,
            "get_entity_schema" => self.get_entity_schema(args).await,
            "get_record" => self.get_record(args).await,
            "get_environment_info" => self.get_environment_info().await,
            _ => CallToolResult::error(format!("Unknown tool: {}", name)),
        }
    }

    async fn list_entities(&self) -> CallToolResult {
        match self.client.fetch_metadata().await {
            Ok(metadata) => {
                let entities = extract_entity_sets_from_metadata(&metadata);
                let text = format!("Available entities:\n{}", entities.join("\n"));
                CallToolResult::text(text)
            }
            Err(e) => CallToolResult::error(format!("Error fetching metadata: {}", e)),
        }
    }

    async fn query_entity(&self, args: &HashMap<String, Value>) -> CallToolResult {
        let entity = match args.get("entity").and_then(|v| v.as_str()) {
            Some(e) => e,
            None => return CallToolResult::error("Missing required parameter: entity".to_string()),
        };

        let select = args
            .get("select")
            .and_then(|v| v.as_str())
            .map(|s| s.split(',').map(|f| f.trim().to_string()).collect());

        let filter = args.get("filter").and_then(|v| v.as_str()).map(String::from);

        let top = args
            .get("top")
            .and_then(|v| v.as_str().or_else(|| v.as_i64().map(|_| "")).and_then(|_| v.as_i64().map(|n| n as usize)))
            .or_else(|| args.get("top").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()))
            .unwrap_or(50)
            .min(500);

        let options = QueryOptions {
            select,
            filter,
            top: Some(top),
            ..Default::default()
        };

        match self.client.fetch_entity_page(entity, None, &options).await {
            Ok(response) => {
                let count = response.value.len();
                let has_more = response.next_link.is_some();
                let json = serde_json::to_string_pretty(&response.value)
                    .unwrap_or_else(|_| "[]".to_string());

                let result = format!(
                    "Found {} records{}:\n\n{}",
                    count,
                    if has_more { " (more available)" } else { "" },
                    json
                );
                CallToolResult::text(result)
            }
            Err(e) => CallToolResult::error(format!("Error querying {}: {}", entity, e)),
        }
    }

    async fn get_entity_schema(&self, args: &HashMap<String, Value>) -> CallToolResult {
        let entity = match args.get("entity").and_then(|v| v.as_str()) {
            Some(e) => e,
            None => return CallToolResult::error("Missing required parameter: entity".to_string()),
        };

        let options = QueryOptions {
            top: Some(1),
            ..Default::default()
        };

        match self.client.fetch_entity_page(entity, None, &options).await {
            Ok(response) => {
                if let Some(sample) = response.value.into_iter().next() {
                    if let Value::Object(map) = &sample {
                        let fields: Vec<String> = map.keys().cloned().collect();
                        let result = format!(
                            "Entity: {}\nFields ({}):\n{}\n\nSample record:\n{}",
                            entity,
                            fields.len(),
                            fields.join(", "),
                            serde_json::to_string_pretty(&sample).unwrap_or_default()
                        );
                        CallToolResult::text(result)
                    } else {
                        CallToolResult::text(serde_json::to_string_pretty(&sample).unwrap_or_default())
                    }
                } else {
                    CallToolResult::text(format!("No records found in entity '{}'", entity))
                }
            }
            Err(e) => CallToolResult::error(format!("Error fetching schema for {}: {}", entity, e)),
        }
    }

    async fn get_record(&self, args: &HashMap<String, Value>) -> CallToolResult {
        let entity = match args.get("entity").and_then(|v| v.as_str()) {
            Some(e) => e,
            None => return CallToolResult::error("Missing required parameter: entity".to_string()),
        };

        let id = match args.get("id").and_then(|v| v.as_str()) {
            Some(i) => i,
            None => return CallToolResult::error("Missing required parameter: id".to_string()),
        };

        // Format key - GUIDs should be wrapped in quotes for OData
        let key = if id.contains('-') && !id.starts_with('\'') {
            format!("'{}'", id)
        } else {
            id.to_string()
        };

        match self.client.get_entity(entity, &key).await {
            Ok(record) => {
                let json = serde_json::to_string_pretty(&record).unwrap_or_default();
                CallToolResult::text(json)
            }
            Err(e) => CallToolResult::error(format!("Error: {}", e)),
        }
    }

    async fn get_environment_info(&self) -> CallToolResult {
        let info = format!(
            "D365 Environment Info:\n\
             - Endpoint: {}\n\
             - Product: {:?}\n\
             - Page Size: {}\n\
             - Configured Entities: {}",
            self.client.endpoint(),
            self.client.product(),
            self.config.page_size,
            self.config
                .entities
                .iter()
                .map(|e| e.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
        CallToolResult::text(info)
    }
}

/// Extract entity set names from EDMX metadata XML
fn extract_entity_sets_from_metadata(metadata: &str) -> Vec<String> {
    let mut entities = Vec::new();

    for line in metadata.lines() {
        if line.contains("EntitySet") && line.contains("Name=") {
            if let Some(start) = line.find("Name=\"") {
                let rest = &line[start + 6..];
                if let Some(end) = rest.find('"') {
                    entities.push(rest[..end].to_string());
                }
            }
        }
    }

    if entities.is_empty() {
        entities = vec![
            "accounts".to_string(),
            "contacts".to_string(),
            "leads".to_string(),
            "opportunities".to_string(),
            "(try specific entity name)".to_string(),
        ];
    }

    entities
}
