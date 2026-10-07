import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { z } from "zod";
import { OPERATIONS } from "../generated/operations.js";
import type { WeeekClient } from "../http.js";
import { describeError, errorResult, jsonResult, type ToolResult } from "./common.js";

export type RegisterOptions = { readOnly: boolean; maxChars: number };

const memberId = z.string().min(1).describe("UUID участника воркспейса (weeek_context → members[].id)");
const taskId = z.number().int().positive().describe("ID задачи");
const priority = z.number().int().min(0).max(3).describe("Приоритет: 0 низкий, 1 средний, 2 высокий, 3 отложено");
const taskType = z.enum(["action", "meet", "call"]).describe("Тип задачи");

export function registerCuratedTools(server: McpServer, client: WeeekClient, opts: RegisterOptions): void {
  const { maxChars, readOnly } = opts;

  // --- context cache (5 minutes) -------------------------------------------------
  let cache: { at: number; data: Record<string, unknown> } | null = null;
  const TTL_MS = 5 * 60 * 1000;

  // The live API wraps replies in envelopes ({ success, user }, { success, projects }, …).
  // Unwrap them here so agents get plain objects and arrays.
  const unwrap = (resp: unknown, key: string): unknown => {
    if (resp && typeof resp === "object" && key in (resp as Record<string, unknown>)) {
      return (resp as Record<string, unknown>)[key];
    }
    return resp;
  };

  async function buildContext(): Promise<Record<string, unknown>> {
    const [me, workspace, members, tags, projects] = await Promise.all([
      client.call("GET", "/user/me"),
      client.call("GET", "/ws"),
      client.call("GET", "/ws/members"),
      client.call("GET", "/ws/tags"),
      client.call("GET", "/tm/projects"),
    ]);
    return {
      me: unwrap(me, "user"),
      workspace: unwrap(workspace, "workspace"),
      members: unwrap(members, "members"),
      tags: unwrap(tags, "tags"),
      projects: unwrap(projects, "projects"),
    };
  }

  server.registerTool(
    "weeek_context",
    {
      title: "Weeek: контекст",
      description:
        "Кто вы, воркспейс, участники, теги и проекты одним вызовом. Отсюда берутся все ID для остальных " +
        "инструментов. projectId добавляет доски проекта, boardId — колонки доски. Кэш 5 минут (refresh=true — сбросить).",
      inputSchema: {
        projectId: z.number().int().positive().optional().describe("Добавить доски этого проекта"),
        boardId: z.number().int().positive().optional().describe("Добавить колонки этой доски"),
        refresh: z.boolean().optional().describe("Принудительно обновить кэш"),
      },
      annotations: { readOnlyHint: true, destructiveHint: false, idempotentHint: true, openWorldHint: true },
    },
    async ({ projectId, boardId, refresh }) => {
      try {
        if (refresh || !cache || Date.now() - cache.at > TTL_MS) {
          cache = { at: Date.now(), data: await buildContext() };
        }
        const result: Record<string, unknown> = { ...cache.data };
        if (projectId) result.boards = await client.call("GET", "/tm/boards", { query: { projectId } });
        if (boardId) result.boardColumns = await client.call("GET", "/tm/board-columns", { query: { boardId } });
        return jsonResult(result, maxChars);
      } catch (e) {
        return errorResult(describeError(e));
      }
    },
  );

  // --- reads ---------------------------------------------------------------------
  server.registerTool(
    "weeek_search_tasks",
    {
      title: "Weeek: поиск задач",
      description:
        "Поиск задач с фильтрами (проект, доска, колонка, исполнитель, завершённость, приоритет, тип, теги, текст, даты). " +
        "Пагинация perPage (1–100, по умолчанию 25) и offset. 'search' ищет по заголовку и описанию.",
      inputSchema: {
        projectId: z.number().int().positive().optional(),
        boardId: z.number().int().positive().optional(),
        boardColumnId: z.number().int().positive().optional(),
        userId: memberId.optional().describe("Фильтр по исполнителю (UUID)"),
        completed: z.boolean().optional().describe("true — только завершённые, false — только открытые"),
        all: z.boolean().optional().describe("Вернуть и удалённые/завершённые (перекрывает completed)"),
        priority: priority.optional(),
        type: taskType.optional(),
        tags: z.array(z.number().int().positive()).optional().describe("ID тегов (weeek_context → tags[].id)"),
        search: z.string().optional().describe("Текст по заголовку и описанию"),
        day: z.string().optional().describe("День (формат API)"),
        startDate: z.string().optional().describe("YYYY-MM-DD"),
        endDate: z.string().optional().describe("YYYY-MM-DD"),
        completedAtFrom: z.string().optional().describe("YYYY-MM-DD"),
        completedAtTo: z.string().optional().describe("YYYY-MM-DD"),
        perPage: z.number().int().min(1).max(100).optional(),
        offset: z.number().int().min(0).optional(),
        sortBy: z.enum(["name", "type", "priority", "duration", "overdue", "created", "date", "start"]).optional(),
        desc: z.boolean().optional().describe("Сортировать по убыванию"),
      },
      annotations: { readOnlyHint: true, destructiveHint: false, idempotentHint: true, openWorldHint: true },
    },
    async (args) => {
      try {
        const query: Record<string, unknown> = {};
        for (const [key, value] of Object.entries(args)) {
          if (value !== undefined) query[key] = value;
        }
        return jsonResult(await client.call("GET", "/tm/tasks", { query }), maxChars);
      } catch (e) {
        return errorResult(describeError(e));
      }
    },
  );

  server.registerTool(
    "weeek_get_task",
    {
      title: "Weeek: задача",
      description: "Задача целиком: карточка плюс (по умолчанию) ветка комментариев — свежие первыми.",
      inputSchema: {
        taskId,
        includeComments: z.boolean().optional().describe("Загрузить комментарии (по умолчанию true)"),
        commentsLimit: z.number().int().min(1).max(100).optional().describe("Сколько комментариев вернуть (1–100, по умолчанию 20)"),
      },
      annotations: { readOnlyHint: true, destructiveHint: false, idempotentHint: true, openWorldHint: true },
    },
    async ({ taskId: id, includeComments, commentsLimit }) => {
      try {
        const task = await client.call("GET", `/tm/tasks/${id}`);
        if (includeComments === false) return jsonResult({ task }, maxChars);
        const comments = await client.call("GET", `/tm/tasks/${id}/comments`, {
          query: { limit: commentsLimit ?? 20, offset: 0 },
        });
        return jsonResult({ task, comments }, maxChars);
      } catch (e) {
        return errorResult(describeError(e));
      }
    },
  );

  server.registerTool(
    "weeek_list_comments",
    {
      title: "Weeek: комментарии задачи",
      description: "Ветка комментариев задачи отдельно от карточки. Свежие первыми; offset листает в прошлое.",
      inputSchema: {
        taskId,
        limit: z.number().int().min(1).max(100).optional().describe("1–100, по умолчанию 50"),
        offset: z.number().int().min(0).optional().describe("Сколько пропустить, по умолчанию 0"),
      },
      annotations: { readOnlyHint: true, destructiveHint: false, idempotentHint: true, openWorldHint: true },
    },
    async ({ taskId: id, limit, offset }) => {
      try {
        const data = await client.call("GET", `/tm/tasks/${id}/comments`, {
          query: { limit: limit ?? 50, offset: offset ?? 0 },
        });
        return jsonResult(data, maxChars);
      } catch (e) {
        return errorResult(describeError(e));
      }
    },
  );

  const attachmentOp = OPERATIONS["get-attachment"];
  server.registerTool(
    "weeek_download_attachment",
    {
      title: "Weeek: скачать вложение",
      description: "Скачивает вложение задачи во временный каталог (`%TEMP%\\weeeking`) и возвращает путь к файлу.",
      inputSchema: {
        fileId: z.string().min(1).describe("ID вложения из задачи (поле attachments)"),
      },
      annotations: { readOnlyHint: true, destructiveHint: false, idempotentHint: true, openWorldHint: true },
    },
    async ({ fileId }) => {
      try {
        const pathName = (attachmentOp?.path ?? "/ws/attachments/{file_id}").replace("{file_id}", encodeURIComponent(fileId));
        return jsonResult(await client.download(pathName), maxChars);
      } catch (e) {
        return errorResult(describeError(e));
      }
    },
  );

  if (readOnly) return;

  // --- writes --------------------------------------------------------------------
  server.registerTool(
    "weeek_create_task",
    {
      title: "Weeek: создать задачу",
      description:
        "Создаёт задачу. Описание задаётся только при создании — потом его изменить нельзя (ограничение API). " +
        "boardColumnId кладёт задачу в колонку доски. userId назначает исполнителя.",
      inputSchema: {
        title: z.string().min(1).describe("Заголовок"),
        projectId: z.number().int().positive().describe("ID проекта (weeek_context → projects[].id)"),
        boardColumnId: z.number().int().positive().nullable().optional().describe("ID колонки доски"),
        description: z.string().optional().describe("Описание (markdown)"),
        parentId: z.number().int().positive().optional().describe("Родительская задача (подзадача)"),
        userId: memberId.optional().describe("Исполнитель (UUID)"),
        type: taskType.optional(),
        priority: priority.optional(),
        customFields: z.record(z.unknown()).optional().describe("Кастомные поля: {fieldId: value}"),
      },
      annotations: { readOnlyHint: false, destructiveHint: false, idempotentHint: false, openWorldHint: true },
    },
    async ({ title, projectId, boardColumnId, ...rest }) => {
      try {
        const body: Record<string, unknown> = {
          title,
          locations: [{ projectId, boardColumnId: boardColumnId ?? null }],
        };
        for (const [key, value] of Object.entries(rest)) if (value !== undefined) body[key] = value;
        return jsonResult(await client.call("POST", "/tm/tasks", { body }), maxChars);
      } catch (e) {
        return errorResult(describeError(e));
      }
    },
  );

  server.registerTool(
    "weeek_update_task",
    {
      title: "Weeek: изменить задачу",
      description:
        "Меняет поля задачи: title, priority, type, даты (start/due, date и dateTime), duration (минуты), tags, customFields. " +
        "null очищает поле. tags заменяет ВЕСЬ список тегов — сначала прочитайте задачу, чтобы не потерять теги.",
      inputSchema: {
        taskId,
        title: z.string().min(1).optional(),
        priority: priority.nullable().optional(),
        type: taskType.nullable().optional(),
        startDate: z.string().nullable().optional().describe("YYYY-MM-DD, null — очистить"),
        dueDate: z.string().nullable().optional().describe("YYYY-MM-DD, null — очистить"),
        startDateTime: z.string().nullable().optional().describe("ISO 8601, null — очистить"),
        dueDateTime: z.string().nullable().optional().describe("ISO 8601, null — очистить"),
        duration: z.number().int().nullable().optional().describe("Оценка, минуты; null — очистить"),
        tags: z.array(z.number().int().positive()).optional().describe("Полный список тегов (заменяет текущий)"),
        customFields: z.record(z.unknown()).optional(),
      },
      annotations: { readOnlyHint: false, destructiveHint: false, idempotentHint: true, openWorldHint: true },
    },
    async ({ taskId: id, ...fields }) => {
      try {
        const body: Record<string, unknown> = {};
        for (const [key, value] of Object.entries(fields)) if (value !== undefined) body[key] = value;
        if (Object.keys(body).length === 0) return errorResult("Не указано ни одно поле для изменения.");
        return jsonResult(await client.call("PUT", `/tm/tasks/${id}`, { body }), maxChars);
      } catch (e) {
        return errorResult(describeError(e));
      }
    },
  );

  server.registerTool(
    "weeek_move_task",
    {
      title: "Weeek: перенести задачу",
      description: "Переносит задачу в другую доску и/или колонку (колонка и есть статус). Нужно хотя бы одно из полей.",
      inputSchema: {
        taskId,
        boardId: z.number().int().positive().optional().describe("Доска (weeek_context → boards[].id)"),
        boardColumnId: z.number().int().positive().optional().describe("Колонка (weeek_context → boardColumns[].id)"),
      },
      annotations: { readOnlyHint: false, destructiveHint: false, idempotentHint: true, openWorldHint: true },
    },
    async ({ taskId: id, boardId, boardColumnId }) => {
      try {
        if (!boardId && !boardColumnId) return errorResult("Укажите boardId и/или boardColumnId.");
        const result: Record<string, unknown> = {};
        if (boardId) result.board = await client.call("POST", `/tm/tasks/${id}/board`, { body: { boardId } });
        if (boardColumnId) {
          result.boardColumn = await client.call("POST", `/tm/tasks/${id}/board-column`, { body: { boardColumnId } });
        }
        return jsonResult(result, maxChars);
      } catch (e) {
        return errorResult(describeError(e));
      }
    },
  );

  server.registerTool(
    "weeek_complete_task",
    {
      title: "Weeek: завершить/открыть задачу",
      description: "completed=true завершает задачу, false — открывает заново (un-complete).",
      inputSchema: { taskId, completed: z.boolean() },
      annotations: { readOnlyHint: false, destructiveHint: false, idempotentHint: true, openWorldHint: true },
    },
    async ({ taskId: id, completed }) => {
      try {
        const suffix = completed ? "complete" : "un-complete";
        return jsonResult(await client.call("POST", `/tm/tasks/${id}/${suffix}`), maxChars);
      } catch (e) {
        return errorResult(describeError(e));
      }
    },
  );

  server.registerTool(
    "weeek_set_task_people",
    {
      title: "Weeek: исполнители и наблюдатели",
      description: "Добавляет/убирает исполнителей и наблюдателей задачи. Нужен хотя бы один непустой список.",
      inputSchema: {
        taskId,
        addAssignees: z.array(memberId).optional(),
        removeAssignees: z.array(memberId).optional(),
        addWatchers: z.array(memberId).optional(),
        removeWatchers: z.array(memberId).optional(),
      },
      annotations: { readOnlyHint: false, destructiveHint: false, idempotentHint: true, openWorldHint: true },
    },
    async ({ taskId: id, addAssignees, removeAssignees, addWatchers, removeWatchers }) => {
      try {
        const jobs: [string, string, Record<string, unknown>][] = [];
        if (addAssignees?.length) jobs.push(["POST", `/tm/tasks/${id}/assignees`, { assignees: addAssignees }]);
        if (removeAssignees?.length) jobs.push(["DELETE", `/tm/tasks/${id}/assignees`, { assignees: removeAssignees }]);
        if (addWatchers?.length) jobs.push(["POST", `/tm/tasks/${id}/watchers`, { watchers: addWatchers }]);
        if (removeWatchers?.length) jobs.push(["DELETE", `/tm/tasks/${id}/watchers`, { watchers: removeWatchers }]);
        if (jobs.length === 0) return errorResult("Не указано ни одного списка участников.");
        const result: unknown[] = [];
        for (const [method, pathName, body] of jobs) {
          result.push(await client.call(method, pathName, { body }));
        }
        return jsonResult(result.length === 1 ? result[0] : result, maxChars);
      } catch (e) {
        return errorResult(describeError(e));
      }
    },
  );

  server.registerTool(
    "weeek_add_comment",
    {
      title: "Weeek: комментарий",
      description: "Добавляет комментарий к задаче (markdown сохраняется как есть). parentId отвечает в ветку.",
      inputSchema: {
        taskId,
        markdown: z.string().min(1).describe("Текст комментария (markdown)"),
        parentId: z.number().int().positive().optional().describe("ID комментария, на который отвечаем"),
      },
      annotations: { readOnlyHint: false, destructiveHint: false, idempotentHint: false, openWorldHint: true },
    },
    async ({ taskId: id, markdown, parentId }) => {
      try {
        const body: Record<string, unknown> = { markdown };
        if (parentId !== undefined) body.parentId = parentId;
        return jsonResult(await client.call("POST", `/tm/tasks/${id}/comments`, { body }), maxChars);
      } catch (e) {
        return errorResult(describeError(e));
      }
    },
  );

  server.registerTool(
    "weeek_delete_comment",
    {
      title: "Weeek: удалить комментарий",
      description: "Удаляет комментарий безвозвратно (редактировать комментарии API не умеет). Ответы на него остаются.",
      inputSchema: {
        taskId,
        commentId: z.number().int().positive().describe("ID комментария (из weeek_get_task)"),
      },
      annotations: { readOnlyHint: false, destructiveHint: true, idempotentHint: true, openWorldHint: true },
    },
    async ({ taskId: id, commentId }) => {
      try {
        return jsonResult(await client.call("DELETE", `/tm/tasks/${id}/comments/${commentId}`), maxChars);
      } catch (e) {
        return errorResult(describeError(e));
      }
    },
  );
}
