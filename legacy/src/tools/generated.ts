import type { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { z } from "zod";
import { GROUPS, OPERATIONS, type OpDef } from "../generated/operations.js";
import type { WeeekClient } from "../http.js";
import { describeError, errorResult, jsonResult, type ToolResult } from "./common.js";

export type RegisterOptions = { readOnly: boolean; maxChars: number };

function describeOp(op: OpDef): string {
  const parts: string[] = [];
  for (const p of op.pathParams) {
    parts.push(`${p.name} (path, ${p.type}${p.required ? ", обяз." : ""})`);
  }
  for (const q of op.queryParams) {
    const enumHint = q.enum ? `: ${q.enum.join("|")}` : "";
    parts.push(`${q.name} (${q.type}${enumHint}${q.required ? ", обяз." : ""})`);
  }
  for (const b of op.bodyFields) {
    const enumHint = b.enum ? `: ${b.enum.join("|")}` : "";
    parts.push(`${b.name}${b.required ? "*" : ""} (body, ${b.type}${enumHint})`);
  }
  let line = `• ${op.id}: ${op.summary}`;
  if (parts.length) line += ` — ${parts.join("; ")}`;
  else if (op.hasBody) line += " — тело запроса свободной формы";
  return line;
}

async function runOp(client: WeeekClient, op: OpDef, params: Record<string, unknown>, maxChars: number): Promise<ToolResult> {
  try {
    let pathName = op.path;
    const used = new Set<string>();

    for (const p of op.pathParams) {
      const value = params[p.name];
      if (value === undefined || value === null || value === "") {
        if (p.required) return errorResult(`Не задан обязательный path-параметр «${p.name}» для ${op.id}.`);
        continue;
      }
      used.add(p.name);
      pathName = pathName.replace(`{${p.name}}`, encodeURIComponent(String(value)));
    }

    const query: Record<string, unknown> = {};
    for (const q of op.queryParams) {
      if (params[q.name] !== undefined) {
        query[q.name] = params[q.name];
        used.add(q.name);
      }
    }

    const extras = Object.keys(params).filter((key) => !used.has(key));
    let body: Record<string, unknown> | undefined;
    if (op.hasBody) {
      body = {};
      for (const key of extras) body[key] = params[key];
    } else if (extras.length) {
      return errorResult(`Операция ${op.id} не принимает параметры: ${extras.join(", ")}.`);
    }

    const data = await client.call(op.method, pathName, { query, body });
    return jsonResult(data, maxChars);
  } catch (e) {
    return errorResult(describeError(e));
  }
}

export function registerGeneratedTools(server: McpServer, client: WeeekClient, opts: RegisterOptions): void {
  for (const group of GROUPS) {
    const all = group.opIds.map((id) => OPERATIONS[id]).filter((op): op is OpDef => Boolean(op));
    const ops = opts.readOnly ? all.filter((op) => op.method === "GET") : all;
    if (ops.length === 0) continue;

    const readOnlyNow = ops.every((op) => op.method === "GET");
    const hiddenCount = all.length - ops.length;
    const hint =
      hiddenCount > 0
        ? `\n(Режим READ_ONLY: скрыто изменяющих операций — ${hiddenCount}. Включите READ_ONLY=false, чтобы они появились.)`
        : "";

    const descriptions = ops.map(describeOp).join("\n");
    server.registerTool(
      `weeek_${group.slug.replace(/-/g, "_")}`,
      {
        title: `Weeek: ${group.label}`,
        description:
          `Операции Weeek (${group.label}) через параметр action.\n` +
          `params — плоский объект параметров выбранной операции (path/query/body по именам из списка):\n` +
          descriptions +
          hint,
        inputSchema: {
          action: z.enum(ops.map((op) => op.id) as [string, ...string[]]).describe("Операция Weeek API"),
          params: z.record(z.unknown()).optional().describe("Параметры операции (см. список в описании)"),
        },
        annotations: {
          readOnlyHint: readOnlyNow,
          destructiveHint: ops.some((op) => op.method === "DELETE"),
          idempotentHint: false,
          openWorldHint: true,
        },
      },
      async ({ action, params }) => {
        const op = OPERATIONS[action as string];
        if (!op) return errorResult(`Неизвестная операция: ${String(action)}`);
        return runOp(client, op, (params ?? {}) as Record<string, unknown>, opts.maxChars);
      },
    );
  }
}
