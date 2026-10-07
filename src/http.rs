use crate::config::Config;
use serde_json::{Value, json};
use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum WeeekError {
    #[error(
        "WEEEK_API_TOKEN не задан. Создайте токен в Weeek (Настройки workspace → API) и передайте его \
         в окружении MCP-сервера (WEEEK_API_TOKEN), либо задайте READ_ONLY=true только для чтения."
    )]
    Token,
    #[error("Ошибка Weeek API (HTTP {status}): {body}")]
    Api { status: u16, body: String },
    #[error("Сетевая ошибка или таймаут ({method} {path}): {message}")]
    Network {
        method: String,
        path: String,
        message: String,
    },
    #[error("Файловая ошибка: {0}")]
    Io(String),
}

pub struct WeeekClient {
    http: reqwest::Client,
    base_url: String,
    token: Option<String>,
}

impl WeeekClient {
    pub fn new(cfg: &Config, token: Option<String>) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_millis(cfg.timeout_ms))
            .build()
            .expect("failed to build the HTTP client");
        Self {
            http,
            base_url: cfg.base_url.clone(),
            token,
        }
    }

    fn require_token(&self) -> Result<&str, WeeekError> {
        self.token.as_deref().ok_or(WeeekError::Token)
    }

    pub async fn call(
        &self,
        method: &str,
        path: &str,
        query: &[(String, String)],
        body: Option<&Value>,
    ) -> Result<Value, WeeekError> {
        let token = self.require_token()?;
        let url = format!("{}{}", self.base_url, path);

        let http_method = match method {
            "POST" => reqwest::Method::POST,
            "PUT" => reqwest::Method::PUT,
            "PATCH" => reqwest::Method::PATCH,
            "DELETE" => reqwest::Method::DELETE,
            _ => reqwest::Method::GET,
        };
        let net = |e: reqwest::Error| WeeekError::Network {
            method: method.to_string(),
            path: path.to_string(),
            message: e.to_string(),
        };

        let mut request = self
            .http
            .request(http_method, &url)
            .bearer_auth(token)
            .header("Accept", "application/json");
        if !query.is_empty() {
            request = request.query(query);
        }
        if let Some(payload) = body {
            request = request.json(payload);
        }

        let response = request.send().await.map_err(net)?;
        let status = response.status();
        let text = response.text().await.map_err(net)?;
        if !status.is_success() {
            return Err(WeeekError::Api {
                status: status.as_u16(),
                body: text,
            });
        }
        if text.is_empty() {
            return Ok(Value::Null);
        }
        Ok(serde_json::from_str::<Value>(&text).unwrap_or(Value::String(text)))
    }

    /// Downloads an attachment into a temp directory and returns the saved path.
    pub async fn download(&self, path: &str) -> Result<Value, WeeekError> {
        let token = self.require_token()?;
        let url = format!("{}{}", self.base_url, path);
        let net = |e: reqwest::Error| WeeekError::Network {
            method: "GET".to_string(),
            path: path.to_string(),
            message: e.to_string(),
        };

        let response = self
            .http
            .get(&url)
            .bearer_auth(token)
            .send()
            .await
            .map_err(net)?;
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(WeeekError::Api { status, body });
        }
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("application/octet-stream")
            .to_string();
        let disposition = response
            .headers()
            .get(reqwest::header::CONTENT_DISPOSITION)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        let bytes = response.bytes().await.map_err(net)?;

        let mut name = parse_filename(&disposition).unwrap_or_default();
        if name.is_empty() {
            let base = path.rsplit('/').next().unwrap_or("attachment");
            let ext = mime_extension(&content_type);
            name = format!("attachment_{base}{ext}");
        }
        let name = sanitize_filename(&name);

        let dir = std::env::temp_dir().join("weeeking");
        std::fs::create_dir_all(&dir).map_err(|e| WeeekError::Io(e.to_string()))?;
        let file_path = dir.join(&name);
        std::fs::write(&file_path, &bytes).map_err(|e| WeeekError::Io(e.to_string()))?;

        Ok(json!({
            "filePath": file_path.display().to_string(),
            "bytes": bytes.len(),
            "contentType": content_type,
        }))
    }
}

fn parse_filename(disposition: &str) -> Option<String> {
    if disposition.is_empty() {
        return None;
    }
    if let Some(idx) = disposition.find("filename*=") {
        let rest = &disposition[idx + "filename*=".len()..];
        let rest = rest
            .split(';')
            .next()
            .unwrap_or(rest)
            .trim()
            .trim_matches('"');
        let encoded = rest.split("''").last().unwrap_or(rest);
        return Some(percent_decode(encoded));
    }
    if let Some(idx) = disposition.find("filename=") {
        let rest = &disposition[idx + "filename=".len()..];
        let rest = rest
            .split(';')
            .next()
            .unwrap_or(rest)
            .trim()
            .trim_matches('"');
        return Some(rest.to_string());
    }
    None
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(byte) = u8::from_str_radix(&s[i + 1..i + 3], 16)
        {
            out.push(byte);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

fn mime_extension(content_type: &str) -> &'static str {
    let key = content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    match key.as_str() {
        "image/png" => ".png",
        "image/jpeg" => ".jpg",
        "image/gif" => ".gif",
        "image/webp" => ".webp",
        "image/svg+xml" => ".svg",
        "application/pdf" => ".pdf",
        "text/plain" => ".txt",
        "text/csv" => ".csv",
        "application/zip" => ".zip",
        _ => "",
    }
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            other => other,
        })
        .collect()
}
