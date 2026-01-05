//! OData module
//!
//! HTTP client and schema utilities for D365 OData APIs

pub mod client;

pub use client::{EntityInfo, ODataClient, ODataError, ODataResponse, QueryOptions};
