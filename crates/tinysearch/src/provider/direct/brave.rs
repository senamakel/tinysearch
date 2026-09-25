use super::{Prepared, count, required_string};
use crate::{Error, ExecuteToolRequest, Result};
use reqwest::Method;
use serde_json::{Value, json};
use std::time::Duration;

pub(super) fn prepare(request: &ExecuteToolRequest, key: &str) -> Result<Prepared> {
    let path = match request.name.as_str() {
        "brave_web_search" => "/web/search",
        "brave_news_search" => "/news/search",
        "brave_image_search" => "/images/search",
        "brave_video_search" => "/videos/search",
        _ => return Err(Error::UnavailableTool(request.name.clone())),
    };
    let mut params = vec![
        (
            "q",
            required_string(&request.arguments, "query")?.to_owned(),
        ),
        ("count", count(&request.arguments, "count").to_string()),
    ];
    if request.name == "brave_web_search" {
        params.push(("result_filter", "web".into()));
    }
    for field in ["country", "freshness"] {
        if let Some(value) = request.arguments.get(field).and_then(Value::as_str) {
            params.push((field, value.into()));
        }
    }
    Ok((
        Method::GET,
        "https://api.search.brave.com/res/v1",
        path.into(),
        None,
        params,
        ("X-Subscription-Token", key.into()),
        Duration::from_secs(35),
    ))
}

pub(super) fn unwrap(request: &ExecuteToolRequest, value: &Value) -> Value {
    let items = if request.name == "brave_web_search" {
        value.pointer("/web/results")
    } else {
        value.get("results")
    };
    let results: Vec<Value> = items
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .cloned()
        .collect();
    json!({"results":results})
}
