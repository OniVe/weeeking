use crate::config::Config;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenSource {
    Env,
    Keychain,
    None,
}

impl TokenSource {
    pub fn as_str(self) -> &'static str {
        match self {
            TokenSource::Env => "из переменной окружения",
            TokenSource::Keychain => "из системного хранилища",
            TokenSource::None => "НЕ НАЙДЕН",
        }
    }
}

/// Token resolution order:
/// 1. WEEEK_API_TOKEN / WEEEK_TOKEN from the environment.
/// 2. The OS keychain entry `weeek-mcp` / `api-token` (Windows only for now) —
///    the same entry the `weeek-mcp` wizard writes, so the stored token keeps working.
pub fn resolve(cfg: &Config) -> (Option<String>, TokenSource) {
    for key in ["WEEEK_API_TOKEN", "WEEEK_TOKEN"] {
        if let Ok(value) = std::env::var(key) {
            let value = value.trim().to_string();
            if !value.is_empty() {
                return (Some(value), TokenSource::Env);
            }
        }
    }
    if !cfg.disable_keychain
        && let Some(stored) = read_keychain()
    {
        return (Some(stored), TokenSource::Keychain);
    }
    (None, TokenSource::None)
}

#[cfg(windows)]
fn read_keychain() -> Option<String> {
    let entry = keyring::Entry::new("weeek-mcp", "api-token").ok()?;
    let stored = entry.get_password().ok()?;
    let stored = stored.trim().to_string();
    if stored.is_empty() {
        None
    } else {
        Some(stored)
    }
}

#[cfg(not(windows))]
fn read_keychain() -> Option<String> {
    // Linux pilot gets its token from WEEEK_API_TOKEN; a Secret Service
    // integration can be added later without touching the server core.
    None
}
