//! In-memory `TinyBus` integration tests.
use super::{SearchBusService, setup};
use crate::{SearchConfig, SearchService};
use std::{collections::BTreeMap, sync::Arc};
use tinybus::{Connection, Interface, broker::Broker, transport::memory::MemoryBus};
use tinysearch_bus::{ExecuteToolRequest, ExecuteToolResponse, ListToolsResponse, names};

#[test]
fn declared_methods_match_contract() {
    let methods = SearchBusService(Arc::new(SearchService::with_providers(
        SearchConfig::default(),
        BTreeMap::new(),
    )))
    .members()
    .into_iter()
    .map(|member| member.to_string())
    .collect::<Vec<_>>();
    assert_eq!(methods, names::METHODS);
}

#[test]
fn served_interface_matches_contract() {
    let service = SearchBusService(Arc::new(SearchService::with_providers(
        SearchConfig::default(),
        BTreeMap::new(),
    )));
    assert_eq!(service.name().to_string(), names::INTERFACE);
}

#[tokio::test]
async fn empty_configuration_serves_no_tools_and_rejects_execution() -> tinybus::Result<()> {
    let bus = MemoryBus::new();
    Broker::new().spawn(bus.clone());
    let server = Connection::connect(bus.connect().await?).await?;
    setup(server.clone(), SearchConfig::default()).await?;
    let client = Connection::connect(bus.connect().await?).await?;
    let proxy = client.proxy(names::INTERFACE, names::OBJECT_PATH, names::INTERFACE)?;
    let listed: ListToolsResponse = proxy.call(names::methods::LIST_TOOLS, ()).await?;
    assert!(listed.tools.is_empty());
    let result = proxy
        .call::<ExecuteToolResponse>(
            names::methods::EXECUTE_TOOL,
            (ExecuteToolRequest {
                name: "search".into(),
                arguments: serde_json::json!({"query":"test"}),
            },),
        )
        .await;
    assert!(result.is_err());
    Ok(())
}

#[test]
fn builtins_discover_and_reinitialize_from_private_configuration() {
    let mut config = SearchConfig::default();
    config.backend.base_url = Some("http://127.0.0.1:1".into());
    config.backend.credential = Some("private".into());
    config.providers.insert(
        "tinyfish".into(),
        crate::ProviderConfig {
            route: crate::ProviderRoute::Backend,
            ..crate::ProviderConfig::default()
        },
    );
    let service = SearchService::with_providers(config.clone(), crate::provider::builtins());
    let tools = service.list_tools().tools;
    assert_eq!(tools.len(), 9);
    assert!(tools.iter().any(|tool| tool.name == "tinyfish_agent_run"));
    if let Some(provider) = config.providers.get_mut("tinyfish") {
        provider.enabled = false;
    }
    let refreshed = SearchService::with_providers(config.clone(), crate::provider::builtins());
    assert_eq!(refreshed.list_tools().tools.len(), 6);
    config.enabled = false;
    let disabled = SearchService::with_providers(config, crate::provider::builtins());
    assert!(disabled.list_tools().tools.is_empty());
}
