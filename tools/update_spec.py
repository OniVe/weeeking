#!/usr/bin/env python3
"""Кодогенератор спецификации Weeek (Python + QuickJS).

Тянет актуальную OpenAPI-спеку с официального сайта документации
(developers.weeek.net публикует её как Zudoku-чанк), исполняет чанк
в QuickJS прямо из памяти — ни один временный файл не попадает
в репозиторий — и генерирует src/spec_generated.rs: статические данные
(&'static str), которые компилируются в бинарник.

Зависимость: pip install quickjs
Запуск: python tools/update_spec.py && cargo fmt && cargo build --release
"""

from __future__ import annotations

import json
import re
import sys
import urllib.error
import urllib.request
from pathlib import Path

DOCS = "https://developers.weeek.net"
OUT_FILE = Path(__file__).resolve().parent.parent / "src" / "spec_generated.rs"
TIMEOUT_SECONDS = 60

# Операции, покрытые руками написанными (curated) тулами — вынесены из
# сгенерированных групп, чтобы у каждого действия был ровно один путь.
CURATED = [
    # reads
    "get-profile", "get-workspace", "get-workspace-members", "get-projects", "get-tags",
    "get-tasks", "get-task", "get-task-comments", "get-attachment",
    # writes
    "create-task", "update-task", "complete-task", "uncomplete-task",
    "change-task-board", "change-task-board-column",
    "create-task-comment", "delete-task-comment",
    "add-assignees-to-task", "remove-assignees-from-task",
    "add-watchers-to-task", "remove-watchers-from-task",
]

SLUG_FIX = {"boardcolumn": "board-column", "funnelstatuses": "funnel-statuses"}


def fetch_text(url: str) -> str:
    request = urllib.request.Request(url, headers={"User-Agent": "weeeking-spec-generator"})
    try:
        with urllib.request.urlopen(request, timeout=TIMEOUT_SECONDS) as response:
            return response.read().decode("utf-8")
    except urllib.error.HTTPError as exc:
        raise SystemExit(f"не удалось скачать {url}: HTTP {exc.code}") from exc
    except (urllib.error.URLError, TimeoutError, OSError) as exc:
        raise SystemExit(f"не удалось скачать {url}: {exc}") from exc


def find_exported_names(chunk: str) -> tuple[dict[str, str], re.Match[str]]:
    """Разбирает `export{s as schema,r as slugs}` -> {'schema': 's', 'slugs': 'r'}."""
    match = re.search(r"export\s*\{([^}]*)\}", chunk)
    if not match:
        raise SystemExit("в чанке не найден export{...}")
    names: dict[str, str] = {}
    for part in match.group(1).split(","):
        piece = part.strip()
        if not piece:
            continue
        if " as " in piece:
            local, exported = piece.split(" as ", 1)
            names[exported.strip()] = local.strip()
        else:
            names[piece] = piece
    return names, match


# Логика извлечения исполняется в QuickJS ровно так же, как её делал
# прежний Node-скрипт; наружу возвращается уже готовый JSON.
EXTRACTION_JS = """
(function () {
  const schema = __OUT.schema;
  const slugs = __OUT.slugs || {};
  const slugMap = slugs.tags ? slugs.tags : slugs;
  const curated = new Set(__CURATED__);
  const slugFix = __SLUG_FIX__;
  const METHODS = ["get", "post", "put", "patch", "delete"];
  const typeOf = (s) => {
    if (!s) return "unknown";
    const t = s.type;
    if (Array.isArray(t)) return t.join("|");
    return typeof t === "string" ? t : "object";
  };
  const renderEnumHint = (values) =>
    Array.isArray(values) && values.length
      ? values.map((v) => (typeof v === "string" ? v : String(v))).join("|")
      : null;

  if (!schema || !schema.paths) throw new Error("chunk did not export an OpenAPI schema");

  const operations = {};
  const groups = {};
  let total = 0;
  for (const [p, opsObj] of Object.entries(schema.paths)) {
    for (const [m, op] of Object.entries(opsObj)) {
      if (!METHODS.includes(m)) continue;
      total++;
      if (!op.operationId) throw new Error("operation without operationId: " + m + " " + p);
      const label = (op.tags && op.tags[0]) || "Other";
      const rawSlug = slugMap[label] || label.toLowerCase().replace(/[^a-z0-9]+/g, "-");
      const slug = slugFix[rawSlug] || rawSlug;
      const def = {
        id: op.operationId,
        method: m.toUpperCase(),
        path: p,
        summary: op.summary || "",
        slug,
        pathParams: [],
        queryParams: [],
        bodyFields: [],
        hasBody: !!op.requestBody,
      };
      for (const prm of op.parameters || []) {
        const param = {
          name: prm.name,
          ty: typeOf(prm.schema),
          required: !!prm.required,
          enumHint: renderEnumHint(prm.schema && prm.schema.enum),
        };
        (prm.in === "path" ? def.pathParams : def.queryParams).push(param);
      }
      const bodySchema =
        op.requestBody && op.requestBody.content && op.requestBody.content["application/json"]
          ? op.requestBody.content["application/json"].schema
          : undefined;
      const requiredBody = new Set((bodySchema && bodySchema.required) || []);
      for (const [name, fs] of Object.entries((bodySchema && bodySchema.properties) || {})) {
        def.bodyFields.push({
          name,
          ty: typeOf(fs),
          required: requiredBody.has(name),
          enumHint: renderEnumHint(fs.enum),
        });
      }
      operations[op.operationId] = def;
      if (curated.has(op.operationId)) continue;
      if (!groups[label]) groups[label] = { slug, label, opIds: [] };
      groups[label].opIds.push(op.operationId);
    }
  }
  const groupList = Object.values(groups).sort((a, b) => a.slug.localeCompare(b.slug));
  return {
    total,
    specTitle: (schema.info && schema.info.title) || "",
    operations: Object.values(operations),
    groups: groupList,
  };
})()
"""


def extract_spec(chunk: str) -> dict:
    """Исполняет чанк в QuickJS и возвращает извлечённые метаданные."""
    try:
        import quickjs  # установка: pip install quickjs
    except ImportError as exc:  # pragma: no cover - зависит от окружения
        raise SystemExit(
            "нужен движок QuickJS для исполнения чанка спеки: pip install quickjs"
        ) from exc

    names, export_match = find_exported_names(chunk)
    if "schema" not in names or "slugs" not in names:
        raise SystemExit("в export нет schema/slugs")

    prefix = chunk[: export_match.start()]
    glue = f"\nvar __OUT = {{ schema: {names['schema']}, slugs: {names['slugs']} }};\n"
    extraction = (
        EXTRACTION_JS
        .replace("__CURATED__", json.dumps(CURATED))
        .replace("__SLUG_FIX__", json.dumps(SLUG_FIX))
    )
    code = prefix + glue + "var __RESULT = " + extraction + ";\nJSON.stringify(__RESULT);\n"

    context = quickjs.Context()
    try:
        raw = context.eval(code)
    except quickjs.JSException as exc:  # pragma: no cover - защита от изменения чанка
        raise SystemExit(f"QuickJS не смог исполнить чанк: {exc}") from exc
    if not isinstance(raw, str):
        raise SystemExit("неожиданный тип результата из QuickJS")
    try:
        return json.loads(raw)
    except json.JSONDecodeError as exc:
        raise SystemExit(f"QuickJS вернул не-JSON: {exc}") from exc


def esc(value: object) -> str:
    """Экранирование для Rust-строкового литерала (как в прежнем генераторе)."""
    text = str(value)
    return (
        text.replace("\\", "\\\\")
        .replace('"', '\\"')
        .replace("\n", "\\n")
        .replace("\r", "\\r")
        .replace("\t", "\\t")
    )


def opt_str(value: str | None) -> str:
    return "None" if value is None else f'Some("{esc(value)}")'


def param_def(param: dict) -> str:
    return (
        f'ParamDef {{ name: "{esc(param["name"])}", ty: "{esc(param["ty"])}", '
        f'required: {"true" if param["required"] else "false"}, '
        f'enum_hint: {opt_str(param.get("enumHint"))} }}'
    )


def render_params(items: list[dict]) -> str:
    if not items:
        return "&[]"
    return "&[" + ", ".join(param_def(item) for item in items) + "]"


def render_rust(spec_url: str, chunk_hash: str, data: dict) -> str:
    lines: list[str] = []
    lines.append("// @generated by tools/update_spec.py — do not edit by hand.")
    lines.append(f"// Source: {spec_url}")
    lines.append("// Regenerate: python tools/update_spec.py && cargo fmt && cargo build --release")
    lines.append("")
    lines.append("use crate::spec::{GroupDef, OpDef, ParamDef};")
    lines.append("")
    lines.append(f'pub const SPEC_URL: &str = "{esc(spec_url)}";')
    lines.append(f'pub const SPEC_TITLE: &str = "{esc(data["specTitle"])}";')
    lines.append(f'pub const SPEC_VERSION: &str = "{esc(chunk_hash)}";')
    lines.append("")
    lines.append("pub const OPERATIONS: &[OpDef] = &[")
    for op in data["operations"]:
        lines.append("    OpDef {")
        lines.append(f'        id: "{esc(op["id"])}",')
        lines.append(f'        method: "{esc(op["method"])}",')
        lines.append(f'        path: "{esc(op["path"])}",')
        lines.append(f'        summary: "{esc(op["summary"])}",')
        lines.append(f'        path_params: {render_params(op["pathParams"])},')
        lines.append(f'        query_params: {render_params(op["queryParams"])},')
        lines.append(f'        body_fields: {render_params(op["bodyFields"])},')
        lines.append(f'        has_body: {"true" if op["hasBody"] else "false"},')
        lines.append("    },")
    lines.append("];")
    lines.append("")
    lines.append("pub const GROUPS: &[GroupDef] = &[")
    for group in data["groups"]:
        ids = ", ".join(f'"{esc(op_id)}"' for op_id in group["opIds"])
        lines.append(
            f'    GroupDef {{ slug: "{esc(group["slug"])}", label: "{esc(group["label"])}", '
            f"op_ids: &[{ids}] }},"
        )
    lines.append("];")
    lines.append("")
    lines.append("/// Только для тестов: операции вне групп, реализованные curated-тулами.")
    lines.append("#[cfg(test)]")
    lines.append("pub const CURATED: &[&str] = &[")
    for op_id in sorted(CURATED):
        lines.append(f'    "{esc(op_id)}",')
    lines.append("];")
    lines.append("")
    return "\n".join(lines)


def main() -> int:
    home = fetch_text(f"{DOCS}/")
    entry_match = re.search(r"/assets/entry\.client-[A-Za-z0-9_-]+\.js", home)
    if not entry_match:
        raise SystemExit("entry.client чанк не найден в HTML документации")
    entry = fetch_text(DOCS + entry_match.group(0))

    spec_match = re.search(r"\./weeek\.yaml-([A-Za-z0-9_-]+)\.js", entry)
    if not spec_match:
        raise SystemExit("weeeek.yaml чанк не найден в entry-бандле")
    chunk_hash = spec_match.group(1)
    spec_url = f"{DOCS}/assets/{spec_match.group(0)[2:]}"
    print(f"spec chunk: {spec_url}", file=sys.stderr)

    chunk = fetch_text(spec_url)
    data = extract_spec(chunk)

    OUT_FILE.write_text(render_rust(spec_url, chunk_hash, data), encoding="utf-8", newline="\n")
    grouped = sum(len(group["opIds"]) for group in data["groups"])
    print(
        f"operations: {len(data['operations'])} (в группах: {grouped}, curated: {len(CURATED)}); "
        f"groups: {len(data['groups'])}; -> {OUT_FILE}",
        file=sys.stderr,
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
