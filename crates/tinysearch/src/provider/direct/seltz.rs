//! Seltz direct search request and response mapping.
use super::{Prepared, required_string};
use crate::{Error, ExecuteToolRequest, Result};
use reqwest::Method;
use serde_json::{Value, json};
use std::time::Duration;

pub(super) fn prepare(request: &ExecuteToolRequest, key: &str) -> Result<Prepared> {
    if request.name != "seltz_search" {
        return Err(Error::UnavailableTool(request.name.clone()));
    }
    let args = &request.arguments;
    let mut body = json!({
        "query": required_string(args, "query")?,
        "max_results": args.get("max_results").and_then(Value::as_u64).unwrap_or(5).clamp(1, 20),
    });
    for field in [
        "include_domains",
        "exclude_domains",
        "from_date",
        "to_date",
        "scope",
    ] {
        if let Some(value) = args.get(field) {
            body[field] = value.clone();
        }
    }
    Ok((
        Method::POST,
        "https://api.seltz.ai/v1",
        "/search".into(),
        Some(body),
        vec![],
        ("x-api-key", key.into()),
        Duration::from_secs(15),
    ))
}

pub(super) fn unwrap(value: &Value, max_results: usize) -> Value {
    let results = value
        .get("documents")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .take(max_results)
                .map(|item| {
                    let mut item = item.clone();
                    if item
                        .get("title")
                        .and_then(Value::as_str)
                        .is_none_or(|title| title.trim().is_empty())
                    {
                        item["title"] = json!("Untitled");
                    }
                    item
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    json!({"results":results})
}
