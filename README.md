# Weeeking 🛡️

**Русский** · [English](README.en.md)

[![CI](https://github.com/OniVe/weeeking/actions/workflows/ci.yml/badge.svg)](https://github.com/OniVe/weeeking/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/OniVe/weeeking)](https://github.com/OniVe/weeeking/releases)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

> *Weeek and conquer.* MCP-сервер для [Weeek](https://weeek.net) с **полным покрытием публичного API**:
> 157 операций из официальной OpenAPI-спецификации. Один бинарник на Rust, без Node и внешних зависимостей
> в рантайме. Релизы — для семи платформ: **Windows x64/arm64, Linux x64 (glibc/musl), Linux arm64, macOS arm64/x64**.

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
# → target/release/weeeking.exe (Windows) / target/release/weeeking (Linux, macOS)
```

## Релизы

Готовые бинарники — в [GitHub Releases](https://github.com/OniVe/weeeking/releases): архивы
`weeeking-<версия>-<платформа>` и сырые бинарники для npm-обёртки; рядом — файлы `.sha256`,
хэши также перечислены в описании релиза.

| Платформа | Архив | Сырой бинарник |
| --- | --- | --- |
| Windows x64 | `weeeking-<версия>-win-x64.zip` | `weeeking-<версия>-win-x64.exe` |
| Windows arm64 | `weeeking-<версия>-win-arm64.zip` | `weeeking-<версия>-win-arm64.exe` |
| Linux x64 (glibc) | `weeeking-<версия>-linux-x64.tar.gz` | `weeeking-<версия>-linux-x64` |
| Linux x64 (musl, static) | `weeeking-<версия>-linux-musl-x64.tar.gz` | `weeeking-<версия>-linux-musl-x64` |
| Linux arm64 | `weeeking-<версия>-linux-arm64.tar.gz` | `weeeking-<версия>-linux-arm64` |
| macOS arm64 | `weeeking-<версия>-osx-arm64.tar.gz` | `weeeking-<версия>-osx-arm64` |
| macOS x64 | `weeeking-<версия>-osx-x64.tar.gz` | `weeeking-<версия>-osx-x64` |

```bash
# проверка контрольной суммы
certutil -hashfile weeeking-<версия>-win-x64.zip SHA256          # Windows
sha256sum -c weeeking-<версия>-linux-x64.tar.gz.sha256           # Linux
```

Каждый файл релиза подписан GitHub-аттестацией (Sigstore) — проверяемое происхождение сборки:

```bash
gh attestation verify weeeking-<версия>-win-x64.exe -R OniVe/weeeking
```

### npm (npx)

```bash
npx -y weeeking     # MCP-сервер по stdio; бинарник скачается и проверится по sha256
```

Пакет `weeeking` — тонкая обёртка: берёт нативный бинарник нужной платформы (Windows x64/arm64,
Linux x64 gnu/musl, Linux arm64, macOS arm64/x64) из GitHub Releases этой же версии. Публикуется
через npm trusted publishing (OIDC, с provenance).

Сервер также опубликован в официальном MCP Registry — `io.github.OniVe/weeeking`; оттуда его
подхватывают каталог GitHub MCP и агрегаторы.

На macOS скачанный через браузер бинарник может ловить карантин Gatekeeper — снимите его
(`xattr -d com.apple.quarantine <файл>`) или ставьте через `npx`/`curl`.

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
2. В системном хранилище — запись сервиса `weeek-mcp` с именем **`WEEEK_KEYCHAIN_ACCOUNT`**
   (по умолчанию `api-token`): Windows Credential Manager (target `api-token.weeek-mcp`),
   macOS Keychain, Linux Secret Service (GNOME Keyring/KWallet через D-Bus); сохраняется командой
   `weeeking store-token`. Значение никогда не логируется.
3. Если пусто — сервер поднимается, но вызовы возвращают `isError` с подсказкой.

Системное хранилище работает на всех платформах; на **headless-Linux без D-Bus** его нет — задайте
токен переменными окружения `WEEEK_API_TOKEN` или `WEEEK_TOKEN`.

### Несколько аккаунтов Weeek

Личность действий (автор комментариев, задач) определяется токеном: API всегда действует от владельца токена.
Для второго пользователя нужен **его собственный токен** (создаётся в Настройках workspace → API; доступ к разделу
есть у супер-админов и админов).

```text
# 1) Сохранить токен второго аккаунта (ввод скрыт, в argv токен не попадает)
weeeking store-token --account api-token-anna

# 2) Запуск сервера под этим аккаунтом
$env:WEEEK_KEYCHAIN_ACCOUNT = "api-token-anna"; weeeking   # PowerShell (Windows)
WEEEK_KEYCHAIN_ACCOUNT=api-token-anna weeeking             # bash/zsh (macOS, Linux)
```

Пример второго инстанса в конфиге OpenCode (нативная форма V2):

```jsonc
"mcp": {
  "servers": {
    "weeek-anna": {
      "type": "local",
      "command": ["C:\\путь\\weeeking.exe"],
      "environment": {
        "WEEEK_KEYCHAIN_ACCOUNT": "api-token-anna",
        "READ_ONLY": "false"
      }
    }
  }
}
```

Агент сможет выбирать, от кого действовать: тулы `weeek-anna_*` пишут от Анны, `weeek_*` — от основного аккаунта.
Удалить запись из хранилища: Windows — `cmdkey /delete:api-token-anna.weeek-mcp`;
macOS — `security delete-generic-password -s weeek-mcp -a api-token-anna`;
Linux — средствами вашего хранилища (GNOME Keyring / KWallet).

На headless-Linux без D-Bus системного хранилища нет: мультиаккаунт — отдельный инстанс сервера
с собственным `WEEEK_API_TOKEN` в окружении.

### CLI

```text
weeeking                                запустить MCP-сервер (stdio)
weeeking store-token [--account <имя>]  сохранить токен в системное хранилище
weeeking --help                         справка
```

## Подключение (OpenCode)

Нативная форма OpenCode V2 — серверы живут в `mcp.servers`:

```jsonc
"mcp": {
  "servers": {
    "weeek": {
      "type": "local",
      "command": ["C:\\путь\\weeeking.exe"],   // на любой ОС: ["npx", "-y", "weeeking"]
      "environment": { "READ_ONLY": "false" }
    }
  }
}
```

Добавить из CLI: `opencode mcp add weeek -- npx -y weeeking` (с `--global` — для всех проектов).
Отключить сервер, не удаляя из конфига: `"disabled": true` (нативная V2-форма; V1-форма
`"mcp": { "weeek": { … } }` с `enabled` тоже поддерживается).

Любой другой MCP-клиент: команда — путь к бинарнику (или `npx -y weeeking`), транспорт — stdio.

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
