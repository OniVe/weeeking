use crate::config::Config;
use crate::i18n::{t, tf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenSource {
    Env,
    Keychain(String),
    None,
}

impl TokenSource {
    pub fn describe(&self) -> String {
        match self {
            TokenSource::Env => t!("из переменной окружения", "from the environment").to_string(),
            TokenSource::Keychain(account) => tf!(
                "из системного хранилища ({account})",
                "from the system keychain ({account})"
            ),
            TokenSource::None => t!("НЕ НАЙДЕН", "NOT FOUND").to_string(),
        }
    }
}

/// Название системного хранилища текущей платформы (для сообщений).
const KEYCHAIN_LABEL: &str = if cfg!(windows) {
    "Windows Credential Manager"
} else if cfg!(target_os = "macos") {
    "macOS Keychain"
} else {
    "Linux Secret Service"
};

/// Token resolution order:
/// 1. WEEEK_API_TOKEN / WEEEK_TOKEN from the environment.
/// 2. The OS keychain entry `weeek-mcp` / `WEEEK_KEYCHAIN_ACCOUNT` (default
///    `api-token`), filled in by `weeeking store-token`; other accounts live
///    in sibling entries (мультиаккаунт). Хранилища: Windows Credential
///    Manager, macOS Keychain, Linux Secret Service (D-Bus).
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

/// Читает токен из системного хранилища платформы.
///
/// Ошибки (нет D-Bus/Secret Service, headless-сессия, отказ доступа) означают
/// «в хранилище ничего нет» — токен тогда ищется в переменных окружения.
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

/// Сохраняет токен в системное хранилище под записью `account`.
pub fn store_token(account: &str) -> anyhow::Result<()> {
    use anyhow::{Context, bail};
    use std::io::IsTerminal as _;

    // rpassword читает консольный ввод; без терминала процесс завис бы молча.
    if !std::io::stdin().is_terminal() {
        bail!(t!(
            "store-token требует интерактивного терминала (скрытый ввод читается с консоли). \
             Запустите команду в обычном окне терминала.",
            "store-token requires an interactive terminal (the hidden input is read from the console). \
             Run the command in a regular terminal window."
        ));
    }

    let token = rpassword::prompt_password(tf!(
        "Введите токен Weeek для записи «{account}» (ввод скрыт): ",
        "Enter the Weeek token for entry `{account}` (input hidden): "
    ))
    .context(t!("не удалось прочитать ввод", "failed to read the input"))?;
    let token = token.trim();
    if token.len() < 20 {
        bail!(t!(
            "токен подозрительно короткий (меньше 20 символов) — ничего не сохранено.",
            "the token looks too short (fewer than 20 characters) — nothing was saved."
        ));
    }

    let entry = keyring::Entry::new("weeek-mcp", account).with_context(|| {
        tf!(
            "нет доступа к системному хранилищу ({KEYCHAIN_LABEL})",
            "no access to the system keychain ({KEYCHAIN_LABEL})"
        )
    })?;
    entry.set_password(token).with_context(|| {
        tf!(
            "не удалось сохранить токен в хранилище ({KEYCHAIN_LABEL})",
            "failed to save the token to the keychain ({KEYCHAIN_LABEL})"
        )
    })?;
    let stored = entry.get_password().with_context(|| {
        tf!(
            "токен сохранён, но не читается обратно ({KEYCHAIN_LABEL})",
            "the token was saved but cannot be read back ({KEYCHAIN_LABEL})"
        )
    })?;
    if stored.trim() != token {
        bail!(t!(
            "проверка чтением не прошла — токен сохранён некорректно.",
            "the read-back check failed — the token was saved incorrectly."
        ));
    }

    println!(
        "{}",
        tf!(
            "✓ Токен сохранён: хранилище `weeek-mcp`, запись `{account}` ({KEYCHAIN_LABEL})",
            "✓ Token saved: service `weeek-mcp`, entry `{account}` ({KEYCHAIN_LABEL})"
        )
    );
    println!(
        "{}",
        tf!(
            "  Запуск сервера под этим аккаунтом: WEEEK_KEYCHAIN_ACCOUNT={account}",
            "  Run the server as this account: WEEEK_KEYCHAIN_ACCOUNT={account}"
        )
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    /// Полный проход через настоящее хранилище платформы: запись → чтение → удаление.
    /// Запуск вручную (нужно живое хранилище: Credential Manager / Keychain / Secret Service):
    /// `cargo test -- --ignored keychain_round_trip`
    #[test]
    #[ignore = "трогает настоящее системное хранилище (создаёт и удаляет тестовую запись)"]
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
