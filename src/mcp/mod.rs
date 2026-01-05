//! MCP Server implementation for D365 OData
//!
//! Exposes tools for querying and interacting with Dynamics 365 data

pub mod protocol;
mod server;

pub use protocol::*;
pub use server::D365McpServer;
