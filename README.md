Early WIP
Currently authentication, document sync, completion requests/responses are functional. Editor context is forwarded to github copilot servers and completions are provided to the editor in response.

These results can be retrieved by requesting `textDocument/getCompletionsCycling`

You can use this language server by by checking out the `copilot-rs` branch on both `copilot.lua` and `copilot-cmp`. Please note that this is an early stage project, and bugs which break functionality are to be expected.
