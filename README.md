# Weeeking 🛡️

[![CI](https://github.com/OniVe/weeeking/actions/workflows/ci.yml/badge.svg)](https://github.com/OniVe/weeeking/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/OniVe/weeeking)](https://github.com/OniVe/weeeking/releases)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

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
- **Ресурсы и промпты MCP**: `weeek://me` и `weeek://projects` читаются клиентом без tool-call; промпты
  «Мои задачи на сегодня» и «Итоги недели» — готовые сценарии для агента.
- **Эргономика LLM**: `compact=true` убирает null и пустые значения (включая пустые элементы массивов)
  из тяжёлых ответов; `fetchAll=true` (+`maxItems`) собирает страницы задач и комментариев
  (страница по умолчанию 100, не более 50 страниц) с флагом `truncated`.
- **Надёжность**: ретраи 429 и 502/503/504 с `Retry-After` и экспоненциальным бэкоффом (429 — для любого
  метода, 502/503/504 и сетевые сбои — только для GET/HEAD; таймауты не повторяются); `WEEEK_LOG=debug` —
  HTTP-логи в stderr (метод, путь, статус, длительность; без токена).
- **Кодогенератор спеки**: `tools/update_spec.py` тянет OpenAPI с developers.weeek.net и генерирует
  `src/spec_generated.rs` — статические данные (`&'static str`), которые компилируются в бинарник;
  в рантайме нет ни JSON, ни парсинга, чанк спеки в репозитории не сохраняется.

## Сборка

Требуется Rust 1.88+ (MSVC toolchain на Windows).

```bash
cargo build --release
# → target/release/weeeking.exe (Windows) / target/release/weeeking (Linux)
```

## Релизы

Готовые бинарники — в [GitHub Releases](https://github.com/OniVe/weeeking/releases):
`weeeking-x86_64-pc-windows-msvc.zip` и `weeeking-x86_64-unknown-linux-gnu.tar.gz`, рядом контрольные суммы `.sha256`.

```bash
# проверка контрольной суммы
certutil -hashfile weeeking-x86_64-pc-windows-msvc.zip SHA256   # Windows
sha256sum -c weeeking-x86_64-unknown-linux-gnu.tar.gz.sha256     # Linux
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
| `WEEEK_RETRY_MAX` | `2` | Дополнительные попытки при 429, 502/503/504 и сетевых сбоях (0–5); таймауты не повторяются. |
| `WEEEK_RETRY_BASE_MS` | `300` | База экспоненциального бэкоффа (потолок 5 с; `Retry-After` учитывается, потолок 30 с). |
| `WEEEK_LOG` | — | `debug` — подробные HTTP-логи в stderr (метод, путь, статус, мс; токен не логируется). |

### Откуда берётся токен

1. `WEEEK_API_TOKEN` (или `WEEEK_TOKEN`) из окружения — приоритет всегда за ним.
2. На Windows — запись **`weeek-mcp` / `WEEEK_KEYCHAIN_ACCOUNT`** (по умолчанию `api-token`) в Windows Credential
   Manager (сохраняется командой `weeeking store-token`). Значение никогда не логируется.
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
  "command": ["C:\\путь\\weeeking.exe"],
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
  "command": ["C:\\путь\\weeeking.exe"],
  "enabled": true,
  "environment": { "READ_ONLY": "false" }
}
```

Любой другой MCP-клиент: команда — путь к бинарнику, транспорт — stdio.

> Форма записи выше — V1-совместимая (проверена на OpenCode 2.0.24). Native V2-форма
> (`"mcp": { "servers": { … } }`, `"disabled": false`) поддерживается наравне.

## Обновление спецификации

```bash
pip install quickjs            # зависимость кодогенератора (один раз)
python tools/update_spec.py    # developers.weeek.net → src/spec_generated.rs (кодогенерация)
cargo fmt && cargo build --release
```

## Разработка

```bash
cargo fmt
cargo clippy --all-targets
cargo test                   # unit + mock-сюита (HTTP против эмулятора) + smoke
```

Приёмочный smoke против живого API (перед релизом; read-only по умолчанию):

```bash
python tools/live_smoke.py
python tools/live_smoke.py --write-test <PROJECT_ID>   # + цикл мутаций в sandbox-проекте
```

Пересборка при работающем OpenCode (exe залочен запущенным MCP-сервером):

```bash
python tools\rebuild.py
```

## Ограничения API Weeek (важно агенту)

- Описание задачи задаётся **только при создании** — позже не изменить.
- `tags` в `weeek_update_task` **заменяет весь список** — сначала прочитайте задачу.
- Комментарии можно создать и удалить, но **не отредактировать**.

## Лицензия

MIT
