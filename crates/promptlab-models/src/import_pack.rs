use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use tracing::info;

use crate::error::{ModelError, ModelResult};
use crate::registry::ModelRegistry;
use crate::types::ModelFormat;

/// Extract the first `.gguf` file from a ZIP package into `dest_dir`.
pub fn extract_gguf_from_zip(archive_path: &Path, dest_dir: &Path) -> ModelResult<PathBuf> {
    let data = std::fs::read(archive_path).map_err(ModelError::Io)?;
    let mut archive =
        zip::ZipArchive::new(Cursor::new(data)).map_err(|e| ModelError::invalid(e.to_string()))?;

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| ModelError::invalid(e.to_string()))?;
        if file.is_dir() {
            continue;
        }
        let name = file.name().to_string();
        let file_name = Path::new(&name)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        if !file_name.to_ascii_lowercase().ends_with(".gguf") {
            continue;
        }

        std::fs::create_dir_all(dest_dir).map_err(ModelError::Io)?;
        let out_path = dest_dir.join(file_name);
        let mut out = std::fs::File::create(&out_path).map_err(ModelError::Io)?;
        std::io::copy(&mut file, &mut out).map_err(ModelError::Io)?;
        info!(path = %out_path.display(), "extracted gguf from zip package");
        return Ok(out_path);
    }

    Err(ModelError::invalid("zip package does not contain a .gguf file"))
}

pub fn validate_gguf_path(path: &Path) -> ModelResult<()> {
    if ModelFormat::from_path(path).is_none() {
        return Err(ModelError::invalid("file must be a .gguf model"));
    }
    if !path.is_file() {
        return Err(ModelError::invalid(format!("file not found: {}", path.display())));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    #[test]
    fn extracts_gguf_from_zip() {
        let dir = tempfile::tempdir().unwrap();
        let zip_path = dir.path().join("model.zip");
        let mut zip = ZipWriter::new(std::fs::File::create(&zip_path).unwrap());
        zip.start_file(
            "weights/model.gguf",
            SimpleFileOptions::default(),
        )
        .unwrap();
        zip.write_all(b"gguf-bytes").unwrap();
        zip.finish().unwrap();

        let out_dir = dir.path().join("out");
        let extracted = extract_gguf_from_zip(&zip_path, &out_dir).unwrap();
        assert!(extracted.ends_with("model.gguf"));
        assert_eq!(std::fs::read(extracted).unwrap(), b"gguf-bytes");
    }
}
