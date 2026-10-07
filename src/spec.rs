use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Debug, Clone, Deserialize)]
pub struct ParamDef {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: String,
    pub required: bool,
    #[serde(rename = "enum")]
    pub kind: Option<Vec<Value>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpDef {
    pub id: String,
    pub method: String,
    pub path: String,
    pub summary: String,
    pub path_params: Vec<ParamDef>,
    pub query_params: Vec<ParamDef>,
    pub body_fields: Vec<ParamDef>,
    pub has_body: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupDef {
    pub slug: String,
    pub label: String,
    pub op_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Spec {
    pub spec_url: String,
    pub generated_at: String,
    pub spec_title: String,
    pub operations: HashMap<String, OpDef>,
    pub groups: Vec<GroupDef>,
}

impl Spec {
    /// The spec snapshot is embedded into the binary at compile time.
    pub fn load() -> &'static Spec {
        static SPEC: OnceLock<Spec> = OnceLock::new();
        SPEC.get_or_init(|| {
            serde_json::from_str(include_str!("../spec/operations.json"))
                .expect("spec/operations.json must parse (run: node tools/update-spec.mjs)")
        })
    }
}
