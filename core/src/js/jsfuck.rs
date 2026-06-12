//! JSFuck encoder.
//!
//! Turns an arbitrary string into an equivalent JavaScript expression written
//! using only the six characters `[]()!+`. Evaluating the expression yields the
//! original string back, so it is the inverse of the JSFuck *de*obfuscation the
//! rest of this crate performs.
//!
//! Two encoders coexist, selected per character by [`CHARS`]:
//!
//! * A leveled, no-`eval` builder ported from the StackOverflow answer at
//!   <https://stackoverflow.com/a/63713987>. Each character is reconstructed by
//!   indexing into the string forms of `true`/`false`/`undefined`/`NaN`/
//!   `Infinity`, the names of built-in constructors, `Number.prototype.toString`
//!   in a high radix, and `RegExp` source text. The `level` of a character is
//!   how deep that derivation is; higher levels are valid but more expensive for
//!   the JS engine to evaluate.
//! * A universal level-9 builder that wraps the character in
//!   `[]["flat"]["constructor"]("return \"\uXXXX\"")()`. This is the one place a
//!   `Function`-constructor "eval" is allowed, and it covers every code point
//!   the leveled table cannot reach (the full alphabet test relies on it).
//!
//! The output is dramatically shorter than classic JSFuck (jsfuck.com), which
//! spells every character through `constructor`/`Function` chains, while staying
//! `eval`-free below level 9.

use log::trace;
use regex::{Captures, Regex};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

// Re-encoding the same characters over and over is very slow at level 7+, and
// recompiling the regexes on every call is slow too, so both are memoized.
static CHAR_MAP: OnceLock<HashMap<char, (u8, &'static str)>> = OnceLock::new();
static CACHE: OnceLock<Mutex<HashMap<(char, u8), String>>> = OnceLock::new();
static RE_QUOTED: OnceLock<Regex> = OnceLock::new();
static RE_BRACKET_DIGIT: OnceLock<Regex> = OnceLock::new();
static RE_DIGIT: OnceLock<Regex> = OnceLock::new();

/// The highest level the universal `Function`-based builder lives at. Encoding
/// below this level never emits an `eval`/`Function` construct.
pub const EVAL_LEVEL: u8 = 9;

type CharMap = HashMap<char, (u8, &'static str)>;

/// `+!+[]` repeated `n` times builds the integer `n`; `+[]` builds `0`.
fn primitive_number(n: u32) -> String {
    if n == 0 {
        "+[]".to_string()
    } else {
        "+!+[]".repeat(n as usize)
    }
}

fn char_map() -> &'static CharMap {
    CHAR_MAP.get_or_init(|| CHARS.iter().map(|(c, l, t)| (*c, (*l, *t))).collect())
}

fn cache() -> &'static Mutex<HashMap<(char, u8), String>> {
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Encode `input` into a JSFuck expression, building characters up to and
/// including `max_level`.
///
/// Characters the table cannot reach at `max_level` are kept verbatim as a
/// quoted JS string literal, except at [`EVAL_LEVEL`] where the universal
/// builder encodes everything. Adjacent literal characters are merged into a
/// single string literal to keep the output compact.
pub fn encode(input: &str, max_level: u8) -> String {
    if input.is_empty() {
        return "[]+[]".to_string(); // ""
    }

    let map = char_map();
    let mut parts: Vec<String> = Vec::new();
    let mut literal = String::new();

    let flush = |literal: &mut String, parts: &mut Vec<String>| {
        if !literal.is_empty() {
            parts.push(js_string_literal(literal));
            literal.clear();
        }
    };

    for c in input.chars() {
        let buildable = map.get(&c).filter(|(level, _)| *level <= max_level);
        if let Some((_, template)) = buildable {
            flush(&mut literal, &mut parts);
            parts.push(memoize(c, max_level, || {
                format!("({})", resolve(template, map, c))
            }));
        } else if max_level >= EVAL_LEVEL {
            flush(&mut literal, &mut parts);
            parts.push(memoize(c, max_level, || format!("({})", level9(c, map))));
        } else {
            literal.push(c);
        }
    }
    flush(&mut literal, &mut parts);

    parts.join("+")
}

fn memoize(c: char, level: u8, build: impl FnOnce() -> String) -> String {
    cache()
        .lock()
        .unwrap()
        .entry((c, level))
        .or_insert_with(build)
        .clone()
}

/// Expand a template from [`CHARS`] into pure JSFuck.
///
/// Templates are written in a readable shorthand — embedded `"strings"`, the
/// tokens `Infinity`/`RegExp`/`undefined`/`true`/`false`/`NaN`, and bare decimal
/// digits — which this function rewrites into the six-character alphabet. String
/// literals are spelled out character by character (recursively through
/// [`char_spell`]), so the whole derivation bottoms out in `[]()!+`.
pub fn resolve(template: &str, map: &CharMap, c: char) -> String {
    let start = Instant::now();
    let re_quoted = RE_QUOTED.get_or_init(|| Regex::new(r#""([^"]*)""#).unwrap());
    let re_bracket_digit = RE_BRACKET_DIGIT.get_or_init(|| Regex::new(r"\[(\d)]").unwrap());
    let re_digit = RE_DIGIT.get_or_init(|| Regex::new(r"\d").unwrap());

    let mut s = template.to_string();

    // Spell out quoted strings, then the constructor-bearing tokens, until the
    // expression no longer contains either. The guard catches a template that
    // somehow fails to converge instead of looping forever.
    let mut guard = 1_000_000i64;
    loop {
        if s.contains('"') {
            s = re_quoted
                .replace_all(&s, |m: &Captures| format!("({})", char_spell(&m[1], map)))
                .into_owned();
        } else if s.contains("Infinity") || s.contains("RegExp") {
            s = s.replace("Infinity", r#"(+("1e1000"))"#);
            s = s.replace(
                "RegExp",
                r#"([]["flat"]["constructor"]("return RegExp")())"#,
            );
        } else {
            break;
        }
        guard -= 1;
        if guard < 0 {
            panic!("resolve did not converge for template of {c:?}");
        }
    }

    s = s.replace("undefined", "[][[]]");
    s = s.replace("false", "![]");
    s = s.replace("true", "!![]");
    s = s.replace("NaN", "+[![]]");

    // Number shorthand: a bracketed digit `[d]` becomes `[<jsfuck d>]`, then any
    // remaining bare digit becomes its JSFuck integer form.
    s = re_bracket_digit
        .replace_all(&s, |m: &Captures| {
            format!("[{}]", primitive_number(m[1].parse().unwrap()))
        })
        .into_owned();
    s = re_digit
        .replace_all(&s, |m: &Captures| primitive_number(m[0].parse().unwrap()))
        .into_owned();

    trace!("resolved {c:?} in {:?}", start.elapsed());
    s
}

/// Spell a literal string by encoding each of its characters through the table
/// and concatenating them with `+`. Every character appearing inside a template
/// string literal is itself buildable, so this terminates.
pub fn char_spell(s: &str, map: &CharMap) -> String {
    s.chars()
        .map(|c| {
            let (_, template) = map
                .get(&c)
                .unwrap_or_else(|| panic!("char {c:?} used in a template is not buildable"));
            format!("({})", resolve(template, map, c))
        })
        .collect::<Vec<_>>()
        .join("+")
}

/// Universal level-9 builder: `Function("return \"\uXXXX\"")()` expressed as
/// pure JSFuck. This is the only encoder that relies on a `Function`-constructor
/// "eval", and it can produce any character.
///
/// The function body is spelled out with [`char_spell`] rather than embedded in
/// a `resolve` template, because `resolve` reserves `"` as its "spell this
/// string" delimiter. Delimiting the body with `"` (built via the table's
/// double-quote entry) keeps the single quote out of the hot path entirely.
fn level9(c: char, map: &CharMap) -> String {
    let start = Instant::now();
    let cc = c as u32;
    let escape = if cc <= 0xFFFF {
        format!("\\u{cc:04x}")
    } else {
        format!("\\u{{{cc:x}}}")
    };
    // `return"\uXXXX"` — no space needed before the string literal.
    let body = format!("return\"{escape}\"");
    let function = resolve(r#"[]["flat"]["constructor"]"#, map, c);
    let result = format!("{function}({})()", char_spell(&body, map));
    trace!("resolved level9 {c:?} in {:?}", start.elapsed());
    result
}

/// Escape `s` and wrap it in single quotes, producing a literal JS string. Used
/// for characters that cannot be reconstructed below [`EVAL_LEVEL`].
fn js_string_literal(s: &str) -> String {
    let mut out = String::from("'");
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\'' => out.push_str("\\'"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out.push('\'');
    out
}

/// Per-character derivations, as `(character, level, template)`.
///
/// Ported from <https://stackoverflow.com/a/63713987>. The template language is
/// the shorthand understood by [`resolve`]. Levels 1-8 are `eval`-free; the
/// universal level-9 builder in [`level9`] handles everything not listed here.
pub const CHARS: &[(char, u8, &str)] = &[
    ('0', 1, r#"0+[]"#),
    ('1', 1, r#"1+[]"#),
    ('2', 1, r#"2+[]"#),
    ('3', 1, r#"3+[]"#),
    ('4', 1, r#"4+[]"#),
    ('5', 1, r#"5+[]"#),
    ('6', 1, r#"6+[]"#),
    ('7', 1, r#"7+[]"#),
    ('8', 1, r#"8+[]"#),
    ('9', 1, r#"9+[]"#),
    ('a', 1, r#"(false+[])[1]"#),
    ('d', 1, r#"(undefined+[])[2]"#),
    ('e', 1, r#"(true+[])[3]"#),
    ('f', 1, r#"(false+[])[0]"#),
    ('i', 1, r#"([false]+undefined)[1+[0]]"#),
    ('l', 1, r#"(false+[])[2]"#),
    ('n', 1, r#"(undefined+[])[1]"#),
    ('r', 1, r#"(true+[])[1]"#),
    ('s', 1, r#"(false+[])[3]"#),
    ('t', 1, r#"(true+[])[0]"#),
    ('u', 1, r#"(undefined+[])[0]"#),
    ('N', 1, r#"(NaN+[])[0]"#),
    (' ', 2, r#"([false]+[]["flat"])[2+[0]]"#),
    ('(', 2, r#"([]+[]["flat"])[1+[3]]"#),
    (')', 2, r#"([]+[]["flat"])[1+[4]]"#),
    ('+', 2, r#"(+("11e100")+[])[4]"#),
    ('.', 2, r#"(+("11e100")+[])[1]"#),
    ('[', 2, r#"([]+[]["entries"]())[0]"#),
    (']', 2, r#"([]+[]["entries"]())[2+[2]]"#),
    ('{', 2, r#"([true]+[]["flat"])[2+[0]]"#),
    ('c', 2, r#"([]["flat"]+[])[3]"#),
    ('j', 2, r#"([]+[]["entries"]())[3]"#),
    ('o', 2, r#"([true]+[]["flat"])[1+[0]]"#),
    ('y', 2, r#"(true+[Infinity])[1+[1]]"#),
    ('A', 2, r#"([NaN]+([]+[]["entries"]()))[1+[1]]"#),
    ('I', 2, r#"(Infinity+[])[0]"#),
    // `"".link()` is `<a href="undefined"></a>`; index 8 is the double quote.
    // Unlike the function-source tricks below it does not depend on how the
    // engine spaces `function name(){`.
    ('"', 2, r#"(([]+[])["link"]())[8]"#),
    ('-', 3, r#"(+(".0000001")+[])[2]"#),
    ('b', 3, r#"([]+(+[])["constructor"])[1+[2]]"#),
    ('g', 3, r#"(false+[0]+([]+[])["constructor"])[2+[0]]"#),
    ('m', 3, r#"([]+(+[])["constructor"])[1+[1]]"#),
    ('B', 3, r#"([NaN]+(![])["constructor"])[1+[2]]"#),
    ('F', 3, r#"([NaN]+[]["flat"]["constructor"])[1+[2]]"#),
    ('S', 3, r#"([NaN]+([]+[])["constructor"])[1+[2]]"#),
    ('h', 4, r#"(+(1+[0]+[1]))["toString"](2+[1])[1]"#),
    ('k', 4, r#"(+(2+[0]))["toString"](2+[1])"#),
    ('p', 4, r#"(+(2+[1]+[1]))["toString"](3+[1])[1]"#),
    ('q', 4, r#"(+(2+[1]+[2]))["toString"](3+[1])[1]"#),
    ('v', 4, r#"(+(3+[1]))["toString"](3+[2])"#),
    ('w', 4, r#"(+(3+[2]))["toString"](3+[3])"#),
    ('x', 4, r#"(+(1+[0]+[1]))["toString"](3+[4])[1]"#),
    ('z', 4, r#"(+(3+[5]))["toString"](3+[6])"#),
    ('}', 4, r#"([true]+[]["flat"])["slice"]("-1")"#),
    (
        'E',
        5,
        r#"([false]+[]["flat"]["constructor"]("try{String().normalize(false)}catch(f){return f}")())[1+[0]]"#,
    ),
    (
        'R',
        5,
        r#"([]+[]["flat"]["constructor"]("try{String().normalize(false)}catch(f){return f}")())[0]"#,
    ),
    (
        '/',
        6,
        r#"([]+[]["flat"]["constructor"]("return RegExp")()())[0]"#,
    ),
    (
        ':',
        6,
        r#"([]+[]["flat"]["constructor"]("return RegExp")()())[3]"#,
    ),
    (
        '?',
        6,
        r#"([]+[]["flat"]["constructor"]("return RegExp")()())[2]"#,
    ),
    ('\\', 7, r#"([]+RegExp("/"))[1]"#),
    // The single quote (`'`) is intentionally absent: the original
    // StackOverflow trick reads it out of a `Function` SyntaxError message,
    // which is engine-specific (it depends on how the engine renders
    // `function name(){`). It is instead produced by the universal level-9
    // builder, which delimits its body with `"` and never needs `'`.
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;

    /// Run `src` through node and return trimmed stdout, asserting a clean exit.
    /// The script is written to a temp file because level-9 expansions are far
    /// too large to pass as a `node -e` argument.
    fn node_eval(src: &str) -> String {
        let path = std::env::temp_dir().join(format!(
            "minusone_jsfuck_{}_{}.js",
            std::process::id(),
            COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::write(&path, src).unwrap();
        let output = Command::new("node")
            .arg(&path)
            .output()
            .expect("node must be installed to run the JSFuck encoder tests");
        let _ = fs::remove_file(&path);
        assert!(
            output.status.success(),
            "node exited with failure:\nstderr: {}",
            String::from_utf8_lossy(&output.stderr),
        );
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    /// Assert the JSFuck output uses only the six allowed characters.
    fn assert_pure_jsfuck(encoded: &str) {
        for c in encoded.chars() {
            assert!(
                "[]()!+".contains(c),
                "encoding leaked a non-JSFuck character {c:?}: {encoded}"
            );
        }
    }

    #[test]
    fn encodes_full_alphabet_at_level_9() {
        let chars = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!@#$%^&*()_+-=[]{}|;:\",./<>?€";

        // Build one script that prints each decoded character (JSON-encoded, one
        // per line), then compare to the source character. Going through node
        // guarantees the expressions actually evaluate back to what we asked
        // for. `JSON.stringify` on node and Rust's debug formatting agree on the
        // escaping for every character in this set.
        let mut script = String::new();
        for c in chars.chars() {
            let encoded = encode(&c.to_string(), 9);
            assert_pure_jsfuck(&encoded);
            script.push_str(&format!("console.log(JSON.stringify({encoded}));"));
        }

        let output = node_eval(&script);
        let decoded: Vec<&str> = output.lines().collect();
        let expected: Vec<String> = chars
            .chars()
            .map(|c| format!("{:?}", c.to_string()))
            .collect();
        assert_eq!(expected.len(), decoded.len());
        for (exp, got) in expected.iter().zip(decoded.iter()) {
            assert_eq!(exp, got);
        }
    }

    #[test]
    fn encodes_multi_character_strings() {
        for sample in ["alert(1)", "console.log('pwn')", "Hello, World!", "€uro"] {
            let encoded = encode(sample, 9);
            assert_pure_jsfuck(&encoded);
            let decoded = node_eval(&format!("process.stdout.write({encoded})"));
            assert_eq!(sample, decoded);
        }
    }

    #[test]
    fn empty_string_round_trips() {
        let encoded = encode("", 9);
        assert_eq!("[]+[]", encoded);
        assert_eq!("", node_eval(&format!("process.stdout.write({encoded})")));
    }

    #[test]
    fn below_eval_level_keeps_unreachable_chars_literal() {
        // '\'' is a level-8 character, so at level 1 it stays a quoted literal
        // rather than being reconstructed.
        let encoded = encode("a'", 1);
        assert!(
            encoded.contains("'\\''"),
            "expected a literal quote: {encoded}"
        );
        assert_eq!("a'", node_eval(&format!("process.stdout.write({encoded})")));
    }

    #[test]
    fn beats_naive_per_char_constructor_spelling() {
        // The leveled table must be no worse than spelling a word out character
        // by character through char_spell (the classic-JSFuck-style approach),
        // and in practice it is far shorter because of memoized subexpressions.
        let map = char_map();
        let word = "constructor";
        let leveled = encode(word, 9).len();
        let naive = char_spell(word, map).len();
        assert!(
            leveled <= naive,
            "leveled encoding ({leveled}) should not exceed naive spelling ({naive})"
        );
    }

    /// Encoding the same character many times must hit the cache and stay fast.
    #[test]
    fn repeated_chars_are_cached() {
        let repeated = "€".repeat(64);
        let encoded = encode(&repeated, 9);
        assert_pure_jsfuck(&encoded);
        // Each occurrence resolves to the identical cached subexpression.
        let single = encode("€", 9);
        assert_eq!(encoded, vec![single; 64].join("+"));
    }
}
