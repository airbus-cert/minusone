pub fn unescape_backtick(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c != '`' {
            result.push(c);
            continue;
        }
        match chars.next() {
            Some('0') => result.push('\0'),
            Some('a') => result.push('\u{7}'),
            Some('b') => result.push('\u{8}'),
            Some('f') => result.push('\u{c}'),
            Some('n') => result.push('\n'),
            Some('r') => result.push('\r'),
            Some('t') => result.push('\t'),
            Some('v') => result.push('\u{b}'),
            Some(other) => result.push(other),
            None => result.push('`'),
        }
    }
    result
}

pub fn unescape_literal_segment(s: &str) -> String {
    unescape_backtick(&s.replace("\"\"", "\""))
}

pub fn escape_string(src: &str) -> String {
    let mut result = String::with_capacity(src.len());
    for c in src.chars() {
        match c {
            '`' => result.push_str("``"),
            '"' => result.push_str("`\""),
            '$' => result.push_str("`$"),
            '\0' => result.push_str("`0"),
            '\u{7}' => result.push_str("`a"),
            '\u{8}' => result.push_str("`b"),
            '\u{c}' => result.push_str("`f"),
            '\n' => result.push_str("`n"),
            '\r' => result.push_str("`r"),
            '\t' => result.push_str("`t"),
            '\u{b}' => result.push_str("`v"),
            _ => result.push(c),
        }
    }
    result
}

// latin + ascii https://en.wikipedia.org/wiki/Code_page#Various_other_Microsoft_code_pages
// UTF-x + unicode https://en.wikipedia.org/wiki/Code_page#Microsoft_Unicode_code_pages
pub fn encoding_codepage_tag(codepage: i64) -> Option<&'static str> {
    match codepage {
        20127 => Some("ascii"),
        65001 => Some("utf8"),
        1200 => Some("unicode"),
        1201 => Some("bigendianunicode"),
        12000 => Some("utf32"),
        28591 => Some("latin1"),
        _ => None,
    }
}

pub fn encoding_name_tag(name: &str) -> Option<&'static str> {
    match name {
        "ascii" | "us-ascii" => Some("ascii"),
        "utf-8" | "utf8" => Some("utf8"),
        "utf-16" | "utf-16le" | "unicode" => Some("unicode"),
        "utf-16be" | "bigendianunicode" => Some("bigendianunicode"),
        "utf-32" | "utf-32le" => Some("utf32"),
        "iso-8859-1" | "latin1" => Some("latin1"),
        _ => None,
    }
}

pub fn decode(tag: &str, bytes: &[u8]) -> Option<String> {
    match tag {
        "ascii" => Some(
            bytes
                .iter()
                .map(|&b| if b < 128 { b as char } else { '?' })
                .collect(),
        ),
        "default" if bytes.iter().all(|&b| b < 128) => {
            Some(bytes.iter().map(|&b| b as char).collect())
        }
        "latin1" => Some(bytes.iter().map(|&b| b as char).collect()),
        "utf8" => String::from_utf8(bytes.to_vec()).ok(),
        "unicode" => {
            if bytes.len() % 2 != 0 {
                return None;
            }
            let units: Vec<u16> = bytes
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            String::from_utf16(&units).ok()
        }
        "bigendianunicode" => {
            if bytes.len() % 2 != 0 {
                return None;
            }
            let units: Vec<u16> = bytes
                .chunks_exact(2)
                .map(|c| u16::from_be_bytes([c[0], c[1]]))
                .collect();
            String::from_utf16(&units).ok()
        }
        "utf32" => {
            if bytes.len() % 4 != 0 {
                return None;
            }
            let mut result = String::with_capacity(bytes.len() / 4);
            for chunk in bytes.chunks_exact(4) {
                let codepoint = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                result.push(char::from_u32(codepoint)?);
            }
            Some(result)
        }
        _ => None,
    }
}

pub fn encode(tag: &str, s: &str) -> Option<Vec<u8>> {
    match tag {
        "ascii" => Some(
            s.chars()
                .map(|c| if (c as u32) < 128 { c as u8 } else { b'?' })
                .collect(),
        ),
        "default" if s.chars().all(|c| (c as u32) < 128) => {
            Some(s.chars().map(|c| c as u8).collect())
        }
        "latin1" => Some(
            s.chars()
                .map(|c| if (c as u32) <= 0xFF { c as u8 } else { b'?' })
                .collect(),
        ),
        "utf8" => Some(s.as_bytes().to_vec()),
        "unicode" => Some(s.encode_utf16().flat_map(|u| u.to_le_bytes()).collect()),
        "bigendianunicode" => Some(s.encode_utf16().flat_map(|u| u.to_be_bytes()).collect()),
        "utf32" => Some(s.chars().flat_map(|c| (c as u32).to_le_bytes()).collect()),
        _ => None,
    }
}

pub fn is_line_ending_char(c: char) -> bool {
    matches!(
        c,
        '\r' | '\n' | '\u{0B}' | '\u{0C}' | '\u{85}' | '\u{2028}' | '\u{2029}'
    )
}

pub fn rfind_char_index(haystack: &[char], needle: &[char]) -> Option<usize> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return None;
    }
    (0..=haystack.len() - needle.len())
        .rev()
        .find(|&i| haystack[i..i + needle.len()] == *needle)
}
