//! Политика доступа к операциям Weeek.
//!
//! Источники (переменные окружения):
//! - `WEEEK_ALLOW_ACTIONS` — абсолютный белый список (operationId и маски `*`):
//!   всё, что не перечислено, запрещено;
//! - `WEEEK_DENY_ACTIONS` — чёрный список (вычитается из белого/полного);
//! - `WEEEK_PROFILE` — готовый профиль (`executor`: чтения + создание задач и комментариев).
//!
//! Инварианты:
//! - fail-closed: неизвестная маска — ошибка старта; при белом списке запрещено всё,
//!   что не разрешено явно;
//! - `READ_ONLY=true` сильнее списков и профиля — они игнорируются с предупреждением;
//! - deny сильнее allow и профиля.

use crate::config::Config;
use crate::i18n::{t, tf};
use crate::spec::{OPERATIONS, OpDef, find_operation};

/// Имя профиля «узкий исполнитель»: чтения + создание задачи/комментария.
pub const EXECUTOR_PROFILE: &str = "executor";

/// Операции, которые профиль `executor` добавляет к чтениям (GET-операции — по умолчанию).
const EXECUTOR_EXTRA_OPS: &[&str] = &["create-task", "create-task-comment"];

/// Готовая маска операции (`*` — любая подстрока, в том числе пустая).
#[derive(Debug, Clone)]
struct Matcher {
    pattern: String,
}

impl Matcher {
    fn new(pattern: &str) -> Self {
        Self {
            pattern: pattern.to_string(),
        }
    }

    fn matches(&self, id: &str) -> bool {
        glob_matches(&self.pattern, id)
    }
}

fn glob_matches(pattern: &str, id: &str) -> bool {
    if !pattern.contains('*') {
        return pattern == id;
    }
    let parts: Vec<&str> = pattern.split('*').collect();
    let mut rest = id;
    let first = parts[0];
    if !first.is_empty() {
        match rest.strip_prefix(first) {
            Some(stripped) => rest = stripped,
            None => return false,
        }
    }
    for part in &parts[1..parts.len() - 1] {
        if part.is_empty() {
            continue;
        }
        match rest.find(part) {
            Some(pos) => rest = &rest[pos + part.len()..],
            None => return false,
        }
    }
    let last = parts[parts.len() - 1];
    last.is_empty() || rest.ends_with(last)
}

fn join(matchers: &[Matcher]) -> String {
    matchers
        .iter()
        .map(|matcher| matcher.pattern.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

fn empty_list_message(var: &str) -> String {
    tf!(
        "{var} задан пустым — перечислите операции или уберите переменную.",
        "{var} is set but empty — list the operations or remove the variable."
    )
}

fn empty_profile_message() -> String {
    t!(
        "WEEEK_PROFILE задан пустым — укажите профиль (executor) или уберите переменную.",
        "WEEEK_PROFILE is set but empty — specify a profile (executor) or remove the variable."
    )
    .to_string()
}

/// Резолвнутая политика доступа для одного запуска сервера.
#[derive(Debug)]
pub struct Policy {
    read_only: bool,
    allow: Option<Vec<Matcher>>,
    deny: Vec<Matcher>,
    profile: Option<String>,
    /// Предупреждения резолва — печатает `main` до старта сервера.
    pub warnings: Vec<String>,
}

impl Policy {
    /// Собирает политику из конфигурации; ошибка — готовый текст для stderr (fail-closed).
    pub fn from_config(cfg: &Config) -> Result<Self, String> {
        let mut warnings = Vec::new();
        let profile = cfg.profile.clone();
        let allow_patterns = cfg.allow_actions.clone();
        let deny_patterns = cfg.deny_actions.clone();
        let configured = profile.is_some() || allow_patterns.is_some() || deny_patterns.is_some();

        if cfg.read_only {
            if configured {
                warnings.push(
                    t!(
                        "READ_ONLY=true: WEEEK_ALLOW_ACTIONS/WEEEK_DENY_ACTIONS/WEEEK_PROFILE игнорируются — режим только чтения сильнее.",
                        "READ_ONLY=true: WEEEK_ALLOW_ACTIONS/WEEEK_DENY_ACTIONS/WEEEK_PROFILE are ignored — read-only mode takes precedence."
                    )
                    .to_string(),
                );
            }
            // Даже игнорируемые настройки диагностируем: пустые значения и опечатки видны сразу.
            for (var, patterns) in [
                ("WEEEK_ALLOW_ACTIONS", &allow_patterns),
                ("WEEEK_DENY_ACTIONS", &deny_patterns),
            ] {
                if let Some(patterns) = patterns {
                    if patterns.is_empty() {
                        warnings.push(empty_list_message(var));
                    } else if let Err(message) = compile(var, patterns) {
                        warnings.push(message);
                    }
                }
            }
            if let Some(name) = profile.as_deref() {
                if name.is_empty() {
                    warnings.push(empty_profile_message());
                } else if name != EXECUTOR_PROFILE {
                    warnings.push(tf!(
                        "Неизвестный WEEEK_PROFILE «{name}». Доступные профили: {EXECUTOR_PROFILE}.",
                        "Unknown WEEEK_PROFILE `{name}`. Available profiles: {EXECUTOR_PROFILE}."
                    ));
                }
            }
            return Ok(Self {
                read_only: true,
                allow: None,
                deny: Vec::new(),
                profile: None,
                warnings,
            });
        }

        if let Some(name) = profile.as_deref() {
            if name.is_empty() {
                return Err(empty_profile_message());
            }
            if name != EXECUTOR_PROFILE {
                return Err(tf!(
                    "Неизвестный WEEEK_PROFILE «{name}». Доступные профили: {EXECUTOR_PROFILE}.",
                    "Unknown WEEEK_PROFILE `{name}`. Available profiles: {EXECUTOR_PROFILE}."
                ));
            }
            if allow_patterns.is_some() {
                return Err(t!(
                    "WEEEK_PROFILE и WEEEK_ALLOW_ACTIONS заданы одновременно — оставьте один источник белого списка.",
                    "WEEEK_PROFILE and WEEEK_ALLOW_ACTIONS are set together — keep a single allow-list source."
                )
                .to_string());
            }
        }

        let allow = match &allow_patterns {
            Some(patterns) => {
                if patterns.is_empty() {
                    return Err(empty_list_message("WEEEK_ALLOW_ACTIONS"));
                }
                Some(compile("WEEEK_ALLOW_ACTIONS", patterns)?)
            }
            None => None,
        };
        let deny = match &deny_patterns {
            Some(patterns) => {
                if patterns.is_empty() {
                    return Err(empty_list_message("WEEEK_DENY_ACTIONS"));
                }
                compile("WEEEK_DENY_ACTIONS", patterns)?
            }
            None => Vec::new(),
        };

        Ok(Self {
            read_only: false,
            allow,
            deny,
            profile,
            warnings,
        })
    }

    /// Режим только чтения (READ_ONLY=true).
    pub fn read_only(&self) -> bool {
        self.read_only
    }

    /// Разрешена ли операция.
    pub fn allows(&self, op: &OpDef) -> bool {
        if self.read_only {
            return op.method == "GET";
        }
        if self.deny.iter().any(|matcher| matcher.matches(op.id)) {
            return false;
        }
        if self.profile.is_some() {
            return op.method == "GET" || EXECUTOR_EXTRA_OPS.contains(&op.id);
        }
        match &self.allow {
            Some(matchers) => matchers.iter().any(|matcher| matcher.matches(op.id)),
            None => true,
        }
    }

    /// Разрешена ли операция по её id (неизвестный id — запрещён: fail-closed).
    pub fn allows_id(&self, id: &str) -> bool {
        find_operation(id)
            .map(|op| self.allows(op))
            .unwrap_or(false)
    }

    /// Короткое человекочитаемое описание для стартового лога.
    pub fn describe(&self) -> String {
        if self.read_only {
            return t!("только чтение (READ_ONLY)", "read-only (READ_ONLY)").to_string();
        }
        let suffixes = |deny: &[Matcher]| {
            if deny.is_empty() {
                String::new()
            } else {
                tf!(", deny: {}", ", deny: {}", join(deny))
            }
        };
        if let Some(name) = &self.profile {
            return tf!(
                "профиль {name} (чтения + {}){}",
                "profile {name} (reads + {}){}",
                EXECUTOR_EXTRA_OPS.join(", "),
                suffixes(&self.deny)
            );
        }
        match &self.allow {
            Some(allow) => tf!(
                "allow: {}{}",
                "allow: {}{}",
                join(allow),
                suffixes(&self.deny)
            ),
            None if !self.deny.is_empty() => tf!("deny: {}", "deny: {}", join(&self.deny)),
            None => t!("полный доступ", "full access").to_string(),
        }
    }

    /// Техническое описание для debug-лога (как HTTP-логи — всегда английское).
    pub fn describe_debug(&self) -> String {
        format!(
            "read_only={} profile={:?} allow=[{}] deny=[{}]",
            self.read_only,
            self.profile,
            self.allow.as_ref().map(|m| join(m)).unwrap_or_default(),
            join(&self.deny)
        )
    }
}

/// Компилирует список масок; каждая обязана совпасть хотя бы с одной операцией OpenAPI.
fn compile(var: &str, patterns: &[String]) -> Result<Vec<Matcher>, String> {
    let mut out = Vec::new();
    for pattern in patterns {
        let matcher = Matcher::new(pattern);
        if !OPERATIONS.iter().any(|op| matcher.matches(op.id)) {
            return Err(tf!(
                "{var}: «{pattern}» не совпадает ни с одной операцией OpenAPI. Укажите operationId или маску (например, delete-*).",
                "{var}: `{pattern}` does not match any OpenAPI operation. Use an operationId or a mask (e.g. delete-*)."
            ));
        }
        out.push(matcher);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::{Policy, glob_matches};
    use crate::config::Config;

    fn cfg(
        allow: Option<&[&str]>,
        deny: Option<&[&str]>,
        profile: Option<&str>,
        read_only: bool,
    ) -> Config {
        Config {
            base_url: "http://localhost".into(),
            timeout_ms: 1_000,
            read_only,
            allow_actions: allow.map(|list| list.iter().map(|s| s.to_string()).collect()),
            deny_actions: deny.map(|list| list.iter().map(|s| s.to_string()).collect()),
            profile: profile.map(str::to_string),
            rps: None,
            rps_invalid: None,
            rps_burst: None,
            max_chars: 10_000,
            disable_keychain: true,
            keychain_account: "api-token".into(),
            max_attachment_bytes: 1024,
            retry_max: 0,
            retry_base_ms: 10,
            log_debug: false,
        }
    }

    #[test]
    fn glob_patterns() {
        assert!(glob_matches("delete-*", "delete-task"));
        assert!(glob_matches("delete-*", "delete-task-comment"));
        assert!(!glob_matches("delete-*", "uncomplete-task"));
        assert!(glob_matches("*-task", "create-task"));
        assert!(!glob_matches("*-task", "create-task-comment"));
        assert!(glob_matches("*task*", "create-task-comment"));
        assert!(glob_matches("*", "anything"));
        assert!(glob_matches("create-task", "create-task"));
        assert!(!glob_matches("create", "create-task"));
    }

    #[test]
    fn full_access_allows_everything() {
        let policy = Policy::from_config(&cfg(None, None, None, false)).expect("policy");
        assert!(!policy.read_only());
        assert!(policy.allows_id("create-task"));
        assert!(policy.allows_id("delete-tag"));
        assert!(policy.allows_id("get-tasks"));
        assert!(policy.warnings.is_empty());
    }

    #[test]
    fn read_only_ignores_lists_with_warning() {
        let policy =
            Policy::from_config(&cfg(None, Some(&["delete-*"]), None, true)).expect("policy");
        assert!(policy.read_only());
        assert!(!policy.allows_id("create-task"));
        assert!(!policy.allows_id("delete-tag"));
        assert!(policy.allows_id("get-tasks"));
        assert_eq!(policy.warnings.len(), 1);
    }

    #[test]
    fn deny_subtracts_from_full() {
        let policy =
            Policy::from_config(&cfg(None, Some(&["delete-*"]), None, false)).expect("policy");
        assert!(!policy.allows_id("delete-tag"));
        assert!(!policy.allows_id("delete-task"));
        assert!(policy.allows_id("create-task"));
        assert!(policy.allows_id("get-tasks"));
    }

    #[test]
    fn allow_is_absolute() {
        let policy =
            Policy::from_config(&cfg(Some(&["get-tasks", "create-task"]), None, None, false))
                .expect("policy");
        assert!(policy.allows_id("get-tasks"));
        assert!(policy.allows_id("create-task"));
        assert!(!policy.allows_id("get-tags"));
        assert!(!policy.allows_id("create-tag"));
    }

    #[test]
    fn deny_wins_over_allow() {
        let policy = Policy::from_config(&cfg(
            Some(&["get-*", "create-task"]),
            Some(&["create-task"]),
            None,
            false,
        ))
        .expect("policy");
        assert!(policy.allows_id("get-tags"));
        assert!(!policy.allows_id("create-task"));
    }

    #[test]
    fn executor_profile_is_a_narrow_rw_preset() {
        let policy =
            Policy::from_config(&cfg(None, None, Some("executor"), false)).expect("policy");
        assert!(policy.allows_id("get-tags"));
        assert!(policy.allows_id("get-attachment"));
        assert!(policy.allows_id("create-task"));
        assert!(policy.allows_id("create-task-comment"));
        assert!(!policy.allows_id("update-task"));
        assert!(!policy.allows_id("complete-task"));
        assert!(!policy.allows_id("delete-task-comment"));

        let narrowed = Policy::from_config(&cfg(
            None,
            Some(&["create-task-comment"]),
            Some("executor"),
            false,
        ))
        .expect("policy");
        assert!(!narrowed.allows_id("create-task-comment"));
        assert!(narrowed.allows_id("create-task"));
    }

    #[test]
    fn unknown_profile_and_conflicts_are_errors() {
        assert!(Policy::from_config(&cfg(None, None, Some("nope"), false)).is_err());
        assert!(
            Policy::from_config(&cfg(Some(&["create-task"]), None, Some("executor"), false))
                .is_err()
        );
    }

    #[test]
    fn unknown_patterns_are_errors() {
        let err =
            Policy::from_config(&cfg(Some(&["delete-everything"]), None, None, false)).unwrap_err();
        assert!(err.contains("delete-everything"));
        assert!(err.contains("WEEEK_ALLOW_ACTIONS"));
        let err = Policy::from_config(&cfg(None, Some(&["nope-*"]), None, false)).unwrap_err();
        assert!(err.contains("nope-*"));
        assert!(err.contains("WEEEK_DENY_ACTIONS"));
    }

    #[test]
    fn empty_values_fail_closed() {
        let err = Policy::from_config(&cfg(Some(&[]), None, None, false)).unwrap_err();
        assert!(err.contains("WEEEK_ALLOW_ACTIONS"));
        let err = Policy::from_config(&cfg(None, Some(&[]), None, false)).unwrap_err();
        assert!(err.contains("WEEEK_DENY_ACTIONS"));
        let err = Policy::from_config(&cfg(None, None, Some(""), false)).unwrap_err();
        assert!(err.contains("WEEEK_PROFILE"));
    }

    #[test]
    fn read_only_reports_ignored_misconfig_as_warnings() {
        let policy = Policy::from_config(&cfg(Some(&[]), Some(&["nope-*"]), Some(""), true))
            .expect("policy");
        assert!(policy.read_only());
        let all = policy.warnings.join("\n");
        // Ассерты языконезависимые: env WEEEK_LANG не должен влиять на unit-тесты.
        assert!(
            all.contains("WEEEK_ALLOW_ACTIONS/WEEEK_DENY_ACTIONS/WEEEK_PROFILE"),
            "{all}"
        );
        assert!(all.contains("WEEEK_ALLOW_ACTIONS"), "{all}");
        assert!(all.contains("nope-*"), "{all}");
        assert!(all.contains("WEEEK_PROFILE"), "{all}");

        // Неизвестный непустой профиль под READ_ONLY тоже диагностируется.
        let policy = Policy::from_config(&cfg(None, None, Some("nope"), true)).expect("policy");
        let all = policy.warnings.join("\n");
        assert!(all.contains("nope"), "{all}");
    }

    #[test]
    fn describe_mentions_configured_sources() {
        assert!(
            Policy::from_config(&cfg(None, Some(&["delete-*"]), None, false))
                .expect("policy")
                .describe()
                .contains("delete-*")
        );
        assert!(
            Policy::from_config(&cfg(Some(&["get-tasks"]), None, None, false))
                .expect("policy")
                .describe()
                .contains("get-tasks")
        );
        assert!(
            Policy::from_config(&cfg(None, None, Some("executor"), false))
                .expect("policy")
                .describe()
                .contains("executor")
        );
        assert!(
            Policy::from_config(&cfg(None, None, None, true))
                .expect("policy")
                .describe_debug()
                .contains("read_only=true")
        );
    }
}
