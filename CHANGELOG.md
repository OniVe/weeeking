# Changelog

Формат — [Keep a Changelog](https://keepachangelog.com/ru/1.1.0/), версионирование — [SemVer](https://semver.org/lang/ru/).

## [0.2.0] — 2026-10-09

### Добавлено

- MCP resources: `weeek://me`, `weeek://projects` — контекст воркспейса без tool-call (кэш 5 минут,
  read-only-режим включён).
- MCP prompts: «Мои задачи на сегодня», «Итоги недели».
- `compact=true` для `weeek_context`, `weeek_search_tasks`, `weeek_get_task`, `weeek_list_comments` —
  рекурсивно убирает null и пустые значения, включая пустые элементы массивов (`false`/`0` сохраняются).
- `fetchAll=true` + `maxItems` для `weeek_search_tasks` и `weeek_list_comments` — постраничный сбор
  (страница по умолчанию 100, не более 50 страниц) с флагом `truncated` (потолок по умолчанию 200, максимум 1000).
- Ретраи 429 и 502/503/504: `Retry-After` и экспоненциальный бэкофф; 429 — для любого метода, 502/503/504 и
  сетевые сбои (кроме таймаутов) — только для идемпотентных GET/HEAD. Настройки `WEEEK_RETRY_MAX`,
  `WEEEK_RETRY_BASE_MS`.
- `WEEEK_LOG=debug` — HTTP-логи в stderr (метод, путь, статус, длительность) без токена.
- npm-пакет `weeeking` (npx): обёртка скачивает нативный бинарник (Windows x64 / Linux x64) из
  GitHub Releases с проверкой sha256; в релизы добавлены сырые бинарники `weeeking-win32-x64.exe`
  и `weeeking-linux-x64` (+ `.sha256`); публикация — npm trusted publishing (OIDC, provenance).

### Изменено

- `weeek_search_tasks` всегда передаёт `perPage`/`offset` явно (по умолчанию 25/0) — пагинация
  собирается детерминированно.

## [0.1.0] — 2026-10-08

Первый публичный пилот: полное покрытие публичного API Weeek (157 операций) одним Rust-бинарником.

### Добавлено

- 13 админ-групп с `action` (проекты, доски, колонки, кастомные поля, портфели, теги, задачи с таймерами
  и временем, воронки, статусы, сделки, организации, контакты, валюты) — полное покрытие API.
- 12 curated-тулов: контекст, поиск и карточка задач, комментарии, вложения, создание/правка/перенос/завершение,
  исполнители и наблюдатели.
- `READ_ONLY` по умолчанию: изменяющие действия не регистрируются, пока не выставлено `READ_ONLY=false`.
- Мультиаккаунт: `WEEEK_KEYCHAIN_ACCOUNT` и `weeeking store-token` — токены в системном хранилище.
- Кодогенератор спецификации `tools/update_spec.py` (Python + QuickJS) → `src/spec_generated.rs`.
- CI для Windows и Linux; по тегу собираются релизные артефакты и прикладываются к GitHub Release.

### Безопасность

- Защита границ операций: запрет dot-segment и пустых path-параметров, строгая проверка типов.
- Обрезка больших ответов и тел ошибок; лимит на скачивание вложений.
- Токен никогда не логируется и не попадает в argv.

### Исправлено

- Булевы query-параметры (например, `completed`) уходят в Weeek как `1/0`: вариант `true/false`
  API отклоняет с HTTP 422. Найдено приёмочным smoke-тестом перед релизом.

[0.2.0]: https://github.com/OniVe/weeeking/releases/tag/v0.2.0
[0.1.0]: https://github.com/OniVe/weeeking/releases/tag/v0.1.0
