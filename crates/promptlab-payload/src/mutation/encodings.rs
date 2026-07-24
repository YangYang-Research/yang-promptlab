use base64::{engine::general_purpose::STANDARD, Engine as _};

/// Cyrillic homoglyph + zero-width character obfuscation.
pub fn unicode_obfuscate(input: &str) -> String {
    let homoglyph = input
        .replace('a', "а")
        .replace('A', "А")
        .replace('e', "е")
        .replace('E', "Е")
        .replace('o', "о")
        .replace('O', "О")
        .replace('i', "і")
        .replace('I', "І")
        .replace('c', "с")
        .replace('C', "С");

    insert_zero_width(&homoglyph, '\u{200b}', 4)
}

fn insert_zero_width(input: &str, zw: char, every: usize) -> String {
    if every == 0 {
        return input.to_string();
    }
    let mut out = String::with_capacity(input.len() + input.len() / every);
    for (i, ch) in input.chars().enumerate() {
        out.push(ch);
        if (i + 1) % every == 0 {
            out.push(zw);
        }
    }
    out
}

/// Standard base64 encoding (no padding strip).
pub fn base64_encode(input: &str) -> String {
    STANDARD.encode(input.as_bytes())
}

/// Lowercase hex encoding of UTF-8 bytes.
pub fn hex_encode(input: &str) -> String {
    input
        .bytes()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join("")
}

/// HTML entity encoding (decimal numeric entities).
pub fn html_encode(input: &str) -> String {
    input
        .chars()
        .map(|c| match c {
            '&' => "&amp;".to_string(),
            '<' => "&lt;".to_string(),
            '>' => "&gt;".to_string(),
            '"' => "&quot;".to_string(),
            '\'' => "&#39;".to_string(),
            c if c.is_ascii() && !c.is_ascii_alphanumeric() && c != ' ' => {
                format!("&#{};", c as u32)
            }
            c => c.to_string(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unicode_obfuscation_changes_string() {
        let out = unicode_obfuscate("ignore rules");
        assert_ne!(out, "ignore rules");
        assert!(out.contains('а') || out.contains('\u{200b}'));
    }

    #[test]
    fn base64_roundtrip_length() {
        let encoded = base64_encode("secret payload");
        assert_eq!(encoded, "c2VjcmV0IHBheWxvYWQ=");
    }

    #[test]
    fn hex_encode_bytes() {
        assert_eq!(hex_encode("AB"), "4142");
    }

    #[test]
    fn html_encode_special_chars() {
        let encoded = html_encode("<script>alert('x')</script>");
        assert!(encoded.contains("&lt;"));
        assert!(encoded.contains("&gt;"));
        assert!(!encoded.contains("<script>"));
    }
}
