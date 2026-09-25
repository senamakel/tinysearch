//! Lists tools configured for a `TinySearch` service.
use std::collections::BTreeMap;
use tinysearch::{SearchConfig, SearchService};
fn main() {
    let service = SearchService::with_providers(SearchConfig::default(), BTreeMap::new());
    println!(
        "{} search tools available",
        service.list_tools().tools.len()
    );
}
