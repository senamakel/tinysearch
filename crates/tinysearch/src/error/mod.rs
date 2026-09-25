//! Search validation and routing errors.
/// Errors returned by the `TinySearch` service.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// Search has been disabled in module configuration.
    #[error("search is disabled")]
    Disabled,
    /// The requested tool is not currently advertised.
    #[error("unknown or unavailable tool: {0}")]
    UnavailableTool(String),
    /// The selected provider is not currently available.
    #[error("unknown or unavailable provider: {0}")]
    UnavailableProvider(String),
    /// Tool arguments must be an object.
    #[error("tool arguments must be a JSON object")]
    InvalidArguments,
    /// An argument is not in the selected tool schema.
    #[error("unsupported tool argument: {0}")]
    UnsupportedArgument(String),
    /// A configured provider is required for this presentation.
    #[error("presentation requires a provider")]
    MissingProvider,
    /// The provider rejected a request.
    #[error("provider request failed: {0}")]
    Provider(String),
}
/// Standard result type.
pub type Result<T> = std::result::Result<T, Error>;
#[cfg(test)]
mod test;
