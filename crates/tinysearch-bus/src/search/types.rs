//! Wire payloads and configuration for `TinySearch`.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeMap, fmt};

/// Module configuration supplied privately at initialization or refresh.
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SearchConfig {
    /// Whether search is available at all.
    pub enabled: bool,
    /// Managed backend settings.
    pub backend: BackendConfig,
    /// Provider settings keyed by stable provider name.
    pub providers: BTreeMap<String, ProviderConfig>,
    /// Tool presentation settings.
    pub presentation: PresentationConfig,
}
impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            backend: BackendConfig::default(),
            providers: BTreeMap::new(),
            presentation: PresentationConfig::default(),
        }
    }
}
impl fmt::Debug for SearchConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SearchConfig")
            .field("enabled", &self.enabled)
            .field("backend", &self.backend)
            .field("providers", &self.providers)
            .field("presentation", &self.presentation)
            .finish()
    }
}

/// Settings for calls routed through the managed backend.
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct BackendConfig {
    /// Backend base URL.
    pub base_url: Option<String>,
    /// Private credential supplied by the host.
    pub credential: Option<String>,
    /// Product attribution header value.
    pub sdk_name: Option<String>,
    /// Authentication scheme for the backend credential.
    pub auth_mode: BackendAuthMode,
}
impl fmt::Debug for BackendConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BackendConfig")
            .field("base_url", &self.base_url)
            .field(
                "credential",
                &self.credential.as_ref().map(|_| "[REDACTED]"),
            )
            .field("sdk_name", &self.sdk_name)
            .field("auth_mode", &self.auth_mode)
            .finish()
    }
}

/// Authentication scheme for a managed backend credential.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendAuthMode {
    /// Session token sent as an Authorization bearer.
    #[default]
    Session,
    /// API key sent in the x-api-key header.
    ApiKey,
}

/// Route for a provider request.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderRoute {
    /// Call the provider directly.
    #[default]
    Direct,
    /// Call through the managed backend.
    Backend,
}

/// Provider-specific private configuration.
#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ProviderConfig {
    /// Whether the provider is enabled.
    pub enabled: bool,
    /// Optional direct provider credential.
    pub credential: Option<String>,
    /// Optional provider base URL override.
    pub base_url: Option<String>,
    /// Chosen request route.
    pub route: ProviderRoute,
    /// Default result count when a tool call does not specify one.
    pub max_results: Option<u64>,
    /// Direct provider request timeout in seconds.
    pub timeout_secs: Option<u64>,
    /// Default `SearXNG` language when the search call omits one.
    pub default_language: Option<String>,
}
impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            credential: None,
            base_url: None,
            route: ProviderRoute::Direct,
            max_results: None,
            timeout_secs: None,
            default_language: None,
        }
    }
}
impl fmt::Debug for ProviderConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProviderConfig")
            .field("enabled", &self.enabled)
            .field(
                "credential",
                &self.credential.as_ref().map(|_| "[REDACTED]"),
            )
            .field("base_url", &self.base_url)
            .field("route", &self.route)
            .field("max_results", &self.max_results)
            .field("timeout_secs", &self.timeout_secs)
            .field("default_language", &self.default_language)
            .finish()
    }
}

/// How provider tools appear to the host.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PresentationMode {
    /// One tool for each available provider.
    #[default]
    AllTools,
    /// One router tool with an optional provider argument.
    Router,
    /// One tool bound to the configured provider.
    OneProvider,
}

/// Presentation configuration.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PresentationConfig {
    /// Selected presentation mode.
    pub mode: PresentationMode,
    /// Provider selected for `one_provider`, or router default.
    pub provider: Option<String>,
}

/// Tool declaration returned by `ListTools`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolSpec {
    /// Stable tool name.
    pub name: String,
    /// Description for the model.
    pub description: String,
    /// Provider-defined JSON Schema for arguments.
    pub parameters: Value,
}

/// Response from `ListTools`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ListToolsResponse {
    /// Currently available tools.
    pub tools: Vec<ToolSpec>,
}

/// Request to execute a declared tool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecuteToolRequest {
    /// Name from `ListTools`.
    pub name: String,
    /// Provider-defined arguments.
    pub arguments: Value,
}

/// Normalized search hit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchResult {
    /// Result title.
    pub title: String,
    /// Canonical URL.
    pub url: String,
    /// Optional excerpt.
    pub snippet: Option<String>,
    /// Optional publication date reported by the provider.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub published: Option<String>,
}

/// Normalized citation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Citation {
    /// Citation URL.
    pub url: String,
    /// Optional citation title.
    pub title: Option<String>,
}

/// Outcome classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchStatus {
    /// Provider returned a successful response.
    Ok,
    /// Provider returned no results.
    Empty,
    /// An asynchronous research operation is still running.
    InProgress,
}

/// Provider-independent tool result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecuteToolResponse {
    /// Provider that handled the request.
    pub provider: String,
    /// Normalized results.
    pub results: Vec<SearchResult>,
    /// Normalized citations.
    pub citations: Vec<Citation>,
    /// Optional synthesized answer.
    pub answer: Option<String>,
    /// Outcome status.
    pub status: SearchStatus,
    /// Optional provider-specific data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_data: Option<Value>,
}
