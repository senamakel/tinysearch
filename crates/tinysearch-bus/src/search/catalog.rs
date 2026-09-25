//! Tool schemas and presentation selection shared by hosts and modules.
use super::{
    ListToolsResponse, PresentationConfig, PresentationMode, ProviderRoute, SearchConfig, ToolSpec,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn tool(name: &str, description: &str, properties: Value, required: &[&str]) -> ToolSpec {
    let mut parameters = json!({"type":"object","required":required,"additionalProperties":false});
    parameters["properties"] = properties;
    ToolSpec {
        name: name.into(),
        description: description.into(),
        parameters,
    }
}

/// Returns the supported provider operations and their exact input contracts.
#[must_use]
pub fn provider_tool_specs() -> BTreeMap<String, Vec<ToolSpec>> {
    let text = json!({"type":"string","minLength":1});
    let urls = |max| json!({"type":"array","items":{"type":"string","minLength":1},"minItems":1,"maxItems":max});
    let input = json!({"oneOf":[{"type":"string","minLength":1},{"type":"object"}]});
    let processor = json!({"type":"string","enum":["lite","base","core","ultra"]});
    let timeout = json!({"type":"integer","minimum":10,"maximum":900});
    let mut parallel = vec![
        tool(
            "parallel_search",
            "Search the web with Parallel",
            json!({"objective":text,"search_queries":urls(10),"mode":{"type":"string","enum":["fast","one-shot","agentic"]},"num_results":{"type":"integer","minimum":1,"maximum":50},"max_characters_per_excerpt":{"type":"integer","minimum":100,"maximum":10000}}),
            &["objective", "search_queries"],
        ),
        tool(
            "parallel_extract",
            "Extract web pages with Parallel",
            json!({"urls":urls(20),"objective":text,"excerpts":{"type":"boolean"},"full_content":{"type":"boolean"}}),
            &["urls"],
        ),
        tool(
            "parallel_chat",
            "Ask Parallel's web grounded chat",
            json!({"model":{"type":"string","enum":["speed","lite","base","core"]},"messages":{"type":"array","minItems":1,"items":{"type":"object","properties":{"role":{"type":"string","enum":["system","user","assistant"]},"content":text},"required":["role","content"],"additionalProperties":false}}}),
            &["model", "messages"],
        ),
        tool(
            "parallel_research",
            "Run Parallel research",
            json!({"input":input,"processor":processor,"output_schema":{"type":"object"},"timeout_seconds":timeout}),
            &["input", "processor"],
        ),
        tool(
            "parallel_enrich",
            "Enrich an entity with Parallel",
            json!({"input":input,"processor":processor,"output_schema":{"type":"object"},"timeout_seconds":timeout}),
            &["input", "processor", "output_schema"],
        ),
        tool(
            "parallel_dataset",
            "Build a dataset with Parallel",
            json!({"objective":text,"entity_type":text,"match_conditions":{"type":"array","minItems":1,"maxItems":20,"items":{"type":"object","properties":{"name":text,"description":text},"required":["name","description"],"additionalProperties":false}},"generator":{"type":"string","enum":["preview","base","core","pro"]},"match_limit":{"type":"integer","minimum":5,"maximum":1000}}),
            &["objective", "entity_type", "match_conditions"],
        ),
    ];
    let tinyfish = vec![
        tool(
            "tinyfish_search",
            "Search with TinyFish",
            json!({"query":text,"location":{"type":"string"},"language":{"type":"string"},"page":{"type":"integer","minimum":0,"maximum":10},"include_thumbnail":{"type":"boolean"}}),
            &["query"],
        ),
        tool(
            "tinyfish_fetch",
            "Render pages with TinyFish",
            json!({"urls":urls(10),"format":{"type":"string","enum":["markdown","html","json"]},"links":{"type":"boolean"},"image_links":{"type":"boolean"}}),
            &["urls"],
        ),
        tool(
            "tinyfish_agent_run",
            "Run TinyFish browser automation",
            json!({"url":text,"goal":text,"output_schema":{"type":"object"},"browser_profile":{"type":"string","enum":["lite","stealth"]},"proxy_country_code":{"type":"string","enum":["US","GB","CA","DE","FR","JP","AU"]},"use_vault":{"type":"boolean"},"credential_item_ids":{"type":"array","items":{"type":"string"}}}),
            &["url", "goal"],
        ),
    ];
    let gemini = vec![tool(
        "gemini_agentic_search",
        "Search with Gemini grounded by Google Search",
        json!({"query":text,"model":{"type":"string","minLength":1}}),
        &["query"],
    )];
    let deep = vec![tool(
        "gemini_deep_research",
        "Research with Gemini Deep Research",
        json!({"query":text,"interaction_id":{"type":"string","minLength":1},"max_poll_attempts":{"type":"integer","minimum":1,"maximum":30}}),
        &[],
    )];
    parallel.shrink_to_fit();
    let mut specs: BTreeMap<String, Vec<ToolSpec>> = [
        ("parallel".into(), parallel),
        ("tinyfish".into(), tinyfish),
        ("gemini".into(), gemini),
        ("gemini_deep_research".into(), deep),
        (
            "searxng".into(),
            vec![tool(
                "searxng_search",
                "Search a SearXNG instance",
                json!({"query":text,"categories":{"type":"array","items":{"type":"string","enum":["web","general","news","images"]}},"language":text,"max_results":{"type":"integer","minimum":1,"maximum":50}}),
                &["query"],
            )],
        ),
    ]
    .into();
    specs.extend(direct_provider_specs());
    specs
}

fn direct_provider_specs() -> BTreeMap<String, Vec<ToolSpec>> {
    let text = json!({"type":"string","minLength":1});
    let urls = |max| json!({"type":"array","items":{"type":"string","minLength":1},"minItems":1,"maxItems":max});
    let exa = vec![
        tool(
            "exa_search",
            "Search with Exa",
            json!({"query":text,"max_results":{"type":"integer","minimum":1,"maximum":20},"type":{"type":"string","enum":["auto","instant","fast","deep-lite","deep","deep-reasoning"]},"category":text,"include_domains":urls(20),"exclude_domains":urls(20),"start_published_date":text,"end_published_date":text,"include_text":{"type":"boolean"},"include_highlights":{"type":"boolean"}}),
            &["query"],
        ),
        tool(
            "exa_find_similar",
            "Find pages similar to a URL with Exa",
            json!({"url":text,"max_results":{"type":"integer","minimum":1,"maximum":20},"exclude_source_domain":{"type":"boolean"},"include_domains":urls(20),"exclude_domains":urls(20),"include_text":{"type":"boolean"},"include_highlights":{"type":"boolean"}}),
            &["url"],
        ),
        tool(
            "exa_get_contents",
            "Get page contents with Exa",
            json!({"urls":urls(20),"include_summary":{"type":"boolean"},"include_highlights":{"type":"boolean"}}),
            &["urls"],
        ),
    ];
    let brave = ["web", "news", "image", "video"].into_iter().map(|kind| {
        let mut properties = json!({"query":text,"count":{"type":"integer","minimum":1,"maximum":20},"country":{"type":"string","minLength":2,"maxLength":2}});
        if kind != "image" { properties["freshness"] = json!({"type":"string","minLength":1}); }
        tool(&format!("brave_{kind}_search"), &format!("Search {kind} with Brave"), properties, &["query"])
    }).collect();
    let querit = vec![tool(
        "querit_search",
        "Search with Querit",
        json!({"query":text,"max_results":{"type":"integer","minimum":1,"maximum":20},"filters":{"type":"object"},"include_domains":urls(20),"exclude_domains":urls(20),"time_range":text,"date":text,"from_date":text,"to_date":text,"countries":urls(20),"languages":urls(20)}),
        &["query"],
    )];
    let tavily = vec![
        tool(
            "tavily_search",
            "Search with Tavily",
            json!({"query":text,"max_results":{"type":"integer","minimum":1,"maximum":20},"search_depth":{"type":"string","enum":["basic","advanced","fast","ultra-fast"]},"topic":{"type":"string","enum":["general","news","finance"]},"time_range":{"type":"string","enum":["day","week","month","year"]},"start_date":text,"end_date":text,"include_answer":{"type":"boolean"},"include_raw_content":{"type":"boolean"},"include_images":{"type":"boolean"},"include_domains":urls(20),"exclude_domains":urls(20)}),
            &["query"],
        ),
        tool(
            "tavily_extract",
            "Extract pages with Tavily",
            json!({"urls":urls(20),"format":{"type":"string","enum":["markdown","text"]},"extract_depth":{"type":"string","enum":["basic","advanced"]}}),
            &["urls"],
        ),
    ];
    let seltz = vec![tool(
        "seltz_search",
        "Search the web with Seltz",
        json!({"query":text,"max_results":{"type":"integer","minimum":1,"maximum":20},"include_domains":urls(20),"exclude_domains":urls(20),"from_date":text,"to_date":text,"scope":{"type":"string","enum":["news"]}}),
        &["query"],
    )];
    [
        ("exa".into(), exa),
        ("brave".into(), brave),
        ("querit".into(), querit),
        ("tavily".into(), tavily),
        ("seltz".into(), seltz),
    ]
    .into()
}

/// Filters declared providers by configuration and available credentials.
#[must_use]
pub fn configured_provider_tools(
    config: &SearchConfig,
    specs: &BTreeMap<String, Vec<ToolSpec>>,
) -> BTreeMap<String, Vec<ToolSpec>> {
    if !config.enabled {
        return BTreeMap::new();
    }
    specs
        .iter()
        .filter_map(|(name, tools)| {
            let explicit = config.providers.get(name);
            let implicit_parallel =
                name == "parallel" && explicit.is_none() && config.backend.credential.is_some();
            if !implicit_parallel && explicit.is_none() {
                return None;
            }
            let enabled = explicit.is_none_or(|p| p.enabled);
            let route = explicit.map_or(ProviderRoute::Backend, |p| p.route);
            let credential_available = match route {
                ProviderRoute::Backend => {
                    config.backend.credential.is_some()
                        && matches!(name.as_str(), "parallel" | "tinyfish" | "gemini")
                }
                ProviderRoute::Direct => {
                    (name == "searxng"
                        && explicit.is_some_and(|p| {
                            p.base_url
                                .as_deref()
                                .is_some_and(|url| !url.trim().is_empty())
                        }))
                        || (matches!(
                            name.as_str(),
                            "parallel"
                                | "exa"
                                | "brave"
                                | "querit"
                                | "tavily"
                                | "gemini"
                                | "gemini_deep_research"
                                | "seltz"
                        ) && explicit.is_some_and(|p| {
                            p.credential
                                .as_deref()
                                .is_some_and(|key| !key.trim().is_empty())
                        }))
                }
            };
            (enabled && credential_available).then(|| {
                let mut tools = tools.clone();
                if name == "parallel" && route == ProviderRoute::Direct {
                    if let Some(search) = tools.iter_mut().find(|tool| tool.name == "parallel_search") {
                        search.parameters["properties"]["mode"] = json!({"type":"string","enum":["turbo","fast","basic","advanced"]});
                    }
                    if let Some(properties) = tools.iter_mut().find(|tool| tool.name == "parallel_extract")
                        .and_then(|spec| spec.parameters.get_mut("properties"))
                        .and_then(Value::as_object_mut) {
                        properties.remove("excerpts");
                    }
                    for name in ["parallel_research", "parallel_enrich", "parallel_dataset"] {
                        if let Some(properties) = tools.iter_mut().find(|tool| tool.name == name)
                            .and_then(|spec| spec.parameters.get_mut("properties"))
                            .and_then(Value::as_object_mut) {
                            properties.remove("timeout_seconds");
                        }
                    }
                    let id = json!({"type":"string","minLength":1,"maxLength":128,"pattern":"^[A-Za-z0-9_-]+$"});
                    tools.push(tool("parallel_research_status", "Check a Parallel research run and fetch its result", json!({"run_id":id}), &["run_id"]));
                    tools.push(tool("parallel_enrich_status", "Check a Parallel enrichment run and fetch its result", json!({"run_id":id}), &["run_id"]));
                    tools.push(tool("parallel_dataset_status", "Check a Parallel dataset run and fetch its result", json!({"findall_id":id}), &["findall_id"]));
                }
                (name.clone(), tools)
            })
        })
        .collect()
}

/// Selects deterministic presentation from available provider declarations.
#[must_use]
pub fn select_tools(
    provider_tools: &BTreeMap<String, Vec<ToolSpec>>,
    presentation: &PresentationConfig,
) -> ListToolsResponse {
    match presentation.mode {
        PresentationMode::AllTools => ListToolsResponse {
            tools: provider_tools.values().flatten().cloned().collect(),
        },
        PresentationMode::OneProvider => ListToolsResponse {
            tools: presentation
                .provider
                .as_ref()
                .and_then(|name| provider_tools.get(name))
                .cloned()
                .unwrap_or_default(),
        },
        PresentationMode::Router => {
            if provider_tools.is_empty() {
                return ListToolsResponse::default();
            }
            // Router forwards provider-specific options, so it must advertise that fact.
            ListToolsResponse {
                tools: vec![ToolSpec {
                    name: "search".into(),
                    description: "Search with an available provider".into(),
                    parameters: json!({"type":"object","properties":{"provider":{"type":"string"},"query":{"type":"string","minLength":1}},"required":["query"],"additionalProperties":true}),
                }],
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn catalog_and_selection_are_stable() {
        let specs = provider_tool_specs();
        assert_eq!(specs["parallel"][0].name, "parallel_search");
        assert_eq!(specs["parallel"].len(), 6);
        assert_eq!(specs["tinyfish"].len(), 3);
        assert_eq!(
            select_tools(&specs, &PresentationConfig::default())
                .tools
                .len(),
            23
        );
    }
    #[test]
    fn findall_match_conditions_require_name_and_description()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let specs = provider_tool_specs();
        let dataset = specs["parallel"]
            .iter()
            .find(|tool| tool.name == "parallel_dataset")
            .ok_or("missing dataset tool")?;
        let condition = &dataset.parameters["properties"]["match_conditions"]["items"];
        assert_eq!(condition["required"], json!(["name", "description"]));
        assert_eq!(condition["properties"]["description"]["minLength"], 1);
        Ok(())
    }
    #[test]
    fn searxng_categories_match_supported_execution_values() {
        let specs = provider_tool_specs();
        assert_eq!(
            specs["searxng"][0].parameters["properties"]["categories"]["items"]["enum"],
            json!(["web", "general", "news", "images"])
        );
    }
    #[test]
    fn direct_tools_require_a_nonempty_private_credential() {
        let specs = provider_tool_specs();
        let mut config = SearchConfig::default();
        for name in ["exa", "brave", "querit", "tavily", "seltz"] {
            config.providers.insert(
                name.into(),
                super::super::ProviderConfig {
                    credential: Some("  ".into()),
                    ..Default::default()
                },
            );
        }
        assert!(configured_provider_tools(&config, &specs).is_empty());
        for provider in config.providers.values_mut() {
            provider.credential = Some("secret".into());
        }
        let available = configured_provider_tools(&config, &specs);
        assert_eq!(available["exa"].len(), 3);
        assert_eq!(available["brave"].len(), 4);
        assert_eq!(available["querit"].len(), 1);
        assert_eq!(available["tavily"].len(), 2);
        assert_eq!(available["seltz"].len(), 1);
    }
}
