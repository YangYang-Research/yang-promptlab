//! Real in-process llama.cpp GGUF inference demo (no mocks, no subprocess).
//!
//! Run:
//!   MODEL=/path/to/model.gguf \
//!     cargo run -p aisec-models --features llama --example run_local_model

use aisec_models::{InferenceRequest, InferenceRuntime, LlamaInProcessRuntime, LlamaModelConfig};

#[tokio::main]
async fn main() {
    let model = std::env::var("MODEL").expect("set MODEL=/path/to/model.gguf");
    let prompt = std::env::var("PROMPT").unwrap_or_else(|_| {
        "<|im_start|>user\nIn one sentence, what is prompt injection?<|im_end|>\n<|im_start|>assistant\n".into()
    });

    let mut runtime = LlamaInProcessRuntime::new(LlamaModelConfig::default());

    println!("=== AISec Local Model Runtime (in-process llama.cpp) ===");
    println!("GGUF: {model}");
    runtime
        .load_model(std::path::Path::new(&model))
        .await
        .expect("model load failed");
    println!("State after load: {:?}\n", runtime.state());

    let response = runtime
        .complete(InferenceRequest {
            system: None,
            prompt: prompt.clone(),
            max_tokens: 96,
            temperature: 0.0,
        })
        .await
        .expect("inference failed");

    println!("Prompt:\n{prompt}\n");
    println!("--- Model output (real inference) ---");
    println!("{}", response.text.trim());
    println!(
        "\nTokens generated: {}   Duration: {} ms",
        response.tokens_predicted, response.duration_ms
    );

    runtime.unload().await.ok();
    println!("State after unload: {:?}", runtime.state());
}
