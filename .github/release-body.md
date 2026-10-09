## [0.3.0](https://github.com/OniVe/weeeking/compare/v0.2.0...v0.3.0)

Третий релиз Weeeking — MCP-сервер для Weeek: шесть платформ, версия в именах артефактов,
хэши всех файлов в этом описании, npm-обёртка ставит нужный бинарник автоматически.

**Что нового**

- Платформы: Windows x64, Linux x64 (glibc и musl-static), Linux arm64, macOS arm64 и x64
- Имена артефактов с версией: `weeeking-<версия>-<платформа>` (+ `.sha256`); сырые бинарники
  для npm-обёртки — без распаковки архивов
- Все SHA256-суммы перечислены ниже (секция добавляется автоматически при сборке релиза)

**Артефакты**

| Платформа | Архив | Сырой бинарник |
| --- | --- | --- |
| Windows x64 | `weeeking-<версия>-win-x64.zip` | `weeeking-<версия>-win-x64.exe` |
| Linux x64 (glibc) | `weeeking-<версия>-linux-x64.tar.gz` | `weeeking-<версия>-linux-x64` |
| Linux x64 (musl) | `weeeking-<версия>-linux-musl-x64.tar.gz` | `weeeking-<версия>-linux-musl-x64` |
| Linux arm64 | `weeeking-<версия>-linux-arm64.tar.gz` | `weeeking-<версия>-linux-arm64` |
| macOS arm64 | `weeeking-<версия>-osx-arm64.tar.gz` | `weeeking-<версия>-osx-arm64` |
| macOS x64 | `weeeking-<версия>-osx-x64.tar.gz` | `weeeking-<версия>-osx-x64` |

**Установка**

- `npx -y weeeking` — npm-обёртка сама скачает бинарник под вашу платформу (на Linux gnu/musl
  определяется автоматически) и сверит sha256
- Контрольные суммы: `certutil -hashfile <файл> SHA256` (Windows) или `sha256sum -c <файл>.sha256` (Linux/macOS)

Подробности — в [CHANGELOG.md](https://github.com/OniVe/weeeking/blob/main/CHANGELOG.md).
