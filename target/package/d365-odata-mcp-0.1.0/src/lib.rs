//! D365 OData MCP Library
//!
//! Model Context Protocol server for Microsoft Dynamics 365 OData APIs.
//! Supports both Dataverse and Finance & Operations.

pub mod auth;
pub mod config;
pub mod mcp;
pub mod odata;

pub use auth::AzureAdAuth;
pub use config::{Config, ProductType, RuntimeConfig};
pub use odata::{ODataClient, ODataError, QueryOptions};
