use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use minusone::js::trace::Step;

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

fn steps_to_json(initial: &str, steps: &[Step]) -> String {
    let mut json = String::from("[");
    json.push_str(&format!(
        "{{\"i\":0,\"kind\":\"initial\",\"node_id\":0,\"start\":0,\"end\":{},\"source\":\"{}\"}}",
        initial.len(),
        json_escape(initial)
    ));

    for (n, step) in steps.iter().enumerate() {
        json.push(',');
        json.push_str(&format!(
            "{{\"i\":{},\"kind\":\"{}\",\"node_id\":{},\"start\":{},\"end\":{},\"source\":\"{}\"}}",
            n + 1,
            json_escape(step.kind),
            step.node_id,
            step.start,
            step.end,
            json_escape(&step.source)
        ));
    }
    json.push(']');
    json
}

const TEMPLATE: &str = include_str!("../assets/steps_template.html");

pub fn render(initial: &str, steps: &[Step]) -> String {
    let json = steps_to_json(initial, steps);
    let b64 = STANDARD.encode(json.as_bytes());

    TEMPLATE.replace("__STEPS_B64__", &b64)
}
