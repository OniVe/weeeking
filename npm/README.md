# weeeking

npm-обёртка для [Weeeking](https://github.com/OniVe/weeeking) — MCP-сервера Weeek
(полное покрытие публичного API). Обёртка скачивает нативный бинарник
(**Windows x64/arm64**, **Linux x64** (gnu/musl), **Linux arm64**, **macOS arm64/x64**) из GitHub Releases
этой же версии, сверяет sha256 и запускает его.

## Использование

```bash
npx -y weeeking              # stdio MCP-сервер
npx -y weeeking --help       # справка
npx -y weeeking store-token  # сохранить токен в системное хранилище
```

Пример конфигурации OpenCode:

```jsonc
{
  "mcp": {
    "weeek": {
      "type": "local",
      "command": ["npx", "-y", "weeeking"],
      "enabled": true,
      "environment": {
        "WEEEK_API_TOKEN": "…",
        "READ_ONLY": "true"
      }
    }
  }
}
```

## Как это работает

- при установке (`postinstall`) обёртка скачивает бинарник под вашу платформу
  (на Linux x64 вариант gnu/musl определяется автоматически);
  если сети нет или прокси блокирует — установка не падает, launcher повторит при первом запуске;
- `--ignore-scripts` не проблема: скачивание произойдёт при первом `npx weeeking`;
- sha256-сумма сверяется с файлом `.sha256` из того же релиза (fail-closed при несовпадении);
- stdout остаётся чистым (весь служебный вывод — в stderr) — это важно для MCP stdio.

## Переменные окружения обёртки

| Переменная | Назначение |
| --- | --- |
| `WEEEKING_BINARY` | путь к своему бинарнику — скачивание не выполняется |
| `WEEEKING_DOWNLOAD_BASE` | зеркало для скачивания (по умолчанию GitHub Releases) |
| `WEEEKING_SKIP_DOWNLOAD=1` | не скачивать бинарник при установке |

Переменные самого сервера (`WEEEK_API_TOKEN`, `READ_ONLY`, …) описаны
в [README репозитория](https://github.com/OniVe/weeeking#переменные-окружения).

Лицензия — [MIT](./LICENSE).
