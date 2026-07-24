//! GGUF model detection and quantization classification.

use std::path::Path;

use crate::error::{RuntimeError, RuntimeResult};

/// Supported GGUF quantization levels for embedded inference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GgufQuantization {
    Q4,
    Q5,
    Q6,
    Q8,
    Unknown,
}

impl GgufQuantization {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Q4 => "Q4",
            Self::Q5 => "Q5",
            Self::Q6 => "Q6",
            Self::Q8 => "Q8",
            Self::Unknown => "unknown",
        }
    }

    pub fn is_supported(self) -> bool {
        !matches!(self, Self::Unknown)
    }
}

/// Detect quantization from a GGUF filename (case-insensitive).
pub fn detect_quantization(path: &Path) -> GgufQuantization {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    if name.contains("q8_") || name.contains(".q8") || name.contains("q8_k") {
        return GgufQuantization::Q8;
    }
    if name.contains("q6_") || name.contains(".q6") || name.contains("q6_k") {
        return GgufQuantization::Q6;
    }
    if name.contains("q5_") || name.contains(".q5") || name.contains("q5_k") {
        return GgufQuantization::Q5;
    }
    if name.contains("q4_") || name.contains(".q4") || name.contains("q4_k") {
        return GgufQuantization::Q4;
    }
    GgufQuantization::Unknown
}

/// Validate that `path` is a readable GGUF file with a supported quantization.
pub fn validate_gguf_model(path: &Path) -> RuntimeResult<GgufQuantization> {
    if !path.is_file() {
        return Err(RuntimeError::Config(format!(
            "model not found: {}",
            path.display()
        )));
    }

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if ext != "gguf" {
        return Err(RuntimeError::Config(format!(
            "expected .gguf file, got {}",
            path.display()
        )));
    }

    let quant = detect_quantization(path);
    Ok(quant)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_quant_levels() {
        assert_eq!(
            detect_quantization(Path::new("qwen3-8b-q4_k_m.gguf")),
            GgufQuantization::Q4
        );
        assert_eq!(
            detect_quantization(Path::new("model.Q5_K_S.gguf")),
            GgufQuantization::Q5
        );
        assert_eq!(
            detect_quantization(Path::new("model-q6_k.gguf")),
            GgufQuantization::Q6
        );
        assert_eq!(
            detect_quantization(Path::new("model-q8_0.gguf")),
            GgufQuantization::Q8
        );
    }
}
