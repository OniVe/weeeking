//! Mock-сюита: HTTP-слой сервера против локального эмулятора Weeek API.
//! Ни сети, ни токенов: ответы синтетические, формы — по мотивам реальных.

mod common;

use common::{McpClient, result_text};
use serde_json::{Value, json};
use std::collections::VecDeque;
use std::io::Cursor;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tiny_http::{Header, ListenAddr, Response, Server, StatusCode};

struct Canned {
    status: u16,
    content_type: &'static str,
    body: Vec<u8>,
    disposition: Option<String>,
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
        let server = Server::http("127.0.0.1:0").expect("mock server");
        let addr = match server.server_addr() {
            ListenAddr::IP(socket) => format!("http://{socket}"),
            #[allow(unreachable_patterns)]
            _ => panic!("ожидается TCP-адрес"),
        };
        let requests = Arc::new(Mutex::new(Vec::new()));
        let queue: Arc<Mutex<VecDeque<Canned>>> = Arc::new(Mutex::new(script.into()));
        let requests_thread = requests.clone();
        let queue_thread = queue.clone();
        thread::spawn(move || {
            for request in server.incoming_requests() {
                requests_thread
                    .lock()
                    .unwrap()
                    .push((request.method().to_string(), request.url().to_string()));
                let canned = queue_thread.lock().unwrap().pop_front();
                match canned {
                    Some(item) => {
                        if item.delay_ms > 0 {
                            thread::sleep(Duration::from_millis(item.delay_ms));
                        }
                        let mut headers = vec![
                            Header::from_bytes(&b"Content-Type"[..], item.content_type.as_bytes())
                                .unwrap(),
                        ];
                        if let Some(disposition) = &item.disposition {
                            headers.push(
                                Header::from_bytes(
                                    &b"Content-Disposition"[..],
                                    disposition.as_bytes(),
                                )
                                .unwrap(),
                            );
                        }
                        let data = Cursor::new(item.body.clone());
                        let length = if item.chunked {
                            None
                        } else {
                            Some(item.body.len())
                        };
                        let response =
                            Response::new(StatusCode(item.status), headers, data, length, None);
                        let _ = request.respond(response);
                    }
                    None => {
                        let _ = request
                            .respond(Response::from_string("no script").with_status_code(500));
                    }
                }
            }
        });
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
    let mut client = server.client(&[("WEEEK_TIMEOUT_MS", "300")]);

    let call = client.call_tool("weeek_context", json!({}));
    assert_eq!(call["isError"], true, "timeout must map to isError: {call}");
    let text = result_text(&call);
    assert!(
        text.contains("таймаут") || text.contains("Сетевая ошибка"),
        "timeout must be reported: {text}"
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
