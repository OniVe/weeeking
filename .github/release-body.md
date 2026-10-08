Первый публичный релиз Weeeking — MCP-сервер для Weeek (полное покрытие API).

**Артефакты**

- `weeeking-x86_64-pc-windows-msvc.zip` — `weeeking.exe` для Windows x64
- `weeeking-x86_64-unknown-linux-gnu.tar.gz` — `weeeking` для Linux x64
- Файлы `.sha256` — контрольные суммы: `certutil -hashfile <файл> SHA256` (Windows) или `sha256sum -c <файл>.sha256` (Linux)

**Что внутри**

- 157 операций API в 13 админ-группах + 12 curated-инструментов; `READ_ONLY` по умолчанию
- Мультиаккаунт через системное хранилище (`weeeking store-token`)
- Булевы query-параметры сериализуются как `1/0` (требование Weeek API)

Подробности — в [CHANGELOG.md](https://github.com/OniVe/weeeking/blob/main/CHANGELOG.md).
