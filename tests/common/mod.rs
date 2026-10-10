//! Общий харнесс интеграционных тестов: MCP-клиент поверх stdio.
#![allow(dead_code)]

use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

pub struct McpClient {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    stderr_lines: Arc<Mutex<Vec<String>>>,
    next_id: i64,
}

impl McpClient {
    pub fn start(overrides: &[(&str, &str)]) -> Self {
        let mut command = Command::new(env!("CARGO_BIN_EXE_weeeking"));
        command
            .env("WEEEK_API_TOKEN", "")
            .env("WEEEK_TOKEN", "")
            .env("WEEEK_DISABLE_KEYCHAIN", "1")
            .env("WEEEK_LANG", "ru")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (key, value) in overrides {
            command.env(key, value);
        }
        let mut child = command.spawn().expect("failed to spawn weeeking");
        let stdin = child.stdin.take().expect("stdin");
        let stdout = BufReader::new(child.stdout.take().expect("stdout"));
        let stderr = child.stderr.take().expect("stderr");
        // stderr сливаем в фоне: иначе пайп может заполниться и заблокировать сервер.
        let stderr_lines = Arc::new(Mutex::new(Vec::new()));
        let sink = stderr_lines.clone();
        thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines().map_while(Result::ok) {
                sink.lock().expect("stderr sink").push(line);
            }
        });
        let mut client = Self {
            child,
            stdin,
            stdout,
            stderr_lines,
            next_id: 0,
        };
        client.initialize();
        client
    }

    fn send(&mut self, message: Value) {
        writeln!(self.stdin, "{message}").expect("write");
        self.stdin.flush().expect("flush");
    }

    fn wait_for(&mut self, id: i64) -> Value {
        loop {
            let mut line = String::new();
            let read = self.stdout.read_line(&mut line).expect("read");
            assert!(read > 0, "server closed stdout before response id={id}");
            if let Ok(value) = serde_json::from_str::<Value>(line.trim())
                && value.get("id").and_then(Value::as_i64) == Some(id)
            {
                return value;
            }
        }
    }

    fn next(&mut self) -> i64 {
        self.next_id += 1;
        self.next_id
    }

    fn initialize(&mut self) {
        let id = self.next();
        self.send(json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": { "name": "weeeking-tests", "version": "1.0.0" }
            }
        }));
        let response = self.wait_for(id);
        assert!(
            response.get("result").is_some(),
            "initialize failed: {response}"
        );
        self.send(json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));
    }

    /// Отправляет произвольный JSON-RPC запрос и возвращает полное сообщение
    /// (с `result` или `error`) — нужно для проверки error-путей ресурсов.
    pub fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next();
        self.send(json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        self.wait_for(id)
    }

    pub fn list_tools(&mut self) -> Vec<Value> {
        let response = self.request("tools/list", json!({}));
        response["result"]["tools"]
            .as_array()
            .cloned()
            .unwrap_or_default()
    }

    /// Возвращает `result` вызова инструмента (с полями `content`, `isError`).
    pub fn call_tool(&mut self, name: &str, arguments: Value) -> Value {
        let response = self.request(
            "tools/call",
            json!({ "name": name, "arguments": arguments }),
        );
        assert!(
            response.get("result").is_some(),
            "tool call {name} failed at protocol level: {response}"
        );
        response["result"].clone()
    }

    /// Ресурсы сервера (resources/list).
    pub fn list_resources(&mut self) -> Vec<Value> {
        let response = self.request("resources/list", json!({}));
        assert!(
            response.get("result").is_some(),
            "resources/list failed: {response}"
        );
        response["result"]["resources"]
            .as_array()
            .cloned()
            .unwrap_or_default()
    }

    /// Чтение ресурса (resources/read); паникует, если сервер вернул ошибку.
    pub fn read_resource(&mut self, uri: &str) -> Value {
        let response = self.request("resources/read", json!({ "uri": uri }));
        assert!(
            response.get("result").is_some(),
            "resources/read({uri}) failed: {response}"
        );
        response["result"].clone()
    }

    /// Список промптов (prompts/list).
    pub fn list_prompts(&mut self) -> Vec<Value> {
        let response = self.request("prompts/list", json!({}));
        assert!(
            response.get("result").is_some(),
            "prompts/list failed: {response}"
        );
        response["result"]["prompts"]
            .as_array()
            .cloned()
            .unwrap_or_default()
    }

    /// Получение промпта (prompts/get); паникует, если сервер вернул ошибку.
    pub fn get_prompt(&mut self, name: &str) -> Value {
        let response = self.request("prompts/get", json!({ "name": name }));
        assert!(
            response.get("result").is_some(),
            "prompts/get({name}) failed: {response}"
        );
        response["result"].clone()
    }

    pub fn stderr_snapshot(&self) -> String {
        self.stderr_lines.lock().expect("stderr sink").join("\n")
    }

    pub fn wait_for_stderr(&self, needle: &str, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if self.stderr_snapshot().contains(needle) {
                return true;
            }
            thread::sleep(Duration::from_millis(20));
        }
        false
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

pub fn tool_names(tools: &[Value]) -> Vec<String> {
    tools
        .iter()
        .filter_map(|tool| tool["name"].as_str().map(str::to_string))
        .collect()
}

pub fn result_text(result: &Value) -> String {
    result["content"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item["text"].as_str())
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}
