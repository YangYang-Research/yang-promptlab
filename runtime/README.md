# Embedded libllama runtime

Local inference uses **in-process libllama** via `llama-cpp-2` (Rust FFI).

There is no `llama-server` subprocess and no localhost HTTP API.

GGUF models are loaded directly from the model vault at `{data_dir}/models/`.
