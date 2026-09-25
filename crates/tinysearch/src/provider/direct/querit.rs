use super::{Prepared, count, required_string};
use crate::{Error, ExecuteToolRequest, Result};
use reqwest::Method;
use serde_json::{Map, Value, json};
use std::time::Duration;

pub(super) fn prepare(request: &ExecuteToolRequest, key: &str) -> Result<Prepared> {
    if request.name != "querit_search" {
        return Err(Error::UnavailableTool(request.name.clone()));
    }
    let args = &request.arguments;
    let mut count_args = args.clone();
    if count_args.get("max_results").is_none()
        && let Some(alias) = args.get("count")
    {
        count_args["max_results"] = alias.clone();
    }
    let mut body =
        json!({"query":required_string(args,"query")?,"count":count(&count_args,"max_results")});
    if let Some(filters) = build_filters(args) {
        body["filters"] = filters;
    }
    Ok((
        Method::POST,
        "https://api.querit.ai/v1",
        "/search".into(),
        Some(body),
        vec![],
        ("Authorization", format!("Bearer {key}")),
        Duration::from_secs(35),
    ))
}

fn build_filters(args: &Value) -> Option<Value> {
    let mut filters = args
        .get("filters")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let mut sites = object_or_include_map(filters.remove("sites"));
    for (src, dst) in [
        ("include_domains", "include"),
        ("exclude_domains", "exclude"),
    ] {
        if let Some(v) = args.get(src).filter(|v| v.is_array()) {
            sites.insert(dst.into(), v.clone());
        }
    }
    if !sites.is_empty() {
        filters.insert("sites".into(), Value::Object(sites));
    }
    let existing_time_range = filters
        .remove("timeRange")
        .or_else(|| filters.remove("time_range"));
    let mut time_range = match existing_time_range.as_ref() {
        Some(Value::Object(map)) => map.clone(),
        Some(Value::String(date)) if !date.trim().is_empty() => {
            Map::from_iter([("date".into(), json!(date.trim()))])
        }
        _ => Map::new(),
    };
    let date = args
        .get("time_range")
        .or_else(|| args.get("date"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .or_else(|| {
            let from = args
                .get("from_date")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty());
            let to = args
                .get("to_date")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty());
            match (from, to) {
                (Some(a), Some(b)) => Some(format!("{a}to{b}")),
                (Some(a), None) | (None, Some(a)) => Some(a.into()),
                _ => None,
            }
        });
    if let Some(date) = date {
        time_range.insert("date".into(), json!(date));
    }
    if !time_range.is_empty() {
        filters.insert("timeRange".into(), Value::Object(time_range));
    } else if let Some(other) = existing_time_range {
        filters.insert("timeRange".into(), other);
    }
    let mut geo = filters
        .remove("geo")
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();
    let mut countries = object_or_include_map(geo.remove("countries"));
    if let Some(v) = args.get("countries").filter(|v| v.is_array()) {
        countries.insert("include".into(), v.clone());
    }
    if !countries.is_empty() {
        geo.insert("countries".into(), Value::Object(countries));
    }
    if !geo.is_empty() {
        filters.insert("geo".into(), Value::Object(geo));
    }
    let mut languages = object_or_include_map(filters.remove("languages"));
    if let Some(v) = args.get("languages").filter(|v| v.is_array()) {
        languages.insert("include".into(), v.clone());
    }
    if !languages.is_empty() {
        filters.insert("languages".into(), Value::Object(languages));
    }
    (!filters.is_empty()).then_some(Value::Object(filters))
}

fn object_or_include_map(value: Option<Value>) -> Map<String, Value> {
    match value {
        Some(Value::Object(map)) => map,
        Some(Value::Array(items)) => Map::from_iter([("include".into(), Value::Array(items))]),
        _ => Map::new(),
    }
}

pub(super) fn unwrap(value: &Value) -> Result<Value> {
    let payload = value
        .pointer("/response_data/aiapi_res")
        .or_else(|| value.get("aiapi_res"))
        .unwrap_or(value);
    let code = payload
        .get("error_code")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    if code != 0 && code != 200 {
        return Err(Error::Provider(format!(
            "Querit returned error_code {code}"
        )));
    }
    let items = payload
        .pointer("/results/result")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let results: Vec<Value> = items
        .into_iter()
        .map(|mut item| {
            if item.get("snippet").and_then(Value::as_str).is_none()
                && let Some(sentences) = item.get("sentence").and_then(Value::as_array)
            {
                let snippet = sentences
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join(" ");
                item["snippet"] = json!(snippet);
            }
            item
        })
        .collect();
    let mut out = Map::new();
    out.insert("results".into(), Value::Array(results));
    if let Some(id) = payload.get("search_id") {
        out.insert("searchId".into(), id.clone());
    }
    Ok(Value::Object(out))
}
