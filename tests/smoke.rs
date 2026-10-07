//! Integration smoke test: boots the built binary over stdio (no token needed)
//! and checks the tool surface in both modes. Run: cargo test

use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

struct Client {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl Client {
    fn start(overrides: &[(&str, &str)]) -> Self {
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

    fn initialize(&mut self) {
        self.send(json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": { "name": "weeeking-smoke", "version": "1.0.0" }
            }
        }));
        let response = self.wait_for(1);
        assert!(
            response.get("result").is_some(),
            "initialize failed: {response}"
        );
        self.send(json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));
    }

    fn list_tools(&mut self) -> Vec<Value> {
        self.send(json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {} }));
        let response = self.wait_for(2);
        response["result"]["tools"]
            .as_array()
            .cloned()
            .unwrap_or_default()
    }

    fn call_tool(&mut self, id: i64, name: &str, arguments: Value) -> Value {
        self.send(json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/call",
            "params": { "name": name, "arguments": arguments }
        }));
        self.wait_for(id)
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

fn tool_names(tools: &[Value]) -> Vec<String> {
    tools
        .iter()
        .filter_map(|tool| tool["name"].as_str().map(str::to_string))
        .collect()
}

#[test]
fn read_only_surface_is_safe() {
    let mut client = Client::start(&[]);
    let tools = client.list_tools();
    let names = tool_names(&tools);

    for expected in [
        "weeek_context",
        "weeek_search_tasks",
        "weeek_get_task",
        "weeek_list_comments",
        "weeek_download_attachment",
        "weeek_project",
        "weeek_board",
        "weeek_board_column",
        "weeek_deals",
        "weeek_custom_fields",
        "weeek_portfolio",
        "weeek_tags",
    ] {
        assert!(
            names.contains(&expected.to_string()),
            "read-only: missing tool {expected} (have: {names:?})"
        );
    }
    assert!(
        names.len() >= 15,
        "read-only: expected >=15 tools, got {}",
        names.len()
    );
    assert!(
        !names.contains(&"weeek_create_task".to_string()),
        "read-only: write tool must be hidden"
    );
    assert!(
        !names.contains(&"weeek_task".to_string()),
        "read-only: write-only task group must be hidden"
    );

    let call = client.call_tool(3, "weeek_context", json!({}));
    assert_eq!(
        call["result"]["isError"], true,
        "no-token call must return isError: {call}"
    );
    let text = call["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default();
    assert!(
        text.contains("WEEEK_API_TOKEN"),
        "no-token error must mention WEEEK_API_TOKEN: {text}"
    );
}

#[test]
fn writable_surface_exposes_mutations() {
    let mut client = Client::start(&[("READ_ONLY", "false")]);
    let tools = client.list_tools();
    let names = tool_names(&tools);

    for expected in [
        "weeek_create_task",
        "weeek_update_task",
        "weeek_move_task",
        "weeek_complete_task",
        "weeek_set_task_people",
        "weeek_add_comment",
        "weeek_delete_comment",
        "weeek_task",
        "weeek_project",
    ] {
        assert!(
            names.contains(&expected.to_string()),
            "read-write: missing tool {expected}"
        );
    }

    let project = tools
        .iter()
        .find(|tool| tool["name"] == "weeek_project")
        .expect("weeek_project tool must exist");
    let actions = project["inputSchema"]["properties"]["action"]["enum"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(
        actions.iter().any(|action| action == "create-project"),
        "writable project tool must offer create-project: {actions:?}"
    );
}
