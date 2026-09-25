//! Stable `TinyBus` names for `TinySearch`.
/// Interface claimed by the module.
pub const INTERFACE: &str = "ai.tinyhumans.tinysearch.Search";
/// Object path serving the interface.
pub const OBJECT_PATH: &str = "/ai/tinyhumans/tinysearch/Search";
/// Member names.
pub mod methods {
    /// Discover currently available search tools.
    pub const LIST_TOOLS: &str = "ListTools";
    /// Execute a discovered tool.
    pub const EXECUTE_TOOL: &str = "ExecuteTool";
}
/// Members in interface dispatch order.
pub const METHODS: &[&str] = &[methods::LIST_TOOLS, methods::EXECUTE_TOOL];
#[cfg(test)]
mod test;
