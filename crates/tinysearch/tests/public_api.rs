//! Public contract smoke tests.
use std::collections::BTreeMap;
use tinysearch::{SearchConfig, SearchService, names};
#[test]
fn host_can_discover_search_interface() {
    assert_eq!(names::methods::LIST_TOOLS, "ListTools");
    assert!(
        SearchService::with_providers(SearchConfig::default(), BTreeMap::new())
            .list_tools()
            .tools
            .is_empty()
    );
}
