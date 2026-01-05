//! Azure AD Authentication module
//!
//! Implements OAuth2 Client Credentials flow for app-only authentication
//! with Microsoft Dynamics 365 (Dataverse and Finance & Operations).

use reqwest::{Client, Url};
use serde::Deserialize;
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::sync::RwLock;

/// Authentication errors
#[derive(Error, Debug)]
pub enum AuthError {
    #[error("Token request failed: {0}")]
    TokenRequestFailed(String),

    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),

    #[error("Token parse error: {0}")]
    ParseError(String),

    #[error("Missing credentials: {0}")]
    MissingCredentials(String),
}

/// Token response from Azure AD
#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    #[allow(dead_code)]
    token_type: String,
    expires_in: u64,
    #[allow(dead_code)]
    #[serde(default)]
    ext_expires_in: u64,
}

/// Cached token with expiry tracking
#[derive(Debug, Clone)]
struct CachedToken {
    access_token: String,
    expires_at: Instant,
}

impl CachedToken {
    fn is_valid(&self) -> bool {
        // Consider token expired 60 seconds before actual expiry
        self.expires_at > Instant::now() + Duration::from_secs(60)
    }
}

/// Azure AD authentication helper for client credentials flow
#[derive(Debug)]
pub struct AzureAdAuth {
    tenant_id: String,
    client_id: String,
    client_secret: String,
    http_client: Client,
    token_cache: Arc<RwLock<Option<CachedToken>>>,
}

impl AzureAdAuth {
    /// Create a new Azure AD auth helper
    pub fn new(tenant_id: String, client_id: String, client_secret: String) -> Self {
        Self {
            tenant_id,
            client_id,
            client_secret,
            http_client: Client::new(),
            token_cache: Arc::new(RwLock::new(None)),
        }
    }

    /// Get the token endpoint URL for this tenant
    fn token_endpoint(&self) -> String {
        format!(
            "https://login.microsoftonline.com/{}/oauth2/v2.0/token",
            self.tenant_id
        )
    }

    /// Acquire or return a cached access token for the given resource.
    pub async fn get_token(&self, resource: &str) -> Result<String, AuthError> {
        // Check cache first
        {
            let cache = self.token_cache.read().await;
            if let Some(ref cached) = *cache {
                if cached.is_valid() {
                    tracing::debug!("Using cached token");
                    return Ok(cached.access_token.clone());
                }
            }
        }

        // Token expired or not cached, acquire new one
        tracing::info!("Acquiring new access token for resource: {}", resource);
        let token = self.acquire_token(resource).await?;

        Ok(token)
    }

    /// Acquire a new token from Azure AD
    async fn acquire_token(&self, resource: &str) -> Result<String, AuthError> {
        // Build scope from resource
        let scope = if resource.ends_with('/') {
            format!("{}.default", resource)
        } else {
            format!("{}/.default", resource)
        };

        let params = [
            ("grant_type", "client_credentials"),
            ("client_id", &self.client_id),
            ("client_secret", &self.client_secret),
            ("scope", &scope),
        ];

        let response = self
            .http_client
            .post(&self.token_endpoint())
            .form(&params)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            tracing::error!("Token request failed: {} - {}", status, body);
            return Err(AuthError::TokenRequestFailed(format!(
                "Status: {}, Body: {}",
                status, body
            )));
        }

        let token_response: TokenResponse = response.json().await.map_err(|e| {
            AuthError::ParseError(format!("Failed to parse token response: {}", e))
        })?;

        // Cache the token
        let cached = CachedToken {
            access_token: token_response.access_token.clone(),
            expires_at: Instant::now() + Duration::from_secs(token_response.expires_in),
        };

        {
            let mut cache = self.token_cache.write().await;
            *cache = Some(cached);
        }

        tracing::info!(
            "Token acquired successfully, expires in {} seconds",
            token_response.expires_in
        );

        Ok(token_response.access_token)
    }

    /// Clear the token cache
    pub async fn clear_cache(&self) {
        let mut cache = self.token_cache.write().await;
        *cache = None;
    }

    /// Get resource URL from endpoint
    pub fn resource_from_endpoint(endpoint: &str) -> String {
        if let Ok(url) = Url::parse(endpoint) {
            format!("{}://{}", url.scheme(), url.host_str().unwrap_or(""))
        } else {
            endpoint
                .split('/')
                .take(3)
                .collect::<Vec<_>>()
                .join("/")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_auth() {
        let auth = AzureAdAuth::new(
            "tenant-id".to_string(),
            "client-id".to_string(),
            "secret".to_string(),
        );
        assert_eq!(auth.tenant_id, "tenant-id");
        assert_eq!(auth.client_id, "client-id");
    }

    #[test]
    fn test_token_endpoint() {
        let auth = AzureAdAuth::new(
            "my-tenant".to_string(),
            "client-id".to_string(),
            "secret".to_string(),
        );
        assert_eq!(
            auth.token_endpoint(),
            "https://login.microsoftonline.com/my-tenant/oauth2/v2.0/token"
        );
    }

    #[test]
    fn test_resource_from_endpoint() {
        assert_eq!(
            AzureAdAuth::resource_from_endpoint("https://org.crm.dynamics.com/api/data/v9.2/"),
            "https://org.crm.dynamics.com"
        );
        assert_eq!(
            AzureAdAuth::resource_from_endpoint("https://org.operations.dynamics.com/data/"),
            "https://org.operations.dynamics.com"
        );
    }

    #[test]
    fn test_cached_token_validity() {
        let valid_token = CachedToken {
            access_token: "test".to_string(),
            expires_at: Instant::now() + Duration::from_secs(3600),
        };
        assert!(valid_token.is_valid());

        let expired_token = CachedToken {
            access_token: "test".to_string(),
            expires_at: Instant::now() - Duration::from_secs(60),
        };
        assert!(!expired_token.is_valid());
    }
}
