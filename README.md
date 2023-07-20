Early WIP
Currently authentication, document sync, completion requests/responses are functional. Editor context is forwarded to github copilot servers and completions are provided to the editor in response. These results can be retrieved by requesting `textDocument/completion`, but currently lack the extra information used by copilot.lua and copilot-cmp.
