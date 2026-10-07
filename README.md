# Weeeking 🛡️

> *Weeek and conquer.* MCP-сервер для [Weeek](https://weeek.net) с **полным покрытием публичного API**:
> 157 операций из официальной OpenAPI-спецификации. Один бинарник на Rust, без Node и внешних зависимостей
> в рантайме. Целевые платформы пилота: **Windows x86_64 и Linux x86_64**.

## Возможности

- **13 админ-групп с `action`** — `weeek_project`, `weeek_board`, `weeek_board_column`, `weeek_custom_fields`,
  `weeek_portfolio`, `weeek_tags`, `weeek_task` (таймеры, записи времени, вложения, родитель…), `weeek_funnels`,
  `weeek_funnel_statuses`, `weeek_deals`, `weeek_organizations`, `weeek_contacts`, `weeek_currencies`.
- **12 curated-тулов** для повседневной работы: `weeek_context`, `weeek_search_tasks`, `weeek_get_task`,
  `weeek_list_comments`, `weeek_download_attachment` + записи (`weeek_create_task`, `weeek_update_task`,
  `weeek_move_task`, `weeek_complete_task`, `weeek_set_task_people`, `weeek_add_comment`, `weeek_delete_comment`).
- **READ_ONLY по умолчанию**: изменяющие действия не регистрируются, пока не выставлено `READ_ONLY=false`.
- **Спека вшита в бинарник**: `spec/operations.json` генерируется из первоисточника (developers.weeek.net)
  скриптом `tools/update-spec.mjs` и попадает в сборку через `include_str!`.

## Сборка

Требуется Rust 1.88+ (MSVC toolchain на Windows).

```bash
cargo build --release
# → target/release/weeeking.exe (Windows) / target/release/weeeking (Linux)
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
2. На Windows — запись **`weeek-mcp` / `api-token`** в Windows Credential Manager (та же, что создаёт визард
   `npx @dsudomoin/weeek-mcp init`). Значение никогда не логируется.
3. Если пусто — сервер поднимается, но вызовы возвращают `isError` с подсказкой.

На Linux пилота токен задаётся переменной `WEEEK_API_TOKEN` (Secret Service можно добавить позже — `src/token.rs`).

## Подключение (OpenCode)

```jsonc
"weeek": {
  "type": "local",
  "command": ["C:\\project\\ai\\weeeking\\target\\release\\weeeking.exe"],
  "enabled": true,
  "environment": { "READ_ONLY": "false" }
}
```

Любой другой MCP-клиент: команда — путь к бинарнику, транспорт — stdio.

## Обновление спецификации

```bash
node tools/update-spec.mjs   # тянет свежую OpenAPI с developers.weeek.net → spec/operations.json
cargo build --release        # пересобрать бинарник со свежей спекой
```

## Разработка

```bash
cargo fmt
cargo clippy --all-targets
cargo test                   # smoke: read-only и полный режимы (без токена)
```

`legacy/` — прежняя TypeScript-реализация (референс, в OpenCode не подключена).

> Примечание: OpenCode V2 не запускает LSP-серверы, поэтому rust-analyzer используется в редакторе/CLI,
> а диагностика для агента — через `cargo check`, `cargo clippy` и `cargo test`.

## Ограничения API Weeek (важно агенту)

- Описание задачи задаётся **только при создании** — позже не изменить.
- `tags` в `weeek_update_task` **заменяет весь список** — сначала прочитайте задачу.
- Комментарии можно создать и удалить, но **не отредактировать**.

## Лицензия

MIT
