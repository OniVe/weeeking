//! Общий харнесс интеграционных тестов: MCP-клиент поверх stdio.
#![allow(dead_code)]

use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

pub struct McpClient {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: i64,
}

impl McpClient {
    pub fn start(overrides: &[(&str, &str)]) -> Self {
        let mut command = Command::new(env!("CARGO_BIN_EXE_weeeking"));
        command
            .env("WEEEK_API_TOKEN", "")
            .env("WEEEK_TOKEN", "")
            .env("WEEEK_DISABLE_KEYCHAIN", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        for (key, value) in overrides {
            command.env(key, value);
        }
        let mut child = command.spawn().expect("failed to spawn weeeking");
        let stdin = child.stdin.take().expect("stdin");
        let stdout = BufReader::new(child.stdout.take().expect("stdout"));
        let mut client = Self {
            child,
            stdin,
            stdout,
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

    pub fn list_tools(&mut self) -> Vec<Value> {
        let id = self.next();
        self.send(json!({ "jsonrpc": "2.0", "id": id, "method": "tools/list", "params": {} }));
        let response = self.wait_for(id);
        response["result"]["tools"]
            .as_array()
            .cloned()
            .unwrap_or_default()
    }

    /// Возвращает `result` вызова инструмента (с полями `content`, `isError`).
    pub fn call_tool(&mut self, name: &str, arguments: Value) -> Value {
        let id = self.next();
        self.send(json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/call",
            "params": { "name": name, "arguments": arguments }
        }));
        let response = self.wait_for(id);
        assert!(
            response.get("result").is_some(),
            "tool call {name} failed at protocol level: {response}"
        );
        response["result"].clone()
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
