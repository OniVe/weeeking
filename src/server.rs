use crate::curated;
use crate::generated;
use crate::http::WeeekClient;
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, Implementation, ListToolsResult,
    PaginatedRequestParams, ServerCapabilities, ServerConfig, Tool,
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
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("weeeking", env!("CARGO_PKG_VERSION")))
            .with_instructions(
                "Weeeking — полный API Weeek (задачи, проекты, доски, CRM, время, поля). Начните с weeek_context.",
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
}
