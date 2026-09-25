//! Direct Parallel API mapping. Task and `FindAll` runs are resumed by ID.
use super::{configured_timeout, normalize};
use crate::{Error, ExecuteToolRequest, ExecuteToolResponse, ProviderConfig, Result, SearchStatus};
use reqwest::{Client, Method};
use serde_json::{Value, json};
use std::time::Duration;

const BASE: &str = "https://api.parallel.ai";

pub(super) async fn run(
    client: &Client,
    config: &ProviderConfig,
    request: &ExecuteToolRequest,
) -> Result<ExecuteToolResponse> {
    let key = config
        .credential
        .as_deref()
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| Error::Provider("provider credential unavailable".into()))?;
    let (method, path, body, kind) = prepare(config, request)?;
    let value = send(client, config, key, method, &path, body, kind).await?;
    if matches!(kind, Kind::Task | Kind::FindAll) {
        return async_response(client, config, key, request, kind, value).await;
    }
    Ok(normalize("parallel", &request.name, &value))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Sync,
    Task,
    FindAll,
}

fn prepare(
    config: &ProviderConfig,
    request: &ExecuteToolRequest,
) -> Result<(Method, String, Option<Value>, Kind)> {
    let args = &request.arguments;
    let get = |key: &str| args.get(key).cloned().ok_or(Error::InvalidArguments);
    let route = match request.name.as_str() {
        "parallel_search" => search_route(config, args)?,
        "parallel_extract" => extract_route(args)?,
        "parallel_chat" => (
            Method::POST,
            "/v1beta/chat/completions".into(),
            Some(json!({"model":get("model")?,"messages":get("messages")?,"stream":false})),
            Kind::Sync,
        ),
        "parallel_research" | "parallel_enrich" => {
            let mut body = json!({"input":get("input")?,"processor":get("processor")?});
            if let Some(schema) = args.get("output_schema") {
                body["task_spec"] = json!({"output_schema":{"type":"json","json_schema":schema}});
            }
            (
                Method::POST,
                "/v1/tasks/runs".into(),
                Some(body),
                Kind::Task,
            )
        }
        "parallel_dataset" => dataset_route(args)?,
        "parallel_research_status" | "parallel_enrich_status" => (
            Method::GET,
            format!("/v1/tasks/runs/{}", safe_id(&get("run_id")?)?),
            None,
            Kind::Task,
        ),
        "parallel_dataset_status" => (
            Method::GET,
            format!("/v1beta/findall/runs/{}", safe_id(&get("findall_id")?)?),
            None,
            Kind::FindAll,
        ),
        _ => {
            return Err(Error::Provider(
                "unsupported direct Parallel operation".into(),
            ));
        }
    };
    Ok(route)
}

fn search_route(
    config: &ProviderConfig,
    args: &Value,
) -> Result<(Method, String, Option<Value>, Kind)> {
    let get = |key: &str| args.get(key).cloned().ok_or(Error::InvalidArguments);
    let mut body = json!({"objective":get("objective")?,"search_queries":get("search_queries")?});
    if let Some(mode) = args.get("mode") {
        body["mode"] = mode.clone();
    }
    let mut settings = json!({});
    if let Some(count) = args.get("num_results") {
        settings["max_results"] = count.clone();
    } else if let Some(count) = config.max_results {
        settings["max_results"] = json!(count.clamp(1, 50));
    }
    if let Some(chars) = args.get("max_characters_per_excerpt") {
        settings["excerpt_settings"] = json!({"max_chars_per_result":chars});
    }
    if settings.as_object().is_some_and(|map| !map.is_empty()) {
        body["advanced_settings"] = settings;
    }
    Ok((Method::POST, "/v1/search".into(), Some(body), Kind::Sync))
}

fn extract_route(args: &Value) -> Result<(Method, String, Option<Value>, Kind)> {
    let urls = args.get("urls").cloned().ok_or(Error::InvalidArguments)?;
    let mut body = json!({"urls":urls});
    if let Some(objective) = args.get("objective") {
        body["objective"] = objective.clone();
    }
    if args
        .get("excerpts")
        .is_some_and(|excerpts| excerpts == false)
    {
        return Err(Error::Provider(
            "direct Parallel extract cannot disable excerpts".into(),
        ));
    }
    if let Some(full) = args.get("full_content") {
        body["advanced_settings"] = json!({"full_content":full});
    }
    Ok((Method::POST, "/v1/extract".into(), Some(body), Kind::Sync))
}

fn dataset_route(args: &Value) -> Result<(Method, String, Option<Value>, Kind)> {
    let get = |key: &str| args.get(key).cloned().ok_or(Error::InvalidArguments);
    let body = json!({"objective":get("objective")?,"entity_type":get("entity_type")?,"match_conditions":get("match_conditions")?,"generator":args.get("generator").cloned().unwrap_or(json!("base")),"match_limit":args.get("match_limit").cloned().unwrap_or(json!(100))});
    if body["match_conditions"]
        .as_array()
        .is_none_or(|conditions| {
            conditions.is_empty()
                || conditions.iter().any(|condition| {
                    ["name", "description"].iter().any(|field| {
                        condition
                            .get(field)
                            .and_then(Value::as_str)
                            .is_none_or(str::is_empty)
                    })
                })
        })
    {
        return Err(Error::InvalidArguments);
    }
    if body["match_limit"]
        .as_u64()
        .is_none_or(|n| !(5..=1000).contains(&n))
    {
        return Err(Error::InvalidArguments);
    }
    Ok((
        Method::POST,
        "/v1beta/findall/runs".into(),
        Some(body),
        Kind::FindAll,
    ))
}

fn safe_id(value: &Value) -> Result<&str> {
    let id = value.as_str().ok_or(Error::InvalidArguments)?;
    if id.is_empty()
        || id.len() > 128
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return Err(Error::InvalidArguments);
    }
    Ok(id)
}

async fn send(
    client: &Client,
    config: &ProviderConfig,
    key: &str,
    method: Method,
    path: &str,
    body: Option<Value>,
    kind: Kind,
) -> Result<Value> {
    let url = super::direct_url(config.base_url.as_deref(), BASE, path)?;
    let mut builder = client
        .request(method, url)
        .timeout(request_timeout(config, kind))
        .header(reqwest::header::ACCEPT, "application/json")
        .header("x-api-key", key);
    if let Some(body) = body {
        builder = builder.json(&body);
    }
    let mut response = builder
        .send()
        .await
        .map_err(|_| Error::Provider("provider transport failed".into()))?;
    if !response.status().is_success() {
        return Err(Error::Provider(format!(
            "provider returned HTTP {}",
            response.status().as_u16()
        )));
    }
    if response
        .content_length()
        .is_some_and(|n| n > super::super::MAX_BODY_BYTES)
    {
        return Err(Error::Provider("provider response too large".into()));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| Error::Provider("provider response read failed".into()))?
    {
        if bytes.len().saturating_add(chunk.len()) as u64 > super::super::MAX_BODY_BYTES {
            return Err(Error::Provider("provider response too large".into()));
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes)
        .map_err(|_| Error::Provider("provider returned invalid JSON".into()))
}

fn request_timeout(config: &ProviderConfig, kind: Kind) -> Duration {
    let timeout = configured_timeout(config, Duration::from_secs(35));
    if kind == Kind::Sync {
        timeout
    } else {
        timeout.min(Duration::from_secs(35))
    }
}

fn basis_citations(basis: &Value) -> Vec<Value> {
    basis
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|field| {
            field
                .get("citations")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
        })
        .take(super::super::MAX_CITATIONS)
        .cloned()
        .collect()
}

async fn async_response(
    client: &Client,
    config: &ProviderConfig,
    key: &str,
    request: &ExecuteToolRequest,
    kind: Kind,
    value: Value,
) -> Result<ExecuteToolResponse> {
    let (id_field, id) = if kind == Kind::Task {
        ("run_id", value.get("run_id"))
    } else {
        ("findall_id", value.get("findall_id"))
    };
    let id = id
        .and_then(Value::as_str)
        .ok_or_else(|| Error::Provider("provider response missing run ID".into()))?;
    let id = safe_id(&json!(id))?.to_owned();
    let status = if kind == Kind::Task {
        value.get("status")
    } else {
        value.pointer("/status/status")
    };
    let status = status
        .and_then(Value::as_str)
        .ok_or_else(|| Error::Provider("provider response missing status".into()))?;
    if matches!(status, "failed" | "cancelled" | "error") {
        return Err(Error::Provider("provider task failed".into()));
    }
    let mut output = if matches!(status, "completed" | "complete" | "succeeded") {
        let path = if kind == Kind::Task {
            format!("/v1/tasks/runs/{id}/result")
        } else {
            format!("/v1beta/findall/runs/{id}/result")
        };
        let result = send(client, config, key, Method::GET, &path, None, kind).await?;
        let mut response = if kind == Kind::FindAll {
            let candidates = result
                .get("candidates")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let matched: Vec<&Value> = candidates
                .iter()
                .filter(|candidate| {
                    candidate.get("match_status").and_then(Value::as_str) == Some("matched")
                        && candidate
                            .get("url")
                            .and_then(Value::as_str)
                            .is_some_and(|url| !url.is_empty())
                })
                .take(super::super::MAX_RESULTS)
                .collect();
            let results: Vec<Value> = matched.iter().filter_map(|candidate| {
                let url = candidate.get("url")?.as_str()?;
                Some(json!({"url":url,"title":candidate.get("name").and_then(Value::as_str).unwrap_or(""),"snippet":candidate.get("output").map(Value::to_string).unwrap_or_default()}))
            }).collect();
            let citations: Vec<Value> = matched
                .iter()
                .flat_map(|candidate| basis_citations(&candidate["basis"]))
                .take(super::super::MAX_CITATIONS)
                .collect();
            normalize(
                "parallel",
                &request.name,
                &json!({"results":results,"citations":citations}),
            )
        } else {
            let content = result
                .pointer("/output/content")
                .cloned()
                .unwrap_or(Value::Null);
            let basis = result
                .pointer("/output/basis")
                .cloned()
                .unwrap_or(Value::Null);
            let citations = basis_citations(&basis);
            normalize(
                "parallel",
                &request.name,
                &json!({"output":content,"citations":citations}),
            )
        };
        response.status = SearchStatus::Ok;
        response
    } else {
        let mut response = normalize("parallel", &request.name, &Value::Null);
        response.status = SearchStatus::InProgress;
        response
    };
    output.provider_data = Some(json!({id_field:id,"status":status}));
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BackendConfig, ProviderRoute, SearchConfig, SearchService};
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    async fn mock(
        responses: Vec<(u16, Value)>,
    ) -> std::io::Result<(
        String,
        tokio::task::JoinHandle<std::io::Result<Vec<String>>>,
    )> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let url = format!("http://{}", listener.local_addr()?);
        let task = tokio::spawn(async move {
            let mut requests = Vec::new();
            for (status, body) in responses {
                let (mut stream, _) = listener.accept().await?;
                let mut bytes = Vec::new();
                loop {
                    let mut chunk = [0; 4096];
                    let n = stream.read(&mut chunk).await?;
                    if n == 0 {
                        break;
                    }
                    bytes.extend_from_slice(&chunk[..n]);
                    if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                        let header = String::from_utf8_lossy(&bytes[..end]);
                        let length = header
                            .lines()
                            .find_map(|s| {
                                s.to_ascii_lowercase()
                                    .strip_prefix("content-length: ")
                                    .and_then(|v| v.parse::<usize>().ok())
                            })
                            .unwrap_or(0);
                        if bytes.len() >= end + 4 + length {
                            break;
                        }
                    }
                }
                requests.push(String::from_utf8_lossy(&bytes).into_owned());
                let body = body.to_string();
                stream.write_all(format!("HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await?;
            }
            Ok(requests)
        });
        Ok((url, task))
    }
    fn request(name: &str, arguments: Value) -> ExecuteToolRequest {
        ExecuteToolRequest {
            name: name.into(),
            arguments,
        }
    }
    fn config(base_url: String) -> ProviderConfig {
        ProviderConfig {
            route: ProviderRoute::Direct,
            base_url: Some(base_url),
            credential: Some("secret-key".into()),
            ..ProviderConfig::default()
        }
    }

    #[tokio::test]
    async fn search_maps_current_schema_and_bounds_results()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let results: Vec<Value> = (0..25).map(|i| json!({"url":format!("https://example.org/{i}"),"title":"T","publish_date":"2026-09-25","excerpts":["x".repeat(2000)]})).collect();
        let (url, server) = mock(vec![(
            200,
            json!({"results":results,"search_id":"search_1"}),
        )])
        .await?;
        let response = run(&Client::new(), &config(url), &request("parallel_search",json!({"objective":"Find","search_queries":["find this"],"mode":"advanced","num_results":5,"max_characters_per_excerpt":500}))).await?;
        let sent = server.await??.remove(0);
        assert!(sent.starts_with("POST /v1/search "));
        assert!(sent.to_ascii_lowercase().contains("x-api-key: secret-key"));
        let body: Value =
            serde_json::from_str(sent.split("\r\n\r\n").nth(1).ok_or("missing HTTP body")?)?;
        assert_eq!(
            body,
            json!({"objective":"Find","search_queries":["find this"],"mode":"advanced","advanced_settings":{"max_results":5,"excerpt_settings":{"max_chars_per_result":500}}})
        );
        assert_eq!(response.results.len(), 20);
        assert_eq!(
            response.results[0]
                .snippet
                .as_ref()
                .ok_or("missing snippet")?
                .len(),
            1200
        );
        assert_eq!(response.results[0].published.as_deref(), Some("2026-09-25"));
        assert_eq!(response.citations.len(), 20);
        Ok(())
    }

    #[tokio::test]
    async fn extract_and_chat_use_direct_paths()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let (url, server) = mock(vec![(
            200,
            json!({"results":[{"url":"https://example.org","full_content":"body"}]}),
        )])
        .await?;
        let response = run(
            &Client::new(),
            &config(url),
            &request(
                "parallel_extract",
                json!({"urls":["https://example.org"],"full_content":true}),
            ),
        )
        .await?;
        let sent = server.await??.remove(0);
        assert!(sent.starts_with("POST /v1/extract "));
        assert!(sent.contains("\"advanced_settings\":{\"full_content\":true}"));
        assert_eq!(response.results[0].snippet.as_deref(), Some("body"));
        let (url, server) = mock(vec![(
            200,
            json!({"choices":[{"message":{"content":"answer"}}]}),
        )])
        .await?;
        let response = run(
            &Client::new(),
            &config(url),
            &request(
                "parallel_chat",
                json!({"model":"lite","messages":[{"role":"user","content":"hello"}]}),
            ),
        )
        .await?;
        let sent = server.await??.remove(0);
        assert!(sent.starts_with("POST /v1beta/chat/completions "));
        assert_eq!(response.answer.as_deref(), Some("answer"));
        Ok(())
    }

    #[tokio::test]
    async fn task_creation_and_resume_are_bounded()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let (url, server) = mock(vec![(202, json!({"run_id":"trun_1","status":"queued"}))]).await?;
        let response = run(&Client::new(), &config(url), &request("parallel_enrich",json!({"input":{"company":"Example"},"processor":"base","output_schema":{"type":"object","properties":{"name":{"type":"string"}}}}))).await?;
        let sent = server.await??.remove(0);
        assert!(sent.starts_with("POST /v1/tasks/runs "));
        assert!(sent.contains("\"task_spec\":{\"output_schema\":{\"json_schema\":"));
        assert_eq!(response.status, SearchStatus::InProgress);
        assert_eq!(
            response.provider_data.ok_or("missing provider data")?["run_id"],
            "trun_1"
        );
        let (url, server) = mock(vec![(200,json!({"run_id":"trun_1","status":"completed"})),(200,json!({"run":{"run_id":"trun_1"},"output":{"type":"json","content":{"name":"Example"},"basis":[]}}))]).await?;
        let response = run(
            &Client::new(),
            &config(url),
            &request("parallel_enrich_status", json!({"run_id":"trun_1"})),
        )
        .await?;
        let sent = server.await??;
        assert!(sent[0].starts_with("GET /v1/tasks/runs/trun_1 "));
        assert!(sent[1].starts_with("GET /v1/tasks/runs/trun_1/result "));
        assert_eq!(response.status, SearchStatus::Ok);
        assert!(response.answer.ok_or("missing answer")?.contains("Example"));
        Ok(())
    }

    #[tokio::test]
    async fn completed_task_preserves_field_basis_citations()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let (url, server) = mock(vec![
            (200, json!({"run_id":"trun_1","status":"completed"})),
            (200, json!({"run":{"run_id":"trun_1"},"output":{"type":"json","content":{"name":"Example"},"basis":[{"field":"name","reasoning":"Company site","citations":[{"title":"About Example","url":"https://example.org/about","excerpts":["Company profile"]}]}]}})),
        ]).await?;
        let response = run(
            &Client::new(),
            &config(url),
            &request("parallel_research_status", json!({"run_id":"trun_1"})),
        )
        .await?;
        server.await??;
        assert_eq!(response.citations.len(), 1);
        assert_eq!(response.citations[0].url, "https://example.org/about");
        assert_eq!(
            response.citations[0].title.as_deref(),
            Some("About Example")
        );
        Ok(())
    }

    #[test]
    fn dataset_conditions_require_descriptions() {
        let request = request(
            "parallel_dataset",
            json!({"objective":"companies","entity_type":"company","match_conditions":[{"name":"is_public"}]}),
        );
        assert!(matches!(
            prepare(&ProviderConfig::default(), &request),
            Err(Error::InvalidArguments)
        ));
    }

    #[test]
    fn async_http_timeout_is_capped_without_changing_sync_calls() {
        let config = ProviderConfig {
            timeout_secs: Some(1800),
            ..ProviderConfig::default()
        };
        assert_eq!(
            request_timeout(&config, Kind::Task),
            Duration::from_secs(35)
        );
        assert_eq!(
            request_timeout(&config, Kind::FindAll),
            Duration::from_secs(35)
        );
        assert_eq!(
            request_timeout(&config, Kind::Sync),
            Duration::from_secs(1800)
        );
    }

    #[test]
    fn configured_search_count_is_clamped_to_parallel_api_range()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        for (configured, expected) in [(0, 1), (51, 50), (u64::MAX, 50)] {
            let config = ProviderConfig {
                max_results: Some(configured),
                ..ProviderConfig::default()
            };
            let (_, _, body, _) = prepare(
                &config,
                &request(
                    "parallel_search",
                    json!({"objective":"Find","search_queries":["find"]}),
                ),
            )?;
            assert_eq!(
                body.ok_or("missing request body")?["advanced_settings"]["max_results"],
                expected
            );
        }
        Ok(())
    }

    #[tokio::test]
    async fn dataset_creation_and_resume_normalize_candidates()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let (url, server) = mock(vec![(
            200,
            json!({"findall_id":"findall_1","status":{"status":"running"}}),
        )])
        .await?;
        let response = run(&Client::new(), &config(url), &request("parallel_dataset",json!({"objective":"companies","entity_type":"company","match_conditions":[{"name":"is_public","description":"Company is publicly listed"}]}))).await?;
        let sent = server.await??.remove(0);
        assert!(sent.starts_with("POST /v1beta/findall/runs "));
        assert!(sent.contains("\"generator\":\"base\""));
        assert!(sent.contains("\"match_limit\":100"));
        assert_eq!(response.status, SearchStatus::InProgress);
        assert_eq!(
            response.provider_data.ok_or("missing provider data")?["findall_id"],
            "findall_1"
        );
        let mut candidates: Vec<Value> = (0..20).map(|i| json!({"candidate_id":format!("candidate_{i}"),"name":"Unmatched","url":format!("https://excluded.org/{i}"),"match_status":"unmatched"})).collect();
        candidates.extend([
            json!({"candidate_id":"candidate_generated","name":"Generated","url":"https://generated.org","match_status":"generated"}),
            json!({"candidate_id":"candidate_discarded","name":"Discarded","url":"https://discarded.org","match_status":"discarded"}),
            json!({"candidate_id":"candidate_matched","name":"Example","url":"https://example.org","match_status":"matched","output":{"public":true},"basis":[{"field":"public","reasoning":"Official listing","citations":[{"title":"Exchange listing","url":"https://exchange.org/example","excerpts":["Listed"]}]}]})
        ]);
        let (url, server) = mock(vec![
            (
                200,
                json!({"findall_id":"findall_1","status":{"status":"completed"}}),
            ),
            (200, json!({"candidates":candidates})),
        ])
        .await?;
        let response = run(
            &Client::new(),
            &config(url),
            &request("parallel_dataset_status", json!({"findall_id":"findall_1"})),
        )
        .await?;
        let sent = server.await??;
        assert!(sent[0].starts_with("GET /v1beta/findall/runs/findall_1 "));
        assert!(sent[1].starts_with("GET /v1beta/findall/runs/findall_1/result "));
        assert_eq!(response.results.len(), 1);
        assert_eq!(response.results[0].url, "https://example.org");
        assert!(
            response
                .citations
                .iter()
                .any(|citation| citation.url == "https://exchange.org/example")
        );
        assert_eq!(response.status, SearchStatus::Ok);
        Ok(())
    }

    #[tokio::test]
    async fn errors_do_not_expose_secret_or_response_body()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let (url, server) = mock(vec![(401, json!({"error":"secret-key private query"}))]).await?;
        let Err(error) = run(
            &Client::new(),
            &config(url),
            &request(
                "parallel_search",
                json!({"objective":"private query","search_queries":["private query"]}),
            ),
        )
        .await
        else {
            return Err("expected provider failure".into());
        };
        server.await??;
        assert!(!error.to_string().contains("secret-key"));
        assert!(!error.to_string().contains("private query"));
        assert!(error.to_string().contains("401"));
        assert!(
            prepare(
                &ProviderConfig::default(),
                &request("parallel_research_status", json!({"run_id":"../../bad"}))
            )
            .is_err()
        );
        Ok(())
    }

    #[test]
    fn catalog_is_route_aware() -> std::result::Result<(), Box<dyn std::error::Error>> {
        let mut config = SearchConfig {
            backend: BackendConfig {
                credential: Some("backend".into()),
                ..BackendConfig::default()
            },
            ..SearchConfig::default()
        };
        config.providers.insert(
            "parallel".into(),
            ProviderConfig {
                route: ProviderRoute::Direct,
                credential: Some("direct".into()),
                ..ProviderConfig::default()
            },
        );
        let tools = SearchService::with_providers(config.clone(), super::super::super::builtins())
            .list_tools()
            .tools;
        assert!(tools.iter().any(|t| t.name == "parallel_research_status"));
        let search = tools
            .iter()
            .find(|t| t.name == "parallel_search")
            .ok_or("missing search tool")?;
        assert!(
            search.parameters["properties"]["mode"]["enum"]
                .as_array()
                .ok_or("missing mode enum")?
                .contains(&json!("advanced"))
        );
        assert!(
            tools
                .iter()
                .find(|t| t.name == "parallel_extract")
                .ok_or("missing extract tool")?
                .parameters["properties"]
                .get("excerpts")
                .is_none()
        );
        config
            .providers
            .get_mut("parallel")
            .ok_or("missing provider")?
            .route = ProviderRoute::Backend;
        let tools = SearchService::with_providers(config, super::super::super::builtins())
            .list_tools()
            .tools;
        assert!(!tools.iter().any(|t| t.name == "parallel_research_status"));
        assert!(
            tools
                .iter()
                .find(|t| t.name == "parallel_extract")
                .ok_or("missing extract tool")?
                .parameters["properties"]
                .get("excerpts")
                .is_some()
        );
        Ok(())
    }
}
