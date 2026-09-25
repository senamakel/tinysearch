//! Wire-format and credential-redaction tests.
use super::*;

#[test]
fn config_defaults_and_redacts_credentials() -> serde_json::Result<()> {
    let mut config = SearchConfig::default();
    config.backend.credential = Some("backend-secret".into());
    config.providers.insert(
        "parallel".into(),
        ProviderConfig {
            credential: Some("provider-secret".into()),
            ..ProviderConfig::default()
        },
    );
    let debug = format!("{config:?}");
    assert!(!debug.contains("backend-secret"));
    assert!(!debug.contains("provider-secret"));
    let decoded: SearchConfig = serde_json::from_value(serde_json::json!({}))?;
    assert!(decoded.enabled);
    assert_eq!(decoded.presentation.mode, PresentationMode::AllTools);
    Ok(())
}

#[test]
fn execution_wire_names_are_stable() -> serde_json::Result<()> {
    let request = ExecuteToolRequest {
        name: "parallel_search".into(),
        arguments: serde_json::json!({"query":"rust"}),
    };
    assert_eq!(
        serde_json::to_value(request)?,
        serde_json::json!({"name":"parallel_search","arguments":{"query":"rust"}})
    );
    Ok(())
}

#[test]
fn search_result_publication_is_optional_on_the_wire() -> serde_json::Result<()> {
    let old: SearchResult = serde_json::from_value(serde_json::json!({
        "title":"Old", "url":"https://example.test", "snippet":null
    }))?;
    assert_eq!(old.published, None);
    assert!(serde_json::to_value(&old)?.get("published").is_none());

    let dated: SearchResult = serde_json::from_value(serde_json::json!({
        "title":"Dated", "url":"https://example.test", "snippet":null,
        "published":"2026-01-02"
    }))?;
    assert_eq!(dated.published.as_deref(), Some("2026-01-02"));
    assert_eq!(serde_json::to_value(dated)?["published"], "2026-01-02");
    Ok(())
}

#[test]
fn provider_limits_round_trip_without_exposing_credential_in_debug() -> serde_json::Result<()> {
    let config: ProviderConfig = serde_json::from_value(serde_json::json!({
        "credential":"private-key", "max_results":12, "timeout_secs":8,
        "default_language":"fr"
    }))?;
    assert_eq!(config.max_results, Some(12));
    assert_eq!(config.timeout_secs, Some(8));
    assert_eq!(config.default_language.as_deref(), Some("fr"));
    let serialized = serde_json::to_value(&config)?;
    assert_eq!(serialized["max_results"], 12);
    assert_eq!(serialized["timeout_secs"], 8);
    assert_eq!(serialized["default_language"], "fr");
    assert!(!format!("{config:?}").contains("private-key"));
    let legacy: ProviderConfig =
        serde_json::from_value(serde_json::json!({"credential":"private-key"}))?;
    assert_eq!(legacy.max_results, None);
    assert_eq!(legacy.timeout_secs, None);
    assert_eq!(legacy.default_language, None);
    Ok(())
}
