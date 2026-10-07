use rmcp::model::{CallToolResult, ContentBlock, JsonObject, ToolAnnotations};
use serde_json::{Value, json};
use std::sync::Arc;

pub fn json_result(value: &Value, max_chars: usize) -> CallToolResult {
    let text = serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string());
    CallToolResult::success(vec![ContentBlock::text(truncate(&text, max_chars))])
}

pub fn error_result(message: impl Into<String>) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(message.into())])
}

pub fn truncate(s: &str, max: usize) -> String {
    let total = s.chars().count();
    if total <= max {
        return s.to_string();
    }
    let taken: String = s.chars().take(max).collect();
    format!("{taken}\n…[ответ обрезан: {max} из {total} символов]")
}

pub fn ann(read_only: bool, destructive: bool, idempotent: bool) -> ToolAnnotations {
    ToolAnnotations::new()
        .read_only(read_only)
        .destructive(destructive)
        .idempotent(idempotent)
        .open_world(true)
}

pub fn schema(props: Value, required: &[&str]) -> Arc<JsonObject> {
    let mut m = JsonObject::new();
    m.insert("type".into(), json!("object"));
    m.insert("properties".into(), props);
    if !required.is_empty() {
        m.insert("required".into(), json!(required));
    }
    m.insert("additionalProperties".into(), json!(false));
    Arc::new(m)
}

pub fn arg_str(args: &JsonObject, key: &str) -> Option<String> {
    match args.get(key) {
        Some(Value::String(s)) => Some(s.clone()),
        _ => None,
    }
}

pub fn arg_i64(args: &JsonObject, key: &str) -> Option<i64> {
    match args.get(key) {
        Some(Value::Number(n)) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)),
        _ => None,
    }
}

pub fn arg_bool(args: &JsonObject, key: &str) -> Option<bool> {
    args.get(key).and_then(Value::as_bool)
}

pub fn arg_str_array(args: &JsonObject, key: &str) -> Option<Vec<String>> {
    match args.get(key) {
        Some(Value::Array(items)) => {
            let values: Vec<String> = items
                .iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect();
            if values.is_empty() {
                None
            } else {
                Some(values)
            }
        }
        _ => None,
    }
}

pub fn req_i64(args: &JsonObject, key: &str) -> Result<i64, String> {
    arg_i64(args, key)
        .ok_or_else(|| format!("Не задан обязательный параметр «{key}» (целое число)."))
}

pub fn req_str(args: &JsonObject, key: &str) -> Result<String, String> {
    arg_str(args, key)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("Не задан обязательный параметр «{key}» (строка)."))
}

/// Adds a query pair, expanding arrays into repeated keys.
pub fn push_query(q: &mut Vec<(String, String)>, key: &str, value: &Value) {
    match value {
        Value::Null => {}
        Value::Array(items) => {
            for item in items {
                push_scalar(q, key, item);
            }
        }
        other => push_scalar(q, key, other),
    }
}

fn push_scalar(q: &mut Vec<(String, String)>, key: &str, value: &Value) {
    let s = match value {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    };
    q.push((key.to_string(), s));
}

/// Percent-encodes a value for use in a single URL path segment.
pub fn encode_segment(s: &str) -> String {
    let mut out = String::new();
    for b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char);
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Unwraps the live API envelope ({ success, user }, { success, projects }, …)
/// when the key is present; otherwise returns the value unchanged.
pub fn unwrap_key(value: Value, key: &str) -> Value {
    match value {
        Value::Object(mut map) => {
            if map.contains_key(key) {
                map.remove(key).unwrap_or(Value::Null)
            } else {
                Value::Object(map)
            }
        }
        other => other,
    }
}

/// Renders a parameter's enum values for tool descriptions.
pub fn fmt_enum(kind: &Option<Vec<Value>>) -> Option<String> {
    let values = kind.as_ref()?;
    let rendered: Vec<String> = values
        .iter()
        .map(|v| match v {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        })
        .collect();
    if rendered.is_empty() {
        None
    } else {
        Some(rendered.join("|"))
    }
}
