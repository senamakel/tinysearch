# TinySearch

TinySearch is a loadable TinyBus module for web search. `tinysearch-bus` is the
transport-free wire contract; `tinysearch` is the module and provider dispatch
layer. Hosts pass credentials in private module initialization configuration,
which TinyBus can refresh through reinitialization.

The bus serves `ai.tinyhumans.tinysearch.Search` at
`/ai/tinyhumans/tinysearch/Search`. `ListTools` returns currently available
model-facing declarations; `ExecuteTool` invokes a declared tool and returns
normalized results, citations, an optional answer, status, and optional provider
data. The default presentation exposes all available provider tools. Router
mode exposes one `search` tool and selects managed Parallel when available,
then the first enabled provider in stable name order.

A direct provider requires its own credential unless it is explicitly keyless.
A backend route requires a backend credential. Search can be disabled globally.
Provider configuration includes a route, optional base URL, credential,
`max_results`, `timeout_secs`, and a SearXNG `default_language`.
Debug output redacts credentials.

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo build --all-targets --all-features
cargo test --all-features
```

The module crate is `crates/tinysearch`; its compiled `cdylib` can be loaded by
TinyBus. The contract crate is `crates/tinysearch-bus` and has no transport or
HTTP dependencies. `vendor/tinybus` is a pinned git submodule.

## Built-in providers

Managed backend credentials enable Parallel's six operations by default. Add an
explicit `tinyfish` or `gemini` provider entry with `route: "backend"` to
expose those tools. `backend.auth_mode` is `session` (Authorization bearer) or
`api_key` (`x-api-key`); only backend requests receive `x-sdk-name`.

Direct Gemini uses a provider credential and `route: "direct"`. Its
`gemini_agentic_search` operation calls the Gemini `generateContent` API with
Google Search grounding. `gemini_deep_research` calls the asynchronous
Interactions API; it can be resumed with `interaction_id` when the bounded poll
returns `in_progress`. Direct Google calls receive `x-goog-api-key` and never
receive backend attribution or credentials. TinyFish uses the managed backend
route only.

Parallel can also use `route: "direct"` with its own provider credential and
optional `base_url`. Direct requests send `x-api-key` to Parallel's official
`/v1/search`, `/v1/extract`, `/v1beta/chat/completions`, `/v1/tasks/runs`, and
`/v1beta/findall/runs` endpoints. Research and enrichment create Task runs;
dataset creates a FindAll run. They return `in_progress` with `run_id` or
`findall_id` in `provider_data`. Call `parallel_research_status`,
`parallel_enrich_status`, or `parallel_dataset_status` with that ID to check
status and fetch completed results. Each call has a bounded timeout and does
not wait indefinitely. The direct catalog exposes Parallel's current search
mode values (`turbo`, `fast`, `basic`, `advanced`). Direct extract does not
advertise the backend-only `excerpts` switch, and direct async runs do not
advertise the backend-only `timeout_seconds` wait option. Backend tool schemas
and routes remain unchanged.

Exa (`exa_search`, `exa_find_similar`, `exa_get_contents`), Brave web, news,
image, and video search, Querit search, and Tavily search and extract use
`route: "direct"` with a provider credential. Their `base_url` can target a
controlled endpoint for testing. These providers do not have managed backend
routes; unsupported routes and tool arguments are rejected.

Seltz (`seltz_search`) also requires a direct credential. It posts to
`https://api.seltz.ai/v1/search` with `x-api-key`, supports domain and date
filters plus news scope, and returns up to 20 results. SearXNG
(`searxng_search`) is keyless and appears only when the host explicitly enables
it with a `base_url` and a direct route. It requests `/search?format=json`,
maps `web` to the `general` category, uses the configured default language,
and returns up to 50 results with their source names in `provider_data.sources`.

Every response bounds results, citations, snippets, answers, and retained
provider metadata. Upstream error bodies are not returned or logged. Provider
base URL overrides are intended for local testing and controlled deployments.
