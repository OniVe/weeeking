//! Интеграционный smoke: поверхность инструментов в обоих режимах (без токена).
//! Run: cargo test

mod common;

use common::{McpClient, result_text, tool_names};
use serde_json::json;
use std::process::Command;

#[test]
fn read_only_surface_is_safe() {
    let mut client = McpClient::start(&[]);
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

    let call = client.call_tool("weeek_context", json!({}));
    assert_eq!(
        call["isError"], true,
        "no-token call must return isError: {call}"
    );
    let text = result_text(&call);
    assert!(
        text.contains("WEEEK_API_TOKEN"),
        "no-token error must mention WEEEK_API_TOKEN: {text}"
    );
}

#[test]
fn writable_surface_exposes_mutations() {
    let mut client = McpClient::start(&[("READ_ONLY", "false")]);
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

#[test]
fn cli_help_mentions_store_token() {
    let output = Command::new(env!("CARGO_BIN_EXE_weeeking"))
        .arg("--help")
        .output()
        .expect("run --help");
    assert!(output.status.success(), "--help must exit 0");
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        text.contains("store-token"),
        "--help must mention store-token: {text}"
    );
    assert!(
        text.contains("WEEEK_KEYCHAIN_ACCOUNT"),
        "--help must mention the account env var"
    );
}

#[test]
fn cli_unknown_command_exits_2() {
    let output = Command::new(env!("CARGO_BIN_EXE_weeeking"))
        .arg("definitely-not-a-command")
        .output()
        .expect("run unknown command");
    assert_eq!(
        output.status.code(),
        Some(2),
        "unknown command must exit with code 2"
    );
}

#[test]
fn required_params_are_validated_client_side() {
    let mut client = McpClient::start(&[("READ_ONLY", "false")]);

    let boards = client.call_tool("weeek_board", json!({ "action": "get-boards" }));
    assert_eq!(
        boards["isError"], true,
        "get-boards without projectId must fail"
    );
    let text = result_text(&boards);
    assert!(
        text.contains("обязательный query-параметр"),
        "get-boards without projectId must be caught client-side: {text}"
    );

    let project = client.call_tool("weeek_project", json!({ "action": "create-project" }));
    assert_eq!(
        project["isError"], true,
        "create-project without name must fail"
    );
    let text = result_text(&project);
    assert!(
        text.contains("(body)"),
        "create-project without name must be caught client-side: {text}"
    );
}

#[test]
fn dot_segment_path_params_are_rejected() {
    let mut client = McpClient::start(&[]);
    let call = client.call_tool(
        "weeek_project",
        json!({ "action": "get-project", "params": { "id": ".." } }),
    );
    assert_eq!(call["isError"], true, "dot-segment id must be rejected");
    let text = result_text(&call);
    assert!(
        text.contains("сегменты"),
        "dot-segment rejection must explain the reason: {text}"
    );
}

#[test]
fn explicit_null_optional_query_is_ignored() {
    let mut client = McpClient::start(&[]);
    let call = client.call_tool(
        "weeek_contacts",
        json!({ "action": "get-contacts", "params": { "search": null } }),
    );
    let text = result_text(&call);
    assert!(
        !text.contains("не принимает параметры"),
        "explicit null must not become an unexpected-params error: {text}"
    );
    assert!(
        text.contains("WEEEK_API_TOKEN"),
        "call must pass validation and reach the token check: {text}"
    );
}

#[test]
fn wrong_param_types_are_rejected() {
    let mut client = McpClient::start(&[]);
    let call = client.call_tool(
        "weeek_board",
        json!({ "action": "get-boards", "params": { "projectId": "abc" } }),
    );
    assert_eq!(call["isError"], true);
    let text = result_text(&call);
    assert!(
        text.contains("ожидает тип"),
        "wrong type must be rejected client-side: {text}"
    );
}

#[test]
fn path_params_are_validated_for_empty_and_type() {
    let mut client = McpClient::start(&[]);

    let empty = client.call_tool(
        "weeek_project",
        json!({ "action": "get-project", "params": { "id": "" } }),
    );
    let text = result_text(&empty);
    assert!(
        text.contains("пустая строка"),
        "empty path param must be rejected: {text}"
    );

    let wrong = client.call_tool(
        "weeek_tags",
        json!({ "action": "get-tag", "params": { "id": "abc" } }),
    );
    let text = result_text(&wrong);
    assert!(
        text.contains("ожидает тип"),
        "wrong-type path param must be rejected: {text}"
    );

    let valid = client.call_tool(
        "weeek_tags",
        json!({ "action": "get-tag", "params": { "id": 1 } }),
    );
    let text = result_text(&valid);
    assert!(
        text.contains("WEEEK_API_TOKEN"),
        "valid path param must pass validation and reach the token check: {text}"
    );

    // string-типизированный id: числовое значение коэрсится в строку (LLM-эргономика)
    let coerced = client.call_tool(
        "weeek_project",
        json!({ "action": "get-project", "params": { "id": 42 } }),
    );
    let text = result_text(&coerced);
    assert!(
        text.contains("WEEEK_API_TOKEN"),
        "numeric id for a string path param must be coerced and reach the token check: {text}"
    );
}

#[test]
fn resources_and_prompts_are_exposed() {
    let mut client = McpClient::start(&[]);

    let resources = client.list_resources();
    let uris: Vec<&str> = resources
        .iter()
        .filter_map(|resource| resource["uri"].as_str())
        .collect();
    assert!(
        uris.contains(&"weeek://me"),
        "ресурс пользователя: {uris:?}"
    );
    assert!(
        uris.contains(&"weeek://projects"),
        "ресурс проектов: {uris:?}"
    );

    // Без токена чтение даёт понятную JSON-RPC ошибку, а не пустой ответ.
    let response = client.request("resources/read", json!({ "uri": "weeek://me" }));
    let message = response["error"]["message"].as_str().unwrap_or_default();
    assert!(
        message.contains("WEEEK_API_TOKEN"),
        "ошибка чтения должна упоминать токен: {response}"
    );

    let unknown = client.request("resources/read", json!({ "uri": "weeek://nope" }));
    assert!(
        unknown.get("error").is_some(),
        "неизвестный ресурс отвергается: {unknown}"
    );

    let prompts = client.list_prompts();
    let names: Vec<&str> = prompts
        .iter()
        .filter_map(|prompt| prompt["name"].as_str())
        .collect();
    assert!(names.contains(&"my-tasks-today"), "промпты: {names:?}");
    assert!(names.contains(&"week-review"), "промпты: {names:?}");

    let prompt = client.get_prompt("my-tasks-today");
    let text = prompt["messages"][0]["content"]["text"]
        .as_str()
        .unwrap_or_default();
    assert!(
        text.contains("weeek_search_tasks"),
        "промпт должен направлять к инструментам: {text}"
    );

    let unknown_prompt = client.request("prompts/get", json!({ "name": "nope" }));
    assert!(
        unknown_prompt.get("error").is_some(),
        "неизвестный промпт отвергается: {unknown_prompt}"
    );
}
