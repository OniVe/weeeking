#!/usr/bin/env python3
"""Приёмочный smoke-тест против живого Weeek API — перед релизом.

По умолчанию только чтение: поднимает release-бинарник weeeking по stdio,
делает MCP-рукопожатие и вызывает read-only инструменты. Токен скрипт не
трогает вовсе — сервер сам берёт его из системного хранилища.

Флаг --write-test <PROJECT_ID> добавляет контролируемый цикл мутаций
(создать задачу -> комментарий -> завершить -> удалить) в указанном проекте;
запускай его только для специального sandbox-проекта.

Запуск:
    python tools/live_smoke.py
    python tools/live_smoke.py --write-test 123
"""

from __future__ import annotations

import argparse
import json
import os
import queue
import subprocess
import sys
import threading
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_BIN = ROOT / "target" / "release" / ("weeeking.exe" if os.name == "nt" else "weeeking")


class McpClient:
    """Минимальный MCP-клиент поверх stdio (newline-delimited JSON-RPC)."""

    def __init__(self, binary: Path, read_only: bool = True):
        env = dict(os.environ)
        env["WEEEK_API_TOKEN"] = ""
        env["WEEEK_TOKEN"] = ""
        env["READ_ONLY"] = "true" if read_only else "false"
        self.proc = subprocess.Popen(
            [str(binary)],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=env,
            text=True,
            encoding="utf-8",
        )
        self._next_id = 0
        self.stderr_lines: list[str] = []
        # stdout читается отдельным потоком: readline() в основном потоке
        # блокировался бы на живом молчащем сервере и таймаут не срабатывал.
        self._responses: queue.Queue[str | None] = queue.Queue()
        self._reader = threading.Thread(target=self._read_stdout, daemon=True)
        self._reader.start()
        try:
            self._initialize()
        except BaseException:
            # не оставляем живой процесс при сбое рукопожатия
            try:
                self.proc.kill()
            except Exception:  # pragma: no cover
                pass
            raise

    def _send(self, payload: dict) -> None:
        assert self.proc.stdin is not None
        self.proc.stdin.write(json.dumps(payload) + "\n")
        self.proc.stdin.flush()

    def _read_stdout(self) -> None:
        assert self.proc.stdout is not None
        for line in self.proc.stdout:
            self._responses.put(line)
        self._responses.put(None)

    def _wait_for(self, message_id: int, timeout: float = 120.0) -> dict:
        deadline = time.monotonic() + timeout
        while True:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise SystemExit(self._stderr_tail("таймаут ожидания ответа"))
            try:
                line = self._responses.get(timeout=remaining)
            except queue.Empty:
                raise SystemExit(self._stderr_tail("таймаут ожидания ответа"))
            if line is None:
                raise SystemExit(self._stderr_tail("сервер закрыл stdout"))
            try:
                message = json.loads(line)
            except json.JSONDecodeError:
                continue
            if message.get("id") == message_id:
                if "error" in message:
                    raise SystemExit(f"JSON-RPC ошибка: {message['error']}")
                return message["result"]

    def _stderr_tail(self, prefix: str) -> str:
        # Сначала гасим процесс: чтение stderr из живого сервера блокируется до EOF.
        try:
            self.proc.kill()
        except Exception:  # pragma: no cover - процесс уже мёртв
            pass
        try:
            # дожидаемся фактического завершения, чтобы процесс не оставался
            # в состоянии termination после возврата
            self.proc.wait(timeout=5)
        except Exception:  # pragma: no cover - уже мёртв или не дождались
            pass
        rest = ""
        if self.proc.stderr is not None:
            try:
                rest = self.proc.stderr.read()
            except Exception:  # pragma: no cover - поток уже закрыт
                rest = ""
        self.stderr_lines = [line for line in rest.splitlines() if line]
        tail = "\n".join(self.stderr_lines[-5:])
        return f"{prefix}\n{tail}"

    def _initialize(self) -> None:
        self._next_id += 1
        self._send(
            {
                "jsonrpc": "2.0",
                "id": self._next_id,
                "method": "initialize",
                "params": {
                    "protocolVersion": "2025-06-18",
                    "capabilities": {},
                    "clientInfo": {"name": "live-smoke", "version": "1.0.0"},
                },
            }
        )
        self._wait_for(self._next_id)
        self._send({"jsonrpc": "2.0", "method": "notifications/initialized"})

    def call(self, name: str, arguments: dict) -> dict:
        self._next_id += 1
        self._send(
            {
                "jsonrpc": "2.0",
                "id": self._next_id,
                "method": "tools/call",
                "params": {"name": name, "arguments": arguments},
            }
        )
        return self._wait_for(self._next_id)

    def list_tools(self) -> list[str]:
        self._next_id += 1
        self._send({"jsonrpc": "2.0", "id": self._next_id, "method": "tools/list", "params": {}})
        result = self._wait_for(self._next_id)
        return [tool["name"] for tool in result["tools"]]

    def close(self) -> None:
        try:
            if self.proc.stdin:
                self.proc.stdin.close()
            self.proc.wait(timeout=10)
        except Exception:  # pragma: no cover - процесс уже завершился
            self.proc.kill()


def tool_text(result: dict) -> str:
    return "\n".join(item.get("text", "") for item in result.get("content", []))


def parse_json(text: str) -> dict:
    try:
        parsed = json.loads(text)
    except json.JSONDecodeError:
        return {}
    return parsed if isinstance(parsed, dict) else {}


def check(condition: bool, message: str, failures: list[str]) -> None:
    mark = "ok " if condition else "FAIL"
    print(f"  [{mark}] {message}")
    if not condition:
        failures.append(message)


def run_read_only(client: McpClient, failures: list[str], write_mode: bool) -> int | None:
    print("read-only проверки:")
    tools = client.list_tools()
    minimum = 25 if write_mode else 15
    check(len(tools) >= minimum, f"инструментов в режиме {'полном' if write_mode else 'read-only'}: {len(tools)}", failures)
    if write_mode:
        check("weeek_create_task" in tools, "write-тулы доступны (READ_ONLY=false)", failures)
    else:
        check("weeek_create_task" not in tools, "write-тулы скрыты (READ_ONLY=true)", failures)

    context = client.call("weeek_context", {"refresh": True})
    check(not context.get("isError"), "weeek_context отвечает", failures)
    payload = parse_json(tool_text(context))
    me = payload.get("me") or {}
    workspace = payload.get("workspace") or {}
    projects = payload.get("projects") or []
    check(bool(me.get("id")), f"пользователь: {me.get('firstName', '?')} {me.get('lastName', '')}".strip(), failures)
    check(bool(workspace.get("title")), f"воркспейс: {workspace.get('title')}", failures)
    check(len(projects) > 0, f"проектов видно: {len(projects)}", failures)

    search = client.call("weeek_search_tasks", {"completed": False, "perPage": 5})
    check(not search.get("isError"), "weeek_search_tasks отвечает", failures)
    tasks = parse_json(tool_text(search)).get("tasks") or []
    check(isinstance(tasks, list), f"открытых задач в выборке: {len(tasks)}", failures)

    task_id = tasks[0].get("id") if tasks else None
    if task_id:
        task = client.call("weeek_get_task", {"taskId": task_id})
        check(not task.get("isError"), f"weeek_get_task({task_id}) отвечает", failures)
        card = parse_json(tool_text(task))
        check("task" in card, f"карточка задачи {task_id} получена", failures)
    else:
        print("  [skip] задач для выборки нет — карточку не проверяем")
    return task_id


def run_write_test(client: McpClient, project_id: int, failures: list[str]) -> None:
    print(f"write-цикл в проекте {project_id} (создать -> комментарий -> завершить -> удалить):")
    title = f"Weeeking live-smoke {time.strftime('%Y-%m-%d %H:%M:%S')}"
    created = client.call("weeek_create_task", {"title": title, "projectId": project_id})
    check(not created.get("isError"), "задача создана", failures)
    payload = parse_json(tool_text(created))
    task = payload.get("task") if isinstance(payload.get("task"), dict) else payload
    task_id = task.get("id")
    if not task_id:
        check(False, "не удалось получить id созданной задачи", failures)
        return

    comment = client.call("weeek_add_comment", {"taskId": task_id, "markdown": "live-smoke: ок"})
    check(not comment.get("isError"), "комментарий добавлен", failures)

    completed = client.call("weeek_complete_task", {"taskId": task_id, "completed": True})
    check(not completed.get("isError"), "задача завершена", failures)

    deleted = client.call("weeek_task", {"action": "delete-task", "params": {"id": task_id}})
    check(not deleted.get("isError"), "задача удалена (уборка)", failures)


def main() -> int:
    parser = argparse.ArgumentParser(description="Приёмочный smoke против живого Weeek API")
    parser.add_argument("--bin", type=Path, default=DEFAULT_BIN, help="путь к бинарнику weeeking")
    parser.add_argument(
        "--write-test",
        type=int,
        metavar="PROJECT_ID",
        help="контролируемый цикл мутаций в указанном (sandbox) проекте",
    )
    args = parser.parse_args()

    if not args.bin.exists():
        raise SystemExit(f"бинарник не найден: {args.bin} (соберите: python tools/rebuild.py)")

    failures: list[str] = []
    write_mode = args.write_test is not None
    client: McpClient | None = None
    try:
        client = McpClient(args.bin, read_only=not write_mode)
        run_read_only(client, failures, write_mode)
        if write_mode:
            run_write_test(client, args.write_test, failures)
    finally:
        if client is not None:
            client.close()

    if failures:
        print(f"\nFAILED: {len(failures)} проверок не прошло")
        return 1
    print("\nOK: live smoke пройден")
    return 0


if __name__ == "__main__":
    sys.exit(main())
