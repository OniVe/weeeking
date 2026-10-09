use crate::server::WeeekingServer;
use crate::util::*;
use rmcp::model::{CallToolResult, Tool, ToolAnnotations};
use serde_json::{Map, Value, json};
use std::time::Duration;

pub const READ_TOOLS: &[&str] = &[
    "weeek_context",
    "weeek_search_tasks",
    "weeek_get_task",
    "weeek_list_comments",
    "weeek_download_attachment",
];

pub const WRITE_TOOLS: &[&str] = &[
    "weeek_create_task",
    "weeek_update_task",
    "weeek_move_task",
    "weeek_complete_task",
    "weeek_set_task_people",
    "weeek_add_comment",
    "weeek_delete_comment",
];

pub fn is_curated(name: &str, read_only: bool) -> bool {
    READ_TOOLS.contains(&name) || (!read_only && WRITE_TOOLS.contains(&name))
}

fn tool(
    name: &str,
    title: &str,
    description: &str,
    props: Value,
    required: &[&str],
    annotations: ToolAnnotations,
) -> Tool {
    Tool::new(
        name.to_string(),
        description.to_string(),
        schema(props, required),
    )
    .with_title(title)
    .with_annotations(annotations)
}

const TASK_ID: &str = "ID задачи (целое число)";
const MEMBER_ID: &str = "UUID участника воркспейса (weeek_context → members[].id)";

/// Фильтры поиска задач; пагинацией управляют perPage/offset (их добавляет `filtered_query`).
const TASK_FILTER_KEYS: &[&str] = &[
    "day",
    "userId",
    "projectId",
    "completed",
    "boardId",
    "boardColumnId",
    "type",
    "priority",
    "tags",
    "search",
    "sortBy",
    "desc",
    "startDate",
    "endDate",
    "completedAtFrom",
    "completedAtTo",
    "all",
];

fn filtered_query(
    args: &Map<String, Value>,
    keys: &[&str],
    per_page: i64,
    offset: i64,
) -> Vec<(String, String)> {
    let mut query = Vec::new();
    for key in keys {
        if let Some(value) = args.get(*key) {
            push_query(&mut query, key, value);
        }
    }
    query.push(("perPage".to_string(), per_page.to_string()));
    query.push(("offset".to_string(), offset.to_string()));
    query
}

/// Достаёт список из конверта API (`{"tasks": [...]}`), терпимо к прямому массиву.
fn extract_list(value: &Value, key: &str) -> Vec<Value> {
    match value.get(key) {
        Some(Value::Array(items)) => items.clone(),
        None => value.as_array().cloned().unwrap_or_default(),
        _ => Vec::new(),
    }
}

pub fn tools(read_only: bool) -> Vec<Tool> {
    let mut out = vec![
        tool(
            "weeek_context",
            "Weeek: контекст",
            "Кто вы, воркспейс, участники, теги и проекты одним вызовом. Отсюда берутся все ID для остальных \
             инструментов. projectId добавляет доски проекта, boardId — колонки доски. Кэш 5 минут (refresh=true — \
             сбросить). compact=true — убрать null и пустые поля.",
            json!({
                "projectId": { "type": "integer", "description": "Добавить доски этого проекта" },
                "boardId": { "type": "integer", "description": "Добавить колонки этой доски" },
                "refresh": { "type": "boolean", "description": "Принудительно обновить кэш" },
                "compact": { "type": "boolean", "description": "Убрать null и пустые поля из ответа" }
            }),
            &[],
            ann(true, false, true),
        ),
        tool(
            "weeek_search_tasks",
            "Weeek: поиск задач",
            "Поиск задач с фильтрами (проект, доска, колонка, исполнитель, завершённость, приоритет, тип, теги, текст, даты). \
             Пагинация perPage (1–100, по умолчанию 25) и offset. 'search' ищет по заголовку и описанию. \
             fetchAll=true собирает все страницы подряд (страница по умолчанию 100, но не более 50 страниц; \
             maxItems — потолок, по умолчанию 200, максимум 1000); compact=true убирает null и пустые значения \
             (включая пустые элементы массивов).",
            json!({
                "projectId": { "type": "integer" },
                "boardId": { "type": "integer" },
                "boardColumnId": { "type": "integer" },
                "userId": { "type": "string", "description": "Фильтр по исполнителю (UUID)" },
                "completed": { "type": "boolean", "description": "true — только завершённые, false — только открытые" },
                "all": { "type": "boolean", "description": "Вернуть и удалённые/завершённые (перекрывает completed)" },
                "priority": { "type": "integer", "minimum": 0, "maximum": 3 },
                "type": { "type": "string", "enum": ["action", "meet", "call"] },
                "tags": { "type": "array", "items": { "type": "integer" }, "description": "ID тегов (weeek_context → tags[].id)" },
                "search": { "type": "string", "description": "Текст по заголовку и описанию" },
                "day": { "type": "string" },
                "startDate": { "type": "string", "description": "YYYY-MM-DD" },
                "endDate": { "type": "string", "description": "YYYY-MM-DD" },
                "completedAtFrom": { "type": "string", "description": "YYYY-MM-DD" },
                "completedAtTo": { "type": "string", "description": "YYYY-MM-DD" },
                "perPage": { "type": "integer", "minimum": 1, "maximum": 100 },
                "offset": { "type": "integer", "minimum": 0 },
                "sortBy": { "type": "string", "enum": ["name", "type", "priority", "duration", "overdue", "created", "date", "start"] },
                "desc": { "type": "boolean", "description": "Сортировать по убыванию" },
                "fetchAll": { "type": "boolean", "description": "Собрать все страницы до конца или до maxItems" },
                "maxItems": { "type": "integer", "minimum": 1, "maximum": 1000, "description": "Потолок для fetchAll (по умолчанию 200)" },
                "compact": { "type": "boolean", "description": "Убрать null и пустые поля из ответа" }
            }),
            &[],
            ann(true, false, true),
        ),
        tool(
            "weeek_get_task",
            "Weeek: задача",
            "Задача целиком: карточка плюс (по умолчанию) ветка комментариев — свежие первыми. \
             compact=true — убрать null и пустые поля.",
            json!({
                "taskId": { "type": "integer", "description": TASK_ID },
                "includeComments": { "type": "boolean", "description": "Загрузить комментарии (по умолчанию true)" },
                "commentsLimit": { "type": "integer", "minimum": 1, "maximum": 100, "description": "Сколько комментариев вернуть (по умолчанию 20)" },
                "compact": { "type": "boolean", "description": "Убрать null и пустые поля из ответа" }
            }),
            &["taskId"],
            ann(true, false, true),
        ),
        tool(
            "weeek_list_comments",
            "Weeek: комментарии задачи",
            "Ветка комментариев задачи отдельно от карточки. Свежие первыми; offset листает в прошлое. \
             fetchAll=true собирает все страницы (страница по умолчанию 100, но не более 50 страниц; maxItems — \
             потолок, по умолчанию 200); compact=true — убрать null и пустые значения (включая пустые элементы массивов).",
            json!({
                "taskId": { "type": "integer", "description": TASK_ID },
                "limit": { "type": "integer", "minimum": 1, "maximum": 100, "description": "1–100, по умолчанию 50" },
                "offset": { "type": "integer", "minimum": 0, "description": "Сколько пропустить, по умолчанию 0" },
                "fetchAll": { "type": "boolean", "description": "Собрать все страницы до конца или до maxItems" },
                "maxItems": { "type": "integer", "minimum": 1, "maximum": 1000, "description": "Потолок для fetchAll (по умолчанию 200)" },
                "compact": { "type": "boolean", "description": "Убрать null и пустые поля из ответа" }
            }),
            &["taskId"],
            ann(true, false, true),
        ),
        tool(
            "weeek_download_attachment",
            "Weeek: скачать вложение",
            "Скачивает вложение задачи во временный каталог (%TEMP%\\weeeking) и возвращает путь к файлу.",
            json!({
                "fileId": { "type": "string", "description": "ID вложения из задачи (поле attachments)" }
            }),
            &["fileId"],
            ann(true, false, true),
        ),
    ];

    if read_only {
        return out;
    }

    out.extend([
        tool(
            "weeek_create_task",
            "Weeek: создать задачу",
            "Создаёт задачу. Описание задаётся только при создании — потом его изменить нельзя (ограничение API). \
             boardColumnId кладёт задачу в колонку доски. userId назначает исполнителя.",
            json!({
                "title": { "type": "string", "description": "Заголовок" },
                "projectId": { "type": "integer", "description": "ID проекта (weeek_context → projects[].id)" },
                "boardColumnId": { "type": ["integer", "null"], "description": "ID колонки доски" },
                "description": { "type": "string", "description": "Описание (markdown)" },
                "parentId": { "type": "integer", "description": "Родительская задача (подзадача)" },
                "userId": { "type": "string", "description": MEMBER_ID },
                "type": { "type": "string", "enum": ["action", "meet", "call"] },
                "priority": { "type": "integer", "minimum": 0, "maximum": 3, "description": "0 низкий, 1 средний, 2 высокий, 3 отложено" },
                "customFields": { "type": "object", "description": "Кастомные поля: {fieldId: value}" }
            }),
            &["title", "projectId"],
            ann(false, false, false),
        ),
        tool(
            "weeek_update_task",
            "Weeek: изменить задачу",
            "Меняет поля задачи: title, priority, type, даты (start/due, date и dateTime), duration (минуты), tags, customFields. \
             null очищает поле. tags заменяет ВЕСЬ список тегов — сначала прочитайте задачу, чтобы не потерять теги.",
            json!({
                "taskId": { "type": "integer", "description": TASK_ID },
                "title": { "type": "string" },
                "priority": { "type": ["integer", "null"], "minimum": 0, "maximum": 3 },
                "type": { "type": ["string", "null"], "enum": ["action", "meet", "call", null] },
                "startDate": { "type": ["string", "null"], "description": "YYYY-MM-DD, null — очистить" },
                "dueDate": { "type": ["string", "null"], "description": "YYYY-MM-DD, null — очистить" },
                "startDateTime": { "type": ["string", "null"], "description": "ISO 8601, null — очистить" },
                "dueDateTime": { "type": ["string", "null"], "description": "ISO 8601, null — очистить" },
                "duration": { "type": ["integer", "null"], "description": "Оценка, минуты; null — очистить" },
                "tags": { "type": "array", "items": { "type": "integer" }, "description": "Полный список тегов (заменяет текущий)" },
                "customFields": { "type": "object" }
            }),
            &["taskId"],
            ann(false, false, true),
        ),
        tool(
            "weeek_move_task",
            "Weeek: перенести задачу",
            "Переносит задачу в другую доску и/или колонку (колонка и есть статус). Нужно хотя бы одно из полей.",
            json!({
                "taskId": { "type": "integer", "description": TASK_ID },
                "boardId": { "type": "integer", "description": "Доска (weeek_context → boards[].id)" },
                "boardColumnId": { "type": "integer", "description": "Колонка (weeek_context → boardColumns[].id)" }
            }),
            &["taskId"],
            ann(false, false, true),
        ),
        tool(
            "weeek_complete_task",
            "Weeek: завершить/открыть задачу",
            "completed=true завершает задачу, false — открывает заново (un-complete).",
            json!({
                "taskId": { "type": "integer", "description": TASK_ID },
                "completed": { "type": "boolean" }
            }),
            &["taskId", "completed"],
            ann(false, false, true),
        ),
        tool(
            "weeek_set_task_people",
            "Weeek: исполнители и наблюдатели",
            "Добавляет/убирает исполнителей и наблюдателей задачи. Нужен хотя бы один непустой список.",
            json!({
                "taskId": { "type": "integer", "description": TASK_ID },
                "addAssignees": { "type": "array", "items": { "type": "string" }, "description": MEMBER_ID },
                "removeAssignees": { "type": "array", "items": { "type": "string" }, "description": MEMBER_ID },
                "addWatchers": { "type": "array", "items": { "type": "string" }, "description": MEMBER_ID },
                "removeWatchers": { "type": "array", "items": { "type": "string" }, "description": MEMBER_ID }
            }),
            &["taskId"],
            ann(false, false, true),
        ),
        tool(
            "weeek_add_comment",
            "Weeek: комментарий",
            "Добавляет комментарий к задаче (markdown сохраняется как есть). parentId отвечает в ветку.",
            json!({
                "taskId": { "type": "integer", "description": TASK_ID },
                "markdown": { "type": "string", "description": "Текст комментария (markdown)" },
                "parentId": { "type": "integer", "description": "ID комментария, на который отвечаем" }
            }),
            &["taskId", "markdown"],
            ann(false, false, false),
        ),
        tool(
            "weeek_delete_comment",
            "Weeek: удалить комментарий",
            "Удаляет комментарий безвозвратно (редактировать комментарии API не умеет). Ответы на него остаются.",
            json!({
                "taskId": { "type": "integer", "description": TASK_ID },
                "commentId": { "type": "integer", "description": "ID комментария (из weeek_get_task)" }
            }),
            &["taskId", "commentId"],
            ann(false, true, true),
        ),
    ]);

    out
}

impl WeeekingServer {
    pub(crate) async fn curated_run(&self, name: &str, args: Map<String, Value>) -> CallToolResult {
        let result: Result<Value, String> = match name {
            "weeek_context" => self.tool_context(&args).await,
            "weeek_search_tasks" => self.tool_search_tasks(&args).await,
            "weeek_get_task" => self.tool_get_task(&args).await,
            "weeek_list_comments" => self.tool_list_comments(&args).await,
            "weeek_download_attachment" => self.tool_download_attachment(&args).await,
            "weeek_create_task" => self.tool_create_task(&args).await,
            "weeek_update_task" => self.tool_update_task(&args).await,
            "weeek_move_task" => self.tool_move_task(&args).await,
            "weeek_complete_task" => self.tool_complete_task(&args).await,
            "weeek_set_task_people" => self.tool_set_task_people(&args).await,
            "weeek_add_comment" => self.tool_add_comment(&args).await,
            "weeek_delete_comment" => self.tool_delete_comment(&args).await,
            _ => Err(format!("Неизвестный инструмент: {name}")),
        };
        match result {
            Ok(value) => json_result(&value, self.max_chars),
            Err(message) => error_result(message),
        }
    }

    async fn fetch_context(&self) -> Result<Value, String> {
        let (me, workspace, members, tags, projects) = tokio::join!(
            self.client.call("GET", "/user/me", &[], None),
            self.client.call("GET", "/ws", &[], None),
            self.client.call("GET", "/ws/members", &[], None),
            self.client.call("GET", "/ws/tags", &[], None),
            self.client.call("GET", "/tm/projects", &[], None),
        );
        Ok(json!({
            "me": unwrap_key(me.map_err(|e| e.to_string())?, "user"),
            "workspace": unwrap_key(workspace.map_err(|e| e.to_string())?, "workspace"),
            "members": unwrap_key(members.map_err(|e| e.to_string())?, "members"),
            "tags": unwrap_key(tags.map_err(|e| e.to_string())?, "tags"),
            "projects": unwrap_key(projects.map_err(|e| e.to_string())?, "projects"),
        }))
    }

    /// Контекст из кэша (5 минут) или свежий. Используется тулом и ресурсами.
    pub(crate) async fn context_cached(&self, refresh: bool) -> Result<Value, String> {
        let cached = {
            let guard = self.context_cache.lock().expect("context cache");
            match guard.as_ref() {
                Some((at, data)) if !refresh && at.elapsed() < Duration::from_secs(300) => {
                    Some(data.clone())
                }
                _ => None,
            }
        };
        match cached {
            Some(value) => Ok(value),
            None => {
                let value = self.fetch_context().await?;
                *self.context_cache.lock().expect("context cache") =
                    Some((std::time::Instant::now(), value.clone()));
                Ok(value)
            }
        }
    }

    async fn tool_context(&self, args: &Map<String, Value>) -> Result<Value, String> {
        let refresh = arg_bool(args, "refresh").unwrap_or(false);
        let mut data = self.context_cached(refresh).await?;

        let object = data
            .as_object_mut()
            .ok_or_else(|| "контекст повреждён".to_string())?;
        if let Some(project_id) = arg_i64(args, "projectId") {
            let query = vec![("projectId".to_string(), project_id.to_string())];
            let boards = self
                .client
                .call("GET", "/tm/boards", &query, None)
                .await
                .map_err(|e| e.to_string())?;
            object.insert("boards".into(), unwrap_key(boards, "boards"));
        }
        if let Some(board_id) = arg_i64(args, "boardId") {
            let query = vec![("boardId".to_string(), board_id.to_string())];
            let columns = self
                .client
                .call("GET", "/tm/board-columns", &query, None)
                .await
                .map_err(|e| e.to_string())?;
            object.insert("boardColumns".into(), unwrap_key(columns, "boardColumns"));
        }
        if arg_bool(args, "compact").unwrap_or(false) {
            data = compact(&data);
        }
        Ok(data)
    }

    async fn tool_search_tasks(&self, args: &Map<String, Value>) -> Result<Value, String> {
        let compact_flag = arg_bool(args, "compact").unwrap_or(false);
        let value = if arg_bool(args, "fetchAll").unwrap_or(false) {
            self.search_all_pages(args).await?
        } else {
            let per_page = arg_i64(args, "perPage").unwrap_or(25).clamp(1, 100);
            let offset = arg_i64(args, "offset").unwrap_or(0).max(0);
            let query = filtered_query(args, TASK_FILTER_KEYS, per_page, offset);
            self.client
                .call("GET", "/tm/tasks", &query, None)
                .await
                .map_err(|e| e.to_string())?
        };
        Ok(if compact_flag { compact(&value) } else { value })
    }

    /// Постраничный сбор задач: идём по offset, пока сервер отдаёт `hasMore`,
    /// не упрёмся в `maxItems` (тогда `truncated=true`) или в 50 страниц.
    async fn search_all_pages(&self, args: &Map<String, Value>) -> Result<Value, String> {
        let per_page = arg_i64(args, "perPage").unwrap_or(100).clamp(1, 100);
        let max_items = arg_i64(args, "maxItems").unwrap_or(200).clamp(1, 1000) as usize;
        let mut offset = arg_i64(args, "offset").unwrap_or(0).max(0);
        let mut collected: Vec<Value> = Vec::new();
        let mut truncated = false;
        let mut page_index = 0;
        loop {
            page_index += 1;
            if page_index > 50 {
                truncated = true;
                break;
            }
            let query = filtered_query(args, TASK_FILTER_KEYS, per_page, offset);
            let page = self
                .client
                .call("GET", "/tm/tasks", &query, None)
                .await
                .map_err(|e| e.to_string())?;
            let tasks = extract_list(&page, "tasks");
            let received = tasks.len();
            let mut capped = false;
            for task in tasks {
                if collected.len() >= max_items {
                    capped = true;
                    break;
                }
                collected.push(task);
            }
            if capped {
                truncated = true;
                break;
            }
            match page.get("hasMore").and_then(Value::as_bool) {
                Some(true) => {}
                Some(false) => break,
                None => {
                    // API не отдал hasMore — останавливаемся на неполной странице.
                    if received < per_page as usize {
                        break;
                    }
                }
            }
            if collected.len() >= max_items {
                truncated = true;
                break;
            }
            offset += per_page;
        }
        Ok(json!({ "tasks": collected, "truncated": truncated }))
    }

    async fn tool_get_task(&self, args: &Map<String, Value>) -> Result<Value, String> {
        let task_id = req_i64(args, "taskId")?;
        let compact_flag = arg_bool(args, "compact").unwrap_or(false);
        let task = self
            .client
            .call("GET", &format!("/tm/tasks/{task_id}"), &[], None)
            .await
            .map_err(|e| e.to_string())?;
        let include_comments = arg_bool(args, "includeComments").unwrap_or(true);
        if !include_comments {
            let value = json!({ "task": task });
            return Ok(if compact_flag { compact(&value) } else { value });
        }
        let limit = arg_i64(args, "commentsLimit").unwrap_or(20).clamp(1, 100);
        let query = vec![
            ("limit".to_string(), limit.to_string()),
            ("offset".to_string(), "0".to_string()),
        ];
        let comments = self
            .client
            .call(
                "GET",
                &format!("/tm/tasks/{task_id}/comments"),
                &query,
                None,
            )
            .await
            .map_err(|e| e.to_string())?;
        let value = json!({ "task": task, "comments": comments });
        Ok(if compact_flag { compact(&value) } else { value })
    }

    async fn tool_list_comments(&self, args: &Map<String, Value>) -> Result<Value, String> {
        let task_id = req_i64(args, "taskId")?;
        let compact_flag = arg_bool(args, "compact").unwrap_or(false);
        let value = if arg_bool(args, "fetchAll").unwrap_or(false) {
            self.comments_all_pages(task_id, args).await?
        } else {
            let limit = arg_i64(args, "limit").unwrap_or(50).clamp(1, 100);
            let offset = arg_i64(args, "offset").unwrap_or(0).max(0);
            let query = vec![
                ("limit".to_string(), limit.to_string()),
                ("offset".to_string(), offset.to_string()),
            ];
            self.client
                .call(
                    "GET",
                    &format!("/tm/tasks/{task_id}/comments"),
                    &query,
                    None,
                )
                .await
                .map_err(|e| e.to_string())?
        };
        Ok(if compact_flag { compact(&value) } else { value })
    }

    /// Постраничный сбор комментариев. Если API не отдаёт `hasMore`, страница
    /// считается последней, когда вернулось меньше запрошенного `limit`.
    async fn comments_all_pages(
        &self,
        task_id: i64,
        args: &Map<String, Value>,
    ) -> Result<Value, String> {
        let limit = arg_i64(args, "limit").unwrap_or(100).clamp(1, 100);
        let max_items = arg_i64(args, "maxItems").unwrap_or(200).clamp(1, 1000) as usize;
        let mut offset = arg_i64(args, "offset").unwrap_or(0).max(0);
        let mut collected: Vec<Value> = Vec::new();
        let mut truncated = false;
        let mut page_index = 0;
        loop {
            page_index += 1;
            if page_index > 50 {
                truncated = true;
                break;
            }
            let query = vec![
                ("limit".to_string(), limit.to_string()),
                ("offset".to_string(), offset.to_string()),
            ];
            let page = self
                .client
                .call(
                    "GET",
                    &format!("/tm/tasks/{task_id}/comments"),
                    &query,
                    None,
                )
                .await
                .map_err(|e| e.to_string())?;
            let comments = extract_list(&page, "comments");
            let received = comments.len();
            let mut capped = false;
            for comment in comments {
                if collected.len() >= max_items {
                    capped = true;
                    break;
                }
                collected.push(comment);
            }
            if capped {
                truncated = true;
                break;
            }
            match page.get("hasMore").and_then(Value::as_bool) {
                Some(true) => {}
                Some(false) => break,
                None => {
                    if received < limit as usize {
                        break;
                    }
                }
            }
            if collected.len() >= max_items {
                truncated = true;
                break;
            }
            offset += limit;
        }
        Ok(json!({ "comments": collected, "truncated": truncated }))
    }

    async fn tool_download_attachment(&self, args: &Map<String, Value>) -> Result<Value, String> {
        let file_id = req_str(args, "fileId")?;
        let op = crate::spec::find_operation("get-attachment")
            .ok_or_else(|| "в спецификации нет get-attachment".to_string())?;
        let param_name = op.path_params.first().map(|p| p.name).unwrap_or("file_id");
        let encoded = encode_path_segment(&file_id)?;
        let path = op.path.replace(&format!("{{{param_name}}}"), &encoded);
        self.client.download(&path).await.map_err(|e| e.to_string())
    }

    async fn tool_create_task(&self, args: &Map<String, Value>) -> Result<Value, String> {
        let title = req_str(args, "title")?;
        let project_id = req_i64(args, "projectId")?;
        let column = args.get("boardColumnId").cloned().unwrap_or(Value::Null);

        let mut body = Map::new();
        body.insert("title".into(), Value::String(title));
        body.insert(
            "locations".into(),
            json!([{ "projectId": project_id, "boardColumnId": column }]),
        );
        for key in [
            "description",
            "parentId",
            "userId",
            "type",
            "priority",
            "customFields",
        ] {
            if let Some(value) = args.get(key)
                && !value.is_null()
            {
                body.insert(key.to_string(), value.clone());
            }
        }
        self.client
            .call("POST", "/tm/tasks", &[], Some(&Value::Object(body)))
            .await
            .map_err(|e| e.to_string())
    }

    async fn tool_update_task(&self, args: &Map<String, Value>) -> Result<Value, String> {
        let task_id = req_i64(args, "taskId")?;
        let fields = [
            "title",
            "priority",
            "type",
            "startDate",
            "dueDate",
            "startDateTime",
            "dueDateTime",
            "duration",
            "tags",
            "customFields",
        ];
        let mut body = Map::new();
        for key in fields {
            if let Some(value) = args.get(key) {
                body.insert(key.to_string(), value.clone());
            }
        }
        if body.is_empty() {
            return Err("Не указано ни одно поле для изменения.".into());
        }
        self.client
            .call(
                "PUT",
                &format!("/tm/tasks/{task_id}"),
                &[],
                Some(&Value::Object(body)),
            )
            .await
            .map_err(|e| e.to_string())
    }

    async fn tool_move_task(&self, args: &Map<String, Value>) -> Result<Value, String> {
        let task_id = req_i64(args, "taskId")?;
        let board_id = arg_i64(args, "boardId");
        let column_id = arg_i64(args, "boardColumnId");
        if board_id.is_none() && column_id.is_none() {
            return Err("Укажите boardId и/или boardColumnId.".into());
        }
        let mut out = Map::new();
        if let Some(board) = board_id {
            let value = self
                .client
                .call(
                    "POST",
                    &format!("/tm/tasks/{task_id}/board"),
                    &[],
                    Some(&json!({ "boardId": board })),
                )
                .await
                .map_err(|e| e.to_string())?;
            out.insert("board".into(), value);
        }
        if let Some(column) = column_id {
            let value = self
                .client
                .call(
                    "POST",
                    &format!("/tm/tasks/{task_id}/board-column"),
                    &[],
                    Some(&json!({ "boardColumnId": column })),
                )
                .await
                .map_err(|e| e.to_string())?;
            out.insert("boardColumn".into(), value);
        }
        Ok(Value::Object(out))
    }

    async fn tool_complete_task(&self, args: &Map<String, Value>) -> Result<Value, String> {
        let task_id = req_i64(args, "taskId")?;
        let completed = arg_bool(args, "completed").ok_or_else(|| {
            "Не задан обязательный параметр «completed» (true/false).".to_string()
        })?;
        let suffix = if completed { "complete" } else { "un-complete" };
        self.client
            .call("POST", &format!("/tm/tasks/{task_id}/{suffix}"), &[], None)
            .await
            .map_err(|e| e.to_string())
    }

    async fn tool_set_task_people(&self, args: &Map<String, Value>) -> Result<Value, String> {
        let task_id = req_i64(args, "taskId")?;
        let mut results: Vec<Value> = Vec::new();

        if let Some(assignees) = arg_str_array(args, "addAssignees") {
            results.push(
                self.client
                    .call(
                        "POST",
                        &format!("/tm/tasks/{task_id}/assignees"),
                        &[],
                        Some(&json!({ "assignees": assignees })),
                    )
                    .await
                    .map_err(|e| e.to_string())?,
            );
        }
        if let Some(assignees) = arg_str_array(args, "removeAssignees") {
            results.push(
                self.client
                    .call(
                        "DELETE",
                        &format!("/tm/tasks/{task_id}/assignees"),
                        &[],
                        Some(&json!({ "assignees": assignees })),
                    )
                    .await
                    .map_err(|e| e.to_string())?,
            );
        }
        if let Some(watchers) = arg_str_array(args, "addWatchers") {
            results.push(
                self.client
                    .call(
                        "POST",
                        &format!("/tm/tasks/{task_id}/watchers"),
                        &[],
                        Some(&json!({ "watchers": watchers })),
                    )
                    .await
                    .map_err(|e| e.to_string())?,
            );
        }
        if let Some(watchers) = arg_str_array(args, "removeWatchers") {
            results.push(
                self.client
                    .call(
                        "DELETE",
                        &format!("/tm/tasks/{task_id}/watchers"),
                        &[],
                        Some(&json!({ "watchers": watchers })),
                    )
                    .await
                    .map_err(|e| e.to_string())?,
            );
        }
        if results.is_empty() {
            return Err("Не указано ни одного списка участников.".into());
        }
        Ok(if results.len() == 1 {
            results.pop().unwrap_or(Value::Null)
        } else {
            Value::Array(results)
        })
    }

    async fn tool_add_comment(&self, args: &Map<String, Value>) -> Result<Value, String> {
        let task_id = req_i64(args, "taskId")?;
        let markdown = req_str(args, "markdown")?;
        let mut body = Map::new();
        body.insert("markdown".into(), Value::String(markdown));
        if let Some(parent_id) = arg_i64(args, "parentId") {
            body.insert("parentId".into(), json!(parent_id));
        }
        self.client
            .call(
                "POST",
                &format!("/tm/tasks/{task_id}/comments"),
                &[],
                Some(&Value::Object(body)),
            )
            .await
            .map_err(|e| e.to_string())
    }

    async fn tool_delete_comment(&self, args: &Map<String, Value>) -> Result<Value, String> {
        let task_id = req_i64(args, "taskId")?;
        let comment_id = req_i64(args, "commentId")?;
        self.client
            .call(
                "DELETE",
                &format!("/tm/tasks/{task_id}/comments/{comment_id}"),
                &[],
                None,
            )
            .await
            .map_err(|e| e.to_string())
    }
}
