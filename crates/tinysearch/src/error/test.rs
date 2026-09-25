//! Error behavior tests.
use super::Error;
#[test]
fn errors_have_actionable_messages() {
    assert!(
        Error::UnavailableTool("missing".into())
            .to_string()
            .contains("missing")
    );
    assert_eq!(
        Error::InvalidArguments.to_string(),
        "tool arguments must be a JSON object"
    );
}
