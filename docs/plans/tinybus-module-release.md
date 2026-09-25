# TinySearch module release validation

1. Build the `tinysearch` cdylib for each release target.
2. Package the module and manifest with the standard platform asset name.
3. Load each package through the TinyBus host.
4. Call `ListTools` and confirm the response decodes.
