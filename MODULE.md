# TinySearch module

The `tinysearch` native module implements TinyBus ABI v1 and serves the
`ai.tinyhumans.tinysearch.Search` interface at
`/ai/tinyhumans/tinysearch/Search`.

`ListTools` returns tools available under the current configuration.
`ExecuteTool` accepts a `name` from that list and provider-specific JSON
`arguments`. Module initialization and reinitialization carry the sensitive
`SearchConfig`, including credentials; tool arguments must never carry those
credentials. Reinitialization replaces the connection and service state.

The typed wire contract is in `tinysearch-bus`.
