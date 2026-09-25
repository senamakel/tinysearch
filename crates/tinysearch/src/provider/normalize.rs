//! Bounded normalization of provider payloads.
use super::{MAX_ANSWER_CHARS, MAX_CITATIONS, MAX_RESULTS};
use crate::{Citation, ExecuteToolResponse, SearchResult, SearchStatus};
use serde_json::{Map, Value};

fn clipped(text: &str, max: usize) -> String {
    text.chars().take(max).collect()
}
fn get_text(value: &Value, field: &str) -> Option<String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(|s| clipped(s, MAX_ANSWER_CHARS))
}
fn add_result(results: &mut Vec<SearchResult>, item: &Value) {
    if results.len() >= MAX_RESULTS {
        return;
    }
    let url = item.get("url").and_then(Value::as_str).unwrap_or("");
    if url.is_empty() {
        return;
    }
    let title = item.get("title").and_then(Value::as_str).unwrap_or("");
    let snippet = item
        .get("snippet")
        .and_then(Value::as_str)
        .or_else(|| item.get("summary").and_then(Value::as_str))
        .or_else(|| {
            item.get("highlights")
                .and_then(Value::as_array)
                .and_then(|a| a.first())
                .and_then(Value::as_str)
        })
        .or_else(|| item.get("content").and_then(Value::as_str))
        .or_else(|| item.get("raw_content").and_then(Value::as_str))
        .or_else(|| {
            item.get("excerpts")
                .and_then(Value::as_array)
                .and_then(|a| a.first())
                .and_then(Value::as_str)
        })
        .or_else(|| item.get("description").and_then(Value::as_str))
        .or_else(|| item.get("full_content").and_then(Value::as_str))
        .or_else(|| item.get("text").and_then(Value::as_str));
    results.push(SearchResult {
        title: clipped(title, 300),
        url: clipped(url, 2048),
        snippet: snippet.map(|s| clipped(s, 1200)),
        published: item
            .get("published_date")
            .or_else(|| item.get("publish_date"))
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .map(|s| clipped(s, 100)),
    });
}
fn add_citation(citations: &mut Vec<Citation>, url: &str, title: Option<&str>) {
    if citations.len() < MAX_CITATIONS && !url.is_empty() && !citations.iter().any(|c| c.url == url)
    {
        citations.push(Citation {
            url: clipped(url, 2048),
            title: title.map(|s| clipped(s, 300)),
        });
    }
}
fn answer_for(tool: &str, value: &Value) -> Option<String> {
    match tool {
        "parallel_chat" => value
            .pointer("/choices/0/message/content")
            .and_then(Value::as_str)
            .map(|s| clipped(s, MAX_ANSWER_CHARS)),
        "gemini_agentic_search" => value
            .pointer("/candidates/0/content/parts/0/text")
            .and_then(Value::as_str)
            .map(|s| clipped(s, MAX_ANSWER_CHARS)),
        "gemini_deep_research" => value
            .get("steps")
            .and_then(Value::as_array)
            .and_then(|a| a.last())
            .and_then(|v| v.get("content"))
            .and_then(Value::as_array)
            .and_then(|a| a.first())
            .and_then(|v| v.get("text"))
            .and_then(Value::as_str)
            .or_else(|| value.get("output_text").and_then(Value::as_str))
            .map(|s| clipped(s, MAX_ANSWER_CHARS)),
        "parallel_research"
        | "parallel_enrich"
        | "parallel_dataset"
        | "parallel_research_status"
        | "parallel_enrich_status"
        | "parallel_dataset_status"
        | "tinyfish_agent_run" => value
            .get("result")
            .or_else(|| value.get("output"))
            .map(|v| clipped(v.as_str().unwrap_or(&v.to_string()), MAX_ANSWER_CHARS)),
        "tinyfish_fetch" => value
            .get("results")
            .and_then(Value::as_array)
            .and_then(|a| a.first())
            .and_then(|v| v.get("text"))
            .map(|v| clipped(v.as_str().unwrap_or(&v.to_string()), MAX_ANSWER_CHARS)),
        _ => get_text(value, "answer"),
    }
}

pub(super) fn normalize(provider: &str, tool: &str, value: &Value) -> ExecuteToolResponse {
    let mut results = Vec::new();
    let mut citations = Vec::new();
    if let Some(items) = value.get("results").and_then(Value::as_array) {
        for item in items {
            add_result(&mut results, item);
            if results.len() == MAX_RESULTS {
                break;
            }
        }
    }
    for result in &results {
        add_citation(&mut citations, &result.url, Some(&result.title));
    }
    if let Some(items) = value
        .get("basis")
        .and_then(Value::as_array)
        .or_else(|| value.get("citations").and_then(Value::as_array))
    {
        for item in items.iter().take(MAX_CITATIONS) {
            if let Some(url) = item.get("url").and_then(Value::as_str) {
                add_citation(
                    &mut citations,
                    url,
                    item.get("title").and_then(Value::as_str),
                );
            }
        }
    }
    if let Some(steps) = value.get("steps").and_then(Value::as_array) {
        for step in steps.iter().rev().take(4) {
            if let Some(blocks) = step.get("content").and_then(Value::as_array) {
                for block in blocks.iter().take(8) {
                    if let Some(annotations) = block.get("annotations").and_then(Value::as_array) {
                        for annotation in annotations.iter().take(MAX_CITATIONS) {
                            if let Some(url) = annotation.get("url").and_then(Value::as_str) {
                                add_citation(
                                    &mut citations,
                                    url,
                                    annotation.get("title").and_then(Value::as_str),
                                );
                            }
                        }
                    }
                }
            }
        }
    }
    if let Some(chunks) = value
        .pointer("/candidates/0/groundingMetadata/groundingChunks")
        .and_then(Value::as_array)
    {
        for item in chunks.iter().take(MAX_CITATIONS) {
            if let Some(web) = item.get("web")
                && let Some(url) = web.get("uri").and_then(Value::as_str)
            {
                add_citation(
                    &mut citations,
                    url,
                    web.get("title").and_then(Value::as_str),
                );
            }
        }
    }
    let answer = answer_for(tool, value);
    let mut meta = Map::new();
    for field in [
        "id",
        "findallId",
        "matchLimit",
        "runId",
        "searchId",
        "run_id",
        "findall_id",
        "search_id",
        "extract_id",
        "status",
        "costUsd",
        "total_results",
        "num_of_steps",
        "failed_count",
    ] {
        if let Some(v) = value.get(field)
            && (v.is_number() || v.is_boolean() || v.as_str().is_some_and(|s| s.len() <= 128))
        {
            meta.insert(field.into(), v.clone());
        }
    }
    let status = if results.is_empty() && answer.as_deref().is_none_or(str::is_empty) {
        SearchStatus::Empty
    } else {
        SearchStatus::Ok
    };
    ExecuteToolResponse {
        provider: provider.into(),
        results,
        citations,
        answer,
        status,
        provider_data: (!meta.is_empty()).then_some(Value::Object(meta)),
    }
}
