// Fetches the current Weeek OpenAPI spec from the official docs site
// (developers.weeek.net ships it as a Zudoku data chunk) and writes
// spec/operations.json, which the Rust server embeds at compile time.
//
// Dev-only tool: requires Node 20+. Run: node tools/update-spec.mjs
import { mkdir, writeFile } from "node:fs/promises";
import { pathToFileURL } from "node:url";
import path from "node:path";

const DOCS = "https://developers.weeek.net";
const OUT_DIR = path.resolve("spec");
const TMP_CHUNK = path.join(OUT_DIR, ".spec-chunk.mjs");
const OUT_FILE = path.join(OUT_DIR, "operations.json");
const METHODS = ["get", "post", "put", "patch", "delete"];

// Operations covered by hand-written (curated) tools — kept out of the
// generated admin groups so there is exactly one way to do each thing.
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

// 1. Locate the entry bundle.
const home = await fetchText(`${DOCS}/`);
const entryRel = home.match(/\/assets\/entry\.client-[A-Za-z0-9_-]+\.js/)?.[0];
if (!entryRel) throw new Error("entry.client chunk not found in docs HTML");
const entry = await fetchText(`${DOCS}${entryRel}`);

// 2. Locate the spec chunk it lazy-loads.
const specRel = entry.match(/\.\/weeek\.yaml-[A-Za-z0-9_-]+\.js/)?.[0];
if (!specRel) throw new Error("weeek.yaml chunk not found in entry bundle");
const specUrl = `${DOCS}/assets/${specRel.slice(2)}`;
console.error(`spec chunk: ${specUrl}`);

const specJs = await fetchText(specUrl);
await mkdir(OUT_DIR, { recursive: true });
await writeFile(TMP_CHUNK, specJs);

// 3. Import it as an ES module and pull the schema + slug map out.
const mod = await import(`${pathToFileURL(TMP_CHUNK).href}?v=${Date.now()}`);
const schema = mod.schema;
const slugs = mod.slugs ?? {};
if (!schema?.paths) throw new Error("chunk did not export an OpenAPI schema");

const typeOf = (s) => {
  if (!s) return "unknown";
  const t = s.type;
  if (Array.isArray(t)) return t.join("|");
  return typeof t === "string" ? t : "object";
};

// Карта slug'ов лежит в mod.slugs.tags; mod.slugs — контейнер { operations, tags }.
const slugMap = mod.slugs?.tags ?? mod.slugs ?? {};

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
      tag: label,
      slug,
      pathParams: [],
      queryParams: [],
      bodyFields: [],
      hasBody: Boolean(op.requestBody),
    };
    for (const prm of op.parameters ?? []) {
      const param = { name: prm.name, type: typeOf(prm.schema), required: Boolean(prm.required) };
      if (Array.isArray(prm.schema?.enum)) param.enum = prm.schema.enum;
      (prm.in === "path" ? def.pathParams : def.queryParams).push(param);
    }
    const bodySchema = op.requestBody?.content?.["application/json"]?.schema;
    const requiredBody = new Set(bodySchema?.required ?? []);
    for (const [name, fs] of Object.entries(bodySchema?.properties ?? {})) {
      const f = { name, type: typeOf(fs), required: requiredBody.has(name) };
      if (Array.isArray(fs.enum)) f.enum = fs.enum;
      def.bodyFields.push(f);
    }

    operations[op.operationId] = def;
    if (CURATED.has(op.operationId)) continue;
    if (!groups.has(label)) groups.set(label, { slug, label, opIds: [] });
    groups.get(label).opIds.push(op.operationId);
  }
}

const groupList = [...groups.values()].sort((a, b) => a.slug.localeCompare(b.slug));
const output = {
  specUrl,
  generatedAt: new Date().toISOString(),
  specTitle: schema.info?.title ?? "",
  operations,
  groups: groupList,
};

await writeFile(OUT_FILE, JSON.stringify(output, null, 1));
console.error(
  `operations: ${Object.keys(operations).length}; in groups: ${Object.values(operations).length - CURATED.size}; groups: ${groupList.length}; -> ${path.relative(process.cwd(), OUT_FILE)}`,
);
