//! OData Client module
//!
//! HTTP client for Microsoft Dynamics 365 OData APIs
//! Supports both Dataverse and Finance & Operations endpoints

use crate::auth::AzureAdAuth;
use crate::config::config::ProductType;
use reqwest::{Client, Response, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::time::sleep;

/// OData client errors
#[derive(Error, Debug)]
pub enum ODataError {
    #[error("Authentication error: {0}")]
    AuthError(#[from] crate::auth::AuthError),

    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("Rate limited (429): retry after {0} seconds")]
    RateLimited(u64),

    #[error("Server error ({0}): {1}")]
    ServerError(u16, String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Not found: {0}")]
    NotFound(String),
}

/// Query options for OData requests
#[derive(Debug, Clone, Default)]
pub struct QueryOptions {
    pub select: Option<Vec<String>>,
    pub filter: Option<String>,
    pub top: Option<usize>,
    pub skip: Option<usize>,
    pub orderby: Option<String>,
    pub expand: Option<Vec<String>>,
    pub cross_company: bool, // F&O only
    pub count: bool,         // Include @odata.count in response
}

impl QueryOptions {
    /// Build query string from options
    pub fn to_query_string(&self, product: &ProductType) -> String {
        let mut params = Vec::new();

        if let Some(ref select) = self.select {
            params.push(format!("$select={}", select.join(",")));
        }

        if let Some(ref filter) = self.filter {
            params.push(format!("$filter={}", filter));
        }

        if let Some(top) = self.top {
            params.push(format!("$top={}", top));
        }

        if let Some(skip) = self.skip {
            params.push(format!("$skip={}", skip));
        }

        if let Some(ref orderby) = self.orderby {
            params.push(format!("$orderby={}", orderby));
        }

        if let Some(ref expand) = self.expand {
            params.push(format!("$expand={}", expand.join(",")));
        }

        // Include count in response
        if self.count {
            params.push("$count=true".to_string());
        }

        // F&O specific: cross-company query
        if self.cross_company && *product == ProductType::Finops {
            params.push("cross-company=true".to_string());
        }

        if params.is_empty() {
            String::new()
        } else {
            format!("?{}", params.join("&"))
        }
    }
}

/// OData response with paging support
#[derive(Debug, Deserialize)]
pub struct ODataResponse {
    #[serde(rename = "@odata.context")]
    pub context: Option<String>,

    #[serde(rename = "@odata.nextLink")]
    pub next_link: Option<String>,

    #[serde(rename = "@odata.count")]
    pub count: Option<i64>,

    #[serde(rename = "@odata.deltaLink")]
    pub delta_link: Option<String>,

    #[serde(default)]
    pub value: Vec<Value>,
}

/// Entity metadata information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityInfo {
    pub name: String,
    pub entity_set_name: String,
    #[serde(default)]
    pub description: Option<String>,
}

/// OData client for D365 APIs
#[derive(Debug)]
pub struct ODataClient {
    auth: Arc<AzureAdAuth>,
    endpoint: String,
    product: ProductType,
    http_client: Client,
    max_retries: u32,
    retry_delay_ms: u64,
}

impl ODataClient {
    /// Create a new OData client
    ///
    /// # Arguments
    /// * `auth` - Azure AD auth helper
    /// * `endpoint` - Service root URL (e.g., "https://org.crm.dynamics.com/api/data/v9.2/")
    /// * `product` - Product type (Dataverse or F&O)
    /// * `max_retries` - Maximum retry attempts for failed requests
    /// * `retry_delay_ms` - Initial delay between retries in milliseconds
    /// * `insecure_ssl` - Skip SSL certificate verification
    pub fn new(
        auth: Arc<AzureAdAuth>,
        endpoint: String,
        product: ProductType,
        max_retries: u32,
        retry_delay_ms: u64,
        insecure_ssl: bool,
    ) -> Self {
        // Ensure endpoint ends with /
        let endpoint = if endpoint.ends_with('/') {
            endpoint
        } else {
            format!("{}/", endpoint)
        };

        let http_client = if insecure_ssl {
            Client::builder()
                .timeout(Duration::from_secs(120))  // Longer timeout for large $metadata
                .danger_accept_invalid_certs(true)
                .build()
                .unwrap()
        } else {
            Client::builder()
                .timeout(Duration::from_secs(120))  // Longer timeout for large $metadata
                .build()
                .unwrap()
        };

        Self {
            auth,
            endpoint,
            product,
            http_client,
            max_retries,
            retry_delay_ms,
        }
    }

    /// Get the resource URL for token acquisition
    fn resource(&self) -> String {
        AzureAdAuth::resource_from_endpoint(&self.endpoint)
    }

    /// Execute HTTP request with retry logic
    async fn execute_with_retry(
        &self,
        url: &str,
        token: &str,
    ) -> Result<Response, ODataError> {
        let mut attempt = 0;
        let mut delay = self.retry_delay_ms;

        loop {
            attempt += 1;

            let response = self
                .http_client
                .get(url)
                .header("Authorization", format!("Bearer {}", token))
                .header("Accept", "application/json")
                .header("OData-MaxVersion", "4.0")
                .header("OData-Version", "4.0")
                .header("Prefer", "odata.include-annotations=*")
                .send()
                .await?;

            match response.status() {
                StatusCode::OK | StatusCode::CREATED | StatusCode::NO_CONTENT => {
                    return Ok(response);
                }
                StatusCode::TOO_MANY_REQUESTS => {
                    // Get Retry-After header if available
                    let retry_after = response
                        .headers()
                        .get("Retry-After")
                        .and_then(|v| v.to_str().ok())
                        .and_then(|v| v.parse::<u64>().ok())
                        .unwrap_or(delay / 1000);

                    if attempt >= self.max_retries {
                        return Err(ODataError::RateLimited(retry_after));
                    }

                    tracing::warn!(
                        "Rate limited (429), attempt {}/{}, retrying after {} seconds",
                        attempt,
                        self.max_retries,
                        retry_after
                    );

                    sleep(Duration::from_secs(retry_after)).await;
                    delay *= 2; // Exponential backoff
                }
                StatusCode::NOT_FOUND => {
                    let body = response.text().await.unwrap_or_default();
                    return Err(ODataError::NotFound(body));
                }
                status if status.is_server_error() => {
                    if attempt >= self.max_retries {
                        let body = response.text().await.unwrap_or_default();
                        return Err(ODataError::ServerError(status.as_u16(), body));
                    }

                    tracing::warn!(
                        "Server error ({}), attempt {}/{}, retrying...",
                        status,
                        attempt,
                        self.max_retries
                    );

                    sleep(Duration::from_millis(delay)).await;
                    delay *= 2;
                }
                status => {
                    let body = response.text().await.unwrap_or_default();
                    return Err(ODataError::ServerError(status.as_u16(), body));
                }
            }
        }
    }

    /// Fetch $metadata XML
    pub async fn fetch_metadata(&self) -> Result<String, ODataError> {
        let url = format!("{}$metadata", self.endpoint);
        let token = self.auth.get_token(&self.resource()).await?;

        let response = self
            .http_client
            .get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Accept", "application/xml")
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(ODataError::ServerError(status.as_u16(), body));
        }

        // Get response as bytes to handle large XML and encoding issues
        let bytes = response.bytes().await.map_err(|e| {
            ODataError::ParseError(format!("Failed to read metadata bytes: {}", e))
        })?;

        // Convert bytes to string, handling potential encoding issues
        let xml = String::from_utf8_lossy(&bytes).to_string();

        Ok(xml)
    }

    /// Fetch entity data with paging support
    ///
    /// # Arguments
    /// * `entity` - Entity set name (e.g., "contacts", "accounts")
    /// * `next_link` - Optional next page URL from previous response
    /// * `options` - Query options
    pub async fn fetch_entity_page(
        &self,
        entity: &str,
        next_link: Option<&str>,
        options: &QueryOptions,
    ) -> Result<ODataResponse, ODataError> {
        let url = match next_link {
            Some(link) => link.to_string(),
            None => {
                let query = options.to_query_string(&self.product);
                format!("{}{}{}", self.endpoint, entity, query)
            }
        };

        tracing::debug!("Fetching: {}", url);

        let token = self.auth.get_token(&self.resource()).await?;
        let response = self.execute_with_retry(&url, &token).await?;

        let odata_response: ODataResponse = response.json().await.map_err(|e| {
            ODataError::ParseError(format!("Failed to parse OData response: {}", e))
        })?;

        tracing::debug!(
            "Fetched {} records, next_link: {:?}",
            odata_response.value.len(),
            odata_response.next_link.is_some()
        );

        Ok(odata_response)
    }

    /// Fetch all pages for an entity
    pub async fn fetch_all_pages(
        &self,
        entity: &str,
        options: &QueryOptions,
    ) -> Result<Vec<Value>, ODataError> {
        let mut all_records = Vec::new();
        let mut next_link: Option<String> = None;
        let mut page = 0;

        loop {
            page += 1;
            let response = self
                .fetch_entity_page(entity, next_link.as_deref(), options)
                .await?;

            tracing::info!("Page {}: fetched {} records", page, response.value.len());

            all_records.extend(response.value);

            match response.next_link {
                Some(link) => next_link = Some(link),
                None => break,
            }
        }

        tracing::info!("Total records fetched: {}", all_records.len());
        Ok(all_records)
    }

    /// Get single entity by key
    pub async fn get_entity(
        &self,
        entity: &str,
        key: &str,
    ) -> Result<Value, ODataError> {
        let url = format!("{}{}({})", self.endpoint, entity, key);
        let token = self.auth.get_token(&self.resource()).await?;
        let response = self.execute_with_retry(&url, &token).await?;

        let value: Value = response.json().await.map_err(|e| {
            ODataError::ParseError(format!("Failed to parse entity: {}", e))
        })?;

        Ok(value)
    }

    /// Get endpoint URL
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    /// Get product type
    pub fn product(&self) -> &ProductType {
        &self.product
    }

    /// Parse $metadata XML to extract entity information for a specific entity
    /// Returns: (properties, navigation_properties, key_fields)
    pub fn parse_entity_from_metadata(
        metadata_xml: &str,
        entity_name: &str,
    ) -> Result<(Vec<String>, Vec<String>, Vec<String>), ODataError> {
        use std::collections::HashSet;

        let mut properties = Vec::new();
        let mut nav_properties = Vec::new();
        let mut key_fields = Vec::new();
        let mut in_entity = false;
        let mut in_key = false;
        let mut entity_type_name = String::new();

        // Simple XML parsing for entity properties
        for line in metadata_xml.lines() {
            let trimmed = line.trim();

            // Look for EntityType definition
            if trimmed.contains("<EntityType ") && trimmed.contains(&format!("Name=\"{}\"", entity_name)) {
                in_entity = true;
                entity_type_name = entity_name.to_string();
            }
            // Also check for EntityType that matches without exact name (for partial matches)
            if !in_entity && trimmed.contains("<EntityType ") {
                if let Some(start) = trimmed.find("Name=\"") {
                    let name_start = start + 6;
                    if let Some(end) = trimmed[name_start..].find('"') {
                        let name = &trimmed[name_start..name_start + end];
                        // Match entity name at start (e.g., "CustomersV3" matches "CustomersV3Type")
                        if name.starts_with(entity_name) || entity_name.starts_with(name) {
                            in_entity = true;
                            entity_type_name = name.to_string();
                        }
                    }
                }
            }

            if in_entity {
                // Parse Key fields
                if trimmed.contains("<Key>") {
                    in_key = true;
                }
                if trimmed.contains("</Key>") {
                    in_key = false;
                }
                if in_key && trimmed.contains("<PropertyRef ") {
                    if let Some(start) = trimmed.find("Name=\"") {
                        let name_start = start + 6;
                        if let Some(end) = trimmed[name_start..].find('"') {
                            let name = &trimmed[name_start..name_start + end];
                            key_fields.push(name.to_string());
                        }
                    }
                }

                // Parse Property fields
                if trimmed.starts_with("<Property ") && trimmed.contains("Name=\"") {
                    if let Some(start) = trimmed.find("Name=\"") {
                        let name_start = start + 6;
                        if let Some(end) = trimmed[name_start..].find('"') {
                            let name = &trimmed[name_start..name_start + end];
                            // Get type if available
                            let prop_type = if let Some(type_start) = trimmed.find("Type=\"") {
                                let ts = type_start + 6;
                                if let Some(te) = trimmed[ts..].find('"') {
                                    Some(trimmed[ts..ts + te].to_string())
                                } else {
                                    None
                                }
                            } else {
                                None
                            };

                            let prop_str = match prop_type {
                                Some(t) => format!("{}: {}", name, t.replace("Edm.", "")),
                                None => name.to_string(),
                            };
                            properties.push(prop_str);
                        }
                    }
                }

                // Parse NavigationProperty fields (expandable)
                if trimmed.starts_with("<NavigationProperty ") && trimmed.contains("Name=\"") {
                    if let Some(start) = trimmed.find("Name=\"") {
                        let name_start = start + 6;
                        if let Some(end) = trimmed[name_start..].find('"') {
                            let name = &trimmed[name_start..name_start + end];
                            // Get type/target if available
                            let nav_type = if let Some(type_start) = trimmed.find("Type=\"") {
                                let ts = type_start + 6;
                                if let Some(te) = trimmed[ts..].find('"') {
                                    Some(trimmed[ts..ts + te].to_string())
                                } else {
                                    None
                                }
                            } else {
                                None
                            };

                            let nav_str = match nav_type {
                                Some(t) => {
                                    // Clean up type string
                                    let clean_type = t
                                        .replace("Collection(", "")
                                        .replace(")", "")
                                        .split('.')
                                        .last()
                                        .unwrap_or(&t)
                                        .to_string();
                                    if t.contains("Collection") {
                                        format!("{} -> [{}]", name, clean_type)
                                    } else {
                                        format!("{} -> {}", name, clean_type)
                                    }
                                }
                                None => name.to_string(),
                            };
                            nav_properties.push(nav_str);
                        }
                    }
                }

                // End of EntityType
                if trimmed == "</EntityType>" {
                    if !properties.is_empty() || !nav_properties.is_empty() {
                        break; // Found the entity, stop parsing
                    }
                    in_entity = false;
                }
            }
        }

        if properties.is_empty() && nav_properties.is_empty() {
            return Err(ODataError::NotFound(format!(
                "Entity '{}' not found in metadata",
                entity_name
            )));
        }

        Ok((properties, nav_properties, key_fields))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_options_empty() {
        let options = QueryOptions::default();
        assert_eq!(options.to_query_string(&ProductType::Dataverse), "");
    }

    #[test]
    fn test_query_options_full() {
        let options = QueryOptions {
            select: Some(vec!["name".to_string(), "email".to_string()]),
            filter: Some("status eq 'active'".to_string()),
            top: Some(10),
            skip: None,
            orderby: Some("name asc".to_string()),
            expand: None,
            cross_company: false,
        };

        let query = options.to_query_string(&ProductType::Dataverse);
        assert!(query.contains("$select=name,email"));
        assert!(query.contains("$filter=status eq 'active'"));
        assert!(query.contains("$top=10"));
        assert!(query.contains("$orderby=name asc"));
    }

    #[test]
    fn test_cross_company_finops_only() {
        let options = QueryOptions {
            cross_company: true,
            ..Default::default()
        };

        // Should include cross-company for F&O
        let query = options.to_query_string(&ProductType::Finops);
        assert!(query.contains("cross-company=true"));

        // Should NOT include cross-company for Dataverse
        let query = options.to_query_string(&ProductType::Dataverse);
        assert!(!query.contains("cross-company"));
    }
}
