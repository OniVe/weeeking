use crate::config::Config;
use futures_util::StreamExt;
use serde_json::{Value, json};
use std::time::Duration;

/// Тело ошибки в сообщении ограничиваем, чтобы огромный ответ API не раздувал ответ тула.
const ERROR_BODY_MAX_CHARS: usize = 2000;

fn cap_error_body(mut body: String) -> String {
    if body.chars().count() > ERROR_BODY_MAX_CHARS {
        body = body.chars().take(ERROR_BODY_MAX_CHARS).collect::<String>() + "…[обрезано]";
    }
    body
}

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
    max_attachment_bytes: usize,
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
            max_attachment_bytes: cfg.max_attachment_bytes,
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
                body: cap_error_body(text),
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
            let body = cap_error_body(response.text().await.unwrap_or_default());
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

        // Скачиваем с жёстким лимитом: без него большое вложение съело бы всю память.
        let max_bytes = self.max_attachment_bytes;
        if let Some(length) = response.content_length()
            && length as usize > max_bytes
        {
            return Err(WeeekError::Io(format!(
                "вложение больше лимита ({length} > {max_bytes} байт). Увеличьте WEEEK_MAX_ATTACHMENT_BYTES, если это ожидаемо."
            )));
        }
        let mut bytes: Vec<u8> = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(net)?;
            if bytes.len() + chunk.len() > max_bytes {
                return Err(WeeekError::Io(format!(
                    "вложение превысило лимит {max_bytes} байт — скачивание остановлено (WEEEK_MAX_ATTACHMENT_BYTES)."
                )));
            }
            bytes.extend_from_slice(&chunk);
        }

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
            && let (Some(high), Some(low)) = (hex_digit(bytes[i + 1]), hex_digit(bytes[i + 2]))
        {
            out.push(high * 16 + low);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
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
    let replaced: String = name
        .chars()
        .map(|c| match c {
            '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            other => other,
        })
        .collect();
    // Windows молча отбрасывает хвостовые точки и пробелы — срезаем сами.
    let trimmed = replaced.trim_end_matches([' ', '.']).to_string();
    let candidate = if trimmed.is_empty() {
        "attachment".to_string()
    } else {
        trimmed
    };
    // Зарезервированные имена устройств (CON, NUL, COM1–9, LPT1–9, …) нельзя занимать файлом.
    let base = candidate
        .split('.')
        .next()
        .unwrap_or("")
        .to_ascii_uppercase();
    let reserved = matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (base.len() == 4
            && (base.starts_with("COM") || base.starts_with("LPT"))
            && base.as_bytes()[3].is_ascii_digit()
            && base.as_bytes()[3] != b'0');
    if reserved {
        format!("_{candidate}")
    } else {
        candidate
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_rejects_windows_reserved_names() {
        assert_eq!(sanitize_filename("CON"), "_CON");
        assert_eq!(sanitize_filename("con.txt"), "_con.txt");
        assert_eq!(sanitize_filename("LPT9.pdf"), "_LPT9.pdf");
        assert_eq!(sanitize_filename("COM0.txt"), "COM0.txt");
        assert_eq!(sanitize_filename("report.docx"), "report.docx");
    }

    #[test]
    fn sanitize_strips_trailing_dots_and_spaces() {
        assert_eq!(sanitize_filename("name.txt. "), "name.txt");
        assert_eq!(sanitize_filename("..."), "attachment");
        assert_eq!(sanitize_filename("a/b:c"), "a_b_c");
    }

    #[test]
    fn percent_decode_is_byte_safe() {
        assert_eq!(percent_decode("%D1%82%D0%B5%D1%81%D1%82"), "тест");
        // многобайтный символ перед '%' раньше мог вызвать панику по границе char
        assert_eq!(percent_decode("€%2E"), "€.");
        assert_eq!(percent_decode("100%"), "100%");
        assert_eq!(percent_decode("bad%zz"), "bad%zz");
    }
}
