//! Embedded PromptLab brand assets for report templates.

/// PromptLab mark (teal flask on black squircle), PNG.
pub const LOGO_PNG: &[u8] = include_bytes!("../assets/promptlab-mark.png");

/// `data:image/png;base64,...` for self-contained HTML reports.
pub fn logo_data_uri() -> String {
    format!("data:image/png;base64,{}", encode_base64(LOGO_PNG))
}

fn encode_base64(data: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[((n >> 18) & 0x3f) as usize] as char);
        out.push(TABLE[((n >> 12) & 0x3f) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[((n >> 6) & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(n & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logo_png_is_valid() {
        assert!(LOGO_PNG.starts_with(&[0x89, b'P', b'N', b'G']));
        let uri = logo_data_uri();
        assert!(uri.starts_with("data:image/png;base64,"));
        assert!(uri.len() > 100);
    }
}
