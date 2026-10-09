## [0.4.0](https://github.com/OniVe/weeeking/compare/v0.3.0...v0.4.0)

Четвёртый релиз Weeeking — MCP-сервер для Weeek: седьмая платформа (Windows arm64)
и GitHub-аттестации (Sigstore) для всех файлов релиза.

**Что нового**

- Платформа **Windows arm64**: `weeeking-<версия>-win-arm64.zip` / `.exe` — сборка на
  GitHub arm64-раннере (`windows-11-arm`); npm-обёртка определяет `win32-arm64` автоматически
- **GitHub-аттестации (Sigstore)** для всех файлов: проверяемое происхождение сборки —
  `gh attestation verify <файл> -R OniVe/weeeking`
- Семь платформ: Windows x64/arm64, Linux x64 (glibc и musl-static), Linux arm64, macOS arm64/x64
- Все SHA256-суммы перечислены ниже (секция добавляется автоматически при сборке релиза)

**Артефакты**

| Платформа | Архив | Сырой бинарник |
| --- | --- | --- |
| Windows x64 | `weeeking-<версия>-win-x64.zip` | `weeeking-<версия>-win-x64.exe` |
| Windows arm64 | `weeeking-<версия>-win-arm64.zip` | `weeeking-<версия>-win-arm64.exe` |
| Linux x64 (glibc) | `weeeking-<версия>-linux-x64.tar.gz` | `weeeking-<версия>-linux-x64` |
| Linux x64 (musl) | `weeeking-<версия>-linux-musl-x64.tar.gz` | `weeeking-<версия>-linux-musl-x64` |
| Linux arm64 | `weeeking-<версия>-linux-arm64.tar.gz` | `weeeking-<версия>-linux-arm64` |
| macOS arm64 | `weeeking-<версия>-osx-arm64.tar.gz` | `weeeking-<версия>-osx-arm64` |
| macOS x64 | `weeeking-<версия>-osx-x64.tar.gz` | `weeeking-<версия>-osx-x64` |

**Установка**

- `npx -y weeeking` — npm-обёртка сама скачает бинарник под вашу платформу (на Linux gnu/musl
  определяется автоматически) и сверит sha256
- Контрольные суммы: `certutil -hashfile <файл> SHA256` (Windows) или `sha256sum -c <файл>.sha256` (Linux/macOS)
- Аттестации: `gh attestation verify <файл> -R OniVe/weeeking`

Подробности — в [CHANGELOG.md](https://github.com/OniVe/weeeking/blob/main/CHANGELOG.md).
