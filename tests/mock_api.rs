//! Mock-сюита: HTTP-слой сервера против локального эмулятора Weeek API.
//! Ни сети, ни токенов: ответы синтетические, формы — по мотивам реальных.

mod common;

use common::{McpClient, result_text};
use serde_json::{Value, json};
use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

struct Canned {
    status: u16,
    content_type: &'static str,
    body: Vec<u8>,
    disposition: Option<String>,
    retry_after: Option<String>,
    delay_ms: u64,
    /// true — ответ без Content-Length (chunked transfer).
    chunked: bool,
}

impl Canned {
    fn json(body: &str) -> Self {
        Self {
            status: 200,
            content_type: "application/json",
            body: body.as_bytes().to_vec(),
            disposition: None,
            retry_after: None,
            delay_ms: 0,
            chunked: false,
        }
    }

    fn status_json(status: u16, body: &str) -> Self {
        Self {
            status,
            ..Self::json(body)
        }
    }

    fn bytes(body: Vec<u8>) -> Self {
        Self {
            status: 200,
            content_type: "application/octet-stream",
            body,
            disposition: None,
            retry_after: None,
            delay_ms: 0,
            chunked: false,
        }
    }
}

struct MockServer {
    addr: String,
    requests: Arc<Mutex<Vec<(String, String)>>>,
}

impl MockServer {
    fn start(script: Vec<Canned>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("mock listener");
        let addr = format!("http://{}", listener.local_addr().expect("mock addr"));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let queue: Arc<Mutex<VecDeque<Canned>>> = Arc::new(Mutex::new(script.into()));
        {
            let requests = requests.clone();
            let queue = queue.clone();
            thread::spawn(move || {
                for incoming in listener.incoming() {
                    let Ok(stream) = incoming else { continue };
                    let requests = requests.clone();
                    let queue = queue.clone();
                    // Поток на соединение: параллельные запросы (tokio::join!)
                    // обслуживаются независимо — без общих пулов и без keep-alive.
                    thread::spawn(move || handle_connection(stream, requests, queue));
                }
            });
        }
        Self { addr, requests }
    }

    fn requests(&self) -> Vec<(String, String)> {
        self.requests.lock().unwrap().clone()
    }

    fn client(&self, extra_env: &[(&str, &str)]) -> McpClient {
        let mut env = vec![
            ("WEEEK_API_TOKEN", "test-token"),
            ("WEEEK_BASE_URL", self.addr.as_str()),
        ];
        env.extend_from_slice(extra_env);
        McpClient::start(&env)
    }
}

/// Обслуживает одно соединение: читает запрос, отдаёт следующий сценарный ответ.
fn handle_connection(
    mut stream: TcpStream,
    requests: Arc<Mutex<Vec<(String, String)>>>,
    queue: Arc<Mutex<VecDeque<Canned>>>,
) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let _ = stream.set_nodelay(true);
    let Ok(reader_stream) = stream.try_clone() else {
        return;
    };
    let mut reader = BufReader::new(reader_stream);

    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() || request_line.trim().is_empty() {
        return;
    }
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_string();
    let url = parts.next().unwrap_or_default().to_string();

    let mut content_length = 0usize;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).is_err() {
            return;
        }
        let header = header.trim_end();
        if header.is_empty() {
            break;
        }
        let lower = header.to_ascii_lowercase();
        if let Some(value) = lower.strip_prefix("content-length:") {
            content_length = value.trim().parse().unwrap_or(0);
        }
    }
    if content_length > 0 {
        let mut body = vec![0u8; content_length];
        if reader.read_exact(&mut body).is_err() {
            return;
        }
    }

    requests.lock().unwrap().push((method, url));

    let canned = queue.lock().unwrap().pop_front();
    let Some(item) = canned else {
        let _ = write_response(
            &mut stream,
            500,
            "text/plain",
            b"no script",
            None,
            None,
            false,
        );
        return;
    };
    if item.delay_ms > 0 {
        thread::sleep(Duration::from_millis(item.delay_ms));
    }
    let _ = write_response(
        &mut stream,
        item.status,
        item.content_type,
        &item.body,
        item.disposition.as_deref(),
        item.retry_after.as_deref(),
        item.chunked,
    );
}

#[allow(clippy::too_many_arguments)]
fn write_response(
    stream: &mut TcpStream,
    status: u16,
    content_type: &str,
    body: &[u8],
    disposition: Option<&str>,
    retry_after: Option<&str>,
    chunked: bool,
) -> std::io::Result<()> {
    let mut head = format!("HTTP/1.1 {status} {}\r\n", reason_phrase(status));
    head.push_str(&format!("Content-Type: {content_type}\r\n"));
    if let Some(disposition) = disposition {
        head.push_str(&format!("Content-Disposition: {disposition}\r\n"));
    }
    if let Some(retry_after) = retry_after {
        head.push_str(&format!("Retry-After: {retry_after}\r\n"));
    }
    if !chunked {
        head.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    head.push_str("Connection: close\r\n\r\n");
    stream.write_all(head.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()
}

fn reason_phrase(status: u16) -> &'static str {
    match status {
        200 => "OK",
        401 => "Unauthorized",
        404 => "Not Found",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        _ => "Status",
    }
}

#[test]
fn success_envelope_shapes() {
    let server = MockServer::start(vec![
        Canned::json(r#"{"success":true,"task":{"id":1,"title":"Demo","completed":false}}"#),
        Canned::json(r#"{"success":true,"comments":[{"id":10,"markdown":"привет"}]}"#),
    ]);
    let mut client = server.client(&[]);

    let call = client.call_tool("weeek_get_task", json!({ "taskId": 1 }));
    assert_ne!(call["isError"], true, "expected success: {call}");
    let text = result_text(&call);
    assert!(
        text.contains("\"task\"") && text.contains("привет"),
        "task and comments must be merged: {text}"
    );

    let requests = server.requests();
    assert!(
        requests
            .iter()
            .any(|(method, url)| method == "GET" && url == "/tm/tasks/1"),
        "task request must reach the mock: {requests:?}"
    );
    assert!(
        requests
            .iter()
            .any(|(_, url)| url.starts_with("/tm/tasks/1/comments") && url.contains("limit=20")),
        "comments request must carry pagination: {requests:?}"
    );
}

#[test]
fn http_error_body_is_capped() {
    let body = "x".repeat(5000);
    let server = MockServer::start(vec![Canned::status_json(500, &body)]);
    let mut client = server.client(&[]);

    let call = client.call_tool("weeek_get_task", json!({ "taskId": 1 }));
    assert_eq!(call["isError"], true, "500 must map to isError: {call}");
    let text = result_text(&call);
    assert!(text.contains("HTTP 500"), "status must be visible: {text}");
    assert!(
        text.contains("обрезано"),
        "oversized error body must be capped: {text}"
    );
    assert!(
        text.chars().count() < 2600,
        "capped error text must stay small, got {} chars",
        text.chars().count()
    );
}

#[test]
fn timeout_is_reported() {
    let server = MockServer::start(vec![Canned {
        delay_ms: 1500,
        ..Canned::json("{}")
    }]);
    // Таймауты не ретраятся даже с дефолтными повторами: одна попытка, одна ошибка.
    let mut client = server.client(&[("WEEEK_TIMEOUT_MS", "300")]);

    let call = client.call_tool("weeek_list_comments", json!({ "taskId": 1 }));
    assert_eq!(call["isError"], true, "timeout must map to isError: {call}");
    let text = result_text(&call);
    assert!(
        text.contains("таймаут") || text.contains("Сетевая ошибка"),
        "timeout must be reported: {text}"
    );
    assert_eq!(
        server.requests().len(),
        1,
        "таймаут израсходовал бюджет запроса — повтор не делается"
    );
}

#[test]
fn big_response_is_truncated() {
    let items: Vec<Value> = (0..200)
        .map(|index| json!({ "id": index, "title": "a".repeat(30) }))
        .collect();
    let body = json!({ "tasks": items }).to_string();
    let server = MockServer::start(vec![Canned::json(&body)]);
    let mut client = server.client(&[("WEEEK_MAX_RESPONSE_CHARS", "1200")]);

    let call = client.call_tool("weeek_search_tasks", json!({ "perPage": 50 }));
    assert_ne!(call["isError"], true, "search must succeed: {call}");
    let text = result_text(&call);
    assert!(
        text.contains("обрезан"),
        "oversized response must carry the truncation marker"
    );
}

#[test]
fn attachment_filename_is_sanitized() {
    let server = MockServer::start(vec![Canned {
        disposition: Some("attachment; filename=\"CON\"".to_string()),
        ..Canned::bytes(b"hello".to_vec())
    }]);
    let mut client = server.client(&[]);

    let call = client.call_tool("weeek_download_attachment", json!({ "fileId": "f1" }));
    assert_ne!(call["isError"], true, "download must succeed: {call}");
    let text = result_text(&call);
    let parsed: Value = serde_json::from_str(&text).expect("download result must be JSON");
    let file_path = parsed["filePath"].as_str().expect("filePath").to_string();
    assert!(
        file_path.ends_with("_CON"),
        "reserved device name must be prefixed: {file_path}"
    );
    assert!(
        server
            .requests()
            .iter()
            .any(|(_, url)| url == "/ws/attachments/f1"),
        "attachment request path must match the spec"
    );
    let _ = std::fs::remove_file(file_path);
}

#[test]
fn attachment_over_limit_is_rejected() {
    let server = MockServer::start(vec![Canned::bytes(vec![b'x'; 3000])]);
    let mut client = server.client(&[("WEEEK_MAX_ATTACHMENT_BYTES", "1024")]);

    let call = client.call_tool("weeek_download_attachment", json!({ "fileId": "big" }));
    assert_eq!(
        call["isError"], true,
        "oversized attachment must fail: {call}"
    );
    let text = result_text(&call);
    assert!(
        text.contains("лимита"),
        "limit violation must be explained: {text}"
    );
}

#[test]
fn attachment_stream_cap_without_content_length() {
    let server = MockServer::start(vec![Canned {
        chunked: true,
        ..Canned::bytes(vec![b'x'; 3000])
    }]);
    let mut client = server.client(&[("WEEEK_MAX_ATTACHMENT_BYTES", "1024")]);

    let call = client.call_tool("weeek_download_attachment", json!({ "fileId": "chunked" }));
    assert_eq!(call["isError"], true, "chunked overrun must fail: {call}");
    let text = result_text(&call);
    assert!(
        text.contains("превысило лимит"),
        "streaming cap must be reported: {text}"
    );
}

#[test]
fn english_texts_follow_weeek_lang() {
    // Обрезка большого ответа.
    let items: Vec<Value> = (0..200)
        .map(|index| json!({ "id": index, "title": "a".repeat(30) }))
        .collect();
    let body = json!({ "tasks": items }).to_string();
    let server = MockServer::start(vec![Canned::json(&body)]);
    let mut client = server.client(&[("WEEEK_MAX_RESPONSE_CHARS", "1200"), ("WEEEK_LANG", "en")]);

    let call = client.call_tool("weeek_search_tasks", json!({ "perPage": 50 }));
    let text = result_text(&call);
    assert!(
        text.contains("response truncated"),
        "en truncation marker: {text}"
    );
    assert!(!text.contains("обрезан"), "no Russian leftovers: {text}");

    // Ошибка API и обрезка её тела.
    let server = MockServer::start(vec![Canned::status_json(500, &"x".repeat(5000))]);
    let mut client = server.client(&[("WEEEK_LANG", "en")]);

    let call = client.call_tool("weeek_get_task", json!({ "taskId": 1 }));
    let text = result_text(&call);
    assert!(
        text.contains("Weeek API error (HTTP 500)"),
        "en api error: {text}"
    );
    assert!(text.contains("[truncated]"), "en cap marker: {text}");
    assert!(!text.contains("обрезано"), "no Russian leftovers: {text}");

    // Лимит вложения.
    let server = MockServer::start(vec![Canned::bytes(vec![b'x'; 3000])]);
    let mut client = server.client(&[("WEEEK_MAX_ATTACHMENT_BYTES", "1024"), ("WEEEK_LANG", "en")]);

    let call = client.call_tool("weeek_download_attachment", json!({ "fileId": "big" }));
    assert_eq!(
        call["isError"], true,
        "oversized attachment must fail: {call}"
    );
    let text = result_text(&call);
    assert!(
        text.contains("larger than the limit"),
        "en attachment limit: {text}"
    );

    // Сетевая ошибка/таймаут.
    let server = MockServer::start(vec![Canned {
        delay_ms: 1500,
        ..Canned::json("{}")
    }]);
    let mut client = server.client(&[("WEEEK_TIMEOUT_MS", "300"), ("WEEEK_LANG", "en")]);

    let call = client.call_tool("weeek_list_comments", json!({ "taskId": 1 }));
    assert_eq!(call["isError"], true, "timeout must fail: {call}");
    let text = result_text(&call);
    assert!(
        text.contains("Network error or timeout"),
        "en network error: {text}"
    );
    assert!(
        !text.contains("Сетевая ошибка"),
        "no Russian leftovers: {text}"
    );
}

#[test]
fn boolean_query_is_serialized_as_bit() {
    // Weeek API принимает булевы query-параметры только как 1/0.
    let server = MockServer::start(vec![
        Canned::json(r#"{"tasks":[]}"#),
        Canned::json(r#"{"tasks":[]}"#),
    ]);
    let mut client = server.client(&[]);

    let call = client.call_tool(
        "weeek_search_tasks",
        json!({ "completed": false, "perPage": 3 }),
    );
    assert_ne!(call["isError"], true, "search must succeed: {call}");
    let requests = server.requests();
    assert!(
        requests.iter().any(|(_, url)| url.contains("completed=0")),
        "false must go as 0: {requests:?}"
    );

    let call = client.call_tool("weeek_search_tasks", json!({ "completed": true }));
    assert_ne!(call["isError"], true, "search must succeed: {call}");
    let requests = server.requests();
    assert!(
        requests.iter().any(|(_, url)| url.contains("completed=1")),
        "true must go as 1: {requests:?}"
    );
}

#[test]
fn unauthorized_is_reported() {
    let server = MockServer::start(vec![Canned::status_json(
        401,
        r#"{"message":"Unauthorized"}"#,
    )]);
    let mut client = server.client(&[]);

    let call = client.call_tool("weeek_get_task", json!({ "taskId": 1 }));
    assert_eq!(call["isError"], true, "401 must map to isError: {call}");
    let text = result_text(&call);
    assert!(text.contains("HTTP 401"), "status must be visible: {text}");
}

#[test]
fn retry_on_429_then_success() {
    let server = MockServer::start(vec![
        Canned::status_json(429, r#"{"message":"rate limited"}"#),
        Canned::json(r#"{"success":true,"comments":[]}"#),
    ]);
    let mut client = server.client(&[("WEEEK_RETRY_BASE_MS", "10")]);

    let call = client.call_tool("weeek_list_comments", json!({ "taskId": 1 }));
    assert_ne!(
        call["isError"], true,
        "после 429 запрос должен повториться: {call}"
    );
    assert_eq!(server.requests().len(), 2, "ровно один повтор");
}

#[test]
fn retry_after_header_is_honored() {
    let server = MockServer::start(vec![
        Canned {
            retry_after: Some("1".to_string()),
            ..Canned::status_json(429, "{}")
        },
        Canned::json(r#"{"success":true,"comments":[]}"#),
    ]);
    let mut client = server.client(&[("WEEEK_RETRY_BASE_MS", "10")]);

    let started = Instant::now();
    let call = client.call_tool("weeek_list_comments", json!({ "taskId": 1 }));
    assert_ne!(call["isError"], true);
    assert!(
        started.elapsed() >= Duration::from_millis(700),
        "Retry-After: 1 должен задержать повтор (~1 с), а не бэкофф 10 мс: {:?}",
        started.elapsed()
    );
    assert_eq!(server.requests().len(), 2);
}

#[test]
fn get_retries_on_503_then_succeeds() {
    let server = MockServer::start(vec![
        Canned::status_json(503, "oops"),
        Canned::json(r#"{"success":true,"comments":[]}"#),
    ]);
    let mut client = server.client(&[("WEEEK_RETRY_BASE_MS", "10")]);

    let call = client.call_tool("weeek_list_comments", json!({ "taskId": 1 }));
    assert_ne!(
        call["isError"], true,
        "GET должен повториться после 503: {call}"
    );
    assert_eq!(server.requests().len(), 2);
}

#[test]
fn post_is_not_retried_on_5xx() {
    let server = MockServer::start(vec![Canned::status_json(500, "boom")]);
    let mut client = server.client(&[("READ_ONLY", "false"), ("WEEEK_RETRY_BASE_MS", "10")]);

    let call = client.call_tool("weeek_create_task", json!({ "title": "t", "projectId": 1 }));
    assert_eq!(
        call["isError"], true,
        "500 на POST — ошибка без повторов: {call}"
    );
    assert_eq!(server.requests().len(), 1, "POST не повторяем при 5xx");
    assert!(result_text(&call).contains("HTTP 500"));
}

#[test]
fn retries_are_exhausted() {
    let server = MockServer::start(vec![
        Canned::status_json(503, "a"),
        Canned::status_json(503, "b"),
        Canned::status_json(503, "c"),
    ]);
    let mut client = server.client(&[("WEEEK_RETRY_MAX", "2"), ("WEEEK_RETRY_BASE_MS", "10")]);

    let call = client.call_tool("weeek_list_comments", json!({ "taskId": 1 }));
    assert_eq!(call["isError"], true);
    assert_eq!(server.requests().len(), 3, "1 попытка + 2 ретрая");
    assert!(result_text(&call).contains("HTTP 503"));
}

#[test]
fn debug_log_lines_are_emitted() {
    let server = MockServer::start(vec![Canned::json(r#"{"success":true,"comments":[]}"#)]);
    let mut client = server.client(&[("WEEEK_LOG", "debug")]);

    let call = client.call_tool("weeek_list_comments", json!({ "taskId": 1 }));
    assert_ne!(call["isError"], true);
    assert!(
        client.wait_for_stderr("GET /tm/tasks/1/comments", Duration::from_millis(2000)),
        "debug-лог должен содержать метод и путь: {}",
        client.stderr_snapshot()
    );
    let log = client.stderr_snapshot();
    assert!(log.contains("-> 200"), "в логе должен быть статус: {log}");
    assert!(
        !log.contains("test-token"),
        "токен не должен попадать в логи: {log}"
    );
}

#[test]
fn compact_strips_nulls_and_empties() {
    let server = MockServer::start(vec![
        Canned::json(
            r#"{"success":true,"task":{"id":1,"title":"T","description":"","assignees":[],"customFields":{},"dueDate":null,"completed":false}}"#,
        ),
        Canned::json(r#"{"success":true,"comments":[]}"#),
    ]);
    let mut client = server.client(&[]);

    let call = client.call_tool("weeek_get_task", json!({ "taskId": 1, "compact": true }));
    assert_ne!(call["isError"], true, "compact call must succeed: {call}");
    let parsed: Value = serde_json::from_str(&result_text(&call)).expect("json");
    // Ответ get_task — сырой конверт API: {"task": {"success": true, "task": {...}}}.
    let task = &parsed["task"]["task"];
    assert_eq!(task["completed"], false, "false не должен исчезать");
    assert!(task.get("dueDate").is_none(), "null убран: {task}");
    assert!(task.get("description").is_none(), "пустая строка убрана");
    assert!(task.get("assignees").is_none(), "пустой массив убран");
    assert!(task.get("customFields").is_none(), "пустой объект убран");
}

#[test]
fn fetch_all_pages_are_merged() {
    let server = MockServer::start(vec![
        Canned::json(r#"{"tasks":[{"id":1}],"hasMore":true}"#),
        Canned::json(r#"{"tasks":[{"id":2}],"hasMore":false}"#),
    ]);
    let mut client = server.client(&[]);

    let call = client.call_tool(
        "weeek_search_tasks",
        json!({ "fetchAll": true, "perPage": 1 }),
    );
    assert_ne!(call["isError"], true, "fetchAll must succeed: {call}");
    let parsed: Value = serde_json::from_str(&result_text(&call)).expect("json");
    assert_eq!(parsed["tasks"], json!([{ "id": 1 }, { "id": 2 }]));
    assert_eq!(parsed["truncated"], false);
    let requests = server.requests();
    assert_eq!(requests.len(), 2);
    assert!(
        requests[1].1.contains("offset=1"),
        "вторая страница с offset=1: {requests:?}"
    );
}

#[test]
fn fetch_all_respects_max_items() {
    let server = MockServer::start(vec![Canned::json(
        r#"{"tasks":[{"id":1},{"id":2},{"id":3}],"hasMore":true}"#,
    )]);
    let mut client = server.client(&[]);

    let call = client.call_tool(
        "weeek_search_tasks",
        json!({ "fetchAll": true, "maxItems": 2 }),
    );
    assert_ne!(call["isError"], true);
    let parsed: Value = serde_json::from_str(&result_text(&call)).expect("json");
    assert_eq!(parsed["tasks"], json!([{ "id": 1 }, { "id": 2 }]));
    assert_eq!(parsed["truncated"], true, "потолок достигнут — truncated");
    assert_eq!(
        server.requests().len(),
        1,
        "потолок достигнут на первой странице"
    );
}

#[test]
fn fetch_all_without_has_more_stops_on_partial_page() {
    // API не отдал hasMore: полная страница — идём дальше, неполная — стоп.
    let server = MockServer::start(vec![
        Canned::json(r#"{"tasks":[{"id":1}]}"#),
        Canned::json(r#"{"tasks":[]}"#),
    ]);
    let mut client = server.client(&[]);

    let call = client.call_tool(
        "weeek_search_tasks",
        json!({ "fetchAll": true, "perPage": 1 }),
    );
    assert_ne!(call["isError"], true);
    let parsed: Value = serde_json::from_str(&result_text(&call)).expect("json");
    assert_eq!(parsed["tasks"], json!([{ "id": 1 }]));
    assert_eq!(parsed["truncated"], false, "естественный конец, не потолок");
    assert_eq!(
        server.requests().len(),
        2,
        "неполная страница завершает сбор"
    );
}

#[test]
fn fetch_all_comments_merges_pages() {
    let server = MockServer::start(vec![
        Canned::json(r#"{"success":true,"comments":[{"id":1}],"hasMore":true}"#),
        Canned::json(r#"{"success":true,"comments":[{"id":2}],"hasMore":false}"#),
    ]);
    let mut client = server.client(&[]);

    let call = client.call_tool(
        "weeek_list_comments",
        json!({ "taskId": 5, "fetchAll": true, "limit": 1 }),
    );
    assert_ne!(call["isError"], true);
    let parsed: Value = serde_json::from_str(&result_text(&call)).expect("json");
    assert_eq!(parsed["comments"], json!([{ "id": 1 }, { "id": 2 }]));
    assert_eq!(parsed["truncated"], false);
    assert_eq!(server.requests().len(), 2);
}

#[test]
fn resources_read_from_context_cache() {
    // Пять запросов контекста приходят параллельно — отдаём одинаковые конверты,
    // чтобы результат не зависел от порядка доставки.
    let full = r#"{"success":true,"user":{"id":"u1","firstName":"Test"},"workspace":{"id":1,"title":"WS"},"members":[{"id":"m1"}],"tags":[{"id":2}],"projects":[{"id":7,"name":"P"}]}"#;
    let server = MockServer::start(vec![
        Canned::json(full),
        Canned::json(full),
        Canned::json(full),
        Canned::json(full),
        Canned::json(full),
    ]);
    let mut client = server.client(&[]);

    let resources = client.list_resources();
    assert_eq!(resources.len(), 2, "ресурсы: {resources:?}");
    assert!(resources.iter().any(|r| r["uri"] == "weeek://me"));

    let me = client.read_resource("weeek://me");
    let text = me["contents"][0]["text"].as_str().expect("text");
    assert!(text.contains("u1"), "user JSON: {text}");

    // Контекст закэширован: второе чтение не ходит в сеть.
    let projects = client.read_resource("weeek://projects");
    let text = projects["contents"][0]["text"].as_str().expect("text");
    assert!(text.contains("\"id\": 7"), "projects JSON: {text}");
    assert_eq!(
        server.requests().len(),
        5,
        "контекст закэширован после первого чтения"
    );
}
