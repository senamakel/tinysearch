# TinySearch module release

A release packages the `tinysearch` cdylib and its manifest for each supported
platform. The module serves `ListTools` and `ExecuteTool` under the
`ai.tinyhumans.tinysearch.Search` interface. Release verification loads the
asset through TinyBus and calls `ListTools` over an in-memory bus.
