// Кодогенератор спецификации Weeek.
//
// Тянет актуальную OpenAPI-спеку с официального сайта документации
// (developers.weeek.net публикует её как Zudoku-чанк), исполняет чанк
// прямо из памяти (data: URL — ни один временный файл не попадает
// в репозиторий) и генерирует src/spec_generated.rs — статические
// Rust-данные (&'static str), которые компилируются в бинарник.
//
// Запуск: node tools/update-spec.mjs && cargo fmt && cargo build --release
import { rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { pathToFileURL } from "node:url";
import path from "node:path";

const DOCS = "https://developers.weeek.net";
const OUT_FILE = path.resolve("src/spec_generated.rs");
const METHODS = ["get", "post", "put", "patch", "delete"];

// Операции, покрытые руками написанными (curated) тулами — вынесены из
// сгенерированных групп, чтобы у каждого действия был ровно один путь.
const CURATED = new Set([
  // reads
  "get-profile", "get-workspace", "get-workspace-members", "get-projects", "get-tags",
  "get-tasks", "get-task", "get-task-comments", "get-attachment",
  // writes
  "create-task", "update-task", "complete-task", "uncomplete-task",
  "change-task-board", "change-task-board-column",
  "create-task-comment", "delete-task-comment",
  "add-assignees-to-task", "remove-assignees-from-task",
  "add-watchers-to-task", "remove-watchers-from-task",
]);

const SLUG_FIX = { boardcolumn: "board-column", funnelstatuses: "funnel-statuses" };

const fetchText = async (url) => {
  const res = await fetch(url);
  if (!res.ok) throw new Error(`HTTP ${res.status} for ${url}`);
  return res.text();
};

// 1. Чанк спеки: находим и импортируем из памяти (fallback — системный temp).
const home = await fetchText(`${DOCS}/`);
const entryRel = home.match(/\/assets\/entry\.client-[A-Za-z0-9_-]+\.js/)?.[0];
if (!entryRel) throw new Error("entry.client chunk not found in docs HTML");
const entry = await fetchText(`${DOCS}${entryRel}`);
const specRel = entry.match(/\.\/weeek\.yaml-[A-Za-z0-9_-]+\.js/)?.[0];
if (!specRel) throw new Error("weeek.yaml chunk not found in entry bundle");
const specUrl = `${DOCS}/assets/${specRel.slice(2)}`;
console.error(`spec chunk: ${specUrl}`);

const specJs = await fetchText(specUrl);

async function importChunk(code) {
  try {
    const dataUrl = "data:text/javascript;base64," + Buffer.from(code, "utf8").toString("base64");
    return await import(dataUrl);
  } catch (err) {
    console.error(`data-URL import не сработал (${err.message}); fallback на temp-файл`);
    const file = path.join(tmpdir(), `weeeking-spec-${Date.now()}.mjs`);
    await writeFile(file, code);
    try {
      return await import(pathToFileURL(file).href);
    } finally {
      await rm(file, { force: true });
    }
  }
}

const mod = await importChunk(specJs);
const schema = mod.schema;
// Карта slug'ов лежит в mod.slugs.tags; mod.slugs — контейнер { operations, tags }.
const slugMap = mod.slugs?.tags ?? mod.slugs ?? {};
if (!schema?.paths) throw new Error("chunk did not export an OpenAPI schema");

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

const operations = {};
const groups = new Map();
let total = 0;

for (const [p, opsObj] of Object.entries(schema.paths)) {
  for (const [m, op] of Object.entries(opsObj)) {
    if (!METHODS.includes(m)) continue;
    total++;
    if (!op.operationId) throw new Error(`operation without operationId: ${m} ${p}`);

    const label = op.tags?.[0] ?? "Other";
    const rawSlug = slugMap[label] ?? label.toLowerCase().replace(/[^a-z0-9]+/g, "-");
    const slug = SLUG_FIX[rawSlug] ?? rawSlug;
    const def = {
      id: op.operationId,
      method: m.toUpperCase(),
      path: p,
      summary: op.summary ?? "",
      slug,
      pathParams: [],
      queryParams: [],
      bodyFields: [],
      hasBody: Boolean(op.requestBody),
    };
    for (const prm of op.parameters ?? []) {
      const param = {
        name: prm.name,
        ty: typeOf(prm.schema),
        required: Boolean(prm.required),
        enumHint: renderEnumHint(prm.schema?.enum),
      };
      (prm.in === "path" ? def.pathParams : def.queryParams).push(param);
    }
    const bodySchema = op.requestBody?.content?.["application/json"]?.schema;
    const requiredBody = new Set(bodySchema?.required ?? []);
    for (const [name, fs] of Object.entries(bodySchema?.properties ?? {})) {
      def.bodyFields.push({
        name,
        ty: typeOf(fs),
        required: requiredBody.has(name),
        enumHint: renderEnumHint(fs.enum),
      });
    }

    operations[op.operationId] = def;
    if (CURATED.has(op.operationId)) continue;
    if (!groups.has(label)) groups.set(label, { slug, label, opIds: [] });
    groups.get(label).opIds.push(op.operationId);
  }
}

// 2. Генерация Rust-кода.
const esc = (s) =>
  String(s)
    .replace(/\\/g, "\\\\")
    .replace(/"/g, '\\"')
    .replace(/\n/g, "\\n")
    .replace(/\r/g, "\\r")
    .replace(/\t/g, "\\t");

const optStr = (s) => (s == null ? "None" : `Some("${esc(s)}")`);
const paramDef = (p) =>
  `ParamDef { name: "${esc(p.name)}", ty: "${esc(p.ty)}", required: ${p.required}, enum_hint: ${optStr(p.enumHint)} }`;
const params = (list) => (list.length ? `&[${list.map(paramDef).join(", ")}]` : "&[]");

// Версия спеки = хэш чанка из его имени файла (weeek.yaml-<hash>.js):
// детерминированная метка источника — повторная генерация даёт тот же файл.
const chunkHash = specRel.match(/weeek\.yaml-([A-Za-z0-9_-]+)\.js/)?.[1] ?? "unknown";

const lines = [];
lines.push("// @generated by tools/update-spec.mjs — do not edit by hand.");
lines.push(`// Source: ${specUrl}`);
lines.push("// Regenerate: node tools/update-spec.mjs && cargo fmt && cargo build --release");
lines.push("");
lines.push("use crate::spec::{GroupDef, OpDef, ParamDef};");
lines.push("");
lines.push(`pub const SPEC_URL: &str = "${esc(specUrl)}";`);
lines.push(`pub const SPEC_TITLE: &str = "${esc(schema.info?.title ?? "")}";`);
lines.push(`pub const SPEC_VERSION: &str = "${esc(chunkHash)}";`);
lines.push("");
lines.push("pub const OPERATIONS: &[OpDef] = &[");
for (const op of Object.values(operations)) {
  lines.push("    OpDef {");
  lines.push(`        id: "${esc(op.id)}",`);
  lines.push(`        method: "${esc(op.method)}",`);
  lines.push(`        path: "${esc(op.path)}",`);
  lines.push(`        summary: "${esc(op.summary)}",`);
  lines.push(`        path_params: ${params(op.pathParams)},`);
  lines.push(`        query_params: ${params(op.queryParams)},`);
  lines.push(`        body_fields: ${params(op.bodyFields)},`);
  lines.push(`        has_body: ${op.hasBody},`);
  lines.push("    },");
}
lines.push("];");
lines.push("");
lines.push("pub const GROUPS: &[GroupDef] = &[");
for (const group of [...groups.values()].sort((a, b) => a.slug.localeCompare(b.slug))) {
  const ids = group.opIds.map((id) => `"${esc(id)}"`).join(", ");
  lines.push(`    GroupDef { slug: "${esc(group.slug)}", label: "${esc(group.label)}", op_ids: &[${ids}] },`);
}
lines.push("];");
lines.push("");
lines.push("/// Только для тестов: операции вне групп, реализованные curated-тулами.");
lines.push("#[cfg(test)]");
lines.push("pub const CURATED: &[&str] = &[");
for (const id of [...CURATED].sort()) {
  lines.push(`    "${esc(id)}",`);
}
lines.push("];");
lines.push("");

await writeFile(OUT_FILE, lines.join("\n"));
const grouped = [...groups.values()].reduce((acc, g) => acc + g.opIds.length, 0);
console.error(
  `operations: ${Object.keys(operations).length} (в группах: ${grouped}, curated: ${CURATED.size}); groups: ${groups.size}; -> ${path.relative(process.cwd(), OUT_FILE)}`,
);
