mod config;
mod curated;
mod generated;
mod http;
mod server;
mod spec;
mod spec_generated;
mod token;
mod util;

use rmcp::ServiceExt;

fn print_usage() {
    println!(
        "🛡️  Weeeking — MCP-сервер для Weeek (полный API, Weeek and conquer).\n\
         \n\
         Использование:\n\
         \x20 weeeking                                запустить MCP-сервер (stdio)\n\
         \x20 weeeking store-token [--account <имя>]  сохранить токен в системное хранилище\n\
         \x20 weeeking --help                         эта справка\n\
         \n\
         Токен ищется в порядке: WEEEK_API_TOKEN / WEEEK_TOKEN → системное хранилище\n\
         (запись `weeek-mcp`, имя аккаунта из WEEEK_KEYCHAIN_ACCOUNT, по умолчанию `api-token`).\n\
         \n\
         Основные переменные: READ_ONLY=true|false, WEEEK_BASE_URL, WEEEK_TIMEOUT_MS,\n\
         WEEEK_MAX_RESPONSE_CHARS, WEEEK_DISABLE_KEYCHAIN=1, WEEEK_KEYCHAIN_ACCOUNT."
    );
}

/// Разбирает имя аккаунта: `--account <имя>` или первый позиционный аргумент.
fn account_arg(rest: &[String]) -> String {
    for (index, arg) in rest.iter().enumerate() {
        if arg == "--account"
            && let Some(value) = rest.get(index + 1).filter(|value| !value.starts_with('-'))
        {
            return value.clone();
        }
    }
    rest.iter()
        .find(|arg| !arg.starts_with('-'))
        .cloned()
        .unwrap_or_else(|| "api-token".to_string())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("store-token") => {
            let account = config::keychain_account_from(Some(account_arg(&args[1..])));
            token::store_token(&account)?;
            return Ok(());
        }
        Some("--help") | Some("-h") => {
            print_usage();
            return Ok(());
        }
        Some(other) => {
            eprintln!("Неизвестная команда: {other}\n");
            print_usage();
            std::process::exit(2);
        }
        None => {}
    }

    let cfg = config::Config::from_env();
    let (token_value, source) = token::resolve(&cfg);
    let client = http::WeeekClient::new(&cfg, token_value);
    let server = server::WeeekingServer::new(client, cfg.read_only, cfg.max_chars);

    eprintln!(
        "[weeeking] готов. Спека: {} (chunk {}), {} операций — {}. READ_ONLY={}, токен: {}, API {}",
        spec::SPEC_TITLE,
        spec::SPEC_VERSION,
        spec::OPERATIONS.len(),
        spec::SPEC_URL,
        cfg.read_only,
        source.describe(),
        cfg.base_url
    );

    let running = server.serve(rmcp::transport::io::stdio()).await?;
    running.waiting().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::account_arg;

    fn v(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn account_flag_wins() {
        assert_eq!(
            account_arg(&v(&["--account", "api-token-anna"])),
            "api-token-anna"
        );
    }

    #[test]
    fn account_positional_fallback() {
        assert_eq!(account_arg(&v(&["api-token-bob"])), "api-token-bob");
    }

    #[test]
    fn account_default() {
        assert_eq!(account_arg(&[]), "api-token");
        assert_eq!(account_arg(&v(&["--account"])), "api-token");
        assert_eq!(account_arg(&v(&["--account", "--account"])), "api-token");
    }
}
