use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use minusone::trace::Step;

// how many steps between full-text snapshots ("keyframes")
const KEYFRAME_INTERVAL: usize = 25;

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

fn diff_span<'a>(prev: &'a str, cur: &'a str) -> (usize, usize, &'a str, &'a str) {
    let prev_b = prev.as_bytes();
    let cur_b = cur.as_bytes();
    let max_common = prev_b.len().min(cur_b.len());

    let mut prefix = 0;
    while prefix < max_common && prev_b[prefix] == cur_b[prefix] {
        prefix += 1;
    }
    while prefix > 0 && (!prev.is_char_boundary(prefix) || !cur.is_char_boundary(prefix)) {
        prefix -= 1;
    }

    let max_suffix = max_common - prefix;
    let mut suffix = 0;
    while suffix < max_suffix && prev_b[prev_b.len() - 1 - suffix] == cur_b[cur_b.len() - 1 - suffix]
    {
        suffix += 1;
    }
    while suffix > 0
        && (!prev.is_char_boundary(prev_b.len() - suffix)
            || !cur.is_char_boundary(cur_b.len() - suffix))
    {
        suffix -= 1;
    }

    let dstart = prefix;
    let dend = prev_b.len() - suffix;
    let cend = cur_b.len() - suffix;
    (dstart, dend, &prev[dstart..dend], &cur[prefix..cend])
}

fn common_prefix_len(a: &str, b: &str) -> usize {
    let ab = a.as_bytes();
    let bb = b.as_bytes();
    let max = ab.len().min(bb.len());
    let mut n = 0;
    while n < max && ab[n] == bb[n] {
        n += 1;
    }
    while n > 0 && (!a.is_char_boundary(n) || !b.is_char_boundary(n)) {
        n -= 1;
    }
    n
}

fn locate_node_diff(prev_full: &str, cur_full: &str, old: &str, new: &str) -> Option<(usize, usize)> {
    let shared_lead = common_prefix_len(old, new);
    let raw_prefix = common_prefix_len(prev_full, cur_full);
    let dstart = raw_prefix.checked_sub(shared_lead)?;
    let dend = dstart.checked_add(old.len())?;
    if prev_full.get(dstart..dend) == Some(old) {
        Some((dstart, dend))
    } else {
        None
    }
}

fn find_nearest_occurrence(hay: &str, needle: &str, hint: usize) -> Option<(usize, usize)> {
    if needle.is_empty() {
        return None;
    }
    let mut best: Option<(usize, usize)> = None;
    let mut idx = 0usize;
    while idx < hay.len() {
        match hay[idx..].find(needle) {
            Some(off) => {
                let abs = idx + off;
                let dist = abs.abs_diff(hint);
                if best.is_none_or(|(_, d)| dist < d) {
                    best = Some((abs, dist));
                }
                idx = abs + needle.len().max(1);
            }
            None => break,
        }
    }
    best.map(|(abs, _)| (abs, abs + needle.len()))
}

fn steps_to_json(initial: &str, steps: &[Step]) -> String {
    let mut json = String::from("[");
    json.push_str(&format!(
        "{{\"i\":0,\"phase\":\"start\",\"kind\":\"initial\",\"rule\":\"-\",\"start\":0,\"end\":{},\"same\":false,\"source\":\"{}\"}}",
        initial.len(),
        json_escape(initial)
    ));

    let last_index = steps.len();
    let mut prev_full: &str = initial;
    let mut prev_phase: &str = "start";

    for (n, step) in steps.iter().enumerate() {
        let idx = n + 1;
        let same = prev_full == step.source;
        let is_keyframe =
            idx == last_index || step.phase != prev_phase || idx % KEYFRAME_INTERVAL == 0;

        let (dstart, dend, old, new, located) = if step.has_node_diff && step.old == step.new {
            let hint = (step.start as i64 + (prev_full.len() as i64 - initial.len() as i64))
                .clamp(0, prev_full.len() as i64) as usize;
            match find_nearest_occurrence(prev_full, &step.old, hint) {
                Some((dstart, dend)) => {
                    (dstart, dend, step.old.as_str(), step.new.as_str(), true)
                }
                None => (0, 0, step.old.as_str(), step.new.as_str(), false),
            }
        } else if step.has_node_diff {
            match locate_node_diff(prev_full, &step.source, &step.old, &step.new) {
                Some((dstart, dend)) => {
                    (dstart, dend, step.old.as_str(), step.new.as_str(), true)
                }
                None => {
                    let (dstart, dend, old, new) = diff_span(prev_full, &step.source);
                    (dstart, dend, old, new, true)
                }
            }
        } else {
            let (dstart, dend, old, new) = diff_span(prev_full, &step.source);
            (dstart, dend, old, new, true)
        };

        json.push(',');
        json.push_str(&format!(
            "{{\"i\":{},\"phase\":\"{}\",\"kind\":\"{}\",\"rule\":\"{}\",\"start\":{},\"end\":{},\"same\":{},\"dstart\":{},\"dend\":{},\"old\":\"{}\",\"new\":\"{}\"",
            idx,
            json_escape(step.phase),
            json_escape(step.kind),
            json_escape(&step.rule),
            step.start,
            step.end,
            same,
            dstart,
            dend,
            json_escape(old),
            json_escape(new)
        ));
        if !located {
            json.push_str(",\"loc\":false");
        }
        if is_keyframe {
            json.push_str(&format!(",\"source\":\"{}\"", json_escape(&step.source)));
        }
        json.push('}');

        prev_full = &step.source;
        prev_phase = step.phase;
    }
    json.push(']');
    json
}

const TEMPLATE: &str = include_str!("../assets/steps_template.html");

pub fn render_html(initial: &str, steps: &[Step]) -> String {
    let json = steps_to_json(initial, steps);
    let b64 = STANDARD.encode(json.as_bytes());

    TEMPLATE.replace("__STEPS_B64__", &b64)
}

pub fn render_json(initial: &str, steps: &[Step]) -> String {
    steps_to_json(initial, steps)
}
