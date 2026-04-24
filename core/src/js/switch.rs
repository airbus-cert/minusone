use crate::tree::Node;

#[derive(Clone, Debug, PartialEq)]
enum SwitchLiteral {
    Bool(bool),
    Num(f64),
    Str(String),
    Null,
    Undefined,
}

fn parse_simple_string_literal(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    if bytes.len() < 2 {
        return None;
    }

    let quote = *bytes.first()?;
    if (quote != b'\'' && quote != b'"') || *bytes.last()? != quote {
        return None;
    }

    let inner = &text[1..text.len() - 1];
    if inner.contains('\\') {
        return None;
    }

    Some(inner.to_string())
}

fn parse_literal(node: &Node<()>) -> Option<SwitchLiteral> {
    if node.kind() == "parenthesized_expression" {
        for child in node.iter() {
            if child.kind() != "(" && child.kind() != ")" {
                return parse_literal(&child);
            }
        }
        return None;
    }

    let text = node.text().ok()?.trim();
    match node.kind() {
        "true" => Some(SwitchLiteral::Bool(true)),
        "false" => Some(SwitchLiteral::Bool(false)),
        "null" => Some(SwitchLiteral::Null),
        "undefined" => Some(SwitchLiteral::Undefined),
        "string" => parse_simple_string_literal(text).map(SwitchLiteral::Str),
        "number" => text.parse::<f64>().ok().map(SwitchLiteral::Num),
        _ => None,
    }
}

fn append_clause_statements(clause: &Node<()>, parts: &mut Vec<String>) -> Option<bool> {
    let value_id = clause.named_child("value").map(|value| value.id());
    for stmt in clause
        .iter()
        .filter(|child| !matches!(child.kind(), "case" | "default" | ":"))
        .filter(|child| Some(child.id()) != value_id)
    {
        match stmt.kind() {
            "break_statement" => return Some(true),
            "return_statement" | "throw_statement" => {
                parts.push(stmt.text().ok()?.trim().to_string());
                return Some(true);
            }
            _ => parts.push(stmt.text().ok()?.trim().to_string()),
        }
    }

    Some(false)
}

pub fn simplify_switch_statement_text(node: &Node<()>) -> Option<String> {
    if node.kind() != "switch_statement" {
        return None;
    }

    let switch_value = parse_literal(&node.named_child("value")?)?;
    let body = node.named_child("body")?;

    let mut total_clauses = 0usize;
    let mut selected_idx = None;
    let mut default_idx = None;

    for clause in body.iter() {
        if !matches!(clause.kind(), "switch_case" | "switch_default") {
            continue;
        }

        match clause.kind() {
            "switch_case" => {
                let case_value = parse_literal(&clause.named_child("value")?)?;
                if selected_idx.is_none() && case_value == switch_value {
                    selected_idx = Some(total_clauses);
                }
            }
            "switch_default" => {
                if default_idx.is_none() {
                    default_idx = Some(total_clauses);
                }
            }
            _ => {}
        }

        total_clauses += 1;
    }

    if total_clauses == 0 {
        return Some(String::new());
    }

    let selected_idx = selected_idx.or(default_idx)?;
    let mut clause_idx = 0usize;
    let mut parts = Vec::new();
    let mut collecting = false;

    for clause in body.iter() {
        if !matches!(clause.kind(), "switch_case" | "switch_default") {
            continue;
        }

        if clause_idx == selected_idx {
            collecting = true;
        }

        if collecting && append_clause_statements(&clause, &mut parts)? {
            return Some(parts.join(" "));
        }

        clause_idx += 1;
    }

    Some(parts.join(" "))
}
