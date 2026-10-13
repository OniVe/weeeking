## [0.6.0](https://github.com/OniVe/weeeking/compare/v0.5.0...v0.6.0)

Минорный релиз: тонкая настройка доступа к операциям и клиентский контроль темпа запросов.

**Что нового**

- **Фильтр операций**: `WEEEK_ALLOW_ACTIONS` / `WEEEK_DENY_ACTIONS` (operationId и маски вида
  `delete-*`) и профиль `WEEEK_PROFILE=executor` — узкий rw-доступ без бинарного `READ_ONLY`.
  Fail-closed: пустое значение или неизвестная маска останавливают старт; запрещённые действия
  не рекламируются и отклоняются при вызове; ресурсы, промпты и инструкция сервера следуют
  политике
- **Клиентский rate-limit**: `WEEEK_RPS` (token bucket на все HTTP-запросы, включая ретраи;
  `Retry-After` приоритетнее расчётного интервала) и `WEEEK_RPS_BURST` — сосуществование
  нескольких потребителей одного workspace
- Плюс документы для участников: `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `SECURITY.md`,
  шаблоны issue и pull request

**Установка**

- `npx -y weeeking` — npm-обёртка скачает бинарник под вашу платформу и сверит sha256
- MCP-клиенты с поддержкой реестра: `io.github.OniVe/weeeking`
- Контрольные суммы: `certutil -hashfile <файл> SHA256` (Windows) или `sha256sum -c <файл>.sha256` (Linux/macOS)
- Аттестации: `gh attestation verify <файл> -R OniVe/weeeking`

Артефакты — те же семь платформ (28 файлов), ниже — SHA256-суммы (секция добавляется автоматически).

Подробности — в [CHANGELOG.md](https://github.com/OniVe/weeeking/blob/main/CHANGELOG.md).
