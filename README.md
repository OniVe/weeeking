# Weeeking 🛡️

> *Weeek and conquer.* MCP-сервер для [Weeek](https://weeek.net) с **полным покрытием публичного API**: тулы генерируются из
> официальной OpenAPI-спецификации (157 операций — задачи, проекты, доски, кастомные поля, время, CRM,
> организации, контакты и т.д.), плюс удобный «curated» слой для повседневной работы.

**Read-only по умолчанию:** изменяющие операции не регистрируются, пока вы явно не выставите `READ_ONLY=false`.

## Возможности

- **Генерация из первоисточника.** `npm run update:openapi` тянет актуальную спецификацию с
  developers.weeek.net (она лежит в чанках сайта) и пересобирает `src/generated/operations.ts`.
- **13 админ-групп** (`weeek_project`, `weeek_board`, `weeek_board_column`, `weeek_custom_fields`,
  `weeek_portfolio`, `weeek_tags`, `weeek_funnels`, `weeek_funnel_statuses`, `weeek_deals`,
  `weeek_organizations`, `weeek_contacts`, `weeek_currencies`, `weeek_task`): каждая — один тул с
  параметром `action` (например `weeek_project` → `create-project`, `archive-project`, …).
- **Curated-тулы** для ежедневной работы: `weeek_context`, `weeek_search_tasks`, `weeek_get_task`,
  `weeek_list_comments`, `weeek_download_attachment` + записи (`weeek_create_task`, `weeek_update_task`,
  `weeek_move_task`, `weeek_complete_task`, `weeek_set_task_people`, `weeek_add_comment`, `weeek_delete_comment`).
- **Безопасность:** токен только в заголовке запроса (не логируется), лимит на размер ответа
  (`WEEEK_MAX_RESPONSE_CHARS`, по умолчанию 60 000 символов), ошибки API возвращаются как `isError`
  с HTTP-кодом и телом, stderr — только диагностика.

## Установка

```bash
npm install
npm run build        # typecheck + esbuild → dist/server.mjs
node dist/server.mjs # stdio MCP-сервер
```

## Переменные окружения

| Переменная | По умолчанию | Описание |
| --- | --- | --- |
| `WEEEK_API_TOKEN` | — | Токен из Weeek (Настройки workspace → API). Без него чтения невозможны. |
| `READ_ONLY` | `true` | `false`/`0` — открыть изменяющие операции (создание, правка, удаление, CRM). |
| `WEEEK_BASE_URL` | `https://api.weeek.net/public/v1` | Свой прокси/хост при необходимости. |
| `WEEEK_TIMEOUT_MS` | `30000` | Таймаут запроса. |
| `WEEEK_MAX_RESPONSE_CHARS` | `60000` | Обрезка больших ответов. |
| `WEEEK_DISABLE_KEYCHAIN` | — | `1` — не читать токен из системного хранилища. |

### Откуда берётся токен

1. `WEEEK_API_TOKEN` (или `WEEEK_TOKEN`) из окружения — приоритет всегда за ним.
2. Если переменной нет — токен читается из системного хранилища (Windows Credential Manager,
   macOS Keychain, Linux Secret Service), запись **`weeek-mcp` / `api-token`** — та же, что создаёт
   визард `npx @dsudomoin/weeek-mcp init`. Значение никогда не логируется.
3. Если и там пусто — сервер поднимается, но вызовы возвращают `isError` с подсказкой.

## Подключение (OpenCode)

```jsonc
"weeek-full": {
  "type": "local",
  "command": ["node", "C:\\project\\ai\\weeeking\\dist\\server.mjs"],
  "enabled": true,
  "environment": {
    "WEEEK_API_TOKEN": "{env:WEEEK_API_TOKEN}",
    "READ_ONLY": "true"
  }
}
```

Любой другой MCP-клиент: команда `node <путь>\\dist\\server.mjs`, stdio.

## Примеры

- `weeek_context` — участники/теги/проекты; отсюда берутся ID.
- `weeek_project`, action=`create-project`, params=`{ "name": "ИИ-Разработка", "isPrivate": 0 }`
- `weeek_search_tasks`, `{ "search": "хранилище", "completed": false }`
- `weeek_get_task`, `{ "taskId": 529 }`
- `weeek_complete_task`, `{ "taskId": 478, "completed": true }`

## Обновление спецификации

```bash
npm run update:openapi   # перегенерирует src/generated/operations.ts
npm run build
```

## Разработка

```bash
npm test        # сборка + smoke-тест (stdio, без токена)
```

`src/generated/` — машино-генерируемый код, руками не правится. Curated-тулы живут в
`src/tools/curated.ts`, генератор — в `scripts/update-openapi.mjs`.

## Ограничения API Weeek (важно агенту)

- Описание задачи задаётся **только при создании** — позже не изменить.
- `tags` в `weeek_update_task` **заменяет весь список** — сначала прочитайте задачу.
- Комментарии можно создать и удалить, но **не отредактировать**.

## Лицензия

MIT
