mod config;
mod curated;
mod generated;
mod http;
mod server;
mod spec;
mod token;
mod util;

use rmcp::ServiceExt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = config::Config::from_env();
    let (token, source) = token::resolve(&cfg);
    let client = http::WeeekClient::new(&cfg, token);
    let server = server::WeeekingServer::new(client, cfg.read_only, cfg.max_chars);

    let spec = spec::Spec::load();
    eprintln!(
        "[weeeking] готов. Спека: {} от {} ({} операций) — {}. READ_ONLY={}, токен: {}, API {}",
        spec.spec_title,
        spec.generated_at,
        spec.operations.len(),
        spec.spec_url,
        cfg.read_only,
        source.as_str(),
        cfg.base_url
    );

    let running = server.serve(rmcp::transport::io::stdio()).await?;
    running.waiting().await?;
    Ok(())
}
