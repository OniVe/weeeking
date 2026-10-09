use crate::curated;
use crate::generated;
use crate::http::WeeekClient;
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, GetPromptRequestParams,
    GetPromptResponse, GetPromptResult, Implementation, ListPromptsResult, ListResourcesResult,
    ListToolsResult, PaginatedRequestParams, Prompt, PromptMessage, ReadResourceRequestParams,
    ReadResourceResponse, ReadResourceResult, Resource, ResourceContents, Role, ServerCapabilities,
    ServerConfig, Tool,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

pub struct WeeekingServer {
    pub(crate) client: WeeekClient,
    pub(crate) read_only: bool,
    pub(crate) max_chars: usize,
    tools: Vec<Tool>,
    group_ops: HashMap<String, Vec<String>>,
    pub(crate) context_cache: Mutex<Option<(Instant, Value)>>,
}

impl WeeekingServer {
    pub fn new(client: WeeekClient, read_only: bool, max_chars: usize) -> Self {
        let (gen_tools, group_ops) = generated::build(read_only);
        let mut tools = gen_tools;
        tools.extend(curated::tools(read_only));

        Self {
            client,
            read_only,
            max_chars,
            tools,
            group_ops,
            context_cache: Mutex::new(None),
        }
    }

    async fn dispatch(&self, name: &str, args: Map<String, Value>) -> CallToolResult {
        if curated::is_curated(name, self.read_only) {
            return self.curated_run(name, args).await;
        }
        if let Some(allowed) = self.group_ops.get(name).cloned() {
            return match self.generated_run(&allowed, args).await {
                Ok(value) => crate::util::json_result(&value, self.max_chars),
                Err(message) => crate::util::error_result(message),
            };
        }
        crate::util::error_result(format!("Неизвестный инструмент: {name}"))
    }
}

impl ServerHandler for WeeekingServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .enable_prompts()
                .build(),
        )
        .with_server_info(Implementation::new("weeeking", env!("CARGO_PKG_VERSION")))
        .with_instructions(
            "Weeeking — полный API Weeek (задачи, проекты, доски, CRM, время, поля). Начните с weeek_context. \
             Контекст доступен и как ресурсы: weeek://me, weeek://projects.",
        )
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult::with_all_items(self.tools.clone()))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        let args = request.arguments.unwrap_or_default();
        Ok(self.dispatch(&request.name, args).await.into())
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        Ok(ListResourcesResult::with_all_items(vec![
            Resource::new("weeek://me", "me")
                .with_title("Пользователь")
                .with_description("Текущий пользователь Weeek (user из weeek_context)")
                .with_mime_type("application/json"),
            Resource::new("weeek://projects", "projects")
                .with_title("Проекты")
                .with_description("Проекты воркспейса (projects из weeek_context)")
                .with_mime_type("application/json"),
        ]))
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, McpError> {
        let value = match request.uri.as_str() {
            "weeek://me" => self
                .context_cached(false)
                .await
                .map(|context| context.get("me").cloned().unwrap_or(Value::Null)),
            "weeek://projects" => self
                .context_cached(false)
                .await
                .map(|context| context.get("projects").cloned().unwrap_or(Value::Null)),
            other => {
                return Err(McpError::invalid_params(
                    format!("Неизвестный ресурс: {other}"),
                    None,
                ));
            }
        };
        match value {
            Ok(value) => {
                let text =
                    serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string());
                let contents = ResourceContents::text(text, request.uri.clone())
                    .with_mime_type("application/json");
                Ok(ReadResourceResult::new(vec![contents]).into())
            }
            Err(message) => Err(McpError::internal_error(message, None)),
        }
    }

    async fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, McpError> {
        Ok(ListPromptsResult::with_all_items(vec![
            Prompt::new(
                "my-tasks-today",
                Some("Мои задачи на сегодня: собрать и предложить план дня"),
                None,
            )
            .with_title("Мои задачи на сегодня"),
            Prompt::new(
                "week-review",
                Some("Итоги недели: завершённое, обсуждения, дедлайны"),
                None,
            )
            .with_title("Итоги недели"),
        ]))
    }

    async fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<GetPromptResponse, McpError> {
        let text = match request.name.as_str() {
            "my-tasks-today" => MY_TASKS_TODAY,
            "week-review" => WEEK_REVIEW,
            other => {
                return Err(McpError::invalid_params(
                    format!("Неизвестный промпт: {other}"),
                    None,
                ));
            }
        };
        Ok(GetPromptResult::new(vec![PromptMessage::new_text(Role::User, text)]).into())
    }
}

const MY_TASKS_TODAY: &str = "Собери мои задачи на сегодня: вызови weeek_context, возьми своё id из me, затем \
weeek_search_tasks с userId=<id> и day=<сегодняшняя дата клиента>. Сгруппируй по проектам, отметь просроченные \
и в конце предложи план: что взять в работу.";

const WEEK_REVIEW: &str = "Подготовь итоги недели: завершённые задачи за последние 7 дней (weeek_search_tasks: \
completed=true, completedAtFrom/completedAtTo), самые обсуждаемые задачи (комментарии), дедлайны следующей недели. \
Дай сводку по проектам и 3–5 коротких выводов.";
