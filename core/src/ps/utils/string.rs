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
