# tinysearch-bus

This crate defines TinySearch's TinyBus names, configuration, tool declarations,
and normalized results. It has no runtime or transport dependency. The
`provider_tool_specs` catalog and `select_tools` presentation function are pure
and usable by a host synchronously.

`BackendConfig.auth_mode` distinguishes session bearer credentials from API keys.
`SearchStatus::InProgress` signals asynchronous research that can be resumed
using the returned `interaction_id` in `provider_data`.
