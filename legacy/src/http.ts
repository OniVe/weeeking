import { mkdir, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import type { Config } from "./env.js";

export class WeeekTokenError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "WeeekTokenError";
  }
}

export class WeeekApiError extends Error {
  constructor(
    public readonly status: number,
    public readonly body: string,
  ) {
    super(`HTTP ${status}`);
    this.name = "WeeekApiError";
  }
}

export class WeeekNetworkError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "WeeekNetworkError";
  }
}

const TOKEN_HINT =
  "WEEEK_API_TOKEN не задан. Создайте токен в Weeek (Настройки workspace → API) и передайте его " +
  "в окружении MCP-сервера (WEEEK_API_TOKEN), либо задайте READ_ONLY=true только для чтения.";

function buildQuery(query?: Record<string, unknown>): string {
  if (!query) return "";
  const parts: string[] = [];
  for (const [key, value] of Object.entries(query)) {
    if (value === undefined || value === null || value === "") continue;
    if (Array.isArray(value)) {
      for (const item of value) parts.push(`${encodeURIComponent(key)}=${encodeURIComponent(String(item))}`);
    } else {
      parts.push(`${encodeURIComponent(key)}=${encodeURIComponent(String(value))}`);
    }
  }
  return parts.length ? `?${parts.join("&")}` : "";
}

export class WeeekClient {
  constructor(private readonly cfg: Config) {}

  get hasToken(): boolean {
    return Boolean(this.cfg.token);
  }

  private requireToken(): string {
    if (!this.cfg.token) throw new WeeekTokenError(TOKEN_HINT);
    return this.cfg.token;
  }

  private url(pathName: string, query?: Record<string, unknown>): string {
    return `${this.cfg.baseUrl}${pathName}${buildQuery(query)}`;
  }

  async call(
    method: string,
    pathName: string,
    opts: { query?: Record<string, unknown>; body?: unknown } = {},
  ): Promise<unknown> {
    const token = this.requireToken();
    const headers: Record<string, string> = {
      Authorization: `Bearer ${token}`,
      Accept: "application/json",
    };
    let bodyInit: string | undefined;
    if (opts.body !== undefined) {
      headers["Content-Type"] = "application/json";
      bodyInit = JSON.stringify(opts.body);
    }

    let res: Response;
    try {
      res = await fetch(this.url(pathName, opts.query), {
        method,
        headers,
        body: bodyInit,
        signal: AbortSignal.timeout(this.cfg.timeoutMs),
      });
    } catch (e) {
      throw new WeeekNetworkError(`Сетевая ошибка или таймаут (${method} ${pathName}): ${(e as Error).message}`);
    }

    const text = await res.text();
    if (!res.ok) throw new WeeekApiError(res.status, text);
    if (!text) return null;
    const contentType = res.headers.get("content-type") ?? "";
    if (contentType.includes("json")) {
      try {
        return JSON.parse(text);
      } catch {
        return text;
      }
    }
    return text;
  }

  /** Downloads an attachment (binary) into a temp directory and returns the saved path. */
  async download(pathName: string): Promise<{ filePath: string; bytes: number; contentType: string }> {
    const token = this.requireToken();
    let res: Response;
    try {
      res = await fetch(this.url(pathName), {
        headers: { Authorization: `Bearer ${token}` },
        signal: AbortSignal.timeout(this.cfg.timeoutMs),
      });
    } catch (e) {
      throw new WeeekNetworkError(`Сетевая ошибка или таймаут (GET ${pathName}): ${(e as Error).message}`);
    }
    if (!res.ok) throw new WeeekApiError(res.status, await res.text());

    const buf = Buffer.from(await res.arrayBuffer());
    const contentType = res.headers.get("content-type") ?? "application/octet-stream";
    const disposition = res.headers.get("content-disposition") ?? "";
    const match = /filename\*?=(?:UTF-8'')?"?([^";]+)"?/i.exec(disposition);
    let name = match?.[1] ? decodeURIComponent(match[1]) : "";
    if (!name) {
      const base = pathName.split("/").filter(Boolean).pop() ?? "attachment";
      const ext = mimeExtension(contentType);
      name = `attachment_${base}${ext}`;
    }
    name = name.replace(/[\\/:*?"<>|]+/g, "_");

    const dir = path.join(os.tmpdir(), "weeeking");
    await mkdir(dir, { recursive: true });
    const filePath = path.join(dir, name);
    await writeFile(filePath, buf);
    return { filePath, bytes: buf.length, contentType };
  }
}

function mimeExtension(contentType: string): string {
  const map: Record<string, string> = {
    "image/png": ".png",
    "image/jpeg": ".jpg",
    "image/gif": ".gif",
    "image/webp": ".webp",
    "image/svg+xml": ".svg",
    "application/pdf": ".pdf",
    "text/plain": ".txt",
    "text/csv": ".csv",
    "application/zip": ".zip",
  };
  const key = contentType.split(";")[0]?.trim().toLowerCase() ?? "";
  return map[key] ?? "";
}
