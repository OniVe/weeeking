Второй релиз Weeeking — MCP-сервер для Weeek (полное покрытие API): MCP resources и prompts,
экономия токенов, ретраи и ещё один канал установки — npm.

**Что нового**

- MCP resources `weeek://me`, `weeek://projects` и промпты «Мои задачи на сегодня», «Итоги недели»
- `compact=true` — из ответов убираются null и пустые значения (меньше токенов)
- `fetchAll=true` + `maxItems` — постраничный сбор задач и комментариев с флагом `truncated`
- Ретраи 429/502/503/504 с `Retry-After` и бэкоффом; `WEEEK_LOG=debug` — HTTP-логи в stderr
- Установка через npm: `npx -y weeeking` (обёртка скачивает бинарник под платформу с проверкой sha256)

**Артефакты**

- `weeeking-x86_64-pc-windows-msvc.zip` / `weeeking-x86_64-unknown-linux-gnu.tar.gz` (+ `.sha256`)
- Сырые бинарники для npm-обёртки: `weeeking-win32-x64.exe`, `weeeking-linux-x64` (+ `.sha256`)
- Проверка сумм: `certutil -hashfile <файл> SHA256` (Windows) или `sha256sum -c <файл>.sha256` (Linux)

Подробности — в [CHANGELOG.md](https://github.com/OniVe/weeeking/blob/main/CHANGELOG.md).
