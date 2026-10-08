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
    // Строго целые: float молча усекать нельзя (запрос ушёл бы не туда).
    match args.get(key) {
        Some(Value::Number(n)) => n.as_i64(),
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
    match args.get(key) {
        None | Some(Value::Null) => Err(format!("Не задан обязательный параметр «{key}».")),
        Some(value) => value
            .as_i64()
            .filter(|n| *n > 0)
            .ok_or_else(|| format!("Параметр «{key}» должен быть положительным целым числом.")),
    }
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
        // Weeek принимает булевы query-параметры только как 1/0 (`true/false` -> HTTP 422),
        // как и в квирке с isPrivate в теле запроса.
        Value::Bool(true) => "1".to_string(),
        Value::Bool(false) => "0".to_string(),
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

/// Как `encode_segment`, но дополнительно запрещает RFC dot-segments (`.` и `..`):
/// url-нормализация схлопнула бы `/tm/projects/..` в `/tm/` и увела бы write-метод
/// на родительский коллекционный endpoint.
pub fn encode_path_segment(value: &str) -> Result<String, String> {
    if value.trim().is_empty() {
        return Err("недопустимый path-параметр: пустая строка.".to_string());
    }
    if value == "." || value == ".." {
        return Err(format!(
            "недопустимый path-параметр «{value}»: сегменты '.' и '..' запрещены."
        ));
    }
    Ok(encode_segment(value))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dot_segments_are_rejected() {
        assert!(encode_path_segment(".").is_err());
        assert!(encode_path_segment("..").is_err());
        assert!(encode_path_segment("").is_err());
        assert!(encode_path_segment("   ").is_err());
        assert_eq!(encode_path_segment("normal-id").unwrap(), "normal-id");
        assert_eq!(encode_path_segment("a b").unwrap(), "a%20b");
        assert_eq!(encode_path_segment("50%").unwrap(), "50%25");
    }

    #[test]
    fn boolean_query_is_serialized_as_bit() {
        // Weeek API требует 1/0 вместо true/false в query (иначе HTTP 422).
        let mut query = Vec::new();
        push_query(&mut query, "completed", &json!(true));
        push_query(&mut query, "all", &json!(false));
        push_query(&mut query, "perPage", &json!(5));
        assert_eq!(
            query,
            vec![
                ("completed".to_string(), "1".to_string()),
                ("all".to_string(), "0".to_string()),
                ("perPage".to_string(), "5".to_string()),
            ]
        );
    }
}
