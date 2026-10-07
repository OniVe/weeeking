use crate::http::WeeekClient;
use crate::server::WeeekingServer;
use crate::spec::{OpDef, Spec};
use crate::util::*;
use rmcp::model::Tool;
use serde_json::{Map, Value, json};
use std::collections::{HashMap, HashSet};

/// Builds one tool per API tag with an `action` parameter and returns the
/// dispatch map (tool name -> allowed operation ids).
pub fn build(spec: &'static Spec, read_only: bool) -> (Vec<Tool>, HashMap<String, Vec<String>>) {
    let mut tools = Vec::new();
    let mut dispatch = HashMap::new();

    for group in &spec.groups {
        let all: Vec<&OpDef> = group
            .op_ids
            .iter()
            .filter_map(|id| spec.operations.get(id))
            .collect();
        let ops: Vec<&OpDef> = all
            .iter()
            .copied()
            .filter(|op| !read_only || op.method == "GET")
            .collect();
        if ops.is_empty() {
            continue;
        }
        let hidden = all.len() - ops.len();

        let mut description = format!(
            "Операции Weeek ({}) через параметр action.\nparams — плоский объект параметров выбранной операции (path/query/body по именам из списка):\n",
            group.label
        );
        for op in &ops {
            description.push_str(&describe_op(op));
            description.push('\n');
        }
        if hidden > 0 {
            description.push_str(&format!(
                "(Режим READ_ONLY: скрыто изменяющих операций — {hidden}. Включите READ_ONLY=false, чтобы они появились.)"
            ));
        }

        let action_ids: Vec<&str> = ops.iter().map(|op| op.id.as_str()).collect();
        let input_schema = schema(
            json!({
                "action": {
                    "type": "string",
                    "enum": action_ids,
                    "description": "Операция Weeek API"
                },
                "params": {
                    "type": "object",
                    "additionalProperties": true,
                    "description": "Параметры операции (см. список в описании)"
                }
            }),
            &["action"],
        );

        let read_only_hint = ops.iter().all(|op| op.method == "GET");
        let destructive_hint = ops.iter().any(|op| op.method == "DELETE");
        let tool = Tool::new(
            format!("weeek_{}", group.slug.replace('-', "_")),
            description,
            input_schema,
        )
        .with_title(format!("Weeek: {}", group.label))
        .with_annotations(ann(read_only_hint, destructive_hint, false));

        dispatch.insert(
            tool.name.to_string(),
            ops.iter().map(|op| op.id.clone()).collect(),
        );
        tools.push(tool);
    }

    (tools, dispatch)
}

fn describe_op(op: &OpDef) -> String {
    let mut parts: Vec<String> = Vec::new();
    for p in &op.path_params {
        parts.push(format!(
            "{} (path, {}{})",
            p.name,
            p.ty,
            if p.required { ", обяз." } else { "" }
        ));
    }
    for q in &op.query_params {
        let enum_hint = fmt_enum(&q.kind)
            .map(|values| format!(": {values}"))
            .unwrap_or_default();
        parts.push(format!(
            "{} ({}{}{})",
            q.name,
            q.ty,
            enum_hint,
            if q.required { ", обяз." } else { "" }
        ));
    }
    for b in &op.body_fields {
        let enum_hint = fmt_enum(&b.kind)
            .map(|values| format!(": {values}"))
            .unwrap_or_default();
        parts.push(format!(
            "{}{} (body, {}{})",
            b.name,
            if b.required { "*" } else { "" },
            b.ty,
            enum_hint
        ));
    }

    let mut line = format!("• {}: {}", op.id, op.summary);
    if !parts.is_empty() {
        line.push_str(" — ");
        line.push_str(&parts.join("; "));
    } else if op.has_body {
        line.push_str(" — тело запроса свободной формы");
    }
    line
}

impl WeeekingServer {
    pub(crate) async fn generated_run(
        &self,
        allowed: &[String],
        args: Map<String, Value>,
    ) -> Result<Value, String> {
        let action =
            arg_str(&args, "action").ok_or_else(|| "Не задан параметр «action».".to_string())?;
        if !allowed.iter().any(|id| id == &action) {
            return Err(format!(
                "Операция «{action}» недоступна (проверьте список action в описании инструмента и режим READ_ONLY)."
            ));
        }
        let op = self
            .spec
            .operations
            .get(&action)
            .ok_or_else(|| format!("Неизвестная операция: {action}"))?;
        let params = args
            .get("params")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        run_op(&self.client, op, &params).await
    }
}

async fn run_op(
    client: &WeeekClient,
    op: &OpDef,
    params: &Map<String, Value>,
) -> Result<Value, String> {
    let mut path = op.path.clone();
    let mut used: HashSet<String> = HashSet::new();

    for param in &op.path_params {
        match params.get(&param.name) {
            Some(value) if !value.is_null() => {
                used.insert(param.name.clone());
                path = path.replace(
                    &format!("{{{}}}", param.name),
                    &encode_segment(&scalar_string(value)),
                );
            }
            _ => {
                if param.required {
                    return Err(format!(
                        "Не задан обязательный path-параметр «{}» для {}.",
                        param.name, op.id
                    ));
                }
            }
        }
    }

    let mut query: Vec<(String, String)> = Vec::new();
    for param in &op.query_params {
        if let Some(value) = params.get(&param.name) {
            push_query(&mut query, &param.name, value);
            used.insert(param.name.clone());
        }
    }

    let extras: Vec<&String> = params.keys().filter(|key| !used.contains(*key)).collect();
    let body = if op.has_body {
        let mut map = Map::new();
        for key in extras {
            if let Some(value) = params.get(key) {
                map.insert(key.clone(), value.clone());
            }
        }
        Some(Value::Object(map))
    } else {
        if !extras.is_empty() {
            return Err(format!(
                "Операция {} не принимает параметры: {}.",
                op.id,
                extras
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        None
    };

    client
        .call(&op.method, &path, &query, body.as_ref())
        .await
        .map_err(|e| e.to_string())
}

fn scalar_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}
