//! Configuration-based discovery and routing for search providers.
use crate::{Error, Result};
use std::{collections::BTreeMap, future::Future, pin::Pin, sync::Arc};
use tinysearch_bus::{
    BackendConfig, ExecuteToolRequest, ExecuteToolResponse, ListToolsResponse, PresentationMode,
    ProviderConfig, ProviderRoute, SearchConfig, ToolSpec, configured_provider_tools,
    provider_tool_specs, select_tools,
};

/// Boxed provider future. Providers may perform asynchronous I/O.
pub type ProviderFuture<'a> =
    Pin<Box<dyn Future<Output = Result<ExecuteToolResponse>> + Send + 'a>>;

/// An implementation of one provider's tool family.
pub trait SearchProvider: Send + Sync {
    /// Executes a catalog-declared tool using private configuration.
    fn execute<'a>(
        &'a self,
        config: &'a ProviderConfig,
        backend: &'a BackendConfig,
        request: &'a ExecuteToolRequest,
    ) -> ProviderFuture<'a>;
}

/// Provider-independent `TinySearch` service.
pub struct SearchService {
    config: SearchConfig,
    providers: BTreeMap<String, Arc<dyn SearchProvider>>,
}

impl std::fmt::Debug for SearchService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SearchService")
            .field("config", &self.config)
            .field("provider_names", &self.providers.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl SearchService {
    /// Constructs a service with injected provider implementations.
    #[must_use]
    pub fn with_providers(
        config: SearchConfig,
        providers: BTreeMap<String, Arc<dyn SearchProvider>>,
    ) -> Self {
        Self { config, providers }
    }

    /// Returns declarations for tools currently enabled by configuration.
    #[must_use]
    pub fn list_tools(&self) -> ListToolsResponse {
        if !self.config.enabled {
            return ListToolsResponse::default();
        }
        select_tools(&self.available_tools(), &self.config.presentation)
    }

    /// Executes a currently advertised tool.
    ///
    /// # Errors
    /// Returns an error for disabled search, malformed arguments, or unavailable
    /// tools/providers, and propagates provider errors.
    pub async fn execute_tool(&self, request: ExecuteToolRequest) -> Result<ExecuteToolResponse> {
        if !self.config.enabled {
            return Err(Error::Disabled);
        }
        let arguments = request
            .arguments
            .as_object()
            .ok_or(Error::InvalidArguments)?;
        let available = self.available_tools();
        let (provider_name, tool) = if self.config.presentation.mode == PresentationMode::Router {
            if request.name != "search" {
                return Err(Error::UnavailableTool(request.name));
            }
            let router = select_tools(&available, &self.config.presentation)
                .tools
                .into_iter()
                .next()
                .ok_or_else(|| Error::UnavailableTool(request.name.clone()))?;
            validate_arguments(&router, &request.arguments)?;
            let explicit = arguments
                .get("provider")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned);
            let selected = explicit
                .or_else(|| self.config.presentation.provider.clone())
                .or_else(|| {
                    if self.config.backend.credential.is_some()
                        && available.contains_key("parallel")
                    {
                        Some("parallel".into())
                    } else {
                        available.keys().next().cloned()
                    }
                })
                .ok_or(Error::MissingProvider)?;
            let first_tool = available
                .get(&selected)
                .and_then(|tools| tools.first())
                .ok_or_else(|| Error::UnavailableProvider(selected.clone()))?;
            (selected, first_tool.clone())
        } else {
            let advertised = select_tools(&available, &self.config.presentation);
            if !advertised
                .tools
                .iter()
                .any(|tool| tool.name == request.name)
            {
                return Err(Error::UnavailableTool(request.name));
            }
            available
                .iter()
                .find_map(|(name, tools)| {
                    tools
                        .iter()
                        .find(|tool| tool.name == request.name)
                        .map(|tool| (name.clone(), tool.clone()))
                })
                .ok_or_else(|| Error::UnavailableTool(request.name.clone()))?
        };
        let provider = self
            .providers
            .get(&provider_name)
            .ok_or_else(|| Error::UnavailableProvider(provider_name.clone()))?;
        let config = self
            .config
            .providers
            .get(&provider_name)
            .cloned()
            .unwrap_or(ProviderConfig {
                route: ProviderRoute::Backend,
                ..ProviderConfig::default()
            });
        let mut provider_request = request;
        provider_request.name = tool.name.clone();
        if self.config.presentation.mode == PresentationMode::Router
            && let Some(values) = provider_request.arguments.as_object_mut()
        {
            values.remove("provider");
            if provider_name == "parallel" {
                // The router query is the Parallel objective and its search query.
                if let Some(query) = values.remove("query") {
                    values.entry("objective").or_insert_with(|| query.clone());
                    values
                        .entry("search_queries")
                        .or_insert_with(|| serde_json::json!([query]));
                }
            }
        }
        validate_arguments(&tool, &provider_request.arguments)?;
        let mut response = provider
            .execute(&config, &self.config.backend, &provider_request)
            .await?;
        response.provider = provider_name;
        Ok(response)
    }

    fn available_tools(&self) -> BTreeMap<String, Vec<ToolSpec>> {
        configured_provider_tools(&self.config, &provider_tool_specs())
            .into_iter()
            .filter(|(name, tools)| self.providers.contains_key(name) && !tools.is_empty())
            .collect()
    }
}

fn validate_arguments(tool: &ToolSpec, arguments: &serde_json::Value) -> Result<()> {
    let values = arguments.as_object().ok_or(Error::InvalidArguments)?;
    let schema = &tool.parameters;
    let properties = schema
        .get("properties")
        .and_then(serde_json::Value::as_object);
    if let Some(required) = schema.get("required").and_then(serde_json::Value::as_array) {
        for field in required {
            if !field.as_str().is_some_and(|name| values.contains_key(name)) {
                return Err(Error::InvalidArguments);
            }
        }
    }
    for (name, value) in values {
        if let Some(property) = properties.and_then(|properties| properties.get(name)) {
            if !matches_schema(property, value) {
                return Err(Error::InvalidArguments);
            }
        } else if schema.get("additionalProperties") == Some(&serde_json::Value::Bool(false)) {
            return Err(Error::UnsupportedArgument(name.clone()));
        } else if let Some(extra_schema) =
            schema.get("additionalProperties").filter(|v| v.is_object())
            && !matches_schema(extra_schema, value)
        {
            return Err(Error::InvalidArguments);
        }
    }
    Ok(())
}

fn matches_schema(schema: &serde_json::Value, value: &serde_json::Value) -> bool {
    if let Some(alternatives) = schema.get("oneOf").and_then(serde_json::Value::as_array)
        && alternatives
            .iter()
            .filter(|choice| matches_schema(choice, value))
            .count()
            != 1
    {
        return false;
    }
    if let Some(kind) = schema.get("type").and_then(serde_json::Value::as_str) {
        let valid_type = match kind {
            "string" => value.is_string(),
            "integer" => value.as_i64().is_some() || value.as_u64().is_some(),
            "number" => value.is_number(),
            "boolean" => value.is_boolean(),
            "array" => value.is_array(),
            "object" => value.is_object(),
            "null" => value.is_null(),
            _ => false,
        };
        if !valid_type {
            return false;
        }
    }
    if let Some(options) = schema.get("enum").and_then(serde_json::Value::as_array)
        && !options.contains(value)
    {
        return false;
    }
    if let Some(text) = value.as_str() {
        let length = text.chars().count();
        if !within_usize_bounds(schema, length, "minLength", "maxLength") {
            return false;
        }
    }
    if let Some(items) = value.as_array() {
        if !within_usize_bounds(schema, items.len(), "minItems", "maxItems") {
            return false;
        }
        if let Some(item_schema) = schema.get("items")
            && !items.iter().all(|item| matches_schema(item_schema, item))
        {
            return false;
        }
    }
    if let Some(number) = value.as_f64()
        && (schema
            .get("minimum")
            .and_then(serde_json::Value::as_f64)
            .is_some_and(|min| number < min)
            || schema
                .get("maximum")
                .and_then(serde_json::Value::as_f64)
                .is_some_and(|max| number > max)
            || schema
                .get("exclusiveMinimum")
                .and_then(serde_json::Value::as_f64)
                .is_some_and(|min| number <= min)
            || schema
                .get("exclusiveMaximum")
                .and_then(serde_json::Value::as_f64)
                .is_some_and(|max| number >= max))
    {
        return false;
    }
    if let Some(fields) = value.as_object() {
        if let Some(required) = schema.get("required").and_then(serde_json::Value::as_array)
            && required
                .iter()
                .any(|v| v.as_str().is_none_or(|name| !fields.contains_key(name)))
        {
            return false;
        }
        if let Some(properties) = schema
            .get("properties")
            .and_then(serde_json::Value::as_object)
        {
            for (key, item) in fields {
                if let Some(property) = properties.get(key) {
                    if !matches_schema(property, item) {
                        return false;
                    }
                } else if schema.get("additionalProperties")
                    == Some(&serde_json::Value::Bool(false))
                {
                    return false;
                }
            }
        }
    }
    true
}

fn within_usize_bounds(
    schema: &serde_json::Value,
    size: usize,
    min_key: &str,
    max_key: &str,
) -> bool {
    let size = size as u128;
    schema
        .get(min_key)
        .and_then(serde_json::Value::as_u64)
        .is_none_or(|min| size >= u128::from(min))
        && schema
            .get(max_key)
            .and_then(serde_json::Value::as_u64)
            .is_none_or(|max| size <= u128::from(max))
}

#[cfg(test)]
mod test;
