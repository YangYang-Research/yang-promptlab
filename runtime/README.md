# Runtime notes

PromptLab AI Runtime is **remote-only**. Inference goes to third-party HTTP providers
(OpenAI, Anthropic, Gemini, Azure, Bedrock, OpenRouter, custom OpenAI-compatible including
Ollama over HTTP). Embedded llama.cpp / in-process GGUF has been removed.

See [docs/RUNTIME.md](../docs/RUNTIME.md).
