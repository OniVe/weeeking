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
- **Кодогенератор спеки**: `tools/update-spec.mjs` тянет OpenAPI с developers.weeek.net и генерирует
  `src/spec_generated.rs` — статические данные (`&'static str`), которые компилируются в бинарник;
  в рантайме нет ни JSON, ни парсинга, чанк спеки нигде не сохраняется.

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
| `WEEEK_KEYCHAIN_ACCOUNT` | `api-token` | Имя записи в системном хранилище (для нескольких аккаунтов). |
| `WEEEK_MAX_ATTACHMENT_BYTES` | `67108864` (64 МиБ) | Лимит скачивания вложения (защита от гигантских ответов). |

### Откуда берётся токен

1. `WEEEK_API_TOKEN` (или `WEEEK_TOKEN`) из окружения — приоритет всегда за ним.
2. На Windows — запись **`weeek-mcp` / `WEEEK_KEYCHAIN_ACCOUNT`** (по умолчанию `api-token`) в Windows Credential
   Manager — та же, что создаёт визард `npx @dsudomoin/weeek-mcp init`. Значение никогда не логируется.
3. Если пусто — сервер поднимается, но вызовы возвращают `isError` с подсказкой.

На Linux пилота токен задаётся переменной `WEEEK_API_TOKEN` (Secret Service можно добавить позже — `src/token.rs`).

### Несколько аккаунтов Weeek

Личность действий (автор комментариев, задач) определяется токеном: API всегда действует от владельца токена.
Для второго пользователя нужен **его собственный токен** (создаётся в Настройках workspace → API; доступ к разделу
есть у супер-админов и админов).

```bash
# 1) Сохранить токен второго аккаунта (ввод скрыт, в argv токен не попадает)
weeeking store-token --account api-token-anna

# 2) Инстанс сервера с этим аккаунтом — через переменную окружения
WEEEK_KEYCHAIN_ACCOUNT=api-token-anna weeeking
```

Пример второго инстанса в конфиге OpenCode:

```jsonc
"weeek-anna": {
  "type": "local",
  "command": ["C:\\project\\ai\\weeeking\\target\\release\\weeeking.exe"],
  "enabled": true,
  "environment": {
    "WEEEK_KEYCHAIN_ACCOUNT": "api-token-anna",
    "READ_ONLY": "false"
  }
}
```

Агент сможет выбирать, от кого действовать: тулы `weeek-anna_*` пишут от Анны, `weeek_*` — от основного аккаунта.
Удалить запись из хранилища: `cmdkey /delete:api-token-anna.weeek-mcp`.

### CLI

```text
weeeking                                запустить MCP-сервер (stdio)
weeeking store-token [--account <имя>]  сохранить токен в системное хранилище
weeeking --help                         справка
```

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

> Форма записи выше — V1-совместимая (проверена на OpenCode 2.0.24). Native V2-форма
> (`"mcp": { "servers": { … } }`, `"disabled": false`) поддерживается наравне.

## Обновление спецификации

```bash
node tools/update-spec.mjs    # developers.weeek.net → src/spec_generated.rs (кодогенерация)
cargo fmt && cargo build --release
```

## Разработка

```bash
cargo fmt
cargo clippy --all-targets
cargo test                   # smoke: read-only и полный режимы (без токена)
```

Пересборка при работающем OpenCode (exe залочен запущенным MCP-сервером):

```powershell
pwsh -ExecutionPolicy Bypass -File tools\rebuild.ps1
# или из сессии pwsh: powershell -ExecutionPolicy Bypass -File tools\rebuild.ps1
```

`legacy/` — прежняя TypeScript-реализация (референс, в OpenCode не подключена).

> Примечание: OpenCode V2 не запускает LSP-серверы нативно. Для семантики в агенте подключён MCP-сервер
> `rust-analyzer-mcp` (см. `.opencode/opencode.jsonc`) — hover, references, rename, diagnostics по воркспейсу;
> сам rust-analyzer доступен и в редакторе/CLI, базовая проверка — `cargo check` / `cargo clippy` / `cargo test`.

## Ограничения API Weeek (важно агенту)

- Описание задачи задаётся **только при создании** — позже не изменить.
- `tags` в `weeek_update_task` **заменяет весь список** — сначала прочитайте задачу.
- Комментарии можно создать и удалить, но **не отредактировать**.

## Лицензия

MIT
