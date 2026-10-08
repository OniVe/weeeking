//! Спецификация Weeek: типы + сгенерированные данные (`src/spec_generated.rs`).
//!
//! Сами данные генерируются кодогенератором `tools/update-spec.mjs` из
//! официальной OpenAPI (developers.weeek.net) и компилируются в бинарник —
//! в рантайме нет ни JSON, ни парсинга.

pub use crate::spec_generated::{GROUPS, OPERATIONS, SPEC_GENERATED_AT, SPEC_TITLE, SPEC_URL};

#[derive(Debug)]
pub struct ParamDef {
    pub name: &'static str,
    pub ty: &'static str,
    pub required: bool,
    /// Готовый для описаний рендер enum-значений («a|b|c»), если он есть.
    pub enum_hint: Option<&'static str>,
}

#[derive(Debug)]
pub struct OpDef {
    pub id: &'static str,
    pub method: &'static str,
    pub path: &'static str,
    pub summary: &'static str,
    pub path_params: &'static [ParamDef],
    pub query_params: &'static [ParamDef],
    pub body_fields: &'static [ParamDef],
    pub has_body: bool,
}

#[derive(Debug)]
pub struct GroupDef {
    pub slug: &'static str,
    pub label: &'static str,
    pub op_ids: &'static [&'static str],
}

/// Ищет операцию по id (линейный поиск по ~157 элементам — микросекунды).
pub fn find_operation(id: &str) -> Option<&'static OpDef> {
    OPERATIONS.iter().find(|op| op.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn spec_snapshot_is_consistent() {
        assert!(!OPERATIONS.is_empty(), "operations must not be empty");
        assert!(!GROUPS.is_empty(), "groups must not be empty");

        let ids: HashSet<&str> = OPERATIONS.iter().map(|op| op.id).collect();
        assert_eq!(ids.len(), OPERATIONS.len(), "operation ids must be unique");

        for group in GROUPS {
            assert!(
                !group.op_ids.is_empty(),
                "group {} must have operations",
                group.slug
            );
            for op_id in group.op_ids {
                assert!(
                    ids.contains(*op_id),
                    "group {} references unknown operation {op_id}",
                    group.slug
                );
            }
        }

        // Каждая операция — либо ровно в одной группе, либо curated (вне групп).
        let grouped: usize = GROUPS.iter().map(|g| g.op_ids.len()).sum();
        assert!(
            grouped <= OPERATIONS.len(),
            "grouped operations cannot exceed the total"
        );
    }
}
