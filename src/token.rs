use crate::config::Config;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenSource {
    Env,
    Keychain(String),
    None,
}

impl TokenSource {
    pub fn describe(&self) -> String {
        match self {
            TokenSource::Env => "из переменной окружения".to_string(),
            TokenSource::Keychain(account) => format!("из системного хранилища ({account})"),
            TokenSource::None => "НЕ НАЙДЕН".to_string(),
        }
    }
}

/// Token resolution order:
/// 1. WEEEK_API_TOKEN / WEEEK_TOKEN from the environment.
/// 2. The OS keychain entry `weeek-mcp` / `WEEEK_KEYCHAIN_ACCOUNT` (default
///    `api-token`) — the same entry the wizard writes, so the stored token
///    keeps working; other accounts live in sibling entries (мультиаккаунт).
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
        && let Some(stored) = read_keychain(&cfg.keychain_account)
    {
        return (
            Some(stored),
            TokenSource::Keychain(cfg.keychain_account.clone()),
        );
    }
    (None, TokenSource::None)
}

#[cfg(windows)]
fn read_keychain(account: &str) -> Option<String> {
    let entry = keyring::Entry::new("weeek-mcp", account).ok()?;
    let stored = entry.get_password().ok()?;
    let stored = stored.trim().to_string();
    if stored.is_empty() {
        None
    } else {
        Some(stored)
    }
}

#[cfg(not(windows))]
fn read_keychain(_account: &str) -> Option<String> {
    // Linux pilot gets its token from WEEEK_API_TOKEN; a Secret Service
    // integration can be added later without touching the server core.
    None
}

/// Сохраняет токен в системное хранилище под записью `account`.
#[cfg(windows)]
pub fn store_token(account: &str) -> anyhow::Result<()> {
    use anyhow::{Context, bail};
    use std::io::IsTerminal as _;

    // rpassword читает консольный ввод; без терминала процесс завис бы молча.
    if !std::io::stdin().is_terminal() {
        bail!(
            "store-token требует интерактивного терминала (скрытый ввод читается с консоли). \
             Запустите команду в обычном окне терминала."
        );
    }

    let token = rpassword::prompt_password(format!(
        "Введите токен Weeek для записи «{account}» (ввод скрыт): "
    ))
    .context("не удалось прочитать ввод")?;
    let token = token.trim();
    if token.len() < 20 {
        bail!("токен подозрительно короткий (меньше 20 символов) — ничего не сохранено.");
    }

    let entry = keyring::Entry::new("weeek-mcp", account)
        .context("нет доступа к системному хранилищу (Windows Credential Manager)")?;
    entry
        .set_password(token)
        .context("не удалось сохранить токен в хранилище")?;
    let stored = entry
        .get_password()
        .context("токен сохранён, но не читается обратно — проверьте хранилище")?;
    if stored != token {
        bail!("проверка чтением не прошла — токен сохранён некорректно.");
    }

    println!("✓ Токен сохранён: хранилище `weeek-mcp`, запись `{account}`");
    println!("  Запуск сервера под этим аккаунтом: WEEEK_KEYCHAIN_ACCOUNT={account}");
    Ok(())
}

#[cfg(not(windows))]
pub fn store_token(_account: &str) -> anyhow::Result<()> {
    anyhow::bail!(
        "store-token пока поддержан только на Windows (Credential Manager); \
         на Linux задайте токен через WEEEK_API_TOKEN."
    )
}

#[cfg(all(test, windows))]
mod tests {
    /// Полный проход через настоящее хранилище: запись → чтение → удаление.
    /// Запуск вручную: cargo test -- --ignored keychain_round_trip
    #[test]
    #[ignore = "трогает настоящий Windows Credential Manager (создаёт и удаляет тестовую запись)"]
    fn keychain_round_trip() {
        let account = "api-token-selftest";
        let entry = keyring::Entry::new("weeek-mcp", account).expect("открыть запись");
        entry
            .set_password("selftest-abcdefghijklmnopqrstuvwxyz")
            .expect("записать токен");
        assert_eq!(
            entry.get_password().expect("прочитать токен"),
            "selftest-abcdefghijklmnopqrstuvwxyz"
        );
        entry.delete_credential().expect("удалить запись");
        assert!(
            entry.get_password().is_err(),
            "после удаления чтение должно возвращать ошибку"
        );
    }
}
