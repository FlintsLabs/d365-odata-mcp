//! Configuration module for D365 OData MCP
//!
//! Loads configuration from TOML file and environment variables.
//! Environment variables take precedence over file config.

use serde::Deserialize;
use std::env;
use std::fs;
use std::path::Path;

const USE_KEYCHAIN_ENV: &str = "USE_KEYCHAIN";
const CLIENT_SECRET_ENV: &str = "CLIENT_SECRET";
const CLIENT_SECRET_KEYCHAIN_SERVICE_ENV: &str = "CLIENT_SECRET_KEYCHAIN_SERVICE";
const CLIENT_SECRET_KEYCHAIN_ACCOUNT_ENV: &str = "CLIENT_SECRET_KEYCHAIN_ACCOUNT";

/// Product type - Dataverse or Finance & Operations
#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ProductType {
    Dataverse,
    #[serde(alias = "fno", alias = "fo")]
    Finops,
}

impl Default for ProductType {
    fn default() -> Self {
        ProductType::Dataverse
    }
}

/// Global configuration settings
#[derive(Debug, Deserialize, Clone)]
pub struct GlobalConfig {
    #[serde(default)]
    pub product: ProductType,
    pub endpoint: String,
    #[serde(default)]
    pub page_size: Option<usize>,
    #[serde(default)]
    pub concurrency: Option<usize>,
    #[serde(default)]
    pub max_retries: Option<u32>,
    #[serde(default)]
    pub retry_delay_ms: Option<u64>,
}

/// Observability configuration
#[derive(Debug, Deserialize, Clone, Default)]
pub struct ObservabilityConfig {
    #[serde(default)]
    pub log_level: Option<String>,
    #[serde(default)]
    pub enable_tracing: Option<bool>,
}

/// Delta sync storage configuration
#[derive(Debug, Deserialize, Clone, Default)]
pub struct DeltaConfig {
    #[serde(default)]
    pub storage_path: Option<String>,
}

/// Entity-specific configuration
#[derive(Debug, Deserialize, Clone)]
pub struct EntityConfig {
    pub name: String,
    #[serde(default)]
    pub initial_load: Option<bool>,
    #[serde(default)]
    pub delta_enabled: Option<bool>,
    #[serde(default)]
    pub cross_company: Option<bool>,
}

/// Root configuration structure
#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub global: GlobalConfig,
    #[serde(default)]
    pub observability: Option<ObservabilityConfig>,
    #[serde(default)]
    pub delta: Option<DeltaConfig>,
    #[serde(default)]
    pub entities: Option<Vec<EntityConfig>>,
}

/// Runtime configuration with resolved values from env vars
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    pub product: ProductType,
    pub endpoint: String,
    pub tenant_id: String,
    pub client_id: String,
    pub client_secret: String,
    /// Authentication type: "azure" or "adfs"
    pub auth_type: String,
    /// Custom token URL (for ADFS)
    pub token_url: Option<String>,
    /// Resource/audience (for ADFS)
    pub resource: Option<String>,
    /// Skip SSL certificate verification (for self-signed certs)
    pub insecure_ssl: bool,
    pub page_size: usize,
    pub concurrency: usize,
    pub max_retries: u32,
    pub retry_delay_ms: u64,
    pub log_level: String,
    pub enable_tracing: bool,
    pub delta_storage_path: String,
    pub entities: Vec<EntityConfig>,
    /// Metadata cache TTL in seconds (default: 900 = 15 minutes)
    pub metadata_cache_ttl_secs: u64,
}

impl Config {
    /// Load configuration from a TOML file path
    pub fn load_from_path<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let content = fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    /// Load from default path or create default config
    pub fn load_default() -> Result<Self, Box<dyn std::error::Error>> {
        let default_path = "config/default.toml";
        if Path::new(default_path).exists() {
            Self::load_from_path(default_path)
        } else {
            // Minimal default config
            Ok(Config {
                global: GlobalConfig {
                    product: ProductType::default(),
                    endpoint: String::new(),
                    page_size: Some(500),
                    concurrency: Some(4),
                    max_retries: Some(3),
                    retry_delay_ms: Some(1000),
                },
                observability: Some(ObservabilityConfig::default()),
                delta: Some(DeltaConfig::default()),
                entities: None,
            })
        }
    }

    /// Resolve configuration with environment variables
    /// Environment variables take precedence over file config
    pub fn to_runtime(&self) -> Result<RuntimeConfig, Box<dyn std::error::Error>> {
        self.to_runtime_with_keychain_reader(read_client_secret_from_keychain)
    }

    fn to_runtime_with_keychain_reader<F>(
        &self,
        keychain_reader: F,
    ) -> Result<RuntimeConfig, Box<dyn std::error::Error>>
    where
        F: FnOnce(&str, &str) -> Result<String, Box<dyn std::error::Error>>,
    {
        // Required env vars (no defaults)
        let tenant_id =
            env::var("TENANT_ID").map_err(|_| "TENANT_ID environment variable is required")?;
        let client_id =
            env::var("CLIENT_ID").map_err(|_| "CLIENT_ID environment variable is required")?;
        let client_secret = resolve_client_secret(&client_id, keychain_reader)?;

        // Optional env vars with fallback to config file
        let endpoint = env::var("ENDPOINT").unwrap_or_else(|_| self.global.endpoint.clone());
        if endpoint.is_empty() {
            return Err("ENDPOINT environment variable or config endpoint is required".into());
        }

        let product = env::var("PRODUCT")
            .ok()
            .and_then(|p| match p.to_lowercase().as_str() {
                "dataverse" => Some(ProductType::Dataverse),
                "finops" | "fno" | "fo" => Some(ProductType::Finops),
                _ => None,
            })
            .unwrap_or_else(|| self.global.product.clone());

        let obs = self.observability.clone().unwrap_or_default();
        let delta = self.delta.clone().unwrap_or_default();

        // Auth type (azure or adfs)
        let auth_type = env::var("AUTH_TYPE").unwrap_or_else(|_| "azure".to_string());

        // Custom token URL (for ADFS)
        let token_url = env::var("TOKEN_URL").ok();

        // Resource/audience (for ADFS)
        let resource = env::var("RESOURCE").ok();

        // Skip SSL verification (for self-signed certificates)
        let insecure_ssl = env::var("INSECURE_SSL")
            .map(|v| v.to_lowercase() == "true" || v == "1")
            .unwrap_or(false);

        // Metadata cache TTL in seconds (default: 900 = 15 minutes)
        let metadata_cache_ttl_secs = env::var("METADATA_CACHE_TTL")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(900);

        Ok(RuntimeConfig {
            product,
            endpoint,
            tenant_id,
            client_id,
            client_secret,
            auth_type,
            token_url,
            resource,
            insecure_ssl,
            page_size: self.global.page_size.unwrap_or(500),
            concurrency: self.global.concurrency.unwrap_or(4),
            max_retries: self.global.max_retries.unwrap_or(3),
            retry_delay_ms: self.global.retry_delay_ms.unwrap_or(1000),
            log_level: obs.log_level.unwrap_or_else(|| "info".to_string()),
            enable_tracing: obs.enable_tracing.unwrap_or(false),
            delta_storage_path: delta
                .storage_path
                .unwrap_or_else(|| "./delta_state.json".to_string()),
            entities: self.entities.clone().unwrap_or_default(),
            metadata_cache_ttl_secs,
        })
    }
}

fn resolve_client_secret<F>(
    client_id: &str,
    keychain_reader: F,
) -> Result<String, Box<dyn std::error::Error>>
where
    F: FnOnce(&str, &str) -> Result<String, Box<dyn std::error::Error>>,
{
    if !parse_bool_env(USE_KEYCHAIN_ENV, false)? {
        return env::var(CLIENT_SECRET_ENV)
            .map_err(|_| format!("{CLIENT_SECRET_ENV} environment variable is required").into());
    }

    let service = required_non_empty_env(CLIENT_SECRET_KEYCHAIN_SERVICE_ENV)?;
    let account = env::var(CLIENT_SECRET_KEYCHAIN_ACCOUNT_ENV).unwrap_or_else(|_| client_id.into());

    if account.trim().is_empty() {
        return Err(format!(
            "{CLIENT_SECRET_KEYCHAIN_ACCOUNT_ENV} cannot be empty when {USE_KEYCHAIN_ENV}=true"
        )
        .into());
    }

    keychain_reader(&service, &account)
}

fn parse_bool_env(name: &str, default: bool) -> Result<bool, Box<dyn std::error::Error>> {
    match env::var(name) {
        Ok(value) => match value.trim().to_lowercase().as_str() {
            "true" | "1" | "yes" | "on" => Ok(true),
            "false" | "0" | "no" | "off" => Ok(false),
            _ => Err(
                format!("{name} must be a boolean value: true/false, 1/0, yes/no, or on/off")
                    .into(),
            ),
        },
        Err(env::VarError::NotPresent) => Ok(default),
        Err(err) => Err(format!("{name} environment variable is invalid: {err}").into()),
    }
}

fn required_non_empty_env(name: &str) -> Result<String, Box<dyn std::error::Error>> {
    match env::var(name) {
        Ok(value) if !value.trim().is_empty() => Ok(value),
        Ok(_) | Err(env::VarError::NotPresent) => {
            Err(format!("{name} environment variable is required").into())
        }
        Err(err) => Err(format!("{name} environment variable is invalid: {err}").into()),
    }
}

fn read_client_secret_from_keychain(
    service: &str,
    account: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let entry = keyring::Entry::new(service, account).map_err(|err| {
        format!(
            "Failed to create CLIENT_SECRET native secret store entry \
             (service={service}, account={account}): {err}"
        )
    })?;

    entry.get_password().map_err(|err| {
        format!(
            "Failed to read CLIENT_SECRET from native secret store \
             (service={service}, account={account}): {err}"
        )
        .into()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::sync::{Mutex, OnceLock};

    const RUNTIME_ENV_VARS: &[&str] = &[
        "TENANT_ID",
        "CLIENT_ID",
        CLIENT_SECRET_ENV,
        "ENDPOINT",
        "PRODUCT",
        "AUTH_TYPE",
        "TOKEN_URL",
        "RESOURCE",
        "INSECURE_SSL",
        "METADATA_CACHE_TTL",
        USE_KEYCHAIN_ENV,
        CLIENT_SECRET_KEYCHAIN_SERVICE_ENV,
        CLIENT_SECRET_KEYCHAIN_ACCOUNT_ENV,
    ];

    struct EnvGuard {
        saved: Vec<(&'static str, Option<OsString>)>,
    }

    impl EnvGuard {
        fn new(vars: &[(&str, &str)]) -> Self {
            let saved = RUNTIME_ENV_VARS
                .iter()
                .map(|key| (*key, env::var_os(key)))
                .collect::<Vec<_>>();

            for key in RUNTIME_ENV_VARS {
                env::remove_var(key);
            }

            for (key, value) in vars {
                env::set_var(key, value);
            }

            Self { saved }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (key, value) in &self.saved {
                match value {
                    Some(value) => env::set_var(key, value),
                    None => env::remove_var(key),
                }
            }
        }
    }

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn with_env<T>(vars: &[(&str, &str)], test: impl FnOnce() -> T) -> T {
        let _lock = env_lock().lock().unwrap();
        let _guard = EnvGuard::new(vars);
        test()
    }

    fn test_config() -> Config {
        Config {
            global: GlobalConfig {
                product: ProductType::Dataverse,
                endpoint: "https://example.crm.dynamics.com/api/data/v9.2/".to_string(),
                page_size: Some(500),
                concurrency: Some(4),
                max_retries: Some(3),
                retry_delay_ms: Some(1000),
            },
            observability: Some(ObservabilityConfig::default()),
            delta: Some(DeltaConfig::default()),
            entities: None,
        }
    }

    fn base_env() -> Vec<(&'static str, &'static str)> {
        vec![
            ("TENANT_ID", "tenant-id"),
            ("CLIENT_ID", "client-id"),
            (
                "ENDPOINT",
                "https://example.crm.dynamics.com/api/data/v9.2/",
            ),
            ("PRODUCT", "dataverse"),
        ]
    }

    fn unused_keychain_reader(_: &str, _: &str) -> Result<String, Box<dyn std::error::Error>> {
        Err("keychain reader should not be called".into())
    }

    #[test]
    fn test_product_type_default() {
        assert_eq!(ProductType::default(), ProductType::Dataverse);
    }

    #[test]
    fn test_product_type_deserialize() {
        #[derive(Deserialize)]
        struct Test {
            product: ProductType,
        }

        let toml_str = r#"product = "finops""#;
        let test: Test = toml::from_str(toml_str).unwrap();
        assert_eq!(test.product, ProductType::Finops);

        let toml_str = r#"product = "dataverse""#;
        let test: Test = toml::from_str(toml_str).unwrap();
        assert_eq!(test.product, ProductType::Dataverse);
    }

    #[test]
    fn runtime_uses_client_secret_when_use_keychain_is_missing() {
        let mut vars = base_env();
        vars.push((CLIENT_SECRET_ENV, "direct-secret"));

        with_env(&vars, || {
            let runtime = test_config()
                .to_runtime_with_keychain_reader(unused_keychain_reader)
                .unwrap();

            assert_eq!(runtime.client_secret, "direct-secret");
        });
    }

    #[test]
    fn runtime_uses_client_secret_when_use_keychain_is_false() {
        let mut vars = base_env();
        vars.push((CLIENT_SECRET_ENV, "direct-secret"));
        vars.push((USE_KEYCHAIN_ENV, "false"));

        with_env(&vars, || {
            let runtime = test_config()
                .to_runtime_with_keychain_reader(unused_keychain_reader)
                .unwrap();

            assert_eq!(runtime.client_secret, "direct-secret");
        });
    }

    #[test]
    fn runtime_uses_keychain_without_client_secret_when_enabled() {
        let mut vars = base_env();
        vars.push((USE_KEYCHAIN_ENV, "true"));
        vars.push((CLIENT_SECRET_KEYCHAIN_SERVICE_ENV, "test-service"));

        with_env(&vars, || {
            let runtime = test_config()
                .to_runtime_with_keychain_reader(|service, account| {
                    assert_eq!(service, "test-service");
                    assert_eq!(account, "client-id");
                    Ok("keychain-secret".to_string())
                })
                .unwrap();

            assert_eq!(runtime.client_secret, "keychain-secret");
        });
    }

    #[test]
    fn runtime_requires_keychain_service_when_keychain_is_enabled() {
        let mut vars = base_env();
        vars.push((USE_KEYCHAIN_ENV, "true"));

        with_env(&vars, || {
            let err = test_config()
                .to_runtime_with_keychain_reader(unused_keychain_reader)
                .unwrap_err()
                .to_string();

            assert!(err.contains(CLIENT_SECRET_KEYCHAIN_SERVICE_ENV));
        });
    }

    #[test]
    fn runtime_defaults_keychain_account_to_client_id() {
        let mut vars = base_env();
        vars.push((USE_KEYCHAIN_ENV, "true"));
        vars.push((CLIENT_SECRET_KEYCHAIN_SERVICE_ENV, "test-service"));

        with_env(&vars, || {
            test_config()
                .to_runtime_with_keychain_reader(|_, account| {
                    assert_eq!(account, "client-id");
                    Ok("keychain-secret".to_string())
                })
                .unwrap();
        });
    }

    #[test]
    fn runtime_uses_keychain_account_override() {
        let mut vars = base_env();
        vars.push((USE_KEYCHAIN_ENV, "true"));
        vars.push((CLIENT_SECRET_KEYCHAIN_SERVICE_ENV, "test-service"));
        vars.push((CLIENT_SECRET_KEYCHAIN_ACCOUNT_ENV, "custom-account"));

        with_env(&vars, || {
            test_config()
                .to_runtime_with_keychain_reader(|_, account| {
                    assert_eq!(account, "custom-account");
                    Ok("keychain-secret".to_string())
                })
                .unwrap();
        });
    }

    #[test]
    fn runtime_rejects_invalid_use_keychain_value() {
        let mut vars = base_env();
        vars.push((CLIENT_SECRET_ENV, "direct-secret"));
        vars.push((USE_KEYCHAIN_ENV, "maybe"));

        with_env(&vars, || {
            let err = test_config()
                .to_runtime_with_keychain_reader(unused_keychain_reader)
                .unwrap_err()
                .to_string();

            assert!(err.contains(USE_KEYCHAIN_ENV));
            assert!(err.contains("boolean"));
        });
    }

    #[test]
    fn keychain_lookup_error_does_not_include_client_secret() {
        let mut vars = base_env();
        vars.push((CLIENT_SECRET_ENV, "do-not-leak-this-secret"));
        vars.push((USE_KEYCHAIN_ENV, "true"));
        vars.push((CLIENT_SECRET_KEYCHAIN_SERVICE_ENV, "test-service"));

        with_env(&vars, || {
            let err = test_config()
                .to_runtime_with_keychain_reader(|service, account| {
                    Err(format!("lookup failed for service={service}, account={account}").into())
                })
                .unwrap_err()
                .to_string();

            assert!(err.contains("test-service"));
            assert!(err.contains("client-id"));
            assert!(!err.contains("do-not-leak-this-secret"));
        });
    }

    #[test]
    fn native_keychain_reader_failure_reports_lookup_pair() {
        let _lock = env_lock().lock().unwrap();
        keyring::set_default_credential_builder(keyring::mock::default_credential_builder());

        let err = read_client_secret_from_keychain("mock-service", "mock-account")
            .unwrap_err()
            .to_string();

        assert!(err.contains("mock-service"));
        assert!(err.contains("mock-account"));
        assert!(!err.contains("client-secret"));
    }
}
