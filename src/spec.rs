//! Спецификация Weeek: типы + сгенерированные данные (`src/spec_generated.rs`).
//!
//! Сами данные генерируются кодогенератором `tools/update-spec.mjs` из
//! официальной OpenAPI (developers.weeek.net) и компилируются в бинарник —
//! в рантайме нет ни JSON, ни парсинга.

#[cfg(test)]
pub use crate::spec_generated::CURATED;
pub use crate::spec_generated::{GROUPS, OPERATIONS, SPEC_TITLE, SPEC_URL, SPEC_VERSION};

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

        // Группы: непустые уникальные slug/label, ссылки резолвятся, без дублей между группами.
        let slugs: HashSet<&str> = GROUPS.iter().map(|g| g.slug).collect();
        assert_eq!(slugs.len(), GROUPS.len(), "group slugs must be unique");
        for group in GROUPS {
            assert!(!group.slug.is_empty(), "group slug must not be empty");
            assert!(!group.label.is_empty(), "group label must not be empty");
            assert!(
                !group.op_ids.is_empty(),
                "group {} must have operations",
                group.slug
            );
        }

        let mut grouped_seen: HashSet<&str> = HashSet::new();
        for group in GROUPS {
            for op_id in group.op_ids {
                assert!(
                    ids.contains(*op_id),
                    "group {} references unknown operation {op_id}",
                    group.slug
                );
                assert!(
                    grouped_seen.insert(*op_id),
                    "operation {op_id} is listed in more than one group"
                );
            }
        }

        let curated: HashSet<&str> = CURATED.iter().copied().collect();
        assert_eq!(curated.len(), CURATED.len(), "curated ids must be unique");

        // Каждая операция достижима ровно один раз: либо в группе, либо curated.
        for id in &grouped_seen {
            assert!(
                !curated.contains(id),
                "operation {id} cannot be both grouped and curated"
            );
        }
        let reachable: HashSet<&str> = grouped_seen.union(&curated).copied().collect();
        assert_eq!(
            reachable.len(),
            OPERATIONS.len(),
            "every operation must be reachable exactly once (grouped or curated)"
        );
        for op in OPERATIONS {
            assert!(
                reachable.contains(op.id),
                "operation {} is unreachable",
                op.id
            );
        }
    }
}
