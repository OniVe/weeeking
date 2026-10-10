# Weeeking 🛡️

[Русский](README.md) · **English**

[![CI](https://github.com/OniVe/weeeking/actions/workflows/ci.yml/badge.svg)](https://github.com/OniVe/weeeking/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/OniVe/weeeking)](https://github.com/OniVe/weeeking/releases)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

> *Weeek and conquer.* An MCP server for [Weeek](https://weeek.net) with **full coverage of the public API**:
> 157 operations from the official OpenAPI spec. A single Rust binary, no Node and no runtime
> dependencies. Releases for seven platforms: **Windows x64/arm64, Linux x64 (glibc/musl), Linux arm64, macOS arm64/x64**.

## Features

- **13 admin groups with an `action` parameter** — `weeek_project`, `weeek_board`, `weeek_board_column`,
  `weeek_custom_fields`, `weeek_portfolio`, `weeek_tags`, `weeek_task` (timers, time entries, attachments,
  parent…), `weeek_funnels`, `weeek_funnel_statuses`, `weeek_deals`, `weeek_organizations`, `weeek_contacts`,
  `weeek_currencies`.
- **12 curated tools** for everyday work: `weeek_context`, `weeek_search_tasks`, `weeek_get_task`,
  `weeek_list_comments`, `weeek_download_attachment` + write tools (`weeek_create_task`, `weeek_update_task`,
  `weeek_move_task`, `weeek_complete_task`, `weeek_set_task_people`, `weeek_add_comment`, `weeek_delete_comment`).
- **Read-only by default**: mutating actions are not registered until `READ_ONLY=false` is set.
- **MCP resources and prompts**: `weeek://me` and `weeek://projects` are read by the client without a tool
  call; the “My tasks today” and “Week review” prompts are ready-made agent scenarios.
- **LLM ergonomics**: `compact=true` strips nulls and empty values (including empty array elements) from heavy
  responses; `fetchAll=true` (+`maxItems`) collects pages of tasks and comments (100 items per page by
  default, at most 50 pages) and reports a `truncated` flag.
- **Reliability**: retries on 429 and 502/503/504 with `Retry-After` and exponential backoff (429 — for any
  method, 502/503/504 and network failures — only for GET/HEAD; timeouts are not retried); `WEEEK_LOG=debug`
  — HTTP logs to stderr (method, path, status, duration; token not logged).
- **Spec generator**: `tools/update_spec.py` pulls the OpenAPI spec from developers.weeek.net and generates
  `src/spec_generated.rs` — static data (`&'static str`) compiled into the binary; no JSON and no parsing at
  runtime, and the spec chunk is not kept in the repository.

## Build

Rust 1.88+ required (MSVC toolchain on Windows).

```bash
cargo build --release
# → target/release/weeeking.exe (Windows) / target/release/weeeking (Linux, macOS)
```

## Releases

Prebuilt binaries live in [GitHub Releases](https://github.com/OniVe/weeeking/releases): archives named
`weeeking-<version>-<platform>` and raw binaries for the npm wrapper; `.sha256` files sit next to them, and
the hashes are also listed in the release description.

| Platform | Archive | Raw binary |
| --- | --- | --- |
| Windows x64 | `weeeking-<version>-win-x64.zip` | `weeeking-<version>-win-x64.exe` |
| Windows arm64 | `weeeking-<version>-win-arm64.zip` | `weeeking-<version>-win-arm64.exe` |
| Linux x64 (glibc) | `weeeking-<version>-linux-x64.tar.gz` | `weeeking-<version>-linux-x64` |
| Linux x64 (musl, static) | `weeeking-<version>-linux-musl-x64.tar.gz` | `weeeking-<version>-linux-musl-x64` |
| Linux arm64 | `weeeking-<version>-linux-arm64.tar.gz` | `weeeking-<version>-linux-arm64` |
| macOS arm64 | `weeeking-<version>-osx-arm64.tar.gz` | `weeeking-<version>-osx-arm64` |
| macOS x64 | `weeeking-<version>-osx-x64.tar.gz` | `weeeking-<version>-osx-x64` |

```bash
# checksum verification
certutil -hashfile weeeking-<version>-win-x64.zip SHA256          # Windows
sha256sum -c weeeking-<version>-linux-x64.tar.gz.sha256           # Linux
```

Every release file is signed with a GitHub attestation (Sigstore) — verifiable build provenance:

```bash
gh attestation verify weeeking-<version>-win-x64.exe -R OniVe/weeeking
```

### npm (npx)

```bash
npx -y weeeking     # stdio MCP server; the binary is downloaded and verified against sha256
```

The `weeeking` package is a thin wrapper: it fetches the native binary for your platform (Windows x64/arm64,
Linux x64 gnu/musl, Linux arm64, macOS arm64/x64) from the GitHub release of the same version. It is
published via npm trusted publishing (OIDC, with provenance).

The server is also published in the official MCP Registry — `io.github.OniVe/weeeking`; GitHub's MCP
catalog and other aggregators pick it up from there.

A binary downloaded through a browser on macOS may be quarantined by Gatekeeper — remove the quarantine
(`xattr -d com.apple.quarantine <file>`) or install via `npx`/`curl`.

## Environment variables

| Variable | Default | Description |
| --- | --- | --- |
| `WEEEK_API_TOKEN` | — | Weeek API token (workspace Settings → API). Reads are impossible without it. |
| `READ_ONLY` | `true` | `false`/`0` — enable mutating operations (create, update, delete, CRM). |
| `WEEEK_LANG` | `ru` | Language of user-facing texts (`ru`/`en`): help, tool descriptions, prompts, errors. |
| `WEEEK_BASE_URL` | `https://api.weeek.net/public/v1` | Custom proxy/host if needed. |
| `WEEEK_TIMEOUT_MS` | `30000` | Request timeout. |
| `WEEEK_MAX_RESPONSE_CHARS` | `60000` | Truncation limit for large responses. |
| `WEEEK_DISABLE_KEYCHAIN` | — | `1` — do not read the token from the system keychain. |
| `WEEEK_KEYCHAIN_ACCOUNT` | `api-token` | Keychain entry name (for multiple accounts). |
| `WEEEK_MAX_ATTACHMENT_BYTES` | `67108864` (64 MiB) | Attachment download limit (protects against huge responses). |
| `WEEEK_RETRY_MAX` | `2` | Extra attempts on 429, 502/503/504 and network failures (0–5); timeouts are not retried. |
| `WEEEK_RETRY_BASE_MS` | `300` | Exponential backoff base (capped at 5 s; `Retry-After` is honored, capped at 30 s). |
| `WEEEK_LOG` | — | `debug` — verbose HTTP logs to stderr (method, path, status, ms; the token is never logged). |

### Where the token comes from

1. `WEEEK_API_TOKEN` (or `WEEEK_TOKEN`) from the environment — it always takes priority.
2. Otherwise the system keychain — service `weeek-mcp`, entry name **`WEEEK_KEYCHAIN_ACCOUNT`**
   (default `api-token`): Windows Credential Manager (target `api-token.weeek-mcp`), macOS Keychain,
   Linux Secret Service (GNOME Keyring/KWallet via D-Bus); written by `weeeking store-token`.
   The value is never logged.
3. If both are empty, the server still starts, but calls return `isError` with a hint.

The system keychain works on all platforms; **headless Linux without D-Bus** has none — provide the token
through the `WEEEK_API_TOKEN` or `WEEEK_TOKEN` environment variables.

### Multiple Weeek accounts

The acting identity (author of comments and tasks) is defined by the token: the API always acts as the token
owner. A second user needs **their own token** (created in workspace Settings → API; the section is available
to super admins and admins).

```text
# 1) Store the second account token (hidden input, the token never reaches argv)
weeeking store-token --account api-token-anna

# 2) Run the server as that account
$env:WEEEK_KEYCHAIN_ACCOUNT = "api-token-anna"; weeeking   # PowerShell (Windows)
WEEEK_KEYCHAIN_ACCOUNT=api-token-anna weeeking             # bash/zsh (macOS, Linux)
```

Example of a second instance in the OpenCode config (native V2 form):

```jsonc
"mcp": {
  "servers": {
    "weeek-anna": {
      "type": "local",
      "command": ["C:\\path\\weeeking.exe"],
      "environment": {
        "WEEEK_KEYCHAIN_ACCOUNT": "api-token-anna",
        "READ_ONLY": "false"
      }
    }
  }
}
```

The agent can then choose who acts: `weeek-anna_*` tools write as Anna, `weeek_*` — as the primary account.
To delete a keychain entry: Windows — `cmdkey /delete:api-token-anna.weeek-mcp`;
macOS — `security delete-generic-password -s weeek-mcp -a api-token-anna`;
Linux — via your keyring manager (GNOME Keyring / KWallet).

Headless Linux without D-Bus has no system keychain: use a separate server instance with its own
`WEEEK_API_TOKEN` in the environment.

### CLI

```text
weeeking                                run the MCP server (stdio)
weeeking store-token [--account <name>] save the token to the system keychain
weeeking --help                         show help
```

## Setup (OpenCode)

OpenCode V2 native form — servers live under `mcp.servers`:

```jsonc
"mcp": {
  "servers": {
    "weeek": {
      "type": "local",
      "command": ["C:\\path\\weeeking.exe"],   // on any OS: ["npx", "-y", "weeeking"]
      "environment": { "READ_ONLY": "false" }
    }
  }
}
```

Add it from the CLI: `opencode mcp add weeek -- npx -y weeeking` (add `--global` for all projects).
Disable a server without removing it from the config: `"disabled": true` (native V2 form; the V1 form
`"mcp": { "weeek": { … } }` with `enabled` is still supported).

Any other MCP client: the command is the path to the binary (or `npx -y weeeking`), the transport is stdio.

## Updating the spec

```bash
pip install quickjs            # spec generator dependency (one-time)
python tools/update_spec.py    # developers.weeek.net → src/spec_generated.rs (code generation)
cargo fmt && cargo build --release
```

## Development

```bash
cargo fmt
cargo clippy --all-targets
cargo test                   # unit + mock suite (HTTP against an emulator) + smoke
```

Acceptance smoke against the live API (before a release; read-only by default):

```bash
python tools/live_smoke.py
python tools/live_smoke.py --write-test <PROJECT_ID>   # + a mutation cycle in a sandbox project
```

Rebuild while OpenCode is running (the exe is locked by the running MCP server):

```bash
python tools\rebuild.py
```

## Weeek API limitations (important for the agent)

- A task description can only be set **at creation time** — it cannot be changed later.
- `tags` in `weeek_update_task` **replaces the whole list** — read the task first.
- Comments can be created and deleted, but **not edited**.

## License

MIT
