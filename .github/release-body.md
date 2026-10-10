## [0.4.1](https://github.com/OniVe/weeeking/compare/v0.4.0...v0.4.1)

Патч-релиз: сервер публикуется в официальном MCP Registry и становится виден MCP-клиентам
(VS Code, Copilot и др. — через каталог GitHub MCP).

**Что нового**

- **Официальный MCP Registry**: запись `io.github.OniVe/weeeking` — обновляется автоматически на каждый
  релиз (npm-пакет с полем `mcpName` + `server.json`, публикация из CI через GitHub OIDC, без секретов)
- **Системное хранилище токена на всех платформах**: macOS Keychain и Linux Secret Service
  (GNOME Keyring/KWallet) в дополнение к Windows Credential Manager — `weeeking store-token` больше
  не Windows-only
- README: убраны устаревшие формулировки, актуальные платформы (семь), нативная форма конфигурации
  OpenCode V2 (`mcp.servers`)

**Установка**

- `npx -y weeeking` — npm-обёртка скачает бинарник под вашу платформу и сверит sha256
- MCP-клиенты с поддержкой реестра: `io.github.OniVe/weeeking`
- Контрольные суммы: `certutil -hashfile <файл> SHA256` (Windows) или `sha256sum -c <файл>.sha256` (Linux/macOS)
- Аттестации: `gh attestation verify <файл> -R OniVe/weeeking`

Артефакты — те же семь платформ (28 файлов), ниже — SHA256-суммы (секция добавляется автоматически).

Подробности — в [CHANGELOG.md](https://github.com/OniVe/weeeking/blob/main/CHANGELOG.md).
