use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub base_url: String,
    pub timeout_ms: u64,
    pub read_only: bool,
    pub max_chars: usize,
    pub disable_keychain: bool,
    /// Имя записи в системном хранилище (мультиаккаунт), по умолчанию `api-token`.
    pub keychain_account: String,
}

/// Нормализует имя аккаунта из значения переменной окружения.
pub fn keychain_account_from(value: Option<String>) -> String {
    value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "api-token".to_string())
}

impl Config {
    pub fn from_env() -> Self {
        let read_only = !matches!(
            env::var("READ_ONLY")
                .unwrap_or_else(|_| "true".into())
                .trim()
                .to_ascii_lowercase()
                .as_str(),
            "0" | "false"
        );
        let timeout_ms = env::var("WEEEK_TIMEOUT_MS")
            .ok()
            .and_then(|v| v.trim().parse::<u64>().ok())
            .filter(|v| *v > 0)
            .unwrap_or(30_000);
        let max_chars = env::var("WEEEK_MAX_RESPONSE_CHARS")
            .ok()
            .and_then(|v| v.trim().parse::<usize>().ok())
            .filter(|v| *v > 1000)
            .unwrap_or(60_000);
        let base_url = env::var("WEEEK_BASE_URL")
            .unwrap_or_else(|_| "https://api.weeek.net/public/v1".into())
            .trim_end_matches('/')
            .to_string();
        let disable_keychain = matches!(
            env::var("WEEEK_DISABLE_KEYCHAIN")
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase()
                .as_str(),
            "1" | "true" | "yes"
        );
        let keychain_account = keychain_account_from(env::var("WEEEK_KEYCHAIN_ACCOUNT").ok());

        Self {
            base_url,
            timeout_ms,
            read_only,
            max_chars,
            disable_keychain,
            keychain_account,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::keychain_account_from;

    #[test]
    fn account_defaults_to_api_token() {
        assert_eq!(keychain_account_from(None), "api-token");
        assert_eq!(keychain_account_from(Some("".into())), "api-token");
        assert_eq!(keychain_account_from(Some("   ".into())), "api-token");
    }

    #[test]
    fn account_is_trimmed_and_kept() {
        assert_eq!(
            keychain_account_from(Some("  api-token-anna ".into())),
            "api-token-anna"
        );
    }
}
